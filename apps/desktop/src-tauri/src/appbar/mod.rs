//! Registers the panel as a Windows AppBar: the shell reserves screen space for it,
//! so maximized windows (Chrome, Zoom, Teams) shrink to fit beside it.

mod geometry;

pub use geometry::Edge;
use geometry::Rect;

use serde::Serialize;
use std::sync::Mutex;
use tauri::WebviewWindow;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW, MonitorFromWindow,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, w};

#[derive(Clone)]
struct State {
    edge: Edge,
    /// The screen to dock on, by device name (`\\.\DISPLAY2`). `None` or an unplugged screen: the nearest one.
    monitor: Option<String>,
    width_dip: u32,
    docked: bool,
    fullscreen: bool,
    rect: Rect,
    callback_msg: u32,
    taskbar_created_msg: u32,
}

// Copied in and out; never held across a Win32 call, because those re-enter the subclass proc.
static STATE: Mutex<Option<State>> = Mutex::new(None);

fn state() -> Option<State> {
    STATE.lock().unwrap().clone()
}

fn update(f: impl FnOnce(&mut State)) {
    if let Some(s) = STATE.lock().unwrap().as_mut() {
        f(s);
    }
}

/// Must run on the main thread (the window's owner), e.g. in `setup`.
pub fn attach(window: &WebviewWindow, edge: Edge, width_dip: u32, monitor: Option<String>) -> tauri::Result<()> {
    let hwnd = window.hwnd()?;
    unsafe {
        *STATE.lock().unwrap() = Some(State {
            edge,
            monitor,
            width_dip: geometry::clamp_width(width_dip),
            docked: false,
            fullscreen: false,
            rect: Rect::default(),
            callback_msg: RegisterWindowMessageW(w!("Notetaker.AppBar")),
            taskbar_created_msg: RegisterWindowMessageW(w!("TaskbarCreated")),
        });
        let _ = SetWindowSubclass(hwnd, Some(subclass_proc), 1, 0);
    }
    // No panic hook needed: when the process dies, the shell drops its AppBar by itself.
    dock(window);
    Ok(())
}

pub fn dock(window: &WebviewWindow) {
    let Ok(hwnd) = window.hwnd() else { return };
    let Some(s) = state() else { return };
    if !s.docked {
        let mut abd = appbar_data(hwnd, &s);
        unsafe { SHAppBarMessage(ABM_NEW, &mut abd) };
        update(|s| s.docked = true);
    }
    reposition(hwnd);
}

/// Moves the panel to `edge` and/or resizes it. Returns the width actually used.
/// While hidden, only the stored values change; they apply on the next `dock`.
pub fn set_dock(window: &WebviewWindow, edge: Edge, width_dip: u32) -> u32 {
    let width_dip = geometry::clamp_width(width_dip);
    let Ok(hwnd) = window.hwnd() else { return width_dip };
    let Some(s) = state() else { return width_dip };
    // Switching sides: unregister first so the old strip is released, then register on the new edge.
    let reregister = s.docked && s.edge != edge;
    if reregister {
        remove(hwnd);
    }
    update(|s| (s.edge, s.width_dip) = (edge, width_dip));
    if reregister {
        dock(window);
    } else {
        reposition(hwnd);
    }
    width_dip
}

