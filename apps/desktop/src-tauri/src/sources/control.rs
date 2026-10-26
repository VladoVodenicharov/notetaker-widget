//! Play / pause for the video being watched, sent through its Windows media session.
//! Matching by browser and page title means other players (Spotify, ...) are never touched.

use super::smtc::{self, MediaSession};
use super::title_parsers::same_page;
use serde::Deserialize;
use windows::Media::Control::GlobalSystemMediaTransportControlsSession as Session;

#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Play,
    Pause,
    Toggle,
}

/// Does this media session belong to the source's browser and page?
pub fn matches(session: &MediaSession, app: &str, title: &str) -> bool {
    smtc::browser_exe(&session.app_id) == Some(app.to_ascii_lowercase().as_str()) && same_page(title, &session.title)
}

/// Sends the action to the matching session. Blocks on the system, so call it from a worker thread.
/// Returns false when no session matches or the player refused.
pub fn send(action: Action, app: &str, title: &str) -> bool {
    let Some(manager) = smtc::manager() else { return false };
    let Ok(list) = manager.GetSessions() else { return false };
    let target: Option<Session> = list.into_iter().find(|session| {
        let id = session.SourceAppUserModelId().map(|s| s.to_string()).unwrap_or_default();
        let name = session.TryGetMediaPropertiesAsync().ok().and_then(|p| p.join().ok()).and_then(|p| p.Title().ok()).map(|t| t.to_string());
        let found = MediaSession { app_id: id, title: name.unwrap_or_default(), artist: String::new(), playing: false };
        matches(&found, app, title)
    });
    let Some(session) = target else { return false };
    let operation = match action {
        Action::Play => session.TryPlayAsync(),
        Action::Pause => session.TryPauseAsync(),
        Action::Toggle => session.TryTogglePlayPauseAsync(),
    };
    operation.ok().and_then(|op| op.join().ok()).unwrap_or(false)
}

#[tauri::command]
pub async fn media_command(action: Action, app: String, title: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || send(action, &app, &title)).await.unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(app_id: &str, title: &str) -> MediaSession {
        MediaSession { app_id: app_id.into(), title: title.into(), artist: String::new(), playing: true }
    }

    #[test]
    fn only_the_sources_own_browser_and_page_match() {
        assert!(matches(&session("Chrome", "Intro to SQL"), "chrome.exe", "Intro to SQL"));
        assert!(!matches(&session("Spotify.exe", "Intro to SQL"), "chrome.exe", "Intro to SQL"));
        assert!(!matches(&session("MSEdge", "Intro to SQL"), "chrome.exe", "Intro to SQL"));
        assert!(!matches(&session("Chrome", "Some song"), "chrome.exe", "Intro to SQL"));
    }

    #[test]
    fn actions_parse_from_the_panel() {
        assert_eq!(serde_json::from_str::<Action>("\"toggle\"").unwrap(), Action::Toggle);
        assert!(serde_json::from_str::<Action>("\"seek\"").is_err());
    }
}
