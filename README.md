# Notetaker

A note-taking panel for Windows 11 that docks to the edge of your screen while you watch videos or sit in meetings. The video shrinks into the remaining space, so you can see it and write at the same time. Each note records where it came from (title, channel or course, platform, date) and is saved as a plain Markdown file you choose the location of.

Everything runs offline on your PC. There is no account, no AI and no cloud.

## Install

1. Build the installer (see [Build from source](#build-from-source)) or use the existing `Notetaker_0.1.0_x64-setup.exe`.
2. Run it. It installs for your user only, so it does not ask for administrator rights.
3. When the installer finishes, Notetaker is on your **Desktop** and in the **Start menu**.

Notetaker does **not** start with Windows. You open it yourself, when you want to take notes.

## Start it

- Double-click the **Notetaker** shortcut on the desktop, or
- press the Windows key, type `Notetaker` and press Enter.

The panel docks to the right edge of the screen and other windows resize to fit beside it. Starting it again while it is running does nothing (only one panel can run at a time).

## Use it

| What | How |
|---|---|
| Write notes | Type in the editor. Headings, task lists and code blocks (with highlighting) are supported. |
| Save | Use **Finish**, choose a name, then **New note** (pick a folder; the last one is remembered) or continue an existing `.md` note. |
| Discard | Use **Discard** next to Finish and confirm to throw the note away. When continuing a note, only the new text is dropped; the file is not touched. |
| Play / pause the video | The play/pause button in the bottom bar, or **Ctrl+Alt+Space**. |
| Jump to the editor and back | **Ctrl+Alt+N**: puts the cursor in the editor; press it again to return to the video. |
| Pause while I type | Optional checkbox, off by default. Pauses a playing video when you start typing and resumes 5 seconds after you stop. |
| Show / hide the panel | Tray icon (bottom-right of the taskbar): **Show / hide panel**. Hiding gives the screen space back. |
| Quit | Tray icon, **Quit Notetaker**. |

Notes:

- Drafts are autosaved. If the app is killed, the next start offers to recover your last draft.
- The video controls only support play and pause. There is no skip or 10-second seek.
- Play/pause works through the Windows media session of the browser tab that is the current source (Chrome or Edge). Music apps such as Spotify are left alone, and the controls are disabled for Zoom, Teams and Meet calls.
- If another program already uses a hotkey, the panel shows a warning and that hotkey is off.

## Uninstall

Open **Settings → Apps → Installed apps**, find **Notetaker** and choose **Uninstall**. The panel gives its reserved screen space back when it exits, so nothing is left behind. Your saved notes are ordinary files in the folders you picked and are not deleted.

## Build from source

You need Node.js, Rust (MSVC toolchain), the Visual Studio Build Tools and WebView2 (already part of Windows 11).

```bash
npm install
npm run dev     # run the app in development mode
npm test        # TypeScript and Rust tests
npm run build   # release build and the installer
```

The installer is written to:

```
apps/desktop/src-tauri/target/release/bundle/nsis/Notetaker_0.1.0_x64-setup.exe
```

Installer settings (per-user install, desktop shortcut) are in `apps/desktop/src-tauri/tauri.conf.json` and `apps/desktop/src-tauri/windows/hooks.nsh`.

## Not included

The browser extension from the original plan was dropped. Without it, the video source comes from the browser window title and Windows media sessions, so details such as DataCamp course/chapter or YouTube playlists are not captured, and players inside embedded frames cannot be controlled. See [PLAN.md](PLAN.md) for the full design.
