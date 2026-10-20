//! Continuing a note: reading an existing `.md` for the "Previously" box, and adding a new
//! dated session to its end. The old content is never rewritten; only Notetaker's own
//! front matter (`updated`, `sessions`, `sources`) is updated.

use super::write_atomic;
use serde::Serialize;
use std::fs;
use std::path::Path;

/// How much of the end of the note the "Previously" box shows.
const TAIL_CHARS: usize = 1500;

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub path: String,
    pub title: String,
    /// The note's `notetaker_id`, if Notetaker wrote it.
    pub id: Option<String>,
    /// The last part of the note, starting at a line break.
    pub tail: String,
}

/// The front matter's lines (without the `---` fences) and the byte offset where the body starts.
fn front_matter(text: &str) -> Option<(Vec<&str>, usize)> {
    let rest = text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n"))?;
    let mut offset = text.len() - rest.len();
    let mut lines = Vec::new();
    for line in rest.split_inclusive('\n') {
        offset += line.len();
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            return Some((lines, offset));
        }
        lines.push(trimmed);
    }
    None
}

fn yaml_value<'a>(lines: &[&'a str], key: &str) -> Option<&'a str> {
    lines.iter().find_map(|l| l.strip_prefix(key)?.strip_prefix(':').map(str::trim))
}

/// A YAML scalar as written by Notetaker (a JSON string) or by hand (plain text).
fn unquote(value: &str) -> String {
    serde_json::from_str::<String>(value).unwrap_or_else(|_| value.trim_matches(['"', '\'']).to_owned())
}

pub fn summarize(text: &str, path: &str) -> NoteSummary {
    let fm = front_matter(text);
    let lines = fm.as_ref().map(|(l, _)| l.as_slice()).unwrap_or(&[]);
    let body = &text[fm.as_ref().map_or(0, |(_, at)| *at)..];
    let title = yaml_value(lines, "title")
        .map(unquote)
        .or_else(|| body.lines().find_map(|l| l.strip_prefix("# ")).map(|t| t.trim().to_owned()))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
    NoteSummary { path: path.to_owned(), title, id: yaml_value(lines, "notetaker_id").map(unquote), tail: tail(body) }
}

fn tail(body: &str) -> String {
    let body = body.trim_end();
    if body.chars().count() <= TAIL_CHARS {
        return body.trim_start().to_owned();
    }
    let start = body.char_indices().rev().nth(TAIL_CHARS - 1).map_or(0, |(i, _)| i);
    // Start at the next line so the box doesn't open mid-sentence.
    let from = body[start..].find('\n').map_or(start, |n| start + n + 1);
    body[from..].to_owned()
}

/// Updates Notetaker front matter for one more session. Text without it is returned unchanged.
pub fn merge_front_matter(text: &str, updated: &str, sources: &[String]) -> String {
    let Some((lines, body_at)) = front_matter(text) else { return text.to_owned() };
    if yaml_value(&lines, "notetaker_id").is_none() {
        return text.to_owned();
    }
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let quoted: Vec<String> = sources.iter().map(|s| serde_json::to_string(s).expect("strings serialize")).collect();

    let mut out: Vec<String> = Vec::new();
    let mut in_sources = false;
    let mut have_sources = false;
    let mut listed: Vec<String> = Vec::new();
    for line in &lines {
        // Leaving the `sources:` list: add the new entries at its end.
        if in_sources && !line.starts_with("  - ") {
            push_new(&mut out, &quoted, &listed);
            in_sources = false;
        }
        if line.starts_with("updated:") {
            out.push(format!("updated: {updated}"));
        } else if let Some(n) = line.strip_prefix("sessions:").and_then(|v| v.trim().parse::<u32>().ok()) {
            out.push(format!("sessions: {}", n + 1));
        } else if line.trim_end() == "sources:" {
            have_sources = true;
            in_sources = true;
            out.push((*line).to_owned());
        } else {
            if in_sources {
                listed.push(line.trim_start_matches("  - ").to_owned());
            }
            out.push((*line).to_owned());
        }
    }
    if in_sources {
        push_new(&mut out, &quoted, &listed);
    }
    if !have_sources && !quoted.is_empty() {
        out.push("sources:".into());
        push_new(&mut out, &quoted, &[]);
    }
    format!("---{nl}{}{nl}---{nl}{}", out.join(nl), &text[body_at..])
}

fn push_new(out: &mut Vec<String>, quoted: &[String], listed: &[String]) {
    for q in quoted {
        if !listed.contains(q) && !out.iter().any(|l| l == &format!("  - {q}")) {
            out.push(format!("  - {q}"));
        }
    }
}

/// The note with `section` added at the end, after one blank line, in the note's own line endings.
pub fn append_section(text: &str, section: &str) -> String {
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let section = section.trim().replace("\r\n", "\n").replace('\n', nl);
    let body = text.trim_end();
    if body.is_empty() { format!("{section}{nl}") } else { format!("{body}{nl}{nl}{section}{nl}") }
}

