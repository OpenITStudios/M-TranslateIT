//! Inert keybind configuration storage. No OS shortcut registration or dispatch.
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
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
fn read_locked(path: &Path) -> KeybindSettingsResult {
    match read_file(path) {
        Ok(configuration) => KeybindSettingsResult { ok: true, configuration: Some(configuration), message: "Saved keybind preferences verified. Shortcuts remain unassigned.".into() },
        Err(_) if matches!(fs::symlink_metadata(path), Err(error) if error.kind() == std::io::ErrorKind::NotFound) => {
            // Missing primary is only a clean default when no incomplete/backup
            // transaction exists. Never overwrite uncertain persisted state.
            let backup = path.with_extension("json.bak");
            let temp = path.with_extension("json.tmp");
            if backup.exists() || temp.exists() {
                KeybindSettingsResult { ok: false, configuration: None, message: "Keybind settings recovery is required; no shortcuts were activated.".into() }
            } else {
                KeybindSettingsResult { ok: true, configuration: Some(defaults()), message: "No saved keybind preferences; all shortcuts are unassigned.".into() }
            }
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
        if temp.exists() || backup.exists() {
            return Err(std::io::Error::other("unfinished keybind transaction"));
        }
        // No destructive replacement: preserve the last committed primary if
        // a platform cannot atomically replace the destination.
        {
            let mut file = File::create(&temp)?;
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
            let _ = fs::remove_file(backup);
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
    fn unknown_action_is_rejected() {
        let mut config = defaults();
        config.assignments[0].action = "unknown".into();
        assert!(validate_inert(&config).is_err());
    }
}
