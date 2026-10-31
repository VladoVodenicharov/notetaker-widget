//! Play / pause for the video being watched, sent through its Windows media session.
//! Matching by browser and page title means other players (Spotify, ...) are never touched.

use super::smtc::{self, MediaSession};
use super::title_parsers::page_match;
use super::Candidate;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Play,
    Pause,
    Toggle,
}

/// Why a source did or didn't get a media session. Shown in Diagnostics and on the play button.
#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Reason {
    /// Windows reports no media sessions at all.
    NoSession,
    /// There are sessions, but none from the source's browser.
    OtherBrowser,
    /// The browser has sessions, but no title fits the page (and it has more than one).
    TitleMismatch { page: String, media: String },
    /// Matched, but paused.
    NotPlaying,
    /// Matched and playing.
    Matched,
    /// Matched, but the player refused the command.
    Refused,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct MediaResult {
    pub ok: bool,
    pub reason: Reason,
}

fn same_browser(session: &MediaSession, app: &str) -> bool {
    smtc::browser_exe(&session.app_id) == Some(app.to_ascii_lowercase().as_str())
}

/// The browser's session whose title fits the page best (an exact match before a loose one).
pub fn by_title(sessions: &[MediaSession], app: &str, page: &str) -> Option<usize> {
    sessions
        .iter()
        .enumerate()
        .filter(|(_, s)| same_browser(s, app))
        .filter_map(|(i, s)| page_match(page, &s.title).map(|m| (m, std::cmp::Reverse(i))))
        .max()
        .map(|(_, std::cmp::Reverse(i))| i)
}

/// The browser's session if it has exactly one. Never a session from another app.
pub fn only_session(sessions: &[MediaSession], app: &str) -> Option<usize> {
    let mut own = sessions.iter().enumerate().filter(|(_, s)| same_browser(s, app));
    match (own.next(), own.next()) {
        (Some((i, _)), None) => Some(i),
        _ => None,
    }
}

/// The session for a source: by title, otherwise the browser's only session. The error says why not.
pub fn pick(sessions: &[MediaSession], app: &str, page: &str) -> Result<usize, Reason> {
    if let Some(i) = by_title(sessions, app, page).or_else(|| only_session(sessions, app)) {
        return Ok(i);
    }
    match sessions.iter().find(|s| same_browser(s, app)) {
        Some(s) => Err(Reason::TitleMismatch { page: page.to_owned(), media: s.title.clone() }),
        None if sessions.is_empty() => Err(Reason::NoSession),
        None => Err(Reason::OtherBrowser),
    }
}

fn found(session: &MediaSession) -> Reason {
    if session.playing { Reason::Matched } else { Reason::NotPlaying }
}

/// What Diagnostics shows: every media session, and which one each source would get.
#[derive(Serialize, Clone, Debug)]
pub struct MediaProbe {
    pub sessions: Vec<MediaSession>,
    pub candidates: Vec<CandidateMatch>,
}

#[derive(Serialize, Clone, Debug)]
pub struct CandidateMatch {
    pub key: String,
    pub app: String,
    pub title: String,
    pub reason: Reason,
}

pub fn probe(sessions: Vec<MediaSession>, candidates: &[Candidate]) -> MediaProbe {
    let candidates = candidates
        .iter()
        .filter(|c| !c.is_meeting)
        .map(|c| CandidateMatch {
            key: c.key.clone(),
            app: c.app.clone(),
            title: c.title.clone(),
            reason: pick(&sessions, &c.app, &c.title).map_or_else(|why| why, |i| found(&sessions[i])),
        })
        .collect();
    MediaProbe { sessions, candidates }
}

/// Sends the action to the matching session. Blocks on the system, so call it from a worker thread.
pub fn send(action: Action, app: &str, title: &str) -> MediaResult {
    let fail = |reason| MediaResult { ok: false, reason };
    let Some(manager) = smtc::manager() else { return fail(Reason::NoSession) };
    let (handles, sessions): (Vec<_>, Vec<_>) = smtc::sessions_with_handles(&manager).into_iter().unzip();
    let i = match pick(&sessions, app, title) {
        Ok(i) => i,
        Err(why) => return fail(why),
    };
    let session = &handles[i];
    let operation = match action {
        Action::Play => session.TryPlayAsync(),
        Action::Pause => session.TryPauseAsync(),
        Action::Toggle => session.TryTogglePlayPauseAsync(),
    };
    if operation.ok().and_then(|op| op.join().ok()).unwrap_or(false) {
        MediaResult { ok: true, reason: found(&sessions[i]) }
    } else {
        fail(Reason::Refused)
    }
}

