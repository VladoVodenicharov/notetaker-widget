//! `notes-index.json`: which saved note belongs to which sources, so the panel can suggest
//! "Continue 'PostgreSQL Window Functions'?" when the same video, course or meeting comes back.

use super::write_atomic;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const FILE: &str = "notes-index.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub id: Option<String>,
    pub source_keys: Vec<String>,
    /// Milliseconds since 1970; the newest match wins.
    pub updated: u64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Suggestion {
    pub path: String,
    pub title: String,
}

fn same_path(a: &str, b: &str) -> bool {
    // Windows paths are case-insensitive.
    a.replace('/', "\\").eq_ignore_ascii_case(&b.replace('/', "\\"))
}

/// Adds or refreshes the note's entry. Source keys from earlier sessions are kept.
pub fn upsert(entries: &mut Vec<Entry>, mut entry: Entry) {
    if let Some(old) = entries.iter().position(|e| same_path(&e.path, &entry.path) || (e.id.is_some() && e.id == entry.id)) {
        let old = entries.remove(old);
        for key in old.source_keys {
            if !entry.source_keys.contains(&key) {
                entry.source_keys.push(key);
            }
        }
        entry.id = entry.id.or(old.id);
    }
    entries.push(entry);
}

/// The most recently updated note that shares a source key and still exists.
/// Entries whose file is gone are dropped; returns whether anything was dropped.
pub fn suggest(entries: &mut Vec<Entry>, keys: &[String], exists: impl Fn(&str) -> bool) -> (Option<Suggestion>, bool) {
    let before = entries.len();
    entries.retain(|e| exists(&e.path));
    let best = entries
        .iter()
        .filter(|e| e.source_keys.iter().any(|k| keys.contains(k)))
        .max_by_key(|e| e.updated)
        .map(|e| Suggestion { path: e.path.clone(), title: e.title.clone() });
    (best, entries.len() != before)
}

fn index_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(FILE))
}

/// A missing or damaged index is treated as empty; it only drives suggestions.
fn load(path: &Path) -> Vec<Entry> {
    fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn store(path: &Path, entries: &[Entry]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    write_atomic(path, json.as_bytes()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn index_note(app: AppHandle, entry: Entry) -> Result<(), String> {
    let path = index_path(&app)?;
    let mut entries = load(&path);
    upsert(&mut entries, entry);
    store(&path, &entries)
}

#[tauri::command]
pub async fn suggest_note(app: AppHandle, keys: Vec<String>) -> Result<Option<Suggestion>, String> {
    let path = index_path(&app)?;
    let mut entries = load(&path);
    let (best, pruned) = suggest(&mut entries, &keys, |p| Path::new(p).exists());
    if pruned {
        store(&path, &entries)?;
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, keys: &[&str], updated: u64) -> Entry {
        Entry { path: path.into(), title: path.into(), id: None, source_keys: keys.iter().map(|k| k.to_string()).collect(), updated }
    }

    #[test]
    fn upsert_merges_keys_for_the_same_file() {
        let mut entries = vec![entry(r"C:\Notes\a.md", &["youtube:a"], 1)];
        upsert(&mut entries, entry(r"c:/notes/A.md", &["youtube:b"], 2));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source_keys, ["youtube:b", "youtube:a"]);
        assert_eq!(entries[0].updated, 2);
    }

    #[test]
    fn upsert_follows_a_renamed_file_by_id() {
        let mut entries = vec![Entry { id: Some("x".into()), ..entry(r"C:\old.md", &["k"], 1) }];
        upsert(&mut entries, Entry { id: Some("x".into()), ..entry(r"C:\new.md", &[], 2) });
        assert_eq!((entries.len(), entries[0].path.as_str()), (1, r"C:\new.md"));
    }

    #[test]
    fn suggests_the_newest_matching_note_that_still_exists() {
        let mut entries = vec![entry("old.md", &["datacamp:sql"], 1), entry("new.md", &["datacamp:sql"], 5), entry("gone.md", &["datacamp:sql"], 9), entry("other.md", &["youtube:x"], 7)];
        let (best, pruned) = suggest(&mut entries, &["datacamp:sql".into()], |p| p != "gone.md");
        assert_eq!(best, Some(Suggestion { path: "new.md".into(), title: "new.md".into() }));
        assert!(pruned);
        assert_eq!(entries.len(), 3);
        assert_eq!(suggest(&mut entries, &["teams:standup".into()], |_| true).0, None);
    }
}
