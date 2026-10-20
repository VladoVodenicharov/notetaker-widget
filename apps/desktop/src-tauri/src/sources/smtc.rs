//! Windows media sessions (System Media Transport Controls): what Chrome/Edge report to the
//! volume flyout. They give the title, the channel (as "artist") and whether it is playing.

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};

#[derive(Debug, Clone, PartialEq)]
pub struct MediaSession {
    pub app_id: String,
    pub title: String,
    pub artist: String,
    pub playing: bool,
}

/// The browser process a media session's app id belongs to ("Chrome", "MSEdge").
pub fn browser_exe(app_id: &str) -> Option<&'static str> {
    let id = app_id.to_ascii_lowercase();
    if id.contains("chrome") {
        Some("chrome.exe")
    } else if id.contains("edge") {
        Some("msedge.exe")
    } else {
        None
    }
}

pub fn manager() -> Option<Manager> {
    Manager::RequestAsync().ok()?.join().ok()
}

/// All current sessions. Blocks on the system, so call it from a worker thread.
pub fn sessions(manager: &Manager) -> Vec<MediaSession> {
    let Ok(list) = manager.GetSessions() else { return Vec::new() };
    list.into_iter()
        .filter_map(|session| {
            let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
            let playing = session.GetPlaybackInfo().ok()?.PlaybackStatus().ok()? == Status::Playing;
            Some(MediaSession {
                app_id: session.SourceAppUserModelId().ok()?.to_string(),
                title: props.Title().ok()?.to_string(),
                artist: props.Artist().ok()?.to_string(),
                playing,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_ids_map_to_browsers() {
        assert_eq!(browser_exe("Chrome"), Some("chrome.exe"));
        assert_eq!(browser_exe("MSEdge"), Some("msedge.exe"));
        assert_eq!(browser_exe("Spotify.exe"), None);
    }
}
