//! Read-only inventory of owned TranslateIT data roots. No deletion or mutation.
use std::fs;
use std::path::{Path, PathBuf};
use serde::Serialize;
use crate::engine::paths::ProjectPaths;

#[derive(Debug, Serialize)]
pub struct StorageCategory {
    pub name: &'static str,
    pub bytes: Option<u64>,
    pub files: u64,
    pub directories: u64,
    pub complete: bool,
    pub note: String,
}

#[derive(Debug, Serialize)]
pub struct StorageInventory {
    pub ok: bool,
    pub categories: Vec<StorageCategory>,
    pub message: String,
}

const MAX_VISITS: usize = 250_000;

fn measure(name: &'static str, root: &Path, excluded: Option<&Path>) -> StorageCategory {
    let mut result = StorageCategory {
        name, bytes: Some(0), files: 0, directories: 0, complete: true, note: String::new(),
    };
    // Canonicalize the owned root once. A missing root is empty; an unreadable
    // root is unknown, not zero. Canonical checks also reject Windows junctions.
    let owned_root = match fs::canonicalize(root) {
        Ok(path) => path,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return result,
        Err(_) => {
            result.complete = false;
            result.bytes = None;
            result.note = "Storage root could not be verified.".into();
            return result;
        }
    };
    let mut stack = vec![root.to_path_buf()];
    let mut visited = 0usize;
    // If a directory changes while scanning, prefer an unknown total over a false zero.
    while let Some(path) = stack.pop() {
        // Exclude an entire owned subtree before opening or counting it.
        if excluded.is_some_and(|excluded| path == excluded) { continue; }
        visited += 1;
        if visited > MAX_VISITS {
            result.complete = false;
            result.note = "Inventory limit reached; total is unknown.".into();
            break;
        }
        // symlink_metadata deliberately avoids following junctions/symlinks.
        let metadata = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(err) if path == root && err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                result.complete = false;
                result.note = "Some entries could not be inspected.".into();
                continue;
            }
        };
        // Reject reparse points/junctions escaping the owned root. Files may
        // disappear during a scan; that is an incomplete inventory.
        let inside_owned_root = fs::canonicalize(&path)
            .map(|canonical| canonical.starts_with(&owned_root))
            .unwrap_or(false);
        if !inside_owned_root {
            result.complete = false;
            result.note = "A path changed or escaped the storage root; total is unknown.".into();
            continue;
        }
        if metadata.file_type().is_symlink() {
            result.complete = false;
            result.note = "Linked entries were skipped; total is unknown.".into();
        } else if metadata.is_file() {
            result.files = result.files.saturating_add(1);
            result.bytes = result.bytes.and_then(|bytes| bytes.checked_add(metadata.len()));
            if result.bytes.is_none() {
                result.complete = false;
                result.note = "Storage size overflowed.".into();
            }
        } else if metadata.is_dir() {
            result.directories = result.directories.saturating_add(1);
            match fs::read_dir(&path) {
                Ok(entries) => for entry in entries {
                    match entry {
                        Ok(entry) => stack.push(entry.path()),
                        Err(_) => {
                            result.complete = false;
                            result.note = "Some directory entries could not be read.".into();
                        }
                    }
                },
                Err(_) => {
                    result.complete = false;
                    result.note = "Some directories could not be read.".into();
                }
            }
        } else {
            result.complete = false;
            result.note = "Unsupported filesystem entries were skipped.".into();
        }
    }
    if !result.complete { result.bytes = None; }
    result
}

#[tauri::command]
pub fn get_storage_inventory() -> StorageInventory {
    let paths = ProjectPaths::discover();
    let saved = PathBuf::from(paths.user_saved_dir);
    let cache = PathBuf::from(paths.user_cache_dir);
    let sessions = saved.join("Transcripts").join("Sessions");
    let categories = vec![
        measure("Meeting Transcripts", &sessions, None),
        measure("Other Saved Data (includes exports)", &saved, Some(&sessions)),
        measure("Cache Data", &cache, None),
    ];
    // Categories do not overlap. Unknown sizes must not be included in totals.
    let ok = categories.iter().all(|category| category.complete);
    StorageInventory {
        ok, categories,
        message: if ok {
            "Read-only inventory of non-overlapping categories; no files were modified.".into()
        } else {
            "Storage inventory is incomplete. Unknown sizes are not reported as zero.".into()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_root_is_empty() {
        let path = std::env::temp_dir().join(format!("translateit-missing-inventory-{}", std::process::id()));
        let result = measure("missing", &path, None);
        assert!(result.complete);
        assert_eq!(result.bytes, Some(0));
    }
    #[test]
    fn excluded_root_is_not_counted() {
        let path = std::env::temp_dir().join(format!("translateit-excluded-inventory-{}", std::process::id()));
        fs::write(&path, b"abc").unwrap();
        let result = measure("excluded", &path, Some(&path));
        assert!(result.complete);
        assert_eq!(result.bytes, Some(0));
        let _ = fs::remove_file(path);
    }
    #[test]
    fn excluded_subtree_is_not_counted() {
        let root = std::env::temp_dir().join(format!("translateit-inventory-subtree-{}", std::process::id()));
        let excluded = root.join("Sessions");
        fs::create_dir_all(&excluded).unwrap();
        fs::write(excluded.join("transcript.txt"), b"12345").unwrap();
        fs::write(root.join("other.txt"), b"abc").unwrap();
        let result = measure("other", &root, Some(&excluded));
        assert!(result.complete);
        assert_eq!(result.files, 1);
        assert_eq!(result.bytes, Some(3));
        let _ = fs::remove_dir_all(root);
    }
    #[cfg(unix)]
    #[test]
    fn external_symlink_is_not_followed() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("translateit-inventory-link-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        symlink(std::env::temp_dir(), root.join("outside")).unwrap();
        let result = measure("linked", &root, None);
        assert!(!result.complete);
        assert_eq!(result.bytes, None);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn regular_file_is_counted() {
        let path = std::env::temp_dir().join(format!("translateit-inventory-file-{}", std::process::id()));
        fs::write(&path, b"abc").unwrap();
        let result = measure("file", &path, None);
        assert!(result.complete);
        assert_eq!(result.bytes, Some(3));
        let _ = fs::remove_file(path);
    }
}
