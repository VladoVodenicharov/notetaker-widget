//! Global hotkeys. For now only Ctrl+Alt+N, which bounces focus between the panel and the video.

use std::sync::atomic::{AtomicIsize, Ordering};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsWindow, SetForegroundWindow};

/// The window that was in front when the hotkey brought the panel up (a raw HWND, which isn't `Send`).
static PREVIOUS: AtomicIsize = AtomicIsize::new(0);

/// Set when a hotkey couldn't be registered; the panel shows it as a warning.
pub struct Warning(Option<String>);

pub fn register(app: &AppHandle) {
    let focus = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyN);
    let result = app.global_shortcut().on_shortcut(focus, |app, _, event| {
        if event.state == ShortcutState::Pressed {
            ping_pong(app);
        }
    });
    let warning = match result {
        Ok(()) => {
            eprintln!("hotkey Ctrl+Alt+N registered");
            None
        }
        Err(e) => {
            eprintln!("hotkey Ctrl+Alt+N failed: {e}");
            Some("Ctrl+Alt+N is used by another app, so the hotkey is off.".into())
        }
    };
    app.manage(Warning(warning));
}

#[tauri::command]
pub fn hotkey_warning(warning: State<Warning>) -> Option<String> {
    warning.0.clone()
}

/// Panel hidden or behind: bring it up with the cursor in the editor.
/// Panel already in front: hand focus back to the window we took it from.
/// Windows lets us call `SetForegroundWindow` here because we just received the hotkey.
fn ping_pong(app: &AppHandle) {
    let Some(window) = crate::main_window(app) else { return };
    let Ok(ours) = window.hwnd() else { return };
    let foreground = unsafe { GetForegroundWindow() };
    if foreground == ours && window.is_visible().unwrap_or(false) {
        let previous = HWND(PREVIOUS.load(Ordering::Relaxed) as *mut _);
        if unsafe { IsWindow(Some(previous)) }.as_bool() {
            let _ = unsafe { SetForegroundWindow(previous) };
        }
        return;
    }
    PREVIOUS.store(foreground.0 as isize, Ordering::Relaxed);
    crate::show_panel(app);
    let _ = app.emit("focus-editor", ());
}
