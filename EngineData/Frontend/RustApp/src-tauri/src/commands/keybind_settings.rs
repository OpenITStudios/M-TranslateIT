//! Inert keybind configuration storage. No OS shortcut registration or dispatch.
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use crate::engine::paths::ProjectPaths;

const VERSION: u32 = 1;
const MAX_BYTES: u64 = 8192;
const ACTIONS: [&str; 6] = [
    "toggle-microphone", "toggle-deafen", "push-to-talk",
    "toggle-main-window", "toggle-floating-captions", "start-stop-meeting",
];
static IO_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
fn lock() -> &'static Mutex<()> { IO_LOCK.get_or_init(|| Mutex::new(())) }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    action: String,
    scope: String,
    chord: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    version: u32,
    assignments: Vec<Assignment>,
}
#[derive(Serialize)]
pub struct KeybindSettingsResult {
    ok: bool,
    configuration: Option<Configuration>,
    message: String,
}

fn defaults() -> Configuration {
    Configuration {
        version: VERSION,
        assignments: ACTIONS.iter().map(|action| Assignment {
            action: (*action).to_string(), scope: "local".into(), chord: None,
        }).collect(),
    }
}
fn validate_inert(config: &Configuration) -> Result<(), &'static str> {
    if config.version != VERSION || config.assignments.len() != ACTIONS.len() {
        return Err("Unsupported keybind configuration version or action count.");
    }
    for (index, item) in config.assignments.iter().enumerate() {
        if item.action != ACTIONS[index] || !matches!(item.scope.as_str(), "local" | "global") {
            return Err("Unknown, reordered, or duplicate keybind action.");
        }
        // Until registration/rollback is implemented, NEVER persist assigned
        // chords that might appear active but have no native owner.
        if item.chord.is_some() {
            return Err("Assigned shortcuts are unavailable until native registration is implemented.");
        }
    }
    Ok(())
}
fn config_path() -> PathBuf {
    PathBuf::from(ProjectPaths::discover().user_saved_dir).join("keybind_preferences_v1.json")
}
fn read_file(path: &Path) -> Result<Configuration, String> {
    if fs::symlink_metadata(path).map(|meta| meta.file_type().is_symlink()).unwrap_or(false) {
        return Err("Linked keybind settings file is not trusted.".into());
    }
    let mut file = File::open(path).map_err(|_| "Cannot open saved keybind settings.")?;
    let metadata = file.metadata().map_err(|_| "Cannot inspect saved keybind settings.")?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err("Saved keybind settings are not a supported file.");
    }
    let mut body = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut body)
        .map_err(|_| "Cannot read saved keybind settings.")?;
    if body.len() as u64 > MAX_BYTES { return Err("Saved keybind settings are too large.".into()); }
    let config: Configuration = serde_json::from_slice(&body)
        .map_err(|_| "Saved keybind settings are invalid.".to_string())?;
    validate_inert(&config).map_err(str::to_string)?;
    Ok(config)
}
fn transaction_artifact_present(path: &Path) -> bool {
    [path.with_extension("json.bak"), path.with_extension("json.tmp")]
        .iter()
        // symlink_metadata detects dangling links that Path::exists misses.
        .any(|artifact| fs::symlink_metadata(artifact).is_ok())
}

fn read_locked(path: &Path) -> KeybindSettingsResult {
    // A valid primary is not sufficient evidence that a previous save finished.
    // Never report clean success while backup/temporary artifacts remain.
    if transaction_artifact_present(path) {
        return KeybindSettingsResult {
            ok: false, configuration: None,
            message: "An interrupted keybind settings transaction requires recovery; no shortcuts were activated.".into(),
        };
    }
    match read_file(path) {
        Ok(configuration) => KeybindSettingsResult { ok: true, configuration: Some(configuration), message: "Saved keybind preferences verified. Shortcuts remain unassigned.".into() },
        Err(_) if matches!(fs::symlink_metadata(path), Err(error) if error.kind() == std::io::ErrorKind::NotFound) => {
            // Missing primary is only a clean default when no incomplete/backup
            // transaction exists. Never overwrite uncertain persisted state.
            KeybindSettingsResult { ok: true, configuration: Some(defaults()), message: "No saved keybind preferences; all shortcuts are unassigned.".into() }
        }
        Err(message) => KeybindSettingsResult { ok: false, configuration: None, message },
    }
}

