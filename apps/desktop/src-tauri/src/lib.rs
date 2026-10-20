mod appbar;
mod foreground;
mod hotkeys;
mod notes;
mod settings;
mod sources;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("main")
}

fn show_panel(app: &AppHandle) {
    if let Some(w) = main_window(app) {
        appbar::dock(&w);
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Hiding also releases the reserved space, so the video gets the full screen back.
#[tauri::command]
fn hide_panel(app: AppHandle) {
    if let Some(w) = main_window(&app) {
        appbar::undock(&w);
        let _ = w.hide();
    }
}

fn toggle_panel(app: &AppHandle) {
    match main_window(app).map(|w| w.is_visible().unwrap_or(false)) {
        Some(true) => hide_panel(app.clone()),
        _ => show_panel(app),
    }
}

#[tauri::command]
fn panel_diagnostics(window: WebviewWindow) -> Option<appbar::Diagnostics> {
    appbar::diagnostics(&window)
}

fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Show / hide panel", true, None::<&str>)?;
    let screen = MenuItem::with_id(app, "screen", "Move to the next screen", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Notetaker", true, None::<&str>)?;
    TrayIconBuilder::new()
        .icon(app.default_window_icon().expect("bundled icon").clone())
        .tooltip("Notetaker")
        .menu(&Menu::with_items(app, &[&toggle, &screen, &quit])?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => toggle_panel(app),
            "screen" => {
                if let Some(w) = main_window(app) {
                    settings::move_to_next_screen(app.clone(), w);
                    let _ = app.emit("screens-changed", ());
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                toggle_panel(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Two docked panels would stack side by side, so a second launch just shows the first.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show_panel(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let window = main_window(app.handle()).expect("main window");
            let (edge, width_dip) = settings::load_dock(app.handle());
            appbar::attach(&window, edge, width_dip, settings::load_monitor(app.handle()))?;
            window.show()?;
            create_tray(app.handle())?;
            hotkeys::register(app.handle());
            foreground::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            notes::save_note,
            notes::append::read_note,
            notes::append::append_session,
            notes::index::index_note,
            notes::index::suggest_note,
            notes::drafts::save_draft,
            notes::drafts::load_draft,
            notes::drafts::delete_draft,
            settings::get_dock,
            settings::set_dock,
            settings::list_screens,
            settings::move_to_next_screen,
            hotkeys::hotkey_warning,
            hide_panel,
            panel_diagnostics,
            foreground::get_sources,
            foreground::probe_windows,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Notetaker");
}