#[tauri::command]
pub async fn media_command(action: Action, app: String, title: String) -> MediaResult {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let result = send(action, &app, &title);
        if !result.ok {
            eprintln!("media {action:?} for {app} \"{title}\" failed: {:?}", result.reason);
        }
        result
    });
    result.await.unwrap_or(MediaResult { ok: false, reason: Reason::NoSession })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(app_id: &str, title: &str) -> MediaSession {
        MediaSession { app_id: app_id.into(), title: title.into(), artist: String::new(), playing: true }
    }

    #[test]
    fn only_the_sources_own_browser_and_page_match() {
        let media = [session("Spotify.exe", "Intro to SQL"), session("MSEdge", "Intro to SQL"), session("Chrome", "Some song"), session("Chrome", "Intro to SQL")];
        assert_eq!(by_title(&media, "chrome.exe", "Intro to SQL"), Some(3));
        assert_eq!(by_title(&media[..3], "chrome.exe", "Intro to SQL"), None);
    }

    #[test]
    fn an_exact_title_beats_a_loose_one() {
        let media = [session("Chrome", "Lecture 3"), session("Chrome", "Lecture 3.mp4 - SharePoint")];
        assert_eq!(by_title(&media, "chrome.exe", "Lecture 3.mp4 - SharePoint"), Some(1));
    }

    #[test]
    fn the_fallback_takes_the_browsers_only_session_and_never_another_app() {
        let media = [session("Spotify.exe", "Some song"), session("Chrome", "Untitled")];
        assert_eq!(pick(&media, "chrome.exe", "Lecture 3.mp4 - SharePoint"), Ok(1));
        // Spotify alone is never used for a Chrome source.
        assert_eq!(pick(&media[..1], "chrome.exe", "Lecture 3.mp4 - SharePoint"), Err(Reason::OtherBrowser));
        // Two Chrome sessions and neither fits: no guessing.
        let two = [session("Chrome", "A"), session("Chrome", "B"), session("Spotify.exe", "Lecture 3")];
        assert_eq!(pick(&two, "chrome.exe", "Lecture 3"), Err(Reason::TitleMismatch { page: "Lecture 3".into(), media: "A".into() }));
        assert_eq!(pick(&[], "chrome.exe", "Lecture 3"), Err(Reason::NoSession));
    }

    #[test]
    fn the_probe_says_why_per_source() {
        let mut paused = session("Chrome", "Lecture 3");
        paused.playing = false;
        let candidate = |key: &str, app: &str, title: &str, is_meeting| Candidate {
            key: key.into(),
            platform: super::super::Platform::Web,
            title: title.into(),
            channel: None,
            is_playing: false,
            is_meeting,
            app: app.into(),
            is_focused: false,
            focused_ago_ms: None,
        };
        let candidates = [
            candidate("web:a", "chrome.exe", "Lecture 3.mp4 - SharePoint", false),
            candidate("web:b", "msedge.exe", "Other", false),
            candidate("meet:x", "chrome.exe", "Meet", true),
        ];
        let probe = probe(vec![paused], &candidates);
        let reasons: Vec<_> = probe.candidates.iter().map(|c| c.reason.clone()).collect();
        assert_eq!(reasons, [Reason::NotPlaying, Reason::OtherBrowser]);
    }

    #[test]
    fn reasons_serialize_for_the_panel() {
        let mismatch = Reason::TitleMismatch { page: "p".into(), media: "m".into() };
        assert_eq!(serde_json::to_string(&mismatch).unwrap(), r#"{"kind":"titleMismatch","page":"p","media":"m"}"#);
        assert_eq!(serde_json::to_string(&Reason::NoSession).unwrap(), r#"{"kind":"noSession"}"#);
    }

    #[test]
    fn actions_parse_from_the_panel() {
        assert_eq!(serde_json::from_str::<Action>("\"toggle\"").unwrap(), Action::Toggle);
        assert!(serde_json::from_str::<Action>("\"seek\"").is_err());
    }
}