/// Docks on another screen. Like switching sides, the strip on the old screen is released first.
pub fn set_monitor(window: &WebviewWindow, device: String) {
    let Ok(hwnd) = window.hwnd() else { return };
    let Some(s) = state() else { return };
    if s.docked {
        remove(hwnd);
    }
    update(|s| s.monitor = Some(device));
    if s.docked {
        dock(window);
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Screen {
    /// Device name, e.g. `\\.\DISPLAY1`.
    pub device: String,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
    /// The panel is docked here.
    pub current: bool,
}

struct FoundMonitor {
    handle: HMONITOR,
    device: String,
    rect: RECT,
    primary: bool,
}

/// All screens, left to right.
fn monitors() -> Vec<FoundMonitor> {
    unsafe extern "system" fn each(handle: HMONITOR, _dc: HDC, _rect: *mut RECT, param: LPARAM) -> BOOL {
        let found = unsafe { &mut *(param.0 as *mut Vec<FoundMonitor>) };
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if unsafe { GetMonitorInfoW(handle, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) }.as_bool() {
            let len = info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len());
            found.push(FoundMonitor {
                handle,
                device: String::from_utf16_lossy(&info.szDevice[..len]),
                rect: info.monitorInfo.rcMonitor,
                primary: info.monitorInfo.dwFlags & 1 != 0, // MONITORINFOF_PRIMARY
            });
        }
        BOOL(1)
    }
    let mut found: Vec<FoundMonitor> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM(&mut found as *mut _ as isize));
    }
    found.sort_by_key(|m| (m.rect.left, m.rect.top));
    found
}

/// The chosen screen if it is still connected, otherwise the one the window is on.
fn target_monitor(hwnd: HWND, s: &State) -> HMONITOR {
    s.monitor
        .as_deref()
        .and_then(|device| monitors().into_iter().find(|m| m.device == device))
        .map(|m| m.handle)
        .unwrap_or_else(|| unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) })
}

pub fn screens(window: &WebviewWindow) -> Vec<Screen> {
    let (Ok(hwnd), Some(s)) = (window.hwnd(), state()) else { return Vec::new() };
    let current = target_monitor(hwnd, &s);
    monitors()
        .into_iter()
        .map(|m| Screen {
            current: m.handle == current,
            width: m.rect.right - m.rect.left,
            height: m.rect.bottom - m.rect.top,
            primary: m.primary,
            device: m.device,
        })
        .collect()
}

/// The current side and width (DIP).
pub fn placement() -> Option<(Edge, u32)> {
    state().map(|s| (s.edge, s.width_dip))
}

pub fn undock(window: &WebviewWindow) {
    if let Ok(hwnd) = window.hwnd() {
        remove(hwnd);
    }
}

fn remove(hwnd: HWND) {
    let Some(s) = state() else { return };
    if s.docked {
        let mut abd = appbar_data(hwnd, &s);
        unsafe { SHAppBarMessage(ABM_REMOVE, &mut abd) };
        update(|s| s.docked = false);
    }
}

fn appbar_data(hwnd: HWND, s: &State) -> APPBARDATA {
    APPBARDATA {
        cbSize: size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uCallbackMessage: s.callback_msg,
        uEdge: match s.edge {
            Edge::Left => ABE_LEFT,
            Edge::Right => ABE_RIGHT,
        },
        ..Default::default()
    }
}

fn monitor_info(monitor: HMONITOR) -> MONITORINFO {
    let mut mi = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe {
        let _ = GetMonitorInfoW(monitor, &mut mi);
    }
    mi
}

/// The screen's own DPI: the window may still be on another screen with a different scale.
fn monitor_dpi(monitor: HMONITOR, hwnd: HWND) -> u32 {
    let (mut x, mut y) = (0u32, 0u32);
    match unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) } {
        Ok(()) if x > 0 => x,
        _ => unsafe { GetDpiForWindow(hwnd) },
    }
}

fn reposition(hwnd: HWND) {
    let Some(s) = state() else { return };
    if !s.docked {
        return;
    }
    let monitor = target_monitor(hwnd, &s);
    let width = geometry::to_px(s.width_dip, monitor_dpi(monitor, hwnd));
    let mut abd = appbar_data(hwnd, &s);
    abd.rc = geometry::dock(monitor_info(monitor).rcMonitor.into(), s.edge, width).into();
    unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut abd) };
    abd.rc = geometry::dock(abd.rc.into(), s.edge, width).into();
    unsafe { SHAppBarMessage(ABM_SETPOS, &mut abd) };

    let r: Rect = abd.rc.into();
    update(|s| s.rect = r);
    let z = if s.fullscreen { HWND_BOTTOM } else { HWND_TOPMOST };
    unsafe {
        let _ = SetWindowPos(hwnd, Some(z), r.left, r.top, r.width(), r.height(), SWP_NOACTIVATE);
    }
}

