//! Zoom (`Zoom.exe`): a window counts as the meeting when its class or exact title is in the rules.
//! The real class and title differ by Zoom version, so check them with the Diagnostics "Source probe".

use super::rules::ZoomRules;

pub fn is_meeting_window(class: &str, title: &str, rules: &ZoomRules) -> bool {
    rules.meeting_classes.iter().any(|c| c == class) || rules.meeting_titles.iter().any(|t| t == title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::rules::Rules;

    #[test]
    fn meeting_by_class_or_title() {
        let rules = Rules::embedded().zoom;
        assert!(is_meeting_window("ZPContentViewWndClass", "", &rules));
        assert!(is_meeting_window("SomeOtherClass", "Zoom Meeting", &rules));
        assert!(!is_meeting_window("ZPPTMainFrmWndClassEx", "Zoom Workplace", &rules));
    }
}
