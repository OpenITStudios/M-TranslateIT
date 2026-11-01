use serde::Serialize;
use std::path::PathBuf;

use crate::commands::audio::{probe_input_device_candidate, probe_output_device_candidate};
use crate::commands::diagnostic_trace::{trace_command_end, trace_command_start};
use crate::engine;
use crate::engine::logging::{write_jsonl_event, RuntimeLogEvent};
use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_state::{
    latest_runtime_session_state, APPLICATION_MEETING_OWNER_ID, MIC_TEST_OWNER_ID,
    VOICE_RECORDING_OWNER_ID,
};
use crate::engine::settings::{MeetingEngineAdmission, RuntimeSettings};
use crate::engine::state::{CommandResult, LifecycleState};

#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceSelectionResult {
    pub ok: bool,
    pub kind: String,
    pub device_id: Option<String>,
    pub device_name: String,
    pub message: String,
    pub settings: RuntimeSettings,
}

fn normalized_device_id(device_id: Option<String>) -> Option<String> {
    device_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn persist_runtime_settings(settings: RuntimeSettings) -> CommandResult {
    let project_paths = ProjectPaths::discover();
    let settings_path =
        PathBuf::from(&project_paths.user_cache_dir).join("rust_runtime_settings.json");
    let log_dir = PathBuf::from(project_paths.user_log_dir);
    let settings = settings.sanitized();

    match settings.save_pretty(&settings_path) {
        Ok(()) => {
            let _ = write_jsonl_event(
                &log_dir,
                "rust_runtime_latest.jsonl",
                &RuntimeLogEvent::info("settings", "Runtime settings saved."),
            );
            CommandResult::ok(LifecycleState::Idle, "Runtime settings saved.")
        }
        Err(_) => {
            let _ = write_jsonl_event(
                &log_dir,
                "rust_runtime_latest.jsonl",
                &RuntimeLogEvent::warning("settings", "Runtime settings save failed."),
            );
            CommandResult::blocked(
                LifecycleState::Error,
                "Failed to save runtime settings. Open Diagnostics for details.",
            )
        }
    }
}

fn runtime_session_owns_audio_resources() -> bool {
    // The runtime store is fail-closed on unreadable session authority.
    // An orphaned Meeting writer/audio cleanup can remain after the session
    // snapshot was cleared, so an idle snapshot alone is NOT rebind approval.
    latest_runtime_session_state().has_active_session
        || super::meeting_session::meeting_stop_cleanup_pending()
}

fn audio_resource_lock_message(action: &str) -> String {
    let runtime = latest_runtime_session_state();
    let owner = runtime
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.owner_id.as_str());

    let activity = match owner {
        Some(APPLICATION_MEETING_OWNER_ID) => "Translation",
        Some(MIC_TEST_OWNER_ID) => "Mic Test",
        Some(VOICE_RECORDING_OWNER_ID) => "My Voice recording",
        Some(_) => "the current audio action",
        None => "the current audio action",
    };

    format!("Stop {activity} before {action}. The current audio preferences were kept.")
}

fn engine_preference_changed(current: &RuntimeSettings, candidate: &RuntimeSettings) -> bool {
    current.translation_engine != candidate.translation_engine
}

// Only a literal user-selected engine is acceptable for explicit repair.
// Normal full-settings saves must never turn an uncertain disk state into
// an implicitly authorized Local translator.
fn explicit_engine_choice(engine: &str) -> Option<MeetingEngineAdmission> {
    match engine {
        "local" => Some(MeetingEngineAdmission::Local),
        "codex" => Some(MeetingEngineAdmission::Codex),
        _ => None,
    }
}

fn audio_preferences_changed(current: &RuntimeSettings, candidate: &RuntimeSettings) -> bool {
    current.audio.input_device_id != candidate.audio.input_device_id
        || current.audio.output_device_id != candidate.audio.output_device_id
}

fn current_device_label(settings: &RuntimeSettings, kind: &str) -> String {
    let selected = if kind == "microphone" {
        settings.audio.input_device_id.as_deref()
    } else {
        settings.audio.output_device_id.as_deref()
    };
    selected
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Windows Default")
        .to_string()
}

#[tauri::command]
pub fn load_runtime_settings() -> RuntimeSettings {
    let started = trace_command_start(
        "load_runtime_settings",
        "loading runtime settings for launcher startup",
    );
    let settings = engine::load_settings();
    trace_command_end("load_runtime_settings", started, "ok");
    settings
}

