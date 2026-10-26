//! Turns the open windows and the Windows media sessions into "sources" (a video, a course,
//! a meeting) and picks the most likely one. Everything here is pure, so it is unit-tested
//! with made-up windows and a fake clock; `foreground` supplies the real inputs.

pub mod control;
pub mod rules;
pub mod smtc;
mod teams;
mod title_parsers;
mod zoom;

use rules::Rules;
use serde::Serialize;
use smtc::MediaSession;
use std::collections::HashMap;
use title_parsers::Site;

/// A top-level window as read from Windows.
#[derive(Debug, Clone, Serialize)]
pub struct RawWindow {
    pub hwnd: isize,
    pub exe: String,
    pub class: String,
    pub title: String,
}

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Youtube,
    Datacamp,
    Meet,
    Zoom,
    Teams,
    /// Any other site that is playing media.
    Web,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// Same video / course / meeting gives the same key, across windows and polls.
    pub key: String,
    pub platform: Platform,
    pub title: String,
    pub channel: Option<String>,
    pub is_playing: bool,
    pub is_meeting: bool,
    pub app: String,
    /// Its window is the one in front right now (or was the last one before the panel took focus).
    pub is_focused: bool,
    /// How long ago its window was last in front, if it ever was while we were running.
    pub focused_ago_ms: Option<u64>,
}

#[derive(Serialize, Clone, PartialEq, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Best first.
    pub candidates: Vec<Candidate>,
    /// The key of the likely source (the first candidate), if there is one.
    pub winner: Option<String>,
}

pub struct Inputs<'a> {
    pub windows: &'a [RawWindow],
    pub media: &'a [MediaSession],
    /// hwnd to the time (ms on our own clock) it last came to the front.
    pub focus: &'a HashMap<isize, u64>,
    /// The window in front, not counting the panel itself.
    pub foreground: Option<isize>,
    pub rules: &'a Rules,
    pub now_ms: u64,
}

const RECENT_FOCUS_MS: u64 = 60_000;

fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub fn build_candidates(inputs: &Inputs) -> Vec<Candidate> {
    let mut found: Vec<Candidate> = Vec::new();
    for window in inputs.windows {
        let Some(mut candidate) = from_window(window, inputs) else { continue };
        candidate.is_focused = inputs.foreground == Some(window.hwnd);
        candidate.focused_ago_ms = inputs.focus.get(&window.hwnd).map(|at| inputs.now_ms.saturating_sub(*at));
        match found.iter_mut().find(|c| c.key == candidate.key) {
            Some(existing) => merge(existing, candidate),
            None => found.push(candidate),
        }
    }
    found
}

fn from_window(window: &RawWindow, inputs: &Inputs) -> Option<Candidate> {
    let rules = inputs.rules;
    let exe = window.exe.to_ascii_lowercase();
    let make = |key: String, platform, title: String, meeting| Candidate {
        key,
        platform,
        title,
        channel: None,
        is_playing: false,
        is_meeting: meeting,
        app: exe.clone(),
        is_focused: false,
        focused_ago_ms: None,
    };
    match exe.as_str() {
        "chrome.exe" | "msedge.exe" => {
            let parsed = title_parsers::parse_browser_title(&window.title, &rules.browser, &rules.sites)?;
            let session = inputs
                .media
                .iter()
                .find(|m| smtc::browser_exe(&m.app_id) == Some(exe.as_str()) && title_parsers::same_page(&parsed.page, &m.title));
            let (platform, key_prefix) = match parsed.site {
                Site::YouTube => (Platform::Youtube, "youtube"),
                Site::DataCamp => (Platform::Datacamp, "datacamp"),
                Site::Meet => {
                    let title = format!("Google Meet {}", parsed.page);
                    return Some(make(format!("meet:{}", parsed.page), Platform::Meet, title, true));
                }
                // Other sites only count while they play something.
                Site::Other if session.is_some_and(|s| s.playing) => (Platform::Web, "web"),
                Site::Other => return None,
            };
            let mut candidate = make(format!("{key_prefix}:{}", normalize(&parsed.page)), platform, parsed.page, false);
            if let Some(s) = session {
                candidate.is_playing = s.playing;
                candidate.channel = Some(s.artist.trim().to_owned()).filter(|a| !a.is_empty());
            }
            Some(candidate)
        }
        "zoom.exe" => zoom::is_meeting_window(&window.class, &window.title, &rules.zoom)
            .then(|| make("zoom:meeting".into(), Platform::Zoom, "Zoom meeting".into(), true)),
        "ms-teams.exe" => {
            let subject = teams::meeting_subject(&window.title, &rules.teams)?;
            Some(make(format!("teams:{}", normalize(&subject)), Platform::Teams, subject, true))
        }
        _ => None,
    }
}