#[tauri::command]
pub fn load_keybind_settings() -> KeybindSettingsResult {
    let Ok(_guard) = lock().lock() else {
        return KeybindSettingsResult { ok: false, configuration: None, message: "Keybind settings are unavailable.".into() };
    };
    read_locked(&config_path())
}

/// Explicit recovery never trusts an unfinished temporary write.
/// Prefer a verified primary; otherwise restore a verified backup without
/// deleting the backup before the replacement is confirmed.
#[tauri::command]
pub fn recover_keybind_settings() -> KeybindSettingsResult {
    let Ok(_guard) = lock().lock() else {
        return KeybindSettingsResult { ok: false, configuration: None, message: "Keybind settings are unavailable.".into() };
    };
    recover_locked(&config_path())
}

fn recover_locked(path: &Path) -> KeybindSettingsResult {
    let backup = path.with_extension("json.bak");
    let temp = path.with_extension("json.tmp");
    // Inspect both artifacts BEFORE any restoration or cleanup. A linked or
    // unreadable artifact must never cause partial mutation of other files.
    for artifact in [&temp, &backup] {
        match fs::symlink_metadata(artifact) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
                return KeybindSettingsResult { ok: false, configuration: None, message: "Untrusted keybind transaction artifact requires manual inspection.".into() };
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return KeybindSettingsResult { ok: false, configuration: None, message: "Could not inspect interrupted keybind transaction.".into() },
        }
    }
    // Never accept an unreadable/linked primary as equivalent to a missing
    // primary. A permissions failure must not authorize overwriting it.
    if read_file(path).is_err() {
        if read_file(&backup).is_err() {
            return KeybindSettingsResult { ok: false, configuration: None, message: "Neither primary nor backup keybind preferences could be verified; recovery was not attempted.".into() };
        }
        // Do not overwrite a damaged primary silently. Manual inspection is
        // required unless the primary is verifiably missing.
        if !matches!(fs::symlink_metadata(path), Err(error) if error.kind() == std::io::ErrorKind::NotFound) {
            return KeybindSettingsResult { ok: false, configuration: None, message: "Primary keybind preferences are invalid or inaccessible; recovery requires manual inspection.".into() };
        }
        // Do not allow restoration to follow a link at the destination.
        let restored = (|| -> std::io::Result<()> {
            let mut source = File::open(&backup)?;
            let mut target = OpenOptions::new().write(true).create_new(true).open(path)?;
            std::io::copy(&mut source, &mut target)?;
            target.sync_all()
        })();
        if restored.is_err() || read_file(path).is_err() {
            return KeybindSettingsResult { ok: false, configuration: None, message: "Could not verify restoration of keybind preferences.".into() };
        }
    }
    // Only known artifacts from this command are removed after verifying a
    // committed primary. Refuse linked artifacts instead of following them.
    for artifact in [&temp, &backup] {
        match fs::symlink_metadata(artifact) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
                return KeybindSettingsResult { ok: false, configuration: None, message: "Untrusted keybind transaction artifact requires manual inspection.".into() };
            }
            Ok(_) => if fs::remove_file(artifact).is_err() {
                return KeybindSettingsResult { ok: false, configuration: None, message: "Could not clear interrupted keybind transaction.".into() };
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return KeybindSettingsResult { ok: false, configuration: None, message: "Could not inspect interrupted keybind transaction.".into() },
        }
    }
    read_locked(path)
}

