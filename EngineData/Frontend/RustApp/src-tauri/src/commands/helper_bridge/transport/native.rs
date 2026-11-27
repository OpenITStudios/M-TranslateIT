//! Native ASR/MT dispatch at the existing helper permit, not a second scheduler.
use super::*;
use crate::engine::audio::evidence::condition_asr_pcm;
use crate::engine::audio::meeting_output::decode_functional_probe_asr_pcm;
use super::super::functional_probe::{required_probe_output_path, voice_only_probe_path_matches};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use crate::engine::runtime_state::{latest_runtime_session_state, RuntimeSessionSnapshot,
    APPLICATION_MEETING_OWNER_ID};
use crate::commands::helper_bridge_runtime::HelperTaskPermit;
use crate::commands::helper_bridge::{
    NATIVE_INFERENCE_CLAIM, NATIVE_ASR_MODEL, NATIVE_MT_MODEL, NATIVE_MT_REVISION,
};
use std::time::Duration;
use translateit_native_cuda::{Error, InferenceInput, Language};

fn language(value: &Value, key: &str) -> Option<Language> {
    match value.get(key).and_then(Value::as_str) {
        Some("id") => Some(Language::Indonesian),
        Some("en") => Some(Language::English),
        _ => None,
    }
}

fn language_code(value: Language) -> &'static str {
    match value {
        Language::Indonesian => "id",
        Language::English => "en",
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Scope {
    generation: Option<u64>,
    session_id: Option<String>,
}

fn scope_from_snapshot(payload: &Value, snapshot: Option<&RuntimeSessionSnapshot>) -> Option<Scope> {
    // Native admission must compare exact metadata, not the legacy
    // presentation sanitizer's normalized/truncated string values.
    let generation = match payload.get("meeting_generation") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_u64()?),
    };
    let session = match payload.get("meeting_session_id") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_str()?),
    };
    let lane = match payload.get("meeting_lane") {
        None | Some(Value::Null) => None,
        Some(value) => match value.as_str()? {
            "you" => Some("you"),
            "incoming" => Some("incoming"),
            _ => return None,
        },
    };
    let prepare = match payload.get("meeting_start_prepare") {
        None | Some(Value::Null) => false,
        Some(value) => value.as_bool()?,
    };
    if generation.is_none() && session.is_none() && lane.is_none() && !prepare {
        return snapshot.is_none().then_some(Scope { generation: None, session_id: None });
    }
    let snapshot = snapshot.filter(|value| {
        value.owner_id == APPLICATION_MEETING_OWNER_ID && value.authority_active
            && value.generation != 0 && !value.session_id.is_empty()
    })?;
    let matches = if lane == Some("incoming") {
        generation.is_none() && session == Some(snapshot.session_id.as_str())
            && snapshot.phase == "live" && !prepare
    } else {
        generation == Some(snapshot.generation)
            && session.is_none_or(|id| id == snapshot.session_id.as_str())
            && (if prepare {
                lane.is_none() && matches!(snapshot.phase.as_str(), "starting" | "live")
            } else {
                lane == Some("you") && session.is_some() && snapshot.phase == "live"
            })
    };
    matches.then(|| Scope {
        generation: Some(snapshot.generation), session_id: Some(snapshot.session_id.clone()),
    })
}

fn scope(payload: &Value) -> Option<Scope> {
    let state = latest_runtime_session_state();
    if state.snapshot.is_none() && (state.has_active_session || state.blocker != "runtime_session:no_active_session") { return None; }
    scope_from_snapshot(payload, state.snapshot.as_ref())
}

