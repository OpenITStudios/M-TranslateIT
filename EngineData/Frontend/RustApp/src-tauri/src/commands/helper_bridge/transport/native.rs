//! Native ASR/MT dispatch at the existing helper permit, not a second scheduler.
use super::*;
use crate::engine::audio::evidence::condition_asr_pcm;
use crate::engine::runtime_state::{observe_runtime_session, RuntimeSessionSnapshot,
    APPLICATION_MEETING_OWNER_ID};
use crate::commands::helper_bridge_runtime::HelperTaskPermit;
use crate::commands::helper_bridge::{
    NATIVE_INFERENCE_CLAIM, NATIVE_ASR_MODEL, NATIVE_MT_MODEL, NATIVE_MT_REVISION,
};
use std::time::Duration;
use translateit_native_cuda::{Error, InferenceInput, InferenceOutput, Language};

fn language(value: &Value, key: &str) -> Option<Language> {
    parse_language(value.get(key).and_then(Value::as_str)?)
}

fn parse_language(code: &str) -> Option<Language> {
    match code {
        "id" => Some(Language::Indonesian),
        "en" => Some(Language::English),
        _ => None,
    }
}

// A typed Meeting translation has exactly one source of model content.
fn meeting_translation_metadata_only(payload: &Value) -> bool {
    payload.is_object()
        && ["text", "source_language", "target_language"].iter().all(|key| payload.get(*key).is_none())
        && matches!(payload.get("meeting_lane").and_then(Value::as_str), Some("you" | "incoming"))
}

fn language_code(value: Language) -> &'static str {
    match value {
        Language::Indonesian => "id",
        Language::English => "en",
    }
}

// The owner already returned an owned String from reusable native scratch.
// Move it into the existing receipt without serializing or cloning the text.
fn native_text_receipt(task: &str, pci: &str, from: Language, to: Option<Language>, text: String,
                       translation_elapsed_ms: Option<u64>) -> Value {
    let mut body = json!({
        "ok": true, "stage": task, "blocker": "", "device": "cuda",
        "cuda_pci_id": pci, "note": "Native CUDA inference returned a complete result.",
    });
    if let Some(to) = to {
        body["model_id"] = json!(NATIVE_MT_MODEL);
        body["model_revision"] = json!(NATIVE_MT_REVISION);
        body["precision"] = json!("q6_k");
        body["translation_contract"] = json!("canonical_bidirectional_id_en");
        body["translation_degraded"] = json!(false);
        body["translation_fallback_reason"] = json!("");
        body["source_language"] = json!(language_code(from));
        body["target_language"] = json!(language_code(to));
        body["direction_pair"] = json!(opposite_language_pair(from));
        body["complete"] = json!(true);
        body["finished_with_eos"] = json!(true);
        body["hit_token_ceiling"] = json!(false);
        body["translated_text"] = Value::String(text);
        if let Some(elapsed_ms) = translation_elapsed_ms {
            // Same owner handoff-to-reply interval as the native quality replay,
            // not GPU kernel time or a tokenization/decode breakdown.
            body["elapsed_ms"] = json!(elapsed_ms);
        }
    } else {
        body["model_id"] = json!(NATIVE_ASR_MODEL);
        body["compute_type"] = json!("q8_0");
        body["language"] = json!(language_code(from));
        body["transcript_text"] = Value::String(text);
    }
    body
}

