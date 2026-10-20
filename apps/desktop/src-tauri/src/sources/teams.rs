//! Microsoft Teams (new Teams, `ms-teams.exe`): the window title is "<subject> | Microsoft Teams".

use super::rules::TeamsRules;

/// The meeting subject, or `None` when the window is the main Teams window or has no usable title.
pub fn meeting_subject(title: &str, rules: &TeamsRules) -> Option<String> {
    let mut parts: Vec<&str> = title.split(rules.separator.as_str()).map(str::trim).collect();
    if parts.last().is_some_and(|last| last.eq_ignore_ascii_case(&rules.app_name)) {
        parts.pop();
    }
    let first = *parts.first()?;
    if first.is_empty() || rules.nav_words.iter().any(|w| w.eq_ignore_ascii_case(first)) {
        return None;
    }
    Some(first.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::rules::Rules;

    fn subject(title: &str) -> Option<String> {
        meeting_subject(title, &Rules::embedded().teams)
    }

    #[test]
    fn meeting_window() {
        assert_eq!(subject("Weekly sync | Microsoft Teams").as_deref(), Some("Weekly sync"));
        assert_eq!(subject("Meeting now | Microsoft Teams").as_deref(), Some("Meeting now"));
    }

    #[test]
    fn main_window_is_not_a_meeting() {
        assert_eq!(subject("Chat | Alice | Microsoft Teams"), None);
        assert_eq!(subject("Calendar | Microsoft Teams"), None);
        assert_eq!(subject("Microsoft Teams"), None);
        assert_eq!(subject(""), None);
    }
}
