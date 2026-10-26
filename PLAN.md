# Notetaker: a docked side panel for taking notes while watching videos (Windows 11)

## Context
Vlado takes notes during YouTube tutorials, DataCamp courses, and calls on Zoom, Google Meet and Teams. He wants a panel like Wispr Flow's Notetaker, docked to the right edge of the screen. The video (or meeting) shrinks into the remaining space, so both are visible side by side. Each note should record which video, course or meeting it came from. When he finishes, he chooses a name and either a folder on his PC or an existing note to add to.

**What the user decided:**
- **App type:** a desktop panel docked to the screen edge, plus a small Chrome/Edge extension that tells the app what's playing.
- **Video reference in v1:** source details only (title, URL or course/lesson, platform, date). No timestamps, screenshots or transcript yet.
- **No AI in v1.** Everything runs offline and locally.
- **Extras in v1:** video controls in the panel, and continuing an existing note (picked by hand *and* suggested automatically). Not included: templates, inline markers, automatic folder rules.
- **Saving:** you choose the folder (the last one used is remembered) **or** an existing `.md` note to continue.
- **Format:** plain Markdown that reads well in VS Code. No Obsidian-only syntax.
- **Dock side:** right by default, switchable in Settings.

**Environment (checked):** Windows 11 with a 1920×1200 screen. Node 24, Rust 1.97 (MSVC), VS Build Tools, WebView2, Python 3.14, .NET 10 and git are installed, along with Chrome 156, Edge, Zoom (`%APPDATA%\Zoom\bin`) and new Teams (`ms-teams.exe`). Wispr Flow is running. The project folder `D:\Claude\Projects\Notetaker` is empty.

## Tech stack
- **Tauri v2** (Rust + WebView2), with **React 19 + TypeScript + Vite + Tailwind** for the user interface.
  - **Why Tauri:** Rust can call the Windows APIs we need directly (AppBar docking, the foreground-window hook, Windows media sessions). The UI is web-based, and it uses less memory than Electron while running next to Chrome and Zoom.
- **Editor:** TipTap 3 with `@tiptap/markdown`, `code-block-lowlight` (python/sql/bash/json/yaml), task lists, links and a placeholder. Drafts are stored as ProseMirror JSON so nothing is lost. Markdown is generated only on save.
- **Tauri plugins:** `global-shortcut`, `dialog`, `store`, `single-instance` (required, or two docked panels would stack), `opener`, `log`.
- **Rust crates:** `windows` (Shell, WindowsAndMessaging, HiDpi, Accessibility, Media_Control), `tokio-tungstenite`, `serde`, `ulid`, `time`.
- **Extension:** Chrome extension (Manifest V3, which also works in Edge), built with esbuild. A fixed `key` in the manifest keeps the extension ID the same in both browsers.

## How it works

### 1. Docking (AppBar), `src-tauri/src/appbar/`
- The window has no title bar or shadow and stays on top. We hook its Windows message handling (`SetWindowSubclass`) and register it as an AppBar with `ABM_NEW`.
- `reposition()`:
  1. Take the monitor rectangle.
  2. Convert the width to physical pixels: `width_dip × GetDpiForWindow / 96`.
  3. Send `ABM_QUERYPOS` and `ABM_SETPOS`.
  4. Call `SetWindowPos`.
- **Messages it handles:**
  - `ABN_POSCHANGED`: reposition.
  - `ABN_FULLSCREENAPP`: when a video goes fullscreen, drop always-on-top so the video covers the panel. Restore it when fullscreen ends.
  - `ABM_ACTIVATE` and `ABM_WINDOWPOSCHANGED`: passed on to Windows, as the AppBar docs require.
  - `WM_WINDOWPOSCHANGING`: clamp the window back so Snap and Win+Arrow can't move the panel.
  - `WM_DPICHANGED` and `WM_DISPLAYCHANGE`: reposition.
  - `TaskbarCreated` (Explorer restarted): re-register.
  - Exit or panic: `ABM_REMOVE`.
- **Hide** removes the AppBar so the video gets the full width back. **Width drag** applies the new width once, when you release the mouse.
- The width calculation is a pure function in `appbar/geometry.rs`, so it can be unit-tested.

