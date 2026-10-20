//! Dock side, width and screen, kept in the same `settings.json` store the frontend uses.

use crate::appbar::{self, Edge};
use serde::Serialize;
use tauri::{AppHandle, WebviewWindow};
use tauri_plugin_store::StoreExt;

const STORE: &str = "settings.json";
/// 380 DIP = 570 px at 150% scale.
const DEFAULT_WIDTH_DIP: u32 = 380;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dock {
    edge: Edge,
    width_dip: u32,
}

/// The saved side and width, or right / 380 DIP.
pub fn load_dock(app: &AppHandle) -> (Edge, u32) {
    let store = app.store(STORE).ok();
    let get = |key| store.as_ref()?.get(key);
    let edge = get("dockEdge").and_then(|v| serde_json::from_value(v).ok()).unwrap_or(Edge::Right);
    let width_dip = get("widthDip").and_then(|v| v.as_u64()).map_or(DEFAULT_WIDTH_DIP, |w| w as u32);
    (edge, width_dip)
}

/// Applies a new side and/or width and remembers it. Returns what was applied (width is clamped).
#[tauri::command]
pub fn set_dock(app: AppHandle, window: WebviewWindow, edge: Edge, width_dip: u32) -> Dock {
    let width_dip = appbar::set_dock(&window, edge, width_dip);
    if let Ok(store) = app.store(STORE) {
        store.set("dockEdge", serde_json::json!(edge));
        store.set("widthDip", width_dip);
    }
    Dock { edge, width_dip }
}

/// The screen picked last time (device name), if any.
pub fn load_monitor(app: &AppHandle) -> Option<String> {
    app.store(STORE).ok()?.get("monitor")?.as_str().map(str::to_owned)
}

#[tauri::command]
pub fn list_screens(window: WebviewWindow) -> Vec<appbar::Screen> {
    appbar::screens(&window)
}

/// Docks on the next screen (left to right, wrapping around) and remembers it.
/// Returns the screens afterwards. With one screen it does nothing.
#[tauri::command]
pub fn move_to_next_screen(app: AppHandle, window: WebviewWindow) -> Vec<appbar::Screen> {
    let screens = appbar::screens(&window);
    if screens.len() > 1 {
        let current = screens.iter().position(|s| s.current).unwrap_or(0);
        let next = screens[(current + 1) % screens.len()].device.clone();
        appbar::set_monitor(&window, next.clone());
        if let Ok(store) = app.store(STORE) {
            store.set("monitor", next);
        }
    }
    appbar::screens(&window)
}

#[tauri::command]
pub fn get_dock() -> Option<Dock> {
    appbar::placement().map(|(edge, width_dip)| Dock { edge, width_dip })
}
