//! Which windows can be a source, and which never matter.

/// Only these programs can be a source. VS Code and everything else never changes it.
const SOURCE_APPS: [&str; 4] = ["chrome.exe", "msedge.exe", "zoom.exe", "ms-teams.exe"];

const IGNORED_APPS: [&str; 6] = [
    "wispr flow.exe",
    "explorer.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
    "textinputhost.exe",
];

const SHELL_CLASSES: [&str; 8] = [
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "Windows.UI.Core.CoreWindow",
    "XamlExplorerHostIslandWindow",
    "TopLevelWindowForOverflowXamlIsland",
    "NotifyIconOverflowWindow",
    "Progman",
    "WorkerW",
];

pub fn is_source_app(exe: &str) -> bool {
    SOURCE_APPS.iter().any(|app| app.eq_ignore_ascii_case(exe))
}

/// Our own windows, the shell (taskbar, Start, search) and Wispr Flow.
pub fn is_ignored(exe: &str, class: &str, is_own_process: bool) -> bool {
    is_own_process
        || IGNORED_APPS.iter().any(|app| app.eq_ignore_ascii_case(exe))
        || SHELL_CLASSES.iter().any(|c| *c == class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_browsers_zoom_and_teams_are_sources() {
        for exe in ["chrome.exe", "MSEDGE.EXE", "Zoom.exe", "ms-teams.exe"] {
            assert!(is_source_app(exe), "{exe}");
        }
        for exe in ["Code.exe", "notepad.exe", "Wispr Flow.exe", "notetaker.exe"] {
            assert!(!is_source_app(exe), "{exe}");
        }
    }

    #[test]
    fn own_shell_and_wispr_windows_are_ignored() {
        assert!(is_ignored("notetaker.exe", "Window", true));
        assert!(is_ignored("explorer.exe", "CabinetWClass", false));
        assert!(is_ignored("Wispr Flow.exe", "Chrome_WidgetWin_1", false));
        assert!(is_ignored("anything.exe", "Shell_TrayWnd", false));
        assert!(!is_ignored("chrome.exe", "Chrome_WidgetWin_1", false));
    }
}