### 2. Source detection (Windows side), `src-tauri/src/foreground/`, `src-tauri/src/sources/`
- A dedicated thread runs `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` and records the process, window class and title of each newly focused window. The titles of candidate windows are re-read every 2 seconds.
- **Ignored windows:** our own window, shell windows (taskbar, Start, search), `Wispr Flow.exe`, and windows that are invisible or cloaked.
- **Only these can be a source:** `chrome.exe`, `msedge.exe`, `Zoom.exe`, `ms-teams.exe`. VS Code and similar apps never change the source.
- **Browsers:** use the extension's data. Without the extension, fall back to:
  - the window title suffix (e.g. `- YouTube - Google Chrome`; Edge titles contain a zero-width space, U+200B), and
  - Windows media sessions, which give the title, the channel (as the "artist") and whether it's playing.
- **Zoom:** detect the meeting window by its class (`ZPContentViewWndClass`, to be confirmed with winprobe) and use "Zoom meeting – HH:MM" as the title. If a `zoom.us/j/<id>` tab was opened recently, attach that meeting ID.
- **Teams:** take the window title, split on ` | `, drop the trailing `Microsoft Teams` and the navigation words. What remains is the meeting subject. The rules are kept in `title_rules.toml`.
- **Picking the source when a session starts:**
  - Score each candidate: playing +3, last focused window within 60 s +2, meeting window +2, known platform +1.
  - The winner fills the header. A dropdown on the source chip lists the other candidates and a "Manual" option.
- **Switching sources mid-session** (`session/sourceSwitching.ts`):
  - A new source only counts after it has been stable for 8 seconds and is playing or focused.
  - A section header for it is added *the next time you type*, so flicking between tabs leaves no empty headers.
  - **Pin source** turns switching off.
  - Each switch is recorded as a segment `{sourceKey, startedAt}`. That's what makes timestamps easy to add later.

### 3. Extension ↔ app bridge, `src-tauri/src/bridge/`, `extension/`
- The app runs a WebSocket server at `ws://127.0.0.1:47823/bridge` (ports 47824 and 47825 if that one is taken). It listens on localhost only, rejects any `Origin` other than `chrome-extension://<fixed id>`, and limits messages to 64 KB.
- **Keeping the connection alive:** the extension sends a ping every 20 seconds. Tab events wake the extension's background worker, and a `chrome.alarms` timer reconnects it if needed. Every message is a complete snapshot of the current state, and the app asks for one (`state.request`) when a connection opens.
- **Messages:**
  - `hello`
  - `tab.state` with `{tab {title,url}, source {platform, key, title, canonicalUrl, youtube|datacamp|meet}, media {isPlaying}}`
  - `tab.closed`
  - `ping` / `pong`
  - `media.command` with `{tabId, action: play|pause|toggle|seekBy, seconds}`
  - The types are shared in `packages/shared/src/protocol.ts`, with a matching Rust file `bridge/protocol.rs`, and are checked against shared JSON fixtures.
- **Content scripts:**
  - **YouTube:** `mediaSession.metadata` first, then the page, then the tab title. Re-reads after each in-site navigation (`yt-navigate-finish`). Records the video ID, channel and playlist.
  - **DataCamp:** the URL comes first (`/courses/{course}/{chapter}?ex={n}`). Page selectors only add the readable titles. Runs in all frames, because the video player sits in an embedded frame.
  - **Google Meet:** meeting code from the URL, plus the meeting title when the page shows one.
- **Privacy:** the `t=` and `si=` parameters are removed from URLs. Unknown sites send only the site address and path, and Zoom `pwd=` parameters are always removed.

### 4. Panel UI, `apps/desktop/src/panel/`
- **Idle screen:** **New note** (picks up the current source) and **Continue a note…** (opens a file picker for `.md` files).
- If the current source matches a note saved before, a chip appears: "Continue 'PostgreSQL Window Functions'?"
- **Header:**
  - An editable title.
  - Chips for date, source (platform icon plus channel or course) and folder.
  - A collapsible "Source details" card (URL, course, chapter, lesson), all editable.
