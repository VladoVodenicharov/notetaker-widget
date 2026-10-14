//! Dock side and width, kept in the same `settings.json` store the frontend uses.

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

#[tauri::command]
pub fn get_dock() -> Option<Dock> {
    appbar::placement().map(|(edge, width_dip)| Dock { edge, width_dip })
}