// Both callers reject equal-language translation before constructing receipts.
fn opposite_language_pair(from: Language) -> &'static str {
    match from {
        Language::Indonesian => "id->en",
        Language::English => "en->id",
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Scope {
    generation: Option<u64>,
    session_id: Option<String>,
    prepare: bool,
}

fn meeting_snapshot_admitted(snapshot: &RuntimeSessionSnapshot, prepare: bool) -> bool {
    snapshot.owner_id == APPLICATION_MEETING_OWNER_ID && snapshot.authority_active
        && snapshot.generation != 0 && !snapshot.session_id.is_empty()
        && (snapshot.phase == "live" || (prepare && snapshot.phase == "starting"))
}

fn scope_from_snapshot<'a>(payload: &Value, snapshot: Option<&'a RuntimeSessionSnapshot>) -> Option<(Option<u64>, Option<&'a str>, bool)> {
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
        return snapshot.is_none().then_some((None, None, false));
    }
    let snapshot = snapshot.filter(|value| meeting_snapshot_admitted(value, prepare))?;
    let matches = if lane == Some("incoming") {
        generation.is_none() && session == Some(snapshot.session_id.as_str()) && !prepare
    } else {
        generation == Some(snapshot.generation)
            && session.is_none_or(|id| id == snapshot.session_id.as_str())
            && (if prepare { lane.is_none() } else { lane == Some("you") && session.is_some() })
    };
    matches.then_some((Some(snapshot.generation), Some(snapshot.session_id.as_str()), prepare))
}

fn scope(payload: &Value) -> Option<Scope> {
    observe_runtime_session(|snapshot| {
        scope_from_snapshot(payload, snapshot).map(|(generation, session_id, prepare)| Scope {
            generation, session_id: session_id.map(str::to_owned), prepare,
        })
    }).flatten()
}

fn scope_matches_snapshot(expected: &Scope, snapshot: Option<&RuntimeSessionSnapshot>) -> bool {
    match (expected.generation, expected.session_id.as_deref(), snapshot) {
        (None, None, None) => !expected.prepare,
        (Some(generation), Some(session_id), Some(snapshot)) => {
            meeting_snapshot_admitted(snapshot, expected.prepare)
                && snapshot.generation == generation
                && snapshot.session_id == session_id
        }
        _ => false,
    }
}