pub fn save_runtime_settings(settings: RuntimeSettings) -> CommandResult {
    let started = trace_command_start("save_runtime_settings", "saving runtime settings");
    let _meeting_transition = match super::meeting_session::try_settings_mutation_guard() {
        Ok(guard) => guard,
        Err(_) => {
            let result = CommandResult::blocked(
                LifecycleState::Error,
                "A Meeting Start/Stop transaction is in progress or unavailable. No Settings change was saved.".to_string(),
            );
            trace_command_end("save_runtime_settings", started, "meeting_transition_unavailable");
            return result;
        }
    };
    let current = engine::load_settings();
    let candidate = settings.sanitized();
    let unverified_engine = engine::meeting_engine_admission()
        == MeetingEngineAdmission::Unverified;
    let result = if unverified_engine {
        // A damaged primary/backup is NOT a clean first install. Even if the
        // caller presents an apparently unchanged Local choice, only the
        // narrow explicit engine confirmation command may repair that state.
        CommandResult::blocked(
            LifecycleState::Error,
            "Saved translation engine settings could not be verified. Confirm Local or Codex explicitly before saving other Settings.".to_string(),
        )
    } else if runtime_session_owns_audio_resources()
        && audio_preferences_changed(&current, &candidate)
    {
        CommandResult::blocked(
            LifecycleState::Error,
            audio_resource_lock_message("changing audio devices"),
        )
    } else if runtime_session_owns_audio_resources()
        && engine_preference_changed(&current, &candidate)
    {
        // The selected translation engine is a property of the complete
        // Meeting generation, not a mutable mid-session preference.
        CommandResult::blocked(
            LifecycleState::Error,
            audio_resource_lock_message("changing the translation engine"),
        )
    } else {
        persist_runtime_settings(candidate)
    };
    trace_command_end(
        "save_runtime_settings",
        started,
        format!("state={}", result.state),
    );
    result
}

// Exact-purpose recovery: the caller provides only the chosen engine, not
// an arbitrary full Settings object that may contain stale device/voice state.
// This is an explicit native intent, not proof of operating-system user identity.
pub fn confirm_unverified_translation_engine(engine: String) -> CommandResult {
    let started = trace_command_start(
        "confirm_unverified_translation_engine", "confirming saved translation engine",
    );
    let result = confirm_unverified_translation_engine_inner(&engine);
    trace_command_end(
        "confirm_unverified_translation_engine", started,
        format!("state={} ok={}", result.state, result.ok),
    );
    result
}

fn confirm_unverified_translation_engine_inner(engine: &str) -> CommandResult {
    let Some(expected) = explicit_engine_choice(engine) else {
        return CommandResult::blocked(
            LifecycleState::Error,
            "Choose exactly Local or Codex. No engine settings were changed.",
        );
    };
    let _meeting_transition = match super::meeting_session::try_settings_mutation_guard() {
        Ok(guard) => guard,
        Err(_) => {
            return CommandResult::blocked(
                LifecycleState::Error,
                "A Meeting Start/Stop or Settings change is in progress. Engine confirmation was not saved.",
            );
        }
    };
    if runtime_session_owns_audio_resources() {
        return CommandResult::blocked(
            LifecycleState::Error,
            audio_resource_lock_message("confirming the translation engine"),
        );
    }
    if engine::meeting_engine_admission() != MeetingEngineAdmission::Unverified {
        // A stale UI confirmation must not overwrite a newly verified engine.
        // Ordinary, already-verified engine changes use save_product_settings.
        return CommandResult::blocked(
            LifecycleState::Error,
            "The engine settings have changed since recovery was requested. Refresh Settings before choosing again.",
        );
    }

    let mut recovered = engine::load_settings();
    recovered.translation_engine = engine.to_string();
    let saved = persist_runtime_settings(recovered);
    if !saved.ok {
        return saved;
    }
    if engine::meeting_engine_admission() != expected {
        // A completed write is not itself proof of the canonical admission.
        return CommandResult::blocked(
            LifecycleState::Error,
            "Engine preference was saved, but its current admission could not be verified. Refresh Settings.",
        );
    }
    CommandResult::ok(
        LifecycleState::Idle,
        if expected == MeetingEngineAdmission::Local {
            "Local engine explicitly confirmed. Review microphone and other preferences before starting a Meeting."
        } else {
            "Codex preference confirmed. Review other settings; Codex Voice translation remains unavailable."
        },
    )
}

