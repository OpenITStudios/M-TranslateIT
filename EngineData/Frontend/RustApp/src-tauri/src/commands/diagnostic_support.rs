use serde::Serialize;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SUPPORT_EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

use crate::engine::audio::finalized_utterance::{finalized_callback_lock_metrics, finalized_pcm_drop_breakdown};
use crate::engine::audio::live_capture::live_capture_status;
use crate::engine::audio::meeting_sound_capture::meeting_sound_capture_status;
use crate::engine::logging::sanitize_diagnostic_text;
use crate::engine::paths::ProjectPaths;

use super::device_loss_guard::get_device_loss_guard_status;
use super::helper_bridge::get_helper_bridge_status;
use super::incident_log::{get_recent_runtime_incidents, without_runtime_incident_recording};
use super::long_session_health::get_long_session_health_status;
use super::meeting_session::{
    current_outbound_latency_distribution, get_meeting_session_status,
};
use super::runtime_watchdog::get_runtime_watchdog_status;
use super::startup_recovery::get_startup_recovery_status;
use super::virtual_mic_route::get_virtual_mic_route_selection;

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticSupportBundleResult {
    pub ok: bool,
    pub file_path: Option<String>,
    pub message: String,
    pub blocker: String,
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn sanitized(value: &str) -> String {
    sanitize_diagnostic_text(value)
}

// Each explicit export receives a distinct filename and create_new publication.
// A collision or disk failure must not overwrite an earlier diagnostic report.
fn write_support_bundle(directory: &Path, body: &[u8]) -> io::Result<PathBuf> {
    let path = directory.join(format!(
        "translateit_support_{}_{}_{}.json",
        unix_ms(),
        std::process::id(),
        SUPPORT_EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ));
    let mut file = OpenOptions::new().write(true).create_new(true).open(&path)?;
    let written = file.write_all(body).and_then(|_| file.sync_all());
    drop(file);
    if let Err(error) = written {
        // This is the newly created file only, never any previous report.
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(path)
}

#[tauri::command]
pub fn export_diagnostic_support_bundle() -> DiagnosticSupportBundleResult {
    let paths = ProjectPaths::discover();
    let microphone_capture = live_capture_status();
    let incoming_capture = meeting_sound_capture_status();
    let (meeting, helper, watchdog, devices, recovery, long_session, incidents, route, latency, capture_lock) =
        without_runtime_incident_recording(|| {
            (
                get_meeting_session_status(),
                get_helper_bridge_status(),
                get_runtime_watchdog_status(),
                get_device_loss_guard_status(),
                get_startup_recovery_status(),
                get_long_session_health_status(),
                get_recent_runtime_incidents(),
                get_virtual_mic_route_selection(),
                current_outbound_latency_distribution(),
                finalized_callback_lock_metrics(),
            )
        });

    let capture_health = json!({
        "microphone": {
            "stream_active": microphone_capture.stream_active,
            "frames_received": microphone_capture.frames_received,
            "native_callback_error_count": microphone_capture.callback_error_count,
            "pcm_drop_count": microphone_capture.pcm_drop_count,
            "pcm_drop_breakdown": finalized_pcm_drop_breakdown(false),
            "speech_pipeline_blocker": sanitized(&microphone_capture.blocker),
        },
        "incoming": {
            "stream_active": incoming_capture.stream_active,
            "frames_received": incoming_capture.frames_received,
            "native_callback_error_count": incoming_capture.callback_error_count,
            "pcm_drop_count": incoming_capture.pcm_drop_count,
            "pcm_drop_breakdown": finalized_pcm_drop_breakdown(true),
            "speech_pipeline_blocker": sanitized(&incoming_capture.blocker),
        },
    });
    // Only bounded numeric stage durations; never utterance text,
    // per-turn identifiers or wall-clock speech timestamps.
    let outbound_timing = meeting.outbound.timing.as_ref().map(|timing| json!({
        "speech_boundary": timing.speech_boundary_ms,
        "finalization": timing.finalization_ms,
        "queue": timing.queue_ms,
        "audio_prepare": timing.audio_prepare_ms,
        "asr": timing.asr_ms,
        "translation": timing.translation_ms,
        "tts": timing.tts_ms,
        "playback_queue": timing.playback_queue_ms,
        "delivery": timing.delivery_ms,
        "outbound_latency": timing.outbound_latency_ms,
        "estimated_speech_onset_to_first_audio": timing.estimated_onset_to_first_audio_ms,
        "estimated_speech_end_to_first_audio": timing.estimated_end_to_first_audio_ms,
        "asr_scheduler_wait": timing.asr_scheduler_wait_ms,
        "translation_scheduler_wait": timing.translation_scheduler_wait_ms,
        "tts_scheduler_wait": timing.tts_scheduler_wait_ms,
        // Numeric-only attempt/recovery evidence; never private audio/text.
        "asr_worker_attempt": timing.asr_worker_attempt_timing.as_ref(),
        "translation_worker_attempt": timing.translation_worker_attempt_timing.as_ref(),
        "tts_worker_attempt": timing.tts_worker_attempt_timing.as_ref(),
        "output_stream_build": timing.output_stream_build_ms,
        "output_stream_start_to_first_audio": timing.output_stream_start_to_first_audio_ms,
    }));
    let meeting_status = json!({
        "capture_callback_lock": {
            "outbound_observations": capture_lock.0,
            "outbound_max_wait_us": capture_lock.1,
            "incoming_observations": capture_lock.2,
            "incoming_max_wait_us": capture_lock.3,
        },
        "lifecycle": meeting.lifecycle,
        "has_session": meeting.has_session,
        "authority_active": meeting.authority_active,
        "active_age_ms": meeting.active_age_ms,
        "capture_active": meeting.capture_active,
        "blocker": sanitized(&meeting.blocker),
        "note": sanitized(&meeting.note),
        "outbound": {
            "stage": meeting.outbound.stage,
            "output_active": meeting.outbound.output_active,
            "last_stage_ok": meeting.outbound.last_stage_ok,
            "overflow_dropped_utterance_count": meeting.outbound.overflow_dropped_utterance_count,
            "evicted_pending_utterance_count": meeting.outbound.evicted_pending_utterance_count,
            "timing_ms": outbound_timing,
            // Numeric-only exploratory distribution, derived on explicit
            // support export; not persistent telemetry or an SLA.
            "latency_distribution_ms": latency.map(|(sample_count, p50, p95)| json!({
                "sample_count": sample_count,
                "p50": p50,
                "p95": p95,
            })),
            "blocker": sanitized(&meeting.outbound.blocker),
            "updated_unix_ms": meeting.outbound.updated_unix_ms,
        },
        "incoming": {
            "stage": meeting.incoming.stage,
            "capture_active": meeting.incoming.capture_active,
            "suppressed": meeting.incoming.suppressed,
            "degraded": meeting.incoming.degraded,
            "blocker": sanitized(&meeting.incoming.blocker),
            "updated_unix_ms": meeting.incoming.updated_unix_ms,
        },
    });
    let mut payload = json!({
        "schema": "translateit.support_bundle.v1",
        "created_unix_ms": unix_ms(),
        "app": {
            "version": env!("CARGO_PKG_VERSION"),
            "source_sha": option_env!("TRANSLATEIT_BUILD_SOURCE_SHA"),
            "source_identity_scope": if option_env!("TRANSLATEIT_BUILD_SOURCE_SHA").is_some() {
                "CLEAN_PINNED_BUILD_INPUT"
            } else {
                "UNVERIFIED"
            },
            "source_sha_is_signed_attestation": false,
            "path_mode": paths.path_mode,
            "packaged_context_initialized": paths.packaged_context_initialized,
            "development_root_verified": paths.development_root_verified,
        },
        "startup_recovery": {
            "previous_unclean_shutdown": recovery.previous_unclean_shutdown,
            "another_instance_detected": recovery.another_instance_detected,
            "cleanup_attempted": recovery.cleanup_attempted,
            "cleanup_ok": recovery.cleanup_ok,
            "removed_files": recovery.removed_files,
            "blocker": sanitized(&recovery.blocker),
            "note": sanitized(&recovery.note),
            "checked_unix_ms": recovery.checked_unix_ms,
        },
        "watchdog": {
            "state": watchdog.state,
            "healthy": watchdog.healthy,
            "action_required": watchdog.action_required,
            "component": watchdog.component,
            "stage": watchdog.stage,
            "age_ms": watchdog.age_ms,
            "threshold_ms": watchdog.threshold_ms,
            "blocker": sanitized(&watchdog.blocker),
            "note": sanitized(&watchdog.note),
            "updated_unix_ms": watchdog.updated_unix_ms,
        },
        "device_loss": {
            "state": devices.state,
            "healthy": devices.healthy,
            "action_required": devices.action_required,
            "required_device_lost": devices.required_device_lost,
            "optional_device_lost": devices.optional_device_lost,
            "component": devices.component,
            "blocker": sanitized(&devices.blocker),
            "note": sanitized(&devices.note),
            "updated_unix_ms": devices.updated_unix_ms,
        },
        "meeting": meeting_status,
        "helper": {
            "state": helper.state,
            "cuda_ready": helper.cuda_ready,
            "provider_ready": helper.provider_ready,
            "functional_outbound_ready": helper.functional_outbound_ready,
            "degraded_mode": helper.degraded_mode,
            "active_task": helper.active_task,
            "active_meeting_lane": helper.active_meeting_lane,
            "generation_token": helper.generation_token,
            "last_error": helper.last_error.as_deref().map(sanitized),
            "message": sanitized(&helper.message),
            "updated_unix_ms": helper.updated_unix_ms,
        },
        "long_session": {
            "state": long_session.state,
            "healthy": long_session.healthy,
            "session_age_ms": long_session.session_age_ms,
            "outbound_overflow_dropped": long_session.outbound_overflow_dropped,
            "outbound_evicted_pending": long_session.outbound_evicted_pending,
            "deferred_incoming_depth": long_session.deferred_incoming_depth,
            "deferred_incoming_dropped_overflow": long_session.deferred_incoming_dropped_overflow,
            "deferred_incoming_dropped_stale": long_session.deferred_incoming_dropped_stale,
            "transcript_dropped_turns": long_session.transcript_dropped_turns,
            "transcript_truncated": long_session.transcript_truncated,
            "meeting_temp_file_count": long_session.meeting_temp_file_count,
            "incident_count": long_session.incident_count,
            "incident_history_coverage": long_session.incident_history_coverage,
            "warning_count": long_session.warning_count,
            "note": sanitized(&long_session.note),
        },
        "issue_map": {
            "scope": "RECENT_BOUNDED_INCIDENTS",
            "coverage": incidents.coverage,
            "retained_records": incidents.count,
            "at_capacity": incidents.truncated,
            "areas": &incidents.issue_areas,
            "total_failure_frequency_measured": false,
            "native_root_cause_verified": false,
            "legacy_issue_build_attribution": "UNKNOWN",
            "record_build_identity_scope": "CLEAN_PINNED_BUILD_INPUT_OR_UNKNOWN",
        },
        "incident_summary": {
            "count": incidents.count,
            "truncated": incidents.truncated,
            "latest": incidents.incidents.first().map(|incident| json!({
                "category": &incident.category,
                "component": &incident.component,
                "blocker": &incident.blocker,
                "note": &incident.note,
                "occurred_unix_ms": incident.occurred_unix_ms,
            })),
        },
        "virtual_route": {
            "route_ready": route.route_ready,
            "output_device_found": route.output_device_found,
            "input_device_found": route.input_device_found,
            "blocker": sanitized(&route.blocker),
            "runtime_claim": route.runtime_claim,
            "updated_unix_ms": route.updated_unix_ms,
        },
        "privacy": {
            "contains_transcript": false,
            "contains_audio": false,
            "contains_voice_reference": false,
            "contains_absolute_paths": false,
            "contains_device_names": false,
            "contains_session_id": false,
        }
    });

    payload["meeting"]["capture_health"] = capture_health;
    let directory = PathBuf::from(&paths.user_saved_dir).join("Diagnostics");
    if fs::create_dir_all(&directory).is_err() {
        return DiagnosticSupportBundleResult {
            ok: false,
            file_path: None,
            message: "Diagnostics folder could not be created.".to_string(),
            blocker: "diagnostic_bundle:create_dir_failed".to_string(),
        };
    }
    let body = match serde_json::to_vec_pretty(&payload) {
        Ok(body) => body,
        Err(_) => {
            return DiagnosticSupportBundleResult {
                ok: false,
                file_path: None,
                message: "Diagnostic support data could not be encoded.".to_string(),
                blocker: "diagnostic_bundle:encode_failed".to_string(),
            }
        }
    };
    let path = match write_support_bundle(&directory, &body) {
        Ok(path) => path,
        Err(_) => {
            return DiagnosticSupportBundleResult {
                ok: false,
                file_path: None,
                message: "Diagnostic support bundle could not be written.".to_string(),
                blocker: "diagnostic_bundle:write_failed".to_string(),
            };
        }
    };

    DiagnosticSupportBundleResult {
        ok: true,
        file_path: Some(path.to_string_lossy().replace('\\', "/")),
        message: "Redacted diagnostic support bundle exported. It does not include transcript, audio, voice reference, device names, session identifiers, or absolute runtime paths.".to_string(),
        blocker: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{sanitized, write_support_bundle};
    use std::{fs, time::{SystemTime, UNIX_EPOCH}};

    #[test]
    fn two_support_exports_never_clobber_the_first_file() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let folder = std::env::temp_dir().join(format!(
            "translateit-support-{}-{stamp}", std::process::id()
        ));
        fs::create_dir(&folder).unwrap();
        let first = write_support_bundle(&folder, br#"{"status":"first"}"#).unwrap();
        let second = write_support_bundle(&folder, br#"{"status":"second"}"#).unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read(&first).unwrap(), br#"{"status":"first"}"#);
        assert_eq!(fs::read(&second).unwrap(), br#"{"status":"second"}"#);
        fs::remove_file(first).unwrap();
        fs::remove_file(second).unwrap();
        fs::remove_dir(folder).unwrap();
    }

    #[test]
    fn bundle_text_uses_runtime_redaction() {
        let clean = sanitized(r"failed C:\Users\alice\trace.log alice@example.com token=secret");
        assert!(!clean.contains("alice@example.com"));
        assert!(!clean.contains("token=secret"));
        assert!(clean.contains("[redacted-secret]"));
        assert!(!clean.contains("C:\\Users\\alice"));
    }
}