// The immutable request metadata was validated once when Scope was created.
// Recheck fresh runtime authority at every existing fence, not the JSON.
fn scope_is_current(expected: &Scope) -> bool {
    observe_runtime_session(|snapshot| scope_matches_snapshot(expected, snapshot)).unwrap_or(false)
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
// Same scheduler permit as inference. Control requests observe the Rust
// lifecycle and asset eligibility; never load models or claim a functional pass.
pub(super) fn control(task: &str, payload: &Value, permit: &HelperTaskPermit, priority: HelperTaskPriority, wait_generation: Option<u64>, observe_only: bool)
    -> HelperBridgeWorkerResponse
{
    let request_id = permit.request_id();
    if !matches!(task, "ping" | "status") {
        return standalone_blocked_response(task, request_id, priority, "error",
            "native_voice:typed_request_required", "This caller has not migrated to the native voice contract.");
    }
    let acquired = if observe_only {
        runtime().try_lock()
    } else {
        runtime().lock().map_err(std::sync::TryLockError::Poisoned)
    };
    let mut host = match acquired {
        Ok(host) => host,
        Err(std::sync::TryLockError::WouldBlock) => return status_busy_response(request_id),
        Err(std::sync::TryLockError::Poisoned(_)) => {
            return standalone_blocked_response(task, request_id, priority, "error",
                "helper_bridge:lock_poisoned", "Native lifecycle state is unavailable.");
        }
    };
    if (payload.get("meeting_start_prepare").and_then(Value::as_bool) == Some(true)
        || payload.get("meeting_generation").is_some_and(|value| !value.is_null())
        || payload.get("meeting_session_id").is_some_and(|value| !value.is_null()))
        && !scope(payload).is_some_and(|scope| scope.generation == wait_generation)
    {
        return standalone_blocked_response(task, request_id, priority, "error",
            "native_inference:scope_revoked", "Native control request has no current Meeting authority.");
    }
    if !host.native_lifecycle_ready() || !host.native_owner_healthy() {
        return standalone_blocked_response(task, request_id, priority, "error",
            "native_inference:lifecycle_not_ready", "Native lifecycle is stopped or requires cleanup.");
    }
    if host.active_request_id.is_some() {
        if task == "status" && priority == HelperTaskPriority::Diagnostic {
            return status_busy_response(request_id);
        }
        return standalone_blocked_response(task, request_id, priority, "error",
            "native_inference:lifecycle_not_ready", "Native lifecycle still owns an active request.");
    }
    let mut body = if task == "status" {
        crate::commands::helper_bridge_runtime::project_candidate_status(&mut host, true)
            .expect("explicit status requests a capability payload")
    } else {
        json!({"ok": true, "stage": "ping", "blocker": "",
            "note": "Native Rust lifecycle initialized; no model execution was attempted."})
    };
    body["request_id"] = json!(request_id);
    body["scheduler_priority"] = json!(priority.label());
    body["scheduler_wait_ms"] = json!(permit.scheduler_wait_ms());
    let ok = body.get("ok").and_then(Value::as_bool) == Some(true);
    let message = worker_message(task, &body);
    let claim = if task == "status" { super::super::NATIVE_STATUS_CLAIM.into() }
        else { runtime_claim(&host) };
    response_with_runtime(ok, task, request_id, priority, message, body, &host, claim)
}

/// ASR/MT has one production route. Missing resources are terminal.
pub(super) fn dispatch(
    task: &str, payload: &mut Value, input: Option<MeetingInput<'_>>,
    mut voice: Option<NativeVoiceIo<'_>>,
    priority: HelperTaskPriority, permit: &HelperTaskPermit, deadline_ms: u128,
    admitted_lane: Option<String>, wait_generation: Option<u64>,
) -> HelperBridgeWorkerResponse {
    let admitted_at = Instant::now();
    let request_id = permit.request_id();
    let fail = |blocker, message| standalone_blocked_response(
        task, request_id, priority, "error", blocker, message,
    );
    let pcm = input.and_then(MeetingInput::pcm);
    let translation = input.and_then(MeetingInput::translation);
    let deferred_deadline = input.and_then(MeetingInput::deferred_deadline);
    if input.is_some() && (voice.is_some()
        || !matches!((task, input), ("transcribe", Some(MeetingInput::Pcm(_)))
            | ("translate", Some(MeetingInput::Translation(_))))) {
        return fail("native_inference:typed_request_invalid", "Native Meeting input does not match its task.");
    }
    if translation.is_some() && !meeting_translation_metadata_only(payload) {
        return fail("translation:typed_request_invalid", "Native Meeting translation content must not enter JSON.");
    }
    let is_bind = matches!(voice.as_ref(), Some(NativeVoiceIo::Bind));
    let is_voice = matches!(voice.as_ref(), Some(NativeVoiceIo::Synthesize { .. }));
    let is_probe_speech = matches!(voice.as_ref(), Some(NativeVoiceIo::ProbeSpeech(_)));
    if voice.is_some() && (pcm.is_some()
        || !matches!((task, voice.as_ref()),
            ("voice_actor_preflight", Some(NativeVoiceIo::Bind))
            | ("voice_actor_synthesize", Some(NativeVoiceIo::Synthesize { .. }))
            | ("transcribe", Some(NativeVoiceIo::ProbeSpeech(_))))) {
        return fail("native_voice:typed_request_invalid", "Native voice requires typed input.");
    }
    if is_voice && (payload.get("text").is_some() || payload.get("expected_actor_token").is_some()) {
        return fail("native_voice:typed_request_invalid", "Native synthesis metadata must not duplicate typed text or actor authority.");
    }
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
    // A request that waited for an earlier generation cannot adopt a new
    // generation even if its lane/session metadata otherwise resolves.
    if request_scope.generation != wait_generation {
        let stale = if priority == HelperTaskPriority::MeetingIncoming {
            StaleMeetingScope::Session
        } else { StaleMeetingScope::Generation };
        return stale.response(task, request_id, priority);
    }
    if deferred_deadline_elapsed(deferred_deadline) {
        return expired_deferred_response(task, request_id, priority);
    }
    if host.pending_meeting_cleanup_session_id.is_some() || host.active_request_id.is_some()
        || !host.native_lifecycle_ready() || !host.native_owner_healthy()
        || host.last_error.as_deref() == Some("helper_bridge:cleanup_incomplete")
        || crate::commands::voice_lab::voice_lab_build_blocks_meeting()
    {
        return fail("native_inference:owner_not_admitted", "Native inference cannot claim the current lifecycle.");
    }
    if Instant::now() >= deadline {
        return fail("native_inference:deadline_exceeded", "Native request expired before model admission.");
    }
    let preparing = request_scope.generation.is_none() || request_scope.prepare;
    if (is_bind || is_probe_speech) && !preparing {
        return fail("native_voice:preparation_required", "Voice preparation and probe PCM require preparation authority.");
    }
    // Reject invalid translation direction/content after scope admission and
    // before opening a cold model owner. The Rust ABI repeats its own guard.
    let translation_direction = if matches!(task, "translate" | "translation_preload") {
        let (from, to) = match translation {
            Some(input) => (parse_language(input.source_language), parse_language(input.target_language)),
            None => (language(payload, "source_language"), language(payload, "target_language")),
        };
        let (Some(from), Some(to)) = (from, to) else {
            return fail("translation:direction_not_supported", "Only Indonesian and English translation is supported.");
        };
        if from == to {
            return fail("translation:direction_not_supported", "Translation must change language.");
        }
        if task == "translate" {
            let source = match translation {
                Some(input) => Some(input.source),
                None => payload.get("text").and_then(Value::as_str),
            };
            let Some(source) = source else {
                return fail("translation:empty_text", "Translation source text is required.");
            };
            if !translateit_native_cuda::translation_source_admitted(source) {
                return fail("native_inference:invalid_input", "Translation source text is invalid or outside the admitted limit.");
            }
        }
        Some((from, to))
    } else { None };
    let reference = if is_bind {
        // Refresh the same current capability facts inside the bind permit.
        // No separate status receipt, scheduler turn or stale provider reuse.
        let _ = crate::commands::helper_bridge_runtime::project_candidate_status(&mut host, false);
        if !host.provider_ready {
            return fail(host.last_error.as_deref().unwrap_or("native_voice:owner_not_ready"),
                "Current native voice preparation prerequisites are unavailable.");
        }
        // The capability scan may have taken time. Do not release an idle
        // owner for preparation whose authority or budget has since expired.
        if !scope_is_current(&request_scope) || Instant::now() >= deadline {
            return fail("native_voice:scope_revoked", "Voice preparation authority expired.");
        }
        // A Meeting-owned runtime cannot be replaced by this preparation.
        if host.native_meeting_session_id().is_some() {
            return fail("native_voice:owner_busy", "Existing inference ownership could not be released.");
        }
        // Verify the authorized reference before evicting a healthy idle
        // Text pair. Reference preparation is CPU-only, not a second GPU owner.
        let paths = crate::engine::paths::ProjectPaths::discover();
        let reference = match crate::commands::voice_lab::selected_native_reference(&paths) {
            Ok(reference) => reference,
            Err(blocker) => return fail(blocker, "Selected native reference is not approved or verified."),
        };
        if !scope_is_current(&request_scope) || Instant::now() >= deadline {
            return fail("native_voice:scope_revoked", "Voice preparation authority expired.");
        }
        let previous_device = host.native_inference.as_ref()
            .map(|native| native.owner.device_id().to_owned());
        // Open the exact five-asset closure only after the old owner closes.
        if host.native_inference.is_some() && !host.release_idle_native_inference() {
            return fail("native_voice:owner_busy", "Existing inference ownership could not be released.");
        }
        if !scope_is_current(&request_scope) || Instant::now() >= deadline {
            return fail("native_voice:scope_revoked", "Voice preparation authority expired.");
        }
        match unsafe { crate::commands::helper_bridge_runtime::NativeInferenceOwner::open_voice_candidate(request_scope.session_id.clone()) } {
            Ok(owner) => {
                let device_changed = previous_device.as_deref()
                    .is_some_and(|observed| observed != owner.owner.device_id());
                // Retain exact ownership for rollback even if the device changed.
                host.native_inference = Some(owner);
                if device_changed {
                    host.provider_ready = false;
                    host.cuda_ready = false;
                    host.last_error = Some("native_inference:cuda_device_changed".into());
                    invalidate_required_outbound_ai_readiness();
                    return fail("native_inference:cuda_device_changed", "Native CUDA device changed during voice preparation.");
                }
                host.cuda_ready = true;
            }
            Err(error) => {
                let (blocker, _) = error_blocker(error, task);
                host.provider_ready = false;
                host.cuda_ready = false;
                host.last_error = Some(blocker.into());
                invalidate_required_outbound_ai_readiness();
                return fail(blocker, "Native voice model preparation failed; no fallback was attempted.");
            }
        }
        Some(reference)
    } else { None };
    if host.native_inference.is_none() {
        // Live PCM/MT can never load a model mid-utterance. Only an explicit
        // preparation or standalone Text request opens the resident pair.
        if task != "asr_preload" && !(request_scope.session_id.is_none() && task == "translate") {
            return fail("native_inference:owner_unavailable", "Prepare the native model pair first.");
        }
        // Standalone Text needs only the ASR/MT pair, not an approved voice.
        // Preserve the existing full-stack requirement for Meeting.
        let standalone_candidate_text = request_scope.session_id.is_none() && task == "translate"
            && crate::commands::helper_bridge_runtime::NativeInferenceOwner::assets_present();
        if !host.provider_ready && !standalone_candidate_text {
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
        || !scope_is_current(&request_scope)
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
            let (from, _) = translation_direction.expect("checked translation preload direction");
            body["model_id"] = json!(NATIVE_MT_MODEL);
            body["model_revision"] = json!(NATIVE_MT_REVISION);
            body["precision"] = json!("q6_k");
            body["direction_pair"] = json!(opposite_language_pair(from));
            body["translation_degraded"] = json!(false);
            body["translation_fallback_reason"] = json!("");
        }
        return response_with_runtime(true, task, request_id, priority,
            worker_message(task, &body), body, &host, NATIVE_INFERENCE_CLAIM.into());
    }
    if deferred_deadline_elapsed(deferred_deadline) {
        return expired_deferred_response(task, request_id, priority);
    }
    let mut bound_id = None;
    let mut actor_token = None;
    let (input, from, to) = if is_bind || is_voice {
        if is_voice {
            let Some(NativeVoiceIo::Synthesize { actor_token: token, .. }) = voice.as_ref() else { unreachable!("checked synthesis input") };
            if token.is_empty() || token.len() > 512 {
                return fail("native_voice:actor_authority_missing", "Selected native voice authority is required.");
            }
            actor_token = Some(*token);
        }
        (None, Language::English, None)
    } else if task == "transcribe" {
        let from = match pcm {
            Some(input) => parse_language(input.language),
            None => language(payload, "language"),
        };
        let Some(from) = from else {
            return fail("asr:language_invalid", "Native ASR requires Indonesian or English.");
        };
        let samples = if is_probe_speech {
            let Some(NativeVoiceIo::ProbeSpeech(samples)) = voice.take() else { unreachable!("checked typed probe") };
            if samples.is_empty() || samples.len() > 44_100 * 60
                || samples.iter().any(|sample| !sample.is_finite() || sample.abs() > 1.0) {
                return fail("native_voice:probe_pcm_invalid", "Invalid native voice probe PCM.");
            }
            match crate::engine::audio::guided_take::resample_mono_fft(&samples, 44_100, 16_000) {
                Ok(samples) if (4_800..=960_000).contains(&samples.len()) => samples,
                _ => return fail("native_voice:probe_pcm_invalid", "Native voice probe PCM could not be prepared."),
            }
        } else if let Some(pcm) = pcm {
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
            return fail("asr:typed_pcm_required", "Native ASR requires owned PCM.");
        };
        (Some(InferenceInput::Speech { pcm: samples, language: from }), from, None)
    } else {
        let (from, to) = translation_direction.expect("checked translation direction");
        let source = match translation {
            Some(input) => input.source.to_owned(),
            None => {
                let Some(Value::String(source)) = payload.as_object_mut().and_then(|object| object.remove("text")) else {
                    return fail("translation:empty_text", "Translation source text is required.");
                };
                source
            }
        };
        (Some(InferenceInput::Translation { source, from, to }), from, Some(to))
    };
    // After taking PCM, the request is committed to execution or terminal
    // failure; never requeue an already-conditioned buffer. Stop may still
    // revoke authority during conditioning, so check before beginning.
    if !scope_is_current(&request_scope) {
        return fail("native_inference:scope_revoked", "Inference authority was revoked before execution.");
    }
    if deferred_deadline_elapsed(deferred_deadline) {
        return expired_deferred_response(task, request_id, priority);
    }
    let bridge_generation = host.generation_token;
    let translation_started = (task == "translate").then(Instant::now);
    let pending_result = {
        let owner = host.native_inference.as_mut().expect("checked owner");
        if let Some(reference) = reference {
            owner.begin_selected_voice(request_scope.generation, request_scope.session_id.as_deref(),
                reference, deadline).map(|(id, pending)| { bound_id = Some(id); pending })
        } else if is_voice {
            let Some(NativeVoiceIo::Synthesize { text, .. }) = voice.as_ref() else { unreachable!("checked synthesis input") };
            // Only admitted native work owns a text allocation; the scheduler borrows it.
            owner.begin_bound_voice(request_scope.generation, request_scope.session_id.as_deref(),
                actor_token.expect("checked token"), preparing, (*text).to_owned(), deadline)
        } else {
            owner.owner.begin(input.expect("checked text or speech input"), deadline)
        }
    };
    let pending = match pending_result {
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
    host.active_meeting_lane = admitted_lane;
    host.updated_unix_ms = unix_ms();
    drop(host);
    // The original permit outlives the native reply. Do not wait under the
    // lifecycle lock; Stop can signal and retain the exact active owner.
    let result = pending.wait_output();
    let translation_elapsed_ms = translation_started.map(observed_elapsed_ms);
    let Ok(mut host) = runtime().lock() else {
        return fail("helper_bridge:lock_poisoned_after_native_response", "Native inference result ownership could not be verified.");
    };
    if host.generation_token != bridge_generation
        || host.active_request_id.as_deref() != Some(request_id)
        || host.native_inference.is_none()
        || host.native_meeting_session_id() != request_scope.session_id.as_deref()
        || !scope_is_current(&request_scope)
    {
        clear_active_request(&mut host, request_id);
        let mut response = fail("native_inference:scope_revoked", "Native inference result was discarded because its owner changed.");
        response.runtime_claim = "native_cuda_result_discarded".into();
        return response;
    }
    // Reject a late successful receipt without disguising genuine backend
    // failures as cancellation or revocation.
    let result = if result.is_ok() && Instant::now() >= deadline {
        Err(Error::DeadlineExceeded)
    } else { result };
    // Bind publication is itself scoped and uses the scheduler's unique
    // request identity, not a voice ID supplied by JSON.
    let result = match (bound_id.as_deref(), result) {
        (Some(id), Ok(output @ InferenceOutput::VoiceBound(_))) => {
            host.native_inference.as_mut().expect("checked owner")
                .finish_selected_voice(request_scope.generation, request_scope.session_id.as_deref(),
                    id, request_id, output)
                .map(InferenceOutput::Text)
        }
        (Some(_), Ok(_)) => Err(Error::InvalidReceipt),
        (_, result) => result,
    };
    let body = match result {
        Ok(InferenceOutput::Text(text)) if is_bind && bound_id.is_some() => {
            json!({"ok": true, "stage": task, "blocker": "", "device": "cuda",
                "cuda_pci_id": host.native_inference.as_ref().expect("checked owner").owner.device_id(),
                "actor_token": text, "model_id": "zonos3-mini-q8_0",
                "note": "Selected native voice reference bound; synthesis proof remains separate."})
        }
        Ok(InferenceOutput::Voice(samples)) if is_voice => {
            let Some(NativeVoiceIo::Synthesize { samples: slot, .. }) = voice.as_mut() else { unreachable!("checked output slot") };
            let sample_count = samples.len();
            **slot = Some(samples);
            json!({"ok": true, "stage": task, "blocker": "", "device": "cuda",
                "cuda_pci_id": host.native_inference.as_ref().expect("checked owner").owner.device_id(),
                "actor_token": actor_token, "model_id": "zonos3-mini-q8_0",
                "sample_rate_hz": 44_100, "sample_count": sample_count,
                "note": "Native synthesis returned owned complete PCM."})
        }
        Ok(InferenceOutput::Text(text)) if !is_voice && !is_bind => {
            native_text_receipt(task,
                host.native_inference.as_ref().expect("checked owner").owner.device_id(),
                from, to, text, translation_elapsed_ms)
        }
        unexpected => {
            let error = unexpected.err().unwrap_or(Error::InvalidReceipt);
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
    // The receipt application already stamps the update under this lock.
    clear_active_request(&mut host, request_id);
    let message = worker_message(task, &body);
    response_with_runtime(ok, task, request_id, priority, message, body, &host, NATIVE_INFERENCE_CLAIM.into())
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
    fn native_text_receipt_moves_owned_text_and_preserves_stage_contract() {
        let pci = "0000:01:00.0";
        let transcript = String::from("Halo, 15 bukan 50.");
        let ptr = transcript.as_ptr();
        let asr = native_text_receipt("transcribe", pci, Language::Indonesian, None, transcript, None);
        assert_eq!(asr["transcript_text"].as_str().unwrap().as_ptr(), ptr);
        assert_eq!(asr, json!({
            "ok":true,"stage":"transcribe","blocker":"","device":"cuda","cuda_pci_id":pci,
            "note":"Native CUDA inference returned a complete result.",
            "model_id":NATIVE_ASR_MODEL,"compute_type":"q8_0","language":"id",
            "transcript_text":"Halo, 15 bukan 50."
        }));
        for (from, to, pair, source, target) in [
            (Language::Indonesian, Language::English, "id->en", "id", "en"),
            (Language::English, Language::Indonesian, "en->id", "en", "id"),
        ] {
            let translated = String::from("Complete text.");
            let ptr = translated.as_ptr();
            let mt = native_text_receipt("translate", pci, from, Some(to), translated, Some(42));
            assert_eq!(mt["translated_text"].as_str().unwrap().as_ptr(), ptr);
            assert_eq!(mt, json!({
                "ok":true,"stage":"translate","blocker":"","device":"cuda","cuda_pci_id":pci,
                "note":"Native CUDA inference returned a complete result.",
                "model_id":NATIVE_MT_MODEL,"model_revision":NATIVE_MT_REVISION,"precision":"q6_k",
                "translation_contract":"canonical_bidirectional_id_en",
                "translation_degraded":false,"translation_fallback_reason":"",
                "source_language":source,"target_language":target,"direction_pair":pair,
                "complete":true,"finished_with_eos":true,"hit_token_ceiling":false,
                "translated_text":"Complete text.","elapsed_ms":42
            }));
        }
    }

    #[test]
    fn typed_meeting_translation_rejects_duplicate_content_and_missing_lane() {
        let metadata = json!({"meeting_lane":"you","meeting_session_id":"meeting-1","meeting_generation":41});
        assert!(meeting_translation_metadata_only(&metadata));
        for key in ["text", "source_language", "target_language"] {
            let mut mixed = metadata.clone();
            mixed[key] = Value::Null;
            assert!(!meeting_translation_metadata_only(&mixed));
        }
        assert!(!meeting_translation_metadata_only(&json!({})));
        assert_eq!(parse_language("id"), Some(Language::Indonesian));
        assert_eq!(parse_language("en"), Some(Language::English));
        assert_eq!(parse_language("ID"), None);
    }

    #[test]
    fn scope_rejects_foreign_session_generation_and_unowned_text() {
        let current = snapshot();
        let outbound = json!({"meeting_generation":41,"meeting_session_id":"meeting-1","meeting_lane":"you"});
        let session_ptr = current.session_id.as_ptr();
        let scope = scope_from_snapshot(&outbound, Some(&current)).unwrap();
        assert_eq!(scope.1.unwrap().as_ptr(), session_ptr);
        assert_eq!(scope, (Some(41), Some("meeting-1"), false));
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
        let mut revoked = current.clone();
        revoked.authority_active = false;
        assert!(scope_from_snapshot(&outbound, Some(&revoked)).is_none());
        revoked.authority_active = true;
        revoked.phase = "stopping".into();
        assert!(scope_from_snapshot(&outbound, Some(&revoked)).is_none());
        assert!(scope_from_snapshot(&incoming, Some(&revoked)).is_none());
        revoked.phase = "starting".into();
        let prepare = json!({"meeting_generation":41,"meeting_start_prepare":true});
        assert_eq!(scope_from_snapshot(&prepare, Some(&revoked)), Some((Some(41), Some("meeting-1"), true)));
        assert!(scope_from_snapshot(&outbound, Some(&revoked)).is_none());
    }

    #[test]
    fn resolved_scope_rechecks_match_exact_request_without_reparsing() {
        let current = snapshot();
        let mut starting = current.clone();
        starting.phase = "starting".into();
        let mut stopped = current.clone();
        stopped.authority_active = false;
        let mut stopping = current.clone();
        stopping.phase = "stopping".into();
        let mut foreign = current.clone();
        foreign.owner_id = "foreign".into();
        let mut replacement = current.clone();
        replacement.generation += 1;
        let mut other_id = current.clone();
        other_id.session_id = "meeting-other".into();
        let observations = [
            None, Some(&current), Some(&starting), Some(&stopped),
            Some(&stopping), Some(&foreign), Some(&replacement), Some(&other_id),
        ];
        for (payload, initial) in [
            (json!({}), None),
            (json!({"meeting_generation":41,"meeting_session_id":"meeting-1","meeting_lane":"you"}), Some(&current)),
            (json!({"meeting_session_id":"meeting-1","meeting_lane":"incoming"}), Some(&current)),
            (json!({"meeting_generation":41,"meeting_start_prepare":true}), Some(&starting)),
            (json!({"meeting_generation":41,"meeting_session_id":"meeting-1","meeting_start_prepare":true}), Some(&starting)),
        ] {
            let (generation, session_id, prepare) = scope_from_snapshot(&payload, initial).unwrap();
            let expected = Scope { generation, session_id: session_id.map(str::to_owned), prepare };
            for observed in observations {
                assert_eq!(scope_matches_snapshot(&expected, observed),
                    scope_from_snapshot(&payload, observed) == Some((generation, session_id, prepare)));
            }
        }
        for bad in [
            json!({"meeting_generation":"41","meeting_start_prepare":true}),
            json!({"meeting_generation":41,"meeting_start_prepare":"true"}),
            json!({"meeting_generation":41,"meeting_start_prepare":true,"meeting_lane":"incoming"}),
            json!({"meeting_generation":41,"meeting_lane":"you"}),
        ] {
            assert!(scope_from_snapshot(&bad, Some(&current)).is_none());
        }
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