pub fn select_audio_device(kind: String, device_id: Option<String>) -> AudioDeviceSelectionResult {
    let started = trace_command_start("select_audio_device", format!("kind={kind}"));
    let current = engine::load_settings();
    let requested = normalized_device_id(device_id);

    if !matches!(kind.as_str(), "microphone" | "meeting-sound") {
        trace_command_end("select_audio_device", started, "invalid_kind");
        return AudioDeviceSelectionResult {
            ok: false,
            kind,
            device_id: requested,
            device_name: "Unknown device".to_string(),
            message: "That audio device type is not supported.".to_string(),
            settings: current,
        };
    }

    // A Meeting session and Mic Test own live audio resources until their canonical
    // Stop path releases the runtime session. Persisting a different device while a
    // stream still owns the previous endpoint would make Settings disagree with the
    // active runtime. Defer the change instead of attempting a mid-session hot rebind.
    // Device probes and their subsequent full-settings saves must not race
    // an engine preference update or the canonical Meeting Start/Stop.
    let _meeting_transition = match super::meeting_session::try_settings_mutation_guard() {
        Ok(guard) => guard,
        Err(_) => {
            trace_command_end("select_audio_device", started, "meeting_transition_unavailable");
            return AudioDeviceSelectionResult {
                ok: false,
                kind: kind.clone(),
                device_id: requested,
                device_name: current_device_label(&current, &kind),
                message: "A Meeting Start/Stop or Settings change is in progress. No device preference was saved.".to_string(),
                settings: current,
            };
        }
    };
    // Always read again after acquiring the mutation guard: a stale audio
    // snapshot must never silently overwrite a concurrently saved Codex choice.
    let current = engine::load_settings();
    if engine::meeting_engine_admission()
        == crate::engine::settings::MeetingEngineAdmission::Unverified
    {
        trace_command_end("select_audio_device", started, "settings_engine_unverified");
        return AudioDeviceSelectionResult {
            ok: false,
            kind: kind.clone(),
            device_id: requested,
            device_name: current_device_label(&current, &kind),
            message: "Saved translation engine settings could not be verified. Reconfirm the engine before changing audio devices.".to_string(),
            settings: current,
        };
    }
    if runtime_session_owns_audio_resources() {
        trace_command_end(
            "select_audio_device",
            started,
            "active_runtime_session_locked",
        );
        return AudioDeviceSelectionResult {
            ok: false,
            kind: kind.clone(),
            device_id: requested,
            device_name: current_device_label(&current, &kind),
            message: audio_resource_lock_message("changing audio devices")
                .replace("audio preferences", "preference"),
            settings: current,
        };
    }

    let (available, device_name) = match kind.as_str() {
        "microphone" => {
            let probe = probe_input_device_candidate(requested.clone());
            (
                probe.prepared && probe.functional_verified,
                probe
                    .input_device_name
                    .clone()
                    .or_else(|| requested.clone())
                    .unwrap_or_else(|| "Windows Default".to_string()),
            )
        }
        "meeting-sound" => {
            let probe = probe_output_device_candidate(requested.clone());
            (
                probe.ok,
                probe
                    .resolved_device_name
                    .clone()
                    .or_else(|| requested.clone())
                    .unwrap_or_else(|| "Windows Default".to_string()),
            )
        }
        _ => unreachable!("audio device kind validated above"),
    };

    if !available {
        trace_command_end("select_audio_device", started, "candidate_unavailable");
        let message = if kind == "microphone" {
            "This microphone can't be used right now. Choose another microphone or Windows Default. The previous preference was kept."
        } else {
            "This meeting sound device can't be used right now. Choose another device or Windows Default. The previous preference was kept."
        };
        return AudioDeviceSelectionResult {
            ok: false,
            kind,
            device_id: requested,
            device_name,
            message: message.to_string(),
            settings: current,
        };
    }

    let mut candidate = current.clone();
    if kind == "microphone" {
        candidate.audio.input_device_id = requested.clone();
    } else {
        candidate.audio.output_device_id = requested.clone();
    }

    let save = persist_runtime_settings(candidate);
    if !save.ok {
        trace_command_end("select_audio_device", started, "save_failed");
        return AudioDeviceSelectionResult {
            ok: false,
            kind,
            device_id: requested,
            device_name,
            message: "The device is available, but the preference could not be saved. The previous device preference was kept."
                .to_string(),
            settings: current,
        };
    }

    let saved = engine::load_settings();
    let label = if kind == "microphone" {
        "Microphone"
    } else {
        "Meeting sound"
    };
    trace_command_end("select_audio_device", started, "ok");
    AudioDeviceSelectionResult {
        ok: true,
        kind,
        device_id: requested,
        device_name: device_name.clone(),
        message: format!("{label} set to {device_name}."),
        settings: saved,
    }
}

#[cfg(test)]
mod tests {
    use super::{audio_preferences_changed, engine_preference_changed, explicit_engine_choice};
    use crate::engine::settings::MeetingEngineAdmission;
    use crate::engine::settings::RuntimeSettings;

    #[test]
    fn explicit_engine_recovery_accepts_only_literal_local_or_codex() {
        assert_eq!(explicit_engine_choice("local"), Some(MeetingEngineAdmission::Local));
        assert_eq!(explicit_engine_choice("codex"), Some(MeetingEngineAdmission::Codex));
        for invalid in ["", "LOCAL", " CoDeX ", "apikey", "unknown", "local\\n"] {
            assert_eq!(explicit_engine_choice(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn engine_preference_change_is_separate_from_audio_device_drift() {
        let current = RuntimeSettings::default();
        let mut candidate = current.clone();
        candidate.translation_engine = "codex".to_string();
        assert!(engine_preference_changed(&current, &candidate));
        assert!(!audio_preferences_changed(&current, &candidate));
    }

    #[test]
    fn audio_preference_change_detects_only_audio_drift() {
        let current = RuntimeSettings::default();
        let mut language_only = current.clone();
        language_only.meeting_listen_source_language = "id".to_string();
        language_only.meeting_listen_target_language = "en".to_string();
        assert!(!audio_preferences_changed(&current, &language_only));

        let mut input_change = current.clone();
        input_change.audio.input_device_id = Some("Another Microphone".to_string());
        assert!(audio_preferences_changed(&current, &input_change));

        let mut output_change = current.clone();
        output_change.audio.output_device_id = Some("Another Meeting Sound".to_string());
        assert!(audio_preferences_changed(&current, &output_change));
    }
}
