//! Watches which window is in front and re-reads the candidate windows every 2 seconds.
//! One background thread owns a `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` hook and a timer;
//! whenever the list of sources changes it emits `sources-changed` to the panel.

mod ignore;
mod process;

use crate::sources::{self, rules::Rules, smtc, Candidate, Inputs, Platform, RawWindow, Snapshot};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::BOOL;

const POLL_MS: u32 = 2000;
const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;

/// hwnd to the time (ms since start) it last came to the front. Written by the hook callback.
static FOCUS: Mutex<Option<HashMap<isize, u64>>> = Mutex::new(None);
/// Set by the hook callback: a window came to the front, so refresh now instead of at the next tick.
static DIRTY: AtomicBool = AtomicBool::new(false);
/// The last window that came to the front (our own windows are skipped by the hook).
static FOREGROUND: AtomicIsize = AtomicIsize::new(0);
static LATEST: Mutex<Option<Snapshot>> = Mutex::new(None);
static START: OnceLock<Instant> = OnceLock::new();

fn now_ms() -> u64 {
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

pub fn start(app: AppHandle) {
    let _ = START.get_or_init(Instant::now);
    let spawned = std::thread::Builder::new().name("foreground".into()).spawn(move || run(app));
    if let Err(e) = spawned {
        eprintln!("source detection is off, the thread didn't start: {e}");
    }
}

unsafe extern "system" fn on_foreground(_hook: HWINEVENTHOOK, _event: u32, hwnd: HWND, id_object: i32, _id_child: i32, _thread: u32, _time: u32) {
    // Only the window itself, not a child object inside it.
    if id_object != 0 || hwnd.0.is_null() {
        return;
    }
    if let Ok(mut focus) = FOCUS.lock() {
        focus.get_or_insert_with(HashMap::new).insert(hwnd.0 as isize, now_ms());
    }
    FOREGROUND.store(hwnd.0 as isize, Ordering::Release);
    DIRTY.store(true, Ordering::Release);
}

fn run(app: AppHandle) {
    let rules = Rules::load(app.path().app_local_data_dir().ok().as_deref());
    let mut poller = Poller { app, rules, media: smtc::manager(), first_seen: HashMap::new(), last_signature: None };

    unsafe {
        let hook = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None, Some(on_foreground), 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);
        SetTimer(None, 1, POLL_MS, None);
        poller.refresh();

        // The hook's callbacks and the timer are delivered through this message loop.
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if msg.message == WM_TIMER || DIRTY.swap(false, Ordering::AcqRel) {
                poller.refresh();
            }
        }
        let _ = UnhookWinEvent(hook);
    }
}

struct Poller {
    app: AppHandle,
    rules: Rules,
    media: Option<windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager>,
    /// When a meeting source was first seen, as "HH:MM", so its title doesn't change every minute.
    first_seen: HashMap<String, String>,
    last_signature: Option<Vec<(String, String, Option<String>, bool, bool)>>,
}

impl Poller {
    fn refresh(&mut self) {
        let windows = enumerate_windows();
        if self.media.is_none() {
            self.media = smtc::manager();
        }
        let media = self.media.as_ref().map(smtc::sessions).unwrap_or_default();
        let focus = {
            let mut guard = FOCUS.lock().unwrap();
            let map = guard.get_or_insert_with(HashMap::new);
            map.retain(|hwnd, _| unsafe { IsWindow(Some(HWND(*hwnd as *mut c_void))).as_bool() });
            map.clone()
        };

        let foreground = Some(FOREGROUND.load(Ordering::Acquire)).filter(|h| *h != 0);
        let inputs = Inputs { windows: &windows, media: &media, focus: &focus, foreground, rules: &self.rules, now_ms: now_ms() };
        let mut candidates = sources::build_candidates(&inputs);
        self.name_zoom_meetings(&mut candidates);
        let snapshot = sources::snapshot(candidates);

        // "Focused 12 s ago" changes on every poll; only real changes are sent to the panel.
        let signature: Vec<_> = snapshot
            .candidates
            .iter()
            .map(|c| (c.key.clone(), c.title.clone(), c.channel.clone(), c.is_playing, c.is_focused))
            .chain(snapshot.winner.iter().map(|w| (w.clone(), String::new(), None, false, false)))
            .collect();
        *LATEST.lock().unwrap() = Some(snapshot.clone());
        if self.last_signature.as_ref() != Some(&signature) {
            self.last_signature = Some(signature);
            #[cfg(debug_assertions)]
            eprintln!("sources: {} found, likely: {:?}", snapshot.candidates.len(), snapshot.winner);
            let _ = self.app.emit("sources-changed", &snapshot);
        }
    }

