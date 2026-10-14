//! Crash-safe autosave of the note being written. There is only ever one draft:
//! `drafts/draft.json` plus `draft.json.bak`, the version before it.

use super::write_atomic;
use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const FILE: &str = "draft.json";
const BACKUP: &str = "draft.json.bak";

fn drafts_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("drafts");
    fs::create_dir_all(&dir).map_err(|e| format!("Can't create drafts folder: {e}"))?;
    Ok(dir)
}

/// Keeps the previous draft as `.bak`, then writes the new one atomically.
/// Whenever the process dies, at least one of the two is a complete file.
fn write(dir: &Path, json: &str) -> io::Result<()> {
    let path = dir.join(FILE);
    if path.exists() {
        fs::rename(&path, dir.join(BACKUP))?;
    }
    write_atomic(&path, json.as_bytes())
}

/// The newest draft that still parses: the main file, or the backup if that is damaged.
fn read(dir: &Path) -> Option<Value> {
    [FILE, BACKUP]
        .iter()
        .find_map(|name| serde_json::from_str(&fs::read_to_string(dir.join(name)).ok()?).ok())
}

fn delete(dir: &Path) {
    for name in [FILE, BACKUP] {
        let _ = fs::remove_file(dir.join(name));
    }
}

/// The frontend calls these one at a time (it queues them), so no locking is needed here.
#[tauri::command]
pub async fn save_draft(app: AppHandle, draft: Value) -> Result<(), String> {
    write(&drafts_dir(&app)?, &draft.to_string()).map_err(|e| format!("Can't save draft: {e}"))
}

#[tauri::command]
pub async fn load_draft(app: AppHandle) -> Result<Option<Value>, String> {
    Ok(read(&drafts_dir(&app)?))
}

#[tauri::command]
pub async fn delete_draft(app: AppHandle) -> Result<(), String> {
    delete(&drafts_dir(&app)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("notetaker-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn keeps_the_previous_version_as_backup() {
        let dir = temp_dir("draft-write");
        write(&dir, r#"{"v":1}"#).unwrap();
        write(&dir, r#"{"v":2}"#).unwrap();

        assert_eq!(fs::read_to_string(dir.join(FILE)).unwrap(), r#"{"v":2}"#);
        assert_eq!(fs::read_to_string(dir.join(BACKUP)).unwrap(), r#"{"v":1}"#);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2, "no .tmp files left behind");
        assert_eq!(read(&dir), Some(json!({"v": 2})));

        delete(&dir);
        assert_eq!(read(&dir), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn recovers_from_backup_when_the_draft_is_corrupt() {
        let dir = temp_dir("draft-corrupt");
        write(&dir, r#"{"v":1}"#).unwrap();
        write(&dir, r#"{"v":2}"#).unwrap();
        fs::write(dir.join(FILE), r#"{"v":2, "doc": {"ty"#).unwrap(); // torn write

        assert_eq!(read(&dir), Some(json!({"v": 1})));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn recovers_from_backup_when_the_draft_is_missing() {
        // A crash between moving the draft to .bak and writing the new one.
        let dir = temp_dir("draft-missing");
        write(&dir, r#"{"v":1}"#).unwrap();
        fs::rename(dir.join(FILE), dir.join(BACKUP)).unwrap();

        assert_eq!(read(&dir), Some(json!({"v": 1})));
        fs::remove_dir_all(dir).unwrap();
    }
}