/// Two windows of the same source (a Zoom call has several): one candidate, with the best of both.
fn merge(into: &mut Candidate, other: Candidate) {
    into.is_playing |= other.is_playing;
    into.is_focused |= other.is_focused;
    into.channel = into.channel.take().or(other.channel);
    into.focused_ago_ms = match (into.focused_ago_ms, other.focused_ago_ms) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
}

/// Playing +3, in front within the last 60 s +2, a meeting +2, a known platform +1.
pub fn score(c: &Candidate) -> u32 {
    let focused = c.focused_ago_ms.is_some_and(|ago| ago <= RECENT_FOCUS_MS);
    3 * u32::from(c.is_playing) + 2 * u32::from(focused) + 2 * u32::from(c.is_meeting) + u32::from(c.platform != Platform::Web)
}

/// Ranks the candidates, best first. Ties go to the one in front more recently.
pub fn snapshot(mut candidates: Vec<Candidate>) -> Snapshot {
    candidates.sort_by_key(|c| (std::cmp::Reverse(score(c)), c.focused_ago_ms.unwrap_or(u64::MAX)));
    let winner = candidates.first().map(|c| c.key.clone());
    Snapshot { candidates, winner }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(hwnd: isize, exe: &str, class: &str, title: &str) -> RawWindow {
        RawWindow { hwnd, exe: exe.into(), class: class.into(), title: title.into() }
    }

    fn session(app_id: &str, title: &str, artist: &str, playing: bool) -> MediaSession {
        MediaSession { app_id: app_id.into(), title: title.into(), artist: artist.into(), playing }
    }

    fn run(windows: &[RawWindow], media: &[MediaSession], focus: &[(isize, u64)], now_ms: u64) -> Snapshot {
        let rules = Rules::embedded();
        let focus: HashMap<isize, u64> = focus.iter().copied().collect();
        let foreground = focus.iter().max_by_key(|(_, at)| **at).map(|(hwnd, _)| *hwnd);
        snapshot(build_candidates(&Inputs { windows, media, focus: &focus, foreground, rules: &rules, now_ms }))
    }

    #[test]
    fn youtube_gets_channel_and_playing_from_the_media_session() {
        let windows = [window(1, "chrome.exe", "Chrome_WidgetWin_1", "Intro to SQL - YouTube - Google Chrome")];
        let media = [session("Chrome", "Intro to SQL", "DataLemur", true)];
        let snap = run(&windows, &media, &[], 0);
        let c = &snap.candidates[0];
        assert_eq!((c.platform, c.title.as_str(), c.channel.as_deref(), c.is_playing), (Platform::Youtube, "Intro to SQL", Some("DataLemur"), true));
        assert_eq!(snap.winner.as_deref(), Some("youtube:intro to sql"));
    }

    #[test]
    fn youtube_without_a_media_session_still_counts_but_is_not_playing() {
        let windows = [window(1, "msedge.exe", "c", "Intro - YouTube - Personal - Microsoft\u{200B} Edge")];
        let snap = run(&windows, &[], &[], 0);
        assert_eq!((snap.candidates[0].platform, snap.candidates[0].is_playing), (Platform::Youtube, false));
    }

    #[test]
    fn a_media_session_from_the_other_browser_does_not_match() {
        let windows = [window(1, "chrome.exe", "c", "Intro - YouTube - Google Chrome")];
        let media = [session("MSEdge", "Intro", "Chan", true)];
        let snap = run(&windows, &media, &[], 0);
        assert_eq!(snap.candidates[0].channel, None);
    }

    #[test]
    fn other_sites_count_only_while_playing() {
        let windows = [window(1, "chrome.exe", "c", "Gmail - Google Chrome"), window(2, "chrome.exe", "c", "Lo-fi radio - Google Chrome")];
        let media = [session("Chrome", "Lo-fi radio", "", true)];
        let snap = run(&windows, &media, &[], 0);
        assert_eq!(snap.candidates.len(), 1);
        assert_eq!(snap.candidates[0].platform, Platform::Web);
    }

    #[test]
    fn apps_that_are_not_sources_are_ignored() {
        let windows = [window(1, "Code.exe", "Chrome_WidgetWin_1", "notes.md - Visual Studio Code"), window(2, "Wispr Flow.exe", "c", "Wispr Flow")];
        assert!(run(&windows, &[], &[], 0).candidates.is_empty());
    }

    #[test]
    fn zoom_meeting_alone_is_detected_and_the_main_window_is_not() {
        let main = window(1, "Zoom.exe", "ZPPTMainFrmWndClassEx", "Zoom Workplace");
        assert!(run(&[main.clone()], &[], &[], 0).candidates.is_empty());
        let meeting = window(2, "Zoom.exe", "ZPContentViewWndClass", "");
        let snap = run(&[main, meeting], &[], &[], 0);
        assert_eq!((snap.candidates.len(), snap.candidates[0].platform), (1, Platform::Zoom));
    }

    #[test]
    fn several_zoom_windows_make_one_source_with_the_latest_focus() {
        let windows = [window(1, "Zoom.exe", "ZPContentViewWndClass", ""), window(2, "Zoom.exe", "x", "Zoom Meeting")];
        let snap = run(&windows, &[], &[(1, 1_000), (2, 9_000)], 10_000);
        assert_eq!(snap.candidates.len(), 1);
        assert_eq!(snap.candidates[0].focused_ago_ms, Some(1_000));
    }

    #[test]
    fn teams_meeting_now() {
        let windows = [window(1, "ms-teams.exe", "TeamsWebView", "Meeting now | Microsoft Teams")];
        let snap = run(&windows, &[], &[], 0);
        assert_eq!((snap.candidates[0].platform, snap.candidates[0].title.as_str()), (Platform::Teams, "Meeting now"));
    }

    #[test]
    fn the_focus_bonus_expires_after_a_minute() {
        let windows = [
            window(1, "chrome.exe", "c", "Intro - YouTube - Google Chrome"),
            window(2, "ms-teams.exe", "c", "Standup | Microsoft Teams"),
        ];
        let media = [session("Chrome", "Intro", "Chan", true)];
        // YouTube: playing 3 + known 1 = 4. Teams: meeting 2 + focused 2 + known 1 = 5, so Teams wins.
        let snap = run(&windows, &media, &[(2, 99_000)], 100_000);
        assert_eq!(snap.winner.as_deref(), Some("teams:standup"));
        // 61 s later the focus bonus is gone: Teams 3, YouTube 4.
        let later = run(&windows, &media, &[(2, 99_000)], 160_001);
        assert_eq!(later.winner.as_deref(), Some("youtube:intro"));
    }

    #[test]
    fn a_tie_goes_to_the_one_in_front_more_recently() {
        let windows = [window(1, "chrome.exe", "c", "A - YouTube - Google Chrome"), window(2, "chrome.exe", "c", "B - YouTube - Google Chrome")];
        let snap = run(&windows, &[], &[(1, 50_000), (2, 90_000)], 100_000);
        assert_eq!(snap.winner.as_deref(), Some("youtube:b"));
    }

    #[test]
    fn the_window_in_front_is_marked_focused() {
        let windows = [window(1, "chrome.exe", "c", "A - YouTube - Google Chrome"), window(2, "chrome.exe", "c", "B - YouTube - Google Chrome")];
        let snap = run(&windows, &[], &[(1, 10), (2, 20)], 30);
        let focused: Vec<_> = snap.candidates.iter().filter(|c| c.is_focused).map(|c| c.title.as_str()).collect();
        assert_eq!(focused, ["B"]);
        // VS Code in front: no source is focused.
        let code = window(3, "Code.exe", "c", "x - Visual Studio Code");
        let snap = run(&[windows[0].clone(), windows[1].clone(), code], &[], &[(1, 10), (2, 20), (3, 25)], 30);
        assert!(snap.candidates.iter().all(|c| !c.is_focused));
    }

    #[test]
    fn switching_to_vs_code_changes_nothing() {
        // VS Code taking focus adds no candidate, and the YouTube window keeps its place.
        let youtube = window(1, "chrome.exe", "c", "Intro - YouTube - Google Chrome");
        let before = run(&[youtube.clone()], &[], &[(1, 10_000)], 20_000);
        let code = window(2, "Code.exe", "c", "main.rs - Visual Studio Code");
        let after = run(&[youtube, code], &[], &[(1, 10_000), (2, 25_000)], 30_000);
        assert_eq!(before.winner, after.winner);
        assert_eq!(after.candidates.len(), 1);
    }
}