/// While a fullscreen video or meeting is up, step out of its way.
fn set_fullscreen(hwnd: HWND, fullscreen: bool) {
    update(|s| s.fullscreen = fullscreen);
    let z = if fullscreen { HWND_BOTTOM } else { HWND_TOPMOST };
    unsafe {
        let _ = SetWindowPos(hwnd, Some(z), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    let Some(s) = state() else {
        return unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
    };
    match msg {
        m if m == s.callback_msg => {
            match wparam.0 as u32 {
                ABN_POSCHANGED => reposition(hwnd),
                ABN_FULLSCREENAPP => set_fullscreen(hwnd, lparam.0 != 0),
                _ => {}
            }
            return LRESULT(0);
        }
        // Explorer restarted and forgot every AppBar: register again.
        m if m == s.taskbar_created_msg && s.docked => {
            let mut abd = appbar_data(hwnd, &s);
            unsafe { SHAppBarMessage(ABM_NEW, &mut abd) };
            reposition(hwnd);
        }
        WM_ACTIVATE if s.docked => {
            let mut abd = appbar_data(hwnd, &s);
            unsafe { SHAppBarMessage(ABM_ACTIVATE, &mut abd) };
        }
        WM_WINDOWPOSCHANGED if s.docked => {
            let mut abd = appbar_data(hwnd, &s);
            unsafe { SHAppBarMessage(ABM_WINDOWPOSCHANGED, &mut abd) };
        }
        // Keep Snap, Win+Arrow and stray resizes from moving the docked panel.
        WM_WINDOWPOSCHANGING if s.docked && !unsafe { IsIconic(hwnd) }.as_bool() => {
            let wp = unsafe { &mut *(lparam.0 as *mut WINDOWPOS) };
            if !wp.flags.contains(SWP_NOMOVE) {
                (wp.x, wp.y) = (s.rect.left, s.rect.top);
            }
            if !wp.flags.contains(SWP_NOSIZE) {
                (wp.cx, wp.cy) = (s.rect.width(), s.rect.height());
            }
        }
        // Let the window handle the change first, then recompute our strip.
        WM_DPICHANGED | WM_DISPLAYCHANGE if s.docked => {
            let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
            reposition(hwnd);
            return result;
        }
        WM_NCDESTROY => {
            remove(hwnd);
            unsafe {
                let _ = RemoveWindowSubclass(hwnd, Some(subclass_proc), 1);
            }
        }
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    dpi: u32,
    scale: f64,
    docked: bool,
    edge: Edge,
    width_dip: u32,
    window: Rect,
    monitor: Rect,
    work_area: Rect,
}

pub fn diagnostics(window: &WebviewWindow) -> Option<Diagnostics> {
    let hwnd = window.hwnd().ok()?;
    let s = state()?;
    let mi = monitor_info(target_monitor(hwnd, &s));
    let mut wr = RECT::default();
    let dpi = unsafe {
        let _ = GetWindowRect(hwnd, &mut wr);
        GetDpiForWindow(hwnd)
    };
    Some(Diagnostics {
        dpi,
        scale: dpi as f64 / 96.0,
        docked: s.docked,
        edge: s.edge,
        width_dip: s.width_dip,
        window: wr.into(),
        monitor: mi.rcMonitor.into(),
        work_area: mi.rcWork.into(),
    })
}

impl From<RECT> for Rect {
    fn from(r: RECT) -> Self {
        Rect { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
    }
}

impl From<Rect> for RECT {
    fn from(r: Rect) -> Self {
        RECT { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
    }
}

#[cfg(test)]
mod live {
    /// Lists the real screens: `cargo test live_screens -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads the live desktop"]
    fn live_screens() {
        for m in super::monitors() {
            println!("{} {:?} primary={}", m.device, super::Rect::from(m.rect), m.primary);
        }
    }
}
