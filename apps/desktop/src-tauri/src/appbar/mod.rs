//! Registers the panel as a Windows AppBar: the shell reserves screen space for it,
//! so maximized windows (Chrome, Zoom, Teams) shrink to fit beside it.

mod geometry;

pub use geometry::Edge;
use geometry::Rect;

use serde::Serialize;
use std::sync::Mutex;
use tauri::WebviewWindow;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

#[derive(Clone, Copy)]
struct State {
    edge: Edge,
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
    *STATE.lock().unwrap()
}

fn update(f: impl FnOnce(&mut State)) {
    if let Some(s) = STATE.lock().unwrap().as_mut() {
        f(s);
    }
}

/// Must run on the main thread (the window's owner), e.g. in `setup`.
pub fn attach(window: &WebviewWindow, edge: Edge, width_dip: u32) -> tauri::Result<()> {
    let hwnd = window.hwnd()?;
    unsafe {
        *STATE.lock().unwrap() = Some(State {
            edge,
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

fn monitor_info(hwnd: HWND) -> MONITORINFO {
    let mut mi = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe {
        let _ = GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut mi);
    }
    mi
}

fn reposition(hwnd: HWND) {
    let Some(s) = state() else { return };
    if !s.docked {
        return;
    }
    let width = geometry::to_px(s.width_dip, unsafe { GetDpiForWindow(hwnd) });
    let mut abd = appbar_data(hwnd, &s);
    abd.rc = geometry::dock(monitor_info(hwnd).rcMonitor.into(), s.edge, width).into();
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
    let mi = monitor_info(hwnd);
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