fn error_blocker(error: Error, task: &str) -> (&'static str, bool) {
    // Fatal backend/owner faults affect both native models. A normal empty
    // transcript, input limit or cooperative Stop does not poison the pair.
    match error {
        Error::LibraryUnavailable | Error::LibraryIdentityMismatch | Error::LibrarySymbolMissing | Error::AbiMismatch
            => ("native_inference:library_identity_unverified", true),
        Error::Native(2) => ("native_inference:cuda_device_unavailable", true),
        Error::Native(13) => ("native_inference:model_identity_unverified", true),
        Error::Native(14) => ("native_inference:insufficient_vram", true),
        Error::Cancelled => ("native_inference:cancelled", false),
        Error::DeadlineExceeded => ("native_inference:deadline_exceeded", false),
        Error::InvalidInput | Error::Native(1) => ("native_inference:invalid_input", false),
        Error::Native(6) if task == "translate" => ("translation:input_too_long_for_model", false),
        Error::Native(6) => ("asr:input_too_long_for_model", false),
        Error::Native(9) if task == "transcribe" => ("asr:empty_transcript", false),
        Error::Native(9) => ("translation:empty_output", false),
        Error::Native(8 | 10) => ("native_inference:incomplete_output", false),
        _ => ("native_inference:owner_or_backend_failed", true),
    }
}

// A file is permitted only for the owned TTS functional probe. Live
// Meeting ASR must transfer its finalized PCM, not a path.
fn probe_pcm(payload: &Value, request_scope: &Scope) -> Result<Vec<f32>, &'static str> {
    if payload.get("meeting_lane").is_some_and(|lane| !lane.is_null())
        || (request_scope.generation.is_some()
            && payload.get("meeting_start_prepare").and_then(Value::as_bool) != Some(true))
    { return Err("asr:probe_scope_invalid"); }
    let probe_id = payload.get("probe_request_id").and_then(Value::as_str)
        .ok_or("asr:probe_scope_invalid")?;
    let path = required_probe_output_path(request_scope.generation, probe_id)
        .ok_or("asr:probe_root_invalid")?;
    let reported = payload.get("audio_path").and_then(Value::as_str).ok_or("asr:probe_path_missing")?;
    if !voice_only_probe_path_matches(reported, &path) {
        return Err("asr:probe_path_not_owned");
    }
    // Deny ordinary writes/deletes while decoding the one owned handle;
    // reject a leaf reparse point even if it appeared after the path check.
    let file = std::fs::OpenOptions::new().read(true).share_mode(1)
        .custom_flags(0x00200000).open(&path).map_err(|_| "asr:probe_open_failed")?;
    let metadata = file.metadata().map_err(|_| "asr:probe_open_failed")?;
    if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
        return Err("asr:probe_path_not_owned");
    }
    decode_functional_probe_asr_pcm(file).map_err(|_| "asr:probe_wav_invalid")
}

