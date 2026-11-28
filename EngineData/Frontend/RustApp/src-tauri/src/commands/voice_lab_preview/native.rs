//! Native Quick Preview. Reuse the VoiceLab build gate and the single
//! helper scheduler/native owner. Preview never approves or selects a voice.
use super::*;
use super::super::helper_bridge_runtime::{
    acquire_helper_task_permit, clear_active_request, runtime, set_blocked,
    HelperTaskPriority, NativeInferenceOwner,
};
use super::super::voice_lab_recording::AcceptedGuidedRecording;
use super::super::my_voice_reference::{select_references, synthesis_audio_valid, SYNTHESIS_RATE};
use crate::engine::audio::meeting_output::prepare_native_voice_reference;
use crate::engine::audio::live_segment_writer::write_pcm16_wav;
use translateit_native_cuda::{InferenceInput, InferenceOutput};

const PREVIEW_TEXT: &str = "Good morning, everyone. Thank you for joining this meeting.";

fn authority(generation: u64, deadline: Instant) -> bool {
    let current = current_voice_lab_build_snapshot();
    current.generation == Some(generation) && current.active && current.phase == "preparing"
        && !current.cancel_requested && Instant::now() < deadline
        && {
            let state = crate::engine::runtime_state::latest_runtime_session_state();
            !state.has_active_session && state.snapshot.is_none()
                && state.blocker == "runtime_session:no_active_session"
        }
}

fn synthesize(generation: u64, deadline: Instant, reference: Vec<f32>) -> Result<Vec<f32>, String> {
    let permit = acquire_helper_task_permit(HelperTaskPriority::Diagnostic)
        .map_err(|_| "preview_scheduler_unavailable")?;
    let request_id = permit.request_id();
    let mut host = runtime().lock().map_err(|_| "preview_owner_unavailable")?;
    if !authority(generation, deadline) || host.has_meeting_resources()
        || host.active_request_id.is_some() || !host.release_idle_native_inference() {
        return Err("preview_owner_busy".into());
    }
    // Same exclusive five-asset owner, no parallel voice engine. Cold open
    // is not preemptible; recheck the build generation immediately after.
    let native = unsafe { NativeInferenceOwner::open_voice_candidate(None) }
        .map_err(|_| "preview_native_model_unavailable")?;
    host.native_inference = Some(native);
    host.active_task = Some("voice_lab_quick_preview".into());
    host.active_request_id = Some(request_id.into());
    let bridge_generation = host.generation_token;
    drop(host);

    let result = (|| -> Result<Vec<f32>, String> {
        let begin = |input| {
            let mut host = runtime().lock().map_err(|_| "preview_owner_unavailable")?;
            if !authority(generation, deadline) || host.generation_token != bridge_generation
                || host.active_request_id.as_deref() != Some(request_id) || host.has_meeting_resources() {
                return Err("preview_cancelled_or_timed_out");
            }
            host.native_inference.as_mut().ok_or("preview_owner_unavailable")?
                .owner.begin(input, deadline).map_err(|_| "preview_native_request_failed")
        };
        let binding = begin(InferenceInput::BindVoice { reference })?
            .wait_output_with_cancel(|| !authority(generation, deadline))
            .map_err(|_| "preview_reference_binding_failed")?;
        let InferenceOutput::VoiceBound(binding) = binding else { return Err("preview_invalid_receipt".into()); };
        let output = begin(InferenceInput::Voice { text: PREVIEW_TEXT.into(), binding })?
            .wait_output_with_cancel(|| !authority(generation, deadline))
            .map_err(|_| "preview_native_synthesis_failed")?;
        let InferenceOutput::Voice(samples) = output else { return Err("preview_invalid_receipt".into()); };
        if !authority(generation, deadline) || !synthesis_audio_valid(&samples) {
            return Err("preview_audio_artifacts_or_cancelled".into());
        }
        Ok(samples)
    })();

    // The existing owner remains registered until the request and thread
    // are verified released. Never drop a pending request under the mutex.
    let mut host = runtime().lock().map_err(|_| "preview_owner_unavailable")?;
    if host.generation_token != bridge_generation || host.active_request_id.as_deref() != Some(request_id) {
        return Err("preview_owner_changed".into());
    }
    clear_active_request(&mut host, request_id);
    if !host.release_idle_native_inference() {
        set_blocked(&mut host, "Native voice preview cleanup requires retry.", "helper_bridge:cleanup_incomplete");
        return Err("preview_native_cleanup_incomplete".into());
    }
    result
}

pub(super) fn generate(accepted: Vec<AcceptedGuidedRecording>) -> QuickVoicePreviewResult {
    if super::super::builtin_voice::meeting_blocks_voice_change() {
        return outcome(false, "resource_busy", "Finish the Meeting and any pending cleanup first.");
    }
    if super::super::voice_lab::voice_backup_active() {
        return outcome(false, "backup_active", "Finish the voice backup or restore first.");
    }
    let started = match begin_voice_lab_build() {
        Ok(s) => s,
        Err(_) => return outcome(false, "resource_busy", "Meeting or another voice creation task is active."),
    };
    let Some(generation) = started.generation else {
        return outcome(false, "resource_unavailable", "Voice creation authority is unavailable.");
    };
    ACTIVE_PREVIEW_GENERATION.store(generation, Ordering::Release);
    let deadline = Instant::now() + TIMEOUT;
    let file = preview_dir().join(PREVIEW_WAV);
    let result = (|| -> Result<(), String> {
        let destination = preview_dir();
        fs::create_dir_all(&destination).map_err(|_| "preview_storage_unavailable")?;
        match fs::remove_file(&file) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err("preview_cleanup_failed".into()),
        }
        let (_, samples) = select_references(accepted, 1, || authority(generation, deadline))?
            .pop().ok_or("preview_reference_unusable")?;
        let reference = prepare_native_voice_reference(&samples)?;
        drop(samples);
        let samples = synthesize(generation, deadline, reference)?;
        if !authority(generation, deadline) { return Err("preview_cancelled_or_timed_out".into()); }
        // UI replay only. Meeting still uses direct F32 PCM.
        write_pcm16_wav(&file, SYNTHESIS_RATE as u32, 1, &samples)
            .map_err(|_| "preview_audio_write_failed")?;
        if !authority(generation, deadline) { return Err("preview_cancelled_or_timed_out".into()); }
        Ok(())
    })();
    // A cancellation racing with the final file write wins before success
    // is published. The same build lifecycle serializes that decision.
    let result = result.and_then(|()| {
        super::super::voice_lab::finish_native_voice_preview(generation)
    });
    if result.is_err() { let _ = fail_voice_lab_build(generation); }
    ACTIVE_PREVIEW_GENERATION.store(0, Ordering::Release);
    if let Err(blocker) = result {
        let _ = fs::remove_file(&file);
        outcome(false, &blocker, "Native Quick Preview did not complete. Meeting voice was unchanged.")
    } else {
        outcome(true, "preview_ready", "Native preview is ready to listen to in My Voice only.")
    }
}
