//! Writing notes (and drafts of unfinished ones) to disk.

pub mod drafts;
mod sanitize;

use sanitize::sanitize_file_name;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::{thread, time::Duration};

/// Saves `content` as `<folder>/<name>.md`, adding " (2)", " (3)", … instead of overwriting.
/// Returns the full path that was written. Async so disk waits stay off the UI thread.
#[tauri::command]
pub async fn save_note(folder: String, name: String, content: String) -> Result<String, String> {
    let dir = PathBuf::from(folder);
    fs::create_dir_all(&dir).map_err(|e| format!("Can't create folder: {e}"))?;
    let path = unique_path(&dir, &sanitize_file_name(&name));
    write_atomic(&path, content.as_bytes()).map_err(|e| format!("Can't save note: {e}"))?;
    Ok(path.to_string_lossy().into_owned())
}

fn unique_path(dir: &Path, stem: &str) -> PathBuf {
    let mut path = dir.join(format!("{stem}.md"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stem} ({n}).md"));
        n += 1;
    }
    path
}

/// Write to a temp file, flush it to disk, then rename over the target, so a crash
/// never leaves a half-written note. Retries the rename briefly in case OneDrive or
/// an antivirus scanner has the target open.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    for attempt in 1..=3 {
        if fs::rename(&tmp, path).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(150 * attempt));
    }
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("notetaker-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn saves_and_never_overwrites() {
        let dir = temp_dir("save");
        let folder = dir.to_string_lossy().into_owned();
        let first = tauri::async_runtime::block_on(save_note(folder.clone(), "SQL: JOINs?".into(), "one\n".into())).unwrap();
        let second = tauri::async_runtime::block_on(save_note(folder, "SQL: JOINs?".into(), "two\n".into())).unwrap();

        assert!(first.ends_with("SQL - JOINs.md"));
        assert!(second.ends_with("SQL - JOINs (2).md"));
        assert_eq!(fs::read_to_string(&first).unwrap(), "one\n");
        assert_eq!(fs::read_to_string(&second).unwrap(), "two\n");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2, "no .tmp files left behind");
        fs::remove_dir_all(dir).unwrap();
    }
}