/// ASR/MT has one production route. Missing resources are terminal.
pub(super) fn dispatch(
    task: &str, payload: &mut Value, pcm: Option<MeetingPcmInput<'_>>,
    priority: HelperTaskPriority, permit: &HelperTaskPermit, deadline_ms: u128,
) -> HelperBridgeWorkerResponse {
    debug_assert!(matches!(task, "transcribe" | "translate" | "asr_preload" | "translation_preload"));
    let admitted_at = Instant::now();
    let request_id = permit.request_id();
    let fail = |blocker, message| standalone_blocked_response(
        task, request_id, priority, "error", blocker, message,
    );
    let Some(deadline) = u64::try_from(deadline_ms).ok()
        .and_then(|ms| admitted_at.checked_add(Duration::from_millis(ms))) else {
        return fail("native_inference:deadline_invalid", "Native inference deadline is invalid.");
    };
    let Ok(mut host) = runtime().lock() else {
        return fail("helper_bridge:lock_poisoned", "Inference owner state is unavailable.");
    };
    let Some(request_scope) = scope(payload) else {
        return fail("native_inference:scope_invalid", "Native inference requires current exact session authority.");
    };
    if host.pending_meeting_cleanup_session_id.is_some() || host.active_request_id.is_some()
        || host.state != "ready" || host.child.is_none()
        || host.last_error.as_deref() == Some("helper_bridge:cleanup_incomplete")
        || crate::commands::voice_lab::voice_lab_build_blocks_meeting()
    {
        return fail("native_inference:owner_not_admitted", "Native inference cannot claim the current lifecycle.");
    }
    if Instant::now() >= deadline {
        return fail("native_inference:deadline_exceeded", "Native request expired before model admission.");
    }
    if host.native_inference.is_none() {
        // Live PCM/MT can never load a model mid-utterance. Only an explicit
        // preparation or standalone Text request opens the resident pair.
        if task != "asr_preload" && !(request_scope.session_id.is_none() && task == "translate") {
            return fail("native_inference:owner_unavailable", "Prepare the native model pair first.");
        }
        if !host.provider_ready {
            return fail("native_inference:assets_or_voice_unavailable", "Native preparation requires current asset and voice capability.");
        }
        // Build-supplied immutable pins and the reviewed controlled runtime
        // root are required; no request may override path, hash or GPU.
        match unsafe { crate::commands::helper_bridge_runtime::NativeInferenceOwner::open(request_scope.session_id.clone()) } {
            Ok(owner) => {
                host.native_inference = Some(owner);
                host.cuda_ready = true;
            }
            Err(error) => {
                let (blocker, _) = error_blocker(error, task);
                host.provider_ready = false;
                host.cuda_ready = false;
                host.last_error = Some(blocker.into());
                invalidate_required_outbound_ai_readiness();
                return fail(blocker, "Native model preparation failed; no fallback was attempted.");
            }
        }
    } else if task == "asr_preload" && host.native_meeting_session_id().is_none()
        && request_scope.session_id.is_some()
    {
        host.native_inference.as_mut().expect("checked owner").session_id = request_scope.session_id.clone();
    }
    if host.native_meeting_session_id() != request_scope.session_id.as_deref()
        || scope(payload).as_ref() != Some(&request_scope)
    {
        return fail("native_inference:scope_revoked", "Native owner no longer matches the current session.");
    }
    if priority == HelperTaskPriority::MeetingIncoming && meeting_outbound_pipeline_active() {
        return incoming_deferred_response(task, request_id);
    }
    let owner = &host.native_inference.as_ref().expect("checked owner").owner;
    if !owner.is_available() {
        return fail("native_inference:owner_unavailable", "Native models are not available.");
    }
    // Cold open is not interruptible. Its owner remains attributable for
    // cleanup, but an expired preparation may not issue a success receipt.
    if Instant::now() >= deadline {
        return fail("native_inference:deadline_exceeded", "Native request expired during model preparation.");
    }
    if matches!(task, "asr_preload" | "translation_preload") {
        // Open prepared both resident models atomically. Do not load either
        // again or pretend a preload receipt proves successful inference.
        let mut body = json!({
            "ok": true, "stage": task, "blocker": "", "device": "cuda",
            "cuda_pci_id": owner.device_id(), "request_id": request_id,
            "scheduler_priority": priority.label(), "scheduler_wait_ms": permit.scheduler_wait_ms(),
            "note": "The native model pair is resident; functional inference remains separate.",
        });
        if task == "asr_preload" {
            body["model_id"] = json!(NATIVE_ASR_MODEL);
            body["compute_type"] = json!("q8_0");
        } else {
            let (Some(from), Some(to)) = (language(payload, "source_language"), language(payload, "target_language")) else {
                return fail("translation:direction_not_supported", "Only Indonesian and English translation is supported.");
            };
            if from == to {
                return fail("translation:direction_not_supported", "Translation must change language.");
            }
            body["model_id"] = json!(NATIVE_MT_MODEL);
            body["model_revision"] = json!(NATIVE_MT_REVISION);
            body["precision"] = json!("q6_k");
            body["direction_pair"] = json!(format!("{}->{}", language_code(from), language_code(to)));
            body["translation_degraded"] = json!(false);
            body["translation_fallback_reason"] = json!("");
        }
        let mut response = response_with_runtime(true, task, request_id, priority,
            worker_message(task, &body), body, &host);
        response.runtime_claim = NATIVE_INFERENCE_CLAIM.into();
        return response;
    }
    let (input, from, to) = if task == "transcribe" {
        let Some(from) = language(payload, "language") else {
            return fail("asr:language_invalid", "Native ASR requires Indonesian or English.");
        };
        let samples = if let Some(pcm) = pcm {
            if !pcm.matches(task, payload) {
                return fail("asr:finalized_pcm_contract_invalid", "Finalized PCM identity is invalid.");
            }
            let started = Instant::now();
            let Some(owned) = pcm.utterance.borrow_mut().take() else {
                return fail("asr:finalized_pcm_missing", "Finalized PCM was already consumed.");
            };
            let mut samples = owned.frame.samples;
            condition_asr_pcm(&mut samples, pcm.noise_suppression_enabled);
            pcm.prepare_ms.set(pcm.prepare_ms.get().saturating_add(observed_elapsed_ms(started)));
            samples
        } else {
            // The existing functional readiness probe alone needs to decode
            // synthesized audio. Never pass a WAV or decoder into live ASR.
            match probe_pcm(payload, &request_scope) {
                Ok(samples) => samples,
                Err(blocker) => return fail(blocker, "Owned voice probe PCM could not be decoded."),
            }
        };
        (InferenceInput::Speech { pcm: samples, language: from }, from, None)
    } else {
        let (Some(from), Some(to)) = (language(payload, "source_language"), language(payload, "target_language")) else {
            return fail("translation:direction_not_supported", "Only Indonesian and English translation is supported.");
        };
        if from == to {
            return fail("translation:direction_not_supported", "Translation must change language.");
        }
        let Some(Value::String(source)) = payload.as_object_mut().and_then(|object| object.remove("text")) else {
            return fail("translation:empty_text", "Translation source text is required.");
        };
        (InferenceInput::Translation { source, from, to }, from, Some(to))
    };
    // After taking PCM, the request is committed to execution or terminal
    // failure; never requeue an already-conditioned buffer. Stop may still
    // revoke authority during conditioning, so check before beginning.
    if scope(payload).as_ref() != Some(&request_scope) {
        return fail("native_inference:scope_revoked", "Inference authority was revoked before execution.");
    }
    let bridge_generation = host.generation_token;
    let pending = match host.native_inference.as_mut().expect("checked owner").owner.begin(input, deadline) {
        Ok(pending) => pending,
        Err(error) => {
            let (blocker, fatal) = error_blocker(error, task);
            if fatal {
                host.provider_ready = false;
                host.cuda_ready = false;
                host.last_error = Some(blocker.into());
                invalidate_required_outbound_ai_readiness();
                let _ = host.native_inference.as_mut().expect("checked owner").owner.try_close();
            }
            return fail(blocker, "Native inference could not admit the request.");
        }
    };
    host.active_task = Some(task.into());
    host.active_request_id = Some(request_id.into());
    host.active_meeting_generation = request_scope.generation;
    host.active_meeting_session_id = request_scope.session_id.clone();
    host.active_meeting_lane = meeting_lane(payload);
    host.updated_unix_ms = unix_ms();
    drop(host);
    // The original permit outlives the native reply. Do not wait under the
    // lifecycle lock; Stop can signal and retain the exact active owner.
    let result = pending.wait();
    let Ok(mut host) = runtime().lock() else {
        return fail("helper_bridge:lock_poisoned_after_native_response", "Native inference result ownership could not be verified.");
    };
    if host.generation_token != bridge_generation
        || host.active_request_id.as_deref() != Some(request_id)
        || host.native_inference.is_none()
        || host.native_meeting_session_id() != request_scope.session_id.as_deref()
        || scope(payload).as_ref() != Some(&request_scope)
    {
        clear_active_request(&mut host, request_id);
        let mut response = fail("native_inference:scope_revoked", "Native inference result was discarded because its owner changed.");
        response.runtime_claim = "native_cuda_result_discarded".into();
        return response;
    }
    let body = match result {
        Ok(text) => {
            let mut body = json!({
                "ok": true, "stage": task, "blocker": "", "device": "cuda",
                "cuda_pci_id": host.native_inference.as_ref().expect("checked owner").owner.device_id(),
                "note": "Native CUDA inference returned a complete result.",
            });
            if let Some(to) = to {
                let from = language_code(from);
                let to = language_code(to);
                body["model_id"] = json!(NATIVE_MT_MODEL);
                body["model_revision"] = json!(NATIVE_MT_REVISION);
                body["precision"] = json!("q6_k");
                body["translation_contract"] = json!("canonical_bidirectional_id_en");
                body["translation_degraded"] = json!(false);
                body["translation_fallback_reason"] = json!("");
                body["source_language"] = json!(from);
                body["target_language"] = json!(to);
                body["direction_pair"] = json!(format!("{from}->{to}"));
                body["complete"] = json!(true);
                body["finished_with_eos"] = json!(true);
                body["hit_token_ceiling"] = json!(false);
                body["translated_text"] = json!(text);
            } else {
                body["model_id"] = json!(NATIVE_ASR_MODEL);
                body["compute_type"] = json!("q8_0");
                body["language"] = json!(language_code(from));
                body["transcript_text"] = json!(text);
            }
            body
        }
        Err(error) => {
            let (blocker, fatal) = error_blocker(error, task);
            if fatal {
                host.provider_ready = false;
                host.cuda_ready = false;
                // Keep the slot even if close finishes. A failed native
                // owner cannot be reselected as Python within this session.
                let _ = host.native_inference.as_mut().expect("checked owner").owner.try_close();
                invalidate_required_outbound_ai_readiness();
            }
            json!({"ok": false, "stage": task, "blocker": blocker,
                "note": "Native inference did not return an admitted complete result."})
        }
    };
    let mut body = body;
    body["request_id"] = json!(request_id);
    body["scheduler_priority"] = json!(priority.label());
    body["scheduler_wait_ms"] = json!(permit.scheduler_wait_ms());
    let ok = apply_worker_response(&mut host, &body);
    clear_active_request(&mut host, request_id);
    host.updated_unix_ms = unix_ms();
    let message = worker_message(task, &body);
    let mut response = response_with_runtime(ok, task, request_id, priority, message, body, &host);
    response.runtime_claim = NATIVE_INFERENCE_CLAIM.into();
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot() -> RuntimeSessionSnapshot {
        RuntimeSessionSnapshot {
            started_unix_ms: 0, owner_id: APPLICATION_MEETING_OWNER_ID.into(),
            session_id: "meeting-1".into(), generation: 41, authority_active: true,
            phase: "live".into(), live_capture_stream_active: true, safe_to_stop: true,
            note: String::new(),
        }
    }

    #[test]
    fn scope_rejects_foreign_session_generation_and_unowned_text() {
        let current = snapshot();
        let outbound = json!({"meeting_generation":41,"meeting_session_id":"meeting-1","meeting_lane":"you"});
        let scope = scope_from_snapshot(&outbound, Some(&current)).unwrap();
        assert_eq!(scope.generation, Some(41));
        assert_eq!(scope.session_id.as_deref(), Some("meeting-1"));
        for (key, bad) in [
            ("meeting_generation", json!(42)),
            ("meeting_generation", json!("41")),
            ("meeting_session_id", json!("other")),
            ("meeting_session_id", json!("meeting-1 ")),
            ("meeting_lane", json!("YOU")),
        ] {
            let mut invalid = outbound.clone();
            invalid[key] = bad;
            assert!(scope_from_snapshot(&invalid, Some(&current)).is_none());
        }
        let incoming = json!({"meeting_session_id":"meeting-1","meeting_lane":"incoming"});
        assert_eq!(scope_from_snapshot(&incoming, Some(&current)), Some(scope));
        assert!(scope_from_snapshot(&json!({}), Some(&current)).is_none());
        assert!(scope_from_snapshot(&json!({}), None).is_some());
        let mut revoked = current;
        revoked.authority_active = false;
        assert!(scope_from_snapshot(&outbound, Some(&revoked)).is_none());
    }

    #[test]
    fn only_actual_shared_faults_poison_native_owner() {
        assert_eq!(error_blocker(Error::Native(9), "transcribe"), ("asr:empty_transcript", false));
        assert!(!error_blocker(Error::Cancelled, "translate").1);
        assert!(!error_blocker(Error::Native(6), "translate").1);
        assert!(error_blocker(Error::Native(7), "transcribe").1);
        assert!(error_blocker(Error::InvalidReceipt, "translate").1);
    }
}