- **When continuing a note:** a collapsible, read-only **Previously** box shows the end of the existing note. Your new notes are added to the end of the file; the old content is never re-edited.
- **Editor:** TipTap with the placeholder "Write your notes here...". Markdown shortcuts work as you type, and fenced code blocks are highlighted.
- **Bottom bar:**
  - ⏪10s, ⏯ and ⏩10s, all sent to the source tab.
  - A "Pause video while I type" toggle. It's off by default, resumes after N seconds idle, and never applies to meetings.
  - Elapsed time and a **Finish** button.

### 5. Save flow, `src-tauri/src/notes/`
- **The Finish sheet has two tabs.**
  - **New note:**
    - Name, pre-filled as `{title} – {YYYY-MM-DD}`.
    - Folder chip set to the last folder used. **Browse…** opens a folder picker.
  - **Add to existing note:** the file chosen at the start, or a **Pick note…** file picker.
  - **Enter** saves. A message then offers **Open**, **Show in folder** and **Copy path**.
- **`save_note` (Rust):**
  - **Cleaning the file name:**
    - Remove `<>:"/\|?*` and control characters.
    - Trim trailing dots and spaces.
    - Prefix Windows reserved names (`CON`, `NUL`, `COM1`…) with `_`.
    - Limit the name to 120 characters.
    - If nothing is left, use `Untitled`.
  - **Name collision:** add ` (2)`, ` (3)` and so on (`create_new`), or offer to overwrite.
  - **Writing:**
    - Write a `.tmp` file, flush it to disk (`sync_all`), then rename it over the target.
    - Text is UTF-8 without BOM with LF line endings.
    - If OneDrive or antivirus has the file locked, retry.
- **`append_session` (continuing a note):**
  - Re-read the target file at save time, not when the session started.
  - If it has Notetaker front matter, update `updated` and `sessions` and add the new URLs to `source_urls`. Otherwise leave the top of the file untouched.
  - Add `## {date} ({start}–{end})` and the new content to the end.
  - Write it the same atomic way.
- **Note index:** `%LOCALAPPDATA%\dev.vlado.notetaker\notes-index.json` maps `{notetaker_id → path, sourceKeys[], updated}`.
  - It's used for the "Continue…?" suggestion: same DataCamp course, YouTube playlist (or the same video), or Teams meeting subject.
  - Entries for missing files are removed.
- **Crash-safe autosave:**
  - Drafts are saved to `%LOCALAPPDATA%\dev.vlado.notetaker\drafts\draft.json` (one draft, as there is one session at a time) 750 ms after an edit, at least every 5 seconds, and whenever the window loses focus or closes.
  - Each write renames the current draft to `.bak`, then writes the new one to a temp file, flushes it and moves it into place. A crash at any point leaves at least one complete file.
  - **On the next launch:** "Recover '…' (312 words, 20:31)?" with **Resume / Save now / Discard**.

### Output format (plain Markdown; VS Code's preview hides the front matter)
````markdown
---
title: "PostgreSQL Window Functions"
created: 2026-10-11T19:42
updated: 2026-10-12T20:10
platform: datacamp
course: "PostgreSQL Summary Stats and Window Functions"
source_url: "https://campus.datacamp.com/courses/postgresql-summary-stats-and-window-functions/introduction-to-window-functions?ex=1"
source_urls:
  - "https://campus.datacamp.com/courses/postgresql-summary-stats-and-window-functions/introduction-to-window-functions?ex=1"
  - "https://campus.datacamp.com/courses/postgresql-summary-stats-and-window-functions/introduction-to-window-functions?ex=5"
sessions: 2
notetaker_id: 01JA8ZQ3K6V4M2T9X7B5C1D0EF
---

# PostgreSQL Window Functions

