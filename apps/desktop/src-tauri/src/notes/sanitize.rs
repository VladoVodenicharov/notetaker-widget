//! Turns a free-form note name into a safe Windows file name (without extension).

const MAX_CHARS: usize = 120;
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub fn sanitize_file_name(name: &str) -> String {
    let mapped: String = name
        .chars()
        .map(|c| match c {
            ':' | '/' | '\\' => " - ".to_string(),
            '<' | '>' | '"' | '|' | '?' | '*' => String::new(),
            c if c.is_control() => " ".to_string(),
            c => c.to_string(),
        })
        .collect();

    let collapsed = mapped.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = collapsed.chars().take(MAX_CHARS).collect();
    out = out.trim_end_matches(['.', ' ']).trim_start().to_string();

    let stem = out.split('.').next().unwrap_or("").trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        out.insert(0, '_');
    }
    if out.is_empty() { "Untitled".to_string() } else { out }
}

#[cfg(test)]
mod tests {
    use super::sanitize_file_name as s;

    #[test]
    fn replaces_separators_and_drops_illegal_chars() {
        assert_eq!(s("SQL: JOINs / CTEs?"), "SQL - JOINs - CTEs");
        assert_eq!(s("a<b>c\"d|e*f"), "abcdef");
        assert_eq!(s("tab\there\nnewline"), "tab here newline");
    }

    #[test]
    fn trims_trailing_dots_and_spaces() {
        assert_eq!(s("  notes...  "), "notes");
    }

    #[test]
    fn guards_reserved_device_names() {
        assert_eq!(s("con"), "_con");
        assert_eq!(s("NUL.txt"), "_NUL.txt");
        assert_eq!(s("Console"), "Console");
    }

    #[test]
    fn caps_length_and_handles_unicode() {
        let long = "ж".repeat(200);
        assert_eq!(s(&long).chars().count(), 120);
        assert_eq!(s("Лекция – 2026-10-13"), "Лекция – 2026-10-13");
    }

    #[test]
    fn falls_back_to_untitled() {
        assert_eq!(s(""), "Untitled");
        assert_eq!(s("???"), "Untitled");
        assert_eq!(s(" . "), "Untitled");
    }
}
