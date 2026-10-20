//! Reads Chrome and Edge window titles. Used when the browser helper isn't connected.
//!
//! Examples of what the browsers write:
//!   `Intro to SQL - YouTube - Google Chrome`
//!   `(3) Intro to SQL - YouTube - Personal - Microsoft<U+200B> Edge`
//!   `Intro to SQL - YouTube and 2 more pages - Personal - Microsoft<U+200B> Edge`

use super::rules::{BrowserRules, SiteRules};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Site {
    YouTube,
    DataCamp,
    /// A Google Meet tab; the page text is the meeting code.
    Meet,
    Other,
}

#[derive(Debug, PartialEq)]
pub struct BrowserTitle {
    /// The page title without the site name, notification count and browser suffix.
    pub page: String,
    pub site: Site,
}

pub fn parse_browser_title(raw: &str, browser: &BrowserRules, sites: &SiteRules) -> Option<BrowserTitle> {
    let title = raw.replace('\u{200B}', "");
    let end = browser.suffixes.iter().filter_map(|s| title.rfind(s.as_str())).max()?;
    let mut body = &title[..end];

    // Edge: "Page and 2 more pages - Personal". Everything from " and N more page(s)" on goes.
    if let Some(at) = body.rfind(" and ") {
        let rest = &body[at + 5..];
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits > 0 && rest[digits..].starts_with(" more page") {
            body = &body[..at];
        }
    }
    let body = strip_notification_count(body.trim());

    if let Some(code) = meet_code(body) {
        return Some(BrowserTitle { page: code.to_owned(), site: Site::Meet });
    }
    for (suffixes, site) in [(&sites.youtube, Site::YouTube), (&sites.datacamp, Site::DataCamp)] {
        if let Some(page) = strip_site(body, suffixes) {
            return Some(BrowserTitle { page: page.to_owned(), site });
        }
    }
    Some(BrowserTitle { page: body.to_owned(), site: Site::Other })
}

/// "(3) Title" becomes "Title".
fn strip_notification_count(s: &str) -> &str {
    let Some(rest) = s.strip_prefix('(') else { return s };
    let Some((count, after)) = rest.split_once(") ") else { return s };
    if !count.is_empty() && count.chars().all(|c| c.is_ascii_digit()) { after } else { s }
}

/// Removes a trailing site name, allowing one more " - <profile>" after it (Edge profile names).
fn strip_site<'a>(body: &'a str, suffixes: &[String]) -> Option<&'a str> {
    suffixes.iter().find_map(|suffix| {
        let at = body.rfind(suffix.as_str())?;
        let tail = &body[at + suffix.len()..];
        let only_profile = tail.is_empty() || tail.strip_prefix(" - ").is_some_and(|p| !p.contains(" - "));
        only_profile.then(|| body[..at].trim())
    })
}

/// "Meet - abc-defg-hij" (also with an en dash) gives "abc-defg-hij".
fn meet_code(body: &str) -> Option<&str> {
    let code = body.strip_prefix("Meet - ").or_else(|| body.strip_prefix("Meet – "))?.trim();
    is_meet_code(code).then_some(code)
}

fn is_meet_code(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 3
        && parts.iter().zip([3, 4, 3]).all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_lowercase()))
}

/// Does a media session's title belong to this page title? Edge may leave a " - Profile" tail.
pub fn same_page(page: &str, media_title: &str) -> bool {
    let (page, media) = (page.trim(), media_title.trim());
    !media.is_empty() && (page.eq_ignore_ascii_case(media) || page.to_lowercase().starts_with(&format!("{} - ", media.to_lowercase())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::rules::Rules;

    fn parse(raw: &str) -> Option<BrowserTitle> {
        let rules = Rules::embedded();
        parse_browser_title(raw, &rules.browser, &rules.sites)
    }

    fn title(page: &str, site: Site) -> Option<BrowserTitle> {
        Some(BrowserTitle { page: page.into(), site })
    }

    #[test]
    fn chrome_youtube() {
        assert_eq!(parse("Intro to SQL JOINs - YouTube - Google Chrome"), title("Intro to SQL JOINs", Site::YouTube));
    }

    #[test]
    fn chrome_notification_count_is_dropped() {
        assert_eq!(parse("(3) Intro to SQL - YouTube - Google Chrome"), title("Intro to SQL", Site::YouTube));
        assert_eq!(parse("(Draft) Page - Google Chrome"), title("(Draft) Page", Site::Other));
    }

    #[test]
    fn edge_zero_width_space_and_profile() {
        let raw = "Intro to SQL - YouTube - Personal - Microsoft\u{200B} Edge";
        assert_eq!(parse(raw), title("Intro to SQL", Site::YouTube));
    }

    #[test]
    fn edge_more_pages() {
        let raw = "Intro to SQL - YouTube and 2 more pages - Personal - Microsoft\u{200B} Edge";
        assert_eq!(parse(raw), title("Intro to SQL", Site::YouTube));
        let one = "Intro and 1 more page - Work - Microsoft\u{200B} Edge";
        assert_eq!(parse(one), title("Intro", Site::Other));
    }

    #[test]
    fn a_title_that_merely_contains_and_is_kept() {
        assert_eq!(parse("Salt and 3 eggs - YouTube - Google Chrome"), title("Salt and 3 eggs", Site::YouTube));
    }

    #[test]
    fn datacamp() {
        let raw = "PostgreSQL Summary Stats and Window Functions | DataCamp - Google Chrome";
        assert_eq!(parse(raw), title("PostgreSQL Summary Stats and Window Functions", Site::DataCamp));
    }

    #[test]
    fn google_meet_code() {
        assert_eq!(parse("Meet - abc-defg-hij - Google Chrome"), title("abc-defg-hij", Site::Meet));
        assert_eq!(parse("Meet - Lobby - Google Chrome"), title("Meet - Lobby", Site::Other));
    }

    #[test]
    fn not_a_browser_title() {
        assert_eq!(parse("Visual Studio Code"), None);
    }

    #[test]
    fn media_title_matching() {
        assert!(same_page("Intro to SQL", "intro to sql"));
        assert!(same_page("Podcast ep 4 - Personal", "Podcast ep 4"));
        assert!(!same_page("Intro to SQL", "Intro"));
        assert!(!same_page("Anything", ""));
    }
}