#[tauri::command]
pub fn save_keybind_settings(configuration: Configuration) -> KeybindSettingsResult {
    if let Err(message) = validate_inert(&configuration) {
        return KeybindSettingsResult { ok: false, configuration: None, message: message.into() };
    }
    let Ok(_guard) = lock().lock() else {
        return KeybindSettingsResult { ok: false, configuration: None, message: "Keybind settings are unavailable.".into() };
    };
    let path = config_path();
    // Refuse ambiguous recovery rather than overwriting an invalid primary.
    let prior = read_locked(&path);
    if !prior.ok { return prior; }
    let body = match serde_json::to_vec_pretty(&configuration) {
        Ok(body) if body.len() as u64 <= MAX_BYTES => body,
        _ => return KeybindSettingsResult { ok: false, configuration: None, message: "Keybind settings serialization failed.".into() },
    };
    let operation = (|| -> std::io::Result<()> {
        if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
        let temp = path.with_extension("json.tmp");
        let backup = path.with_extension("json.bak");
        if transaction_artifact_present(&path) {
            return Err(std::io::Error::other("unfinished keybind transaction"));
        }
        // No destructive replacement: preserve the last committed primary if
        // a platform cannot atomically replace the destination.
        {
            let mut file = OpenOptions::new().write(true).create_new(true).open(&temp)?;
            file.write_all(&body)?;
            file.sync_all()?;
        }
        if path.exists() {
            fs::copy(&path, &backup)?;
            if let Err(error) = fs::rename(&temp, &path) {
                // Windows may refuse replacement. Leave the old primary and
                // backup intact; next read reports recovery required.
                return Err(error);
            }
            // Failed cleanup must not be reported as a successful transaction.
            fs::remove_file(backup)?;
        } else {
            fs::rename(&temp, &path)?;
        }
        Ok(())
    })();
    match operation {
        Ok(()) => read_locked(&path),
        Err(_) => KeybindSettingsResult { ok: false, configuration: None, message: "Could not save keybind preferences. Previous settings were not intentionally deleted; recovery may be required.".into() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_inert() { assert!(validate_inert(&defaults()).is_ok()); }
    #[test]
    fn assigned_shortcut_is_rejected() {
        let mut config = defaults();
        config.assignments[0].chord = Some("Ctrl+Shift+M".into());
        assert!(validate_inert(&config).is_err());
    }
    #[test]
    fn interrupted_transaction_is_not_reported_as_saved() {
        let root = std::env::temp_dir().join(format!("translateit-keybind-interrupted-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("keybind_preferences_v1.json");
        fs::write(&path, serde_json::to_vec(&defaults()).unwrap()).unwrap();
        fs::write(path.with_extension("json.tmp"), b"partial").unwrap();
        assert!(!read_locked(&path).ok);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn recovery_clears_only_verified_transaction_artifacts() {
        let root = std::env::temp_dir().join(format!("translateit-keybind-recovery-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("keybind_preferences_v1.json");
        fs::write(&path, serde_json::to_vec(&defaults()).unwrap()).unwrap();
        fs::write(path.with_extension("json.tmp"), b"partial").unwrap();
        assert!(!read_locked(&path).ok);
        assert!(recover_locked(&path).ok);
        assert!(read_locked(&path).ok);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn recovery_restores_missing_primary_from_valid_backup() {
        let root = std::env::temp_dir().join(format!("translateit-keybind-backup-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("keybind_preferences_v1.json");
        fs::write(path.with_extension("json.bak"), serde_json::to_vec(&defaults()).unwrap()).unwrap();
        assert!(recover_locked(&path).ok);
        assert!(read_locked(&path).ok);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_primary_is_not_overwritten() {
        let root = std::env::temp_dir().join(format!("translateit-keybind-invalid-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("keybind_preferences_v1.json");
        fs::write(&path, b"corrupt").unwrap();
        fs::write(path.with_extension("json.bak"), serde_json::to_vec(&defaults()).unwrap()).unwrap();
        assert!(!recover_locked(&path).ok);
        assert_eq!(fs::read(&path).unwrap(), b"corrupt");
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn linked_temp_cannot_trigger_partial_backup_restoration() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("translateit-keybind-no-partial-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("keybind_preferences_v1.json");
        let backup = path.with_extension("json.bak");
        fs::write(&backup, serde_json::to_vec(&defaults()).unwrap()).unwrap();
        symlink(&backup, path.with_extension("json.tmp")).unwrap();
        assert!(!recover_locked(&path).ok);
        assert!(!path.exists(), "linked temp must not create primary");
        assert!(backup.exists(), "backup must remain intact");
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn recovery_rejects_linked_transaction_artifact() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("translateit-keybind-linked-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("keybind_preferences_v1.json");
        fs::write(&path, serde_json::to_vec(&defaults()).unwrap()).unwrap();
        symlink(&path, path.with_extension("json.tmp")).unwrap();
        assert!(!recover_locked(&path).ok);
        assert!(path.with_extension("json.tmp").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unknown_action_is_rejected() {
        let mut config = defaults();
        config.assignments[0].action = "unknown".into();
        assert!(validate_inert(&config).is_err());
    }
}