#[tauri::command]
pub async fn read_note(path: String) -> Result<NoteSummary, String> {
    let text = fs::read_to_string(&path).map_err(|e| format!("Can't open the note: {e}"))?;
    Ok(summarize(&text, &path))
}

/// Adds a session to the end of `path`. The file is read now, not when the session started,
/// so edits made in VS Code meanwhile are kept.
#[tauri::command]
pub async fn append_session(path: String, section: String, updated: String, sources: Vec<String>) -> Result<String, String> {
    let text = fs::read_to_string(&path).map_err(|e| format!("Can't open the note: {e}"))?;
    let merged = append_section(&merge_front_matter(&text, &updated, &sources), &section);
    write_atomic(Path::new(&path), merged.as_bytes()).map_err(|e| format!("Can't save the note: {e}"))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "---\ntitle: \"Window Functions\"\ncreated: 2026-10-11T19:42\nupdated: 2026-10-11T20:31\nsessions: 1\nnotetaker_id: abc\nplatform: datacamp\nsources:\n  - \"DataCamp · Ex 1\"\n---\n\n# Window Functions\n\n## 2026-10-11 (19:42–20:31)\n\n- one\n";

    #[test]
    fn updates_notetaker_front_matter() {
        let merged = merge_front_matter(NOTE, "2026-10-12T20:10", &["DataCamp · Ex 5".into(), "DataCamp · Ex 1".into()]);
        assert!(merged.contains("updated: 2026-10-12T20:10\n"));
        assert!(merged.contains("sessions: 2\n"));
        assert!(merged.contains("sources:\n  - \"DataCamp · Ex 1\"\n  - \"DataCamp · Ex 5\"\n---\n"));
        assert!(merged.contains("created: 2026-10-11T19:42\n"), "created is kept");
        assert!(merged.ends_with("# Window Functions\n\n## 2026-10-11 (19:42–20:31)\n\n- one\n"), "body untouched");
    }

    #[test]
    fn adds_a_sources_list_when_missing() {
        let note = "---\ntitle: \"T\"\nupdated: x\nsessions: 1\nnotetaker_id: abc\n---\nbody\n";
        let merged = merge_front_matter(note, "y", &["YouTube · A".into()]);
        assert_eq!(merged, "---\ntitle: \"T\"\nupdated: y\nsessions: 2\nnotetaker_id: abc\nsources:\n  - \"YouTube · A\"\n---\nbody\n");
    }

    #[test]
    fn leaves_other_front_matter_alone() {
        let hand_written = "---\ntitle: mine\nupdated: never\n---\n# Mine\n";
        assert_eq!(merge_front_matter(hand_written, "now", &["x".into()]), hand_written);
        let plain = "# Just notes\n\n- a\n";
        assert_eq!(merge_front_matter(plain, "now", &[]), plain);
    }

    #[test]
    fn appends_after_one_blank_line() {
        assert_eq!(append_section("# A\n\n- one\n\n\n", "## 2026-10-12\n\n- two\n"), "# A\n\n- one\n\n## 2026-10-12\n\n- two\n");
        assert_eq!(append_section("", "## S\n"), "## S\n");
    }

    #[test]
    fn keeps_windows_line_endings() {
        let crlf = NOTE.replace('\n', "\r\n");
        let out = append_section(&merge_front_matter(&crlf, "u", &["n".into()]), "## S\n\n- x");
        assert!(!out.replace("\r\n", "").contains('\n'), "no bare LF: {out:?}");
        assert!(out.contains("sessions: 2\r\n"));
        assert!(out.ends_with("- one\r\n\r\n## S\r\n\r\n- x\r\n"));
    }

    #[test]
    fn summary_title_and_id() {
        let s = summarize(NOTE, r"C:\notes\wf.md");
        assert_eq!((s.title.as_str(), s.id.as_deref()), ("Window Functions", Some("abc")));
        assert!(s.tail.starts_with("# Window Functions"));
        let hand = summarize("# Hand written\n\ntext\n", r"C:\notes\h.md");
        assert_eq!((hand.title.as_str(), hand.id), ("Hand written", None));
        assert_eq!(summarize("just text\n", r"C:\notes\Plain note.md").title, "Plain note");
    }

    #[test]
    fn tail_starts_at_a_line() {
        let long = format!("{}\nlast line\n", "word ".repeat(1000));
        let t = tail(&long);
        assert!(t.chars().count() <= TAIL_CHARS);
        assert_eq!(t, "last line");
    }

    #[test]
    fn appending_to_a_file_keeps_the_old_content() {
        let dir = std::env::temp_dir().join(format!("notetaker-test-append-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("n.md");
        fs::write(&path, NOTE).unwrap();
        let p = path.to_string_lossy().into_owned();
        tauri::async_runtime::block_on(append_session(p.clone(), "## 2026-10-12 (19:05–20:10)\n\n- two".into(), "2026-10-12T20:10".into(), vec![])).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("---\ntitle: \"Window Functions\""));
        assert!(text.ends_with("- one\n\n## 2026-10-12 (19:05–20:10)\n\n- two\n"));
        fs::remove_dir_all(dir).unwrap();
    }
}