    /// Zoom doesn't expose a meeting title, so it is "Zoom meeting – HH:MM" from when we first saw it.
    fn name_zoom_meetings(&mut self, candidates: &mut [Candidate]) {
        let now = local_hm();
        for c in candidates.iter_mut().filter(|c| c.platform == Platform::Zoom) {
            let started = self.first_seen.entry(c.key.clone()).or_insert_with(|| now.clone());
            c.title = format!("Zoom meeting – {started}");
        }
        self.first_seen.retain(|key, _| candidates.iter().any(|c| &c.key == key));
    }
}

fn local_hm() -> String {
    let t = unsafe { GetLocalTime() };
    format!("{:02}:{:02}", t.wHour, t.wMinute)
}

/// Visible, uncloaked, titled top-level windows of the programs that can be a source.
fn enumerate_windows() -> Vec<RawWindow> {
    struct Scan {
        found: Vec<RawWindow>,
        exes: HashMap<u32, Option<String>>,
    }

    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let scan = unsafe { &mut *(lparam.0 as *mut Scan) };
        if let Some(window) = unsafe { read_window(hwnd, scan) } {
            scan.found.push(window);
        }
        BOOL(1)
    }

    unsafe fn read_window(hwnd: HWND, scan: &mut Scan) -> Option<RawWindow> {
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() {
                return None;
            }
            let mut cloaked = 0u32;
            let _ = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut c_void, size_of::<u32>() as u32);
            if cloaked != 0 {
                return None;
            }
            let mut buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, &mut buf);
            if len <= 0 {
                return None;
            }
            let title = String::from_utf16_lossy(&buf[..len as usize]);

            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let exe = scan.exes.entry(pid).or_insert_with(|| process::exe_name(pid)).clone()?;
            if !ignore::is_source_app(&exe) {
                return None;
            }
            let len = GetClassNameW(hwnd, &mut buf);
            let class = String::from_utf16_lossy(&buf[..len.max(0) as usize]);
            if ignore::is_ignored(&exe, &class, pid == std::process::id()) {
                return None;
            }
            Some(RawWindow { hwnd: hwnd.0 as isize, exe, class, title })
        }
    }

    let mut scan = Scan { found: Vec::new(), exes: HashMap::new() };
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut scan as *mut Scan as isize));
    }
    scan.found
}

#[tauri::command]
pub fn get_sources() -> Snapshot {
    LATEST.lock().unwrap().clone().unwrap_or_default()
}

/// Every window of a source program with its class and title, for finding the right Zoom/Teams rules.
#[tauri::command]
pub fn probe_windows() -> Vec<RawWindow> {
    enumerate_windows()
}

/// Every media session and why each source did or didn't get one, for "pause while I type" problems.
#[tauri::command]
pub async fn probe_media() -> sources::control::MediaProbe {
    let read = || smtc::manager().map(|m| smtc::sessions(&m)).unwrap_or_default();
    let sessions = tauri::async_runtime::spawn_blocking(read).await.unwrap_or_default();
    sources::control::probe(sessions, &get_sources().candidates)
}

#[cfg(test)]
mod live {
    use super::*;

    /// Reads the real desktop: run with `cargo test live_ -- --ignored --nocapture` while a video plays.
    #[test]
    #[ignore = "reads the live desktop"]
    fn live_windows_and_media_sessions() {
        let windows = enumerate_windows();
        println!("windows of source apps: {windows:#?}");
        let media = smtc::manager().map(|m| smtc::sessions(&m));
        println!("media sessions: {media:#?}");
        let rules = Rules::embedded();
        let focus = HashMap::new();
        let inputs = Inputs { windows: &windows, media: media.as_deref().unwrap_or(&[]), focus: &focus, foreground: None, rules: &rules, now_ms: 0 };
        println!("snapshot: {:#?}", sources::snapshot(sources::build_candidates(&inputs)));
    }
}