**Source:** DataCamp · [PostgreSQL Summary Stats and Window Functions](https://campus.datacamp.com/courses/postgresql-summary-stats-and-window-functions)

## 2026-10-11 (19:42–20:31)

### Introduction to window functions · Ex 1
[DataCamp · ex 1](https://campus.datacamp.com/courses/postgresql-summary-stats-and-window-functions/introduction-to-window-functions?ex=1)

- `OVER()` turns an aggregate into a window function; rows are not collapsed.

```sql
SELECT year, event, ROW_NUMBER() OVER (PARTITION BY event ORDER BY year) AS row_n
FROM summer_medals;
```

## 2026-10-12 (19:05–20:10)

### Introduction to window functions · Ex 5
[DataCamp · ex 5](https://campus.datacamp.com/courses/postgresql-summary-stats-and-window-functions/introduction-to-window-functions?ex=5)

- [ ] Redo the RANK vs DENSE_RANK exercise
````
- **Heading levels:**
  - H1 is the note title.
  - H2 is one per session (one date and time range).
  - H3 is one per source change inside a session: a YouTube video, a DataCamp exercise, or nothing for a single meeting.

### Global hotkeys (all configurable)
- `Ctrl+Alt+N`: show or focus the panel. Press it again to return focus to the video.
- `Ctrl+Alt+Space`: play/pause.
- `Ctrl+Alt+Left/Right`: skip back or forward 10 seconds.
- In the Bulgarian keyboard layout, AltGr counts as Ctrl+Alt, so these need testing there. Settings shows a warning when a hotkey fails to register.

## Project structure
```
D:\Claude\Projects\Notetaker\
  package.json (npm workspaces)  README.md  .gitignore  docs\MANUAL_TESTS.md
  packages\shared\src\protocol.ts, platform.ts (URL → platform/sourceKey), fixtures\messages\*.json
  apps\desktop\
    src\panel\   Header, SourceChip, FolderChip, PreviouslyBox, BottomBar, FinishSheet, RecoveryDialog, Settings, Diagnostics
    src\editor\  Editor.tsx, extensions\sourceSection.ts
    src\markdown\ serialize.ts, frontmatter.ts, filenameTemplate.ts
    src\session\ store.ts (zustand), sourceSwitching.ts, autosave.ts
    src-tauri\src\ lib.rs, tray.rs, settings.rs, hotkeys.rs,
                   appbar\{mod,geometry,wndproc}.rs, foreground\{mod,process,ignore}.rs,
                   sources\{mod,browser,zoom,teams,title_parsers,smtc}.rs + title_rules.toml,
                   bridge\{server,protocol}.rs, media\control.rs, notes\{save,append,sanitize,drafts,index}.rs
  extension\ manifest.json, build.mjs, src\background.ts, transport\ws.ts, content\{youtube,datacamp,meet,launchers}.ts
  tools\winprobe\   (small Rust tool that writes the focused window's process, class and title to JSONL, to collect Zoom/Teams title samples)
```

## Milestones (each can be run and tested on its own)
| # | Scope | How to verify on this PC |
|---|---|---|
| **M1: Docked panel + editor + save** | `git init`, Tauri scaffold, AppBar on the right (fullscreen handling, re-docking after Explorer restarts), tray Show/Hide/Quit, single instance, Diagnostics screen (DPI, panel rectangle, work area), TipTap with code highlighting, title/date/folder header, Finish sheet → **New note** (folder picker, last folder remembered), atomic save | 1) `npm run tauri dev`: the panel fills the right edge. 2) A maximized Chrome window ends exactly at the panel's edge. 3) Switch display scale 125%↔150% while it runs: no gap. 4) YouTube fullscreen covers the panel, and Esc restores it. 5) Restart Explorer: the panel re-docks. 6) Quit: Chrome goes back to full width. 7) Type headings, a task list and a sql code block. Save `SQL: JOINs?` → a sanitized file name; saving again gives ` (2)`. 8) The `.md` file previews cleanly in VS Code. 9) RAM (app plus WebView2) is under about 150 MB. 10) Dictating into the editor with Wispr works. |
| **M2: Never lose a note + keyboard** | Autosave and recovery, global hotkeys (focus ping-pong), resizable width, Hide frees the space, dock-side setting | Kill the app with `taskkill /F` → relaunch shows the recovery dialog. A corrupted draft is recovered from `.bak`. Hotkey from Chrome puts the cursor in the editor, and pressing it again returns focus to YouTube. Width drag makes maximized windows re-fit once. |
| **M3: Windows-side source detection** | Foreground tracker, ignore list, Zoom/Teams detection, Chrome/Edge title fallback, Windows media sessions, source chip with candidate list | Without the extension: YouTube fills in the title and channel. Switching to VS Code doesn't change the source. A Zoom "New meeting" (alone) and a Teams "Meet now" are detected. Edge titles parse correctly (U+200B). |
| **M4: Browser helper** | MV3 extension (YouTube, DataCamp, Meet, Zoom/Teams launcher tabs), WebSocket bridge with Origin check, "Helper connected" indicator | Load unpacked in Chrome and Edge. YouTube gives a clean URL plus channel and playlist. DataCamp gives course, chapter and exercise. Meet gives the meeting code. After 5 idle minutes a tab change still arrives within 1 s. Starting the app after Chrome connects within 30 s. A WebSocket opened from a normal web page's console is rejected. |
| **M5: Multi-source sessions + continue a note** | Source switching, section headers added on next keystroke, Pin, `source_urls`; Continue-a-note picker, Previously box, `append_session`, notes index and "Continue…?" suggestion | Two YouTube videos in one session give two H3 sections. Flicking between tabs without typing adds nothing. Reopening the same DataCamp course suggests the existing note, and saving appends a new dated H2 with the front matter updated. Picking a hand-written `.md` file (no front matter) only appends. |
| **M6: Video controls** | ⏪10s ⏯ ⏩10s from the panel and hotkeys (via the extension's `media.command`; play/pause falls back to Windows media sessions), Pause-while-typing (off by default, never for meetings) | With Spotify and YouTube playing together, only YouTube toggles. Seeking works on DataCamp, whose player is in an embedded frame. Pause-while-typing does nothing on Meet. |
| **M7: Ship** | NSIS installer (`npm run build`) with desktop and Start menu shortcuts, README with setup steps. Dropped: start-with-Windows toggle (launched manually) and extension zip (no extension) | Install, launch from the desktop shortcut and from Start search (it docks), uninstall (no reserved space left behind) |

## Automated tests
- **Rust (`cargo test`):**
  - File-name cleaning and the ` (2)` collision suffix.
  - AppBar width math at 100/125/150/175% scale, both dock sides.
  - Title parsers, using winprobe samples (Zoom, Teams, Chrome, Edge).
  - Picking the source and switching mid-session, with a fake clock.
  - Atomic draft writes and recovery from `.bak`.
  - `append_session` front-matter merge.
  - Protocol fixtures.
  - The Origin allowlist.
- **TypeScript (vitest):**
  - URL classifier for YouTube `watch`, `shorts`, `youtu.be` and `list=`; DataCamp URLs; Meet codes; `zoom.us/j`; Teams `meetup-join`; and stripping private URL parameters.
  - Content-script parsers against saved HTML samples (jsdom).
  - Golden tests for the Markdown output (nested lists, tasks, code fences containing backticks, `sourceSection`).
  - Front-matter YAML escaping.
  - Message validation with zod.
- `docs/MANUAL_TESTS.md` holds the checklist above and is the main check for v1.

## Risks to check first
- **AppBar size wrong at 125/150% scale, or the panel stuck over fullscreen video:** M1 Diagnostics screen and a live scale change. Fallback: compare each newly focused window's rectangle with the monitor's.
- **Reserved space left behind after a crash:** `ABM_REMOVE` in every exit path, re-register at startup. Restarting Explorer clears it as a last resort.
- **Chrome blocks the extension's `ws://127.0.0.1` connection (Local Network Access rules):** test this in the first hour of M4. The extension talks through a `Transport` interface, so it can switch to Chrome Native Messaging if needed.
- **Zoom/Teams window titles differ by version or language:** run winprobe during real calls early. Rules are editable in `title_rules.toml`, and the title can always be edited by hand.
- **DataCamp changes its page structure:** parse the URL first, keep all page selectors in one table with fallbacks, and test against saved HTML samples.
- **Hotkeys clash with the Bulgarian keyboard layout or Chrome shortcuts:** hotkeys are configurable and a failed registration shows a warning.

## Later (v2 roadmap; the M5 segment data and the bridge are already in place for these)
1. Clickable timestamps, for example `[12:34](https://youtu.be/ID?t=754)`.
2. Screenshots inserted into the note.
3. Transcript: YouTube captions, or local whisper.cpp with system-audio capture for meetings.
4. Summary and "What did I miss?" with Claude, using your own API key.
5. Library and search view.
6. Multiple monitors.

## How the work will be done
- Build one milestone at a time, M1 first.
- After each milestone, run its checks above and report exactly what was and wasn't verified.
- Offer to make a git commit at the end of each milestone. Nothing is committed without your OK.
