//! Native Quick Preview. Reuse the VoiceLab build gate and the single
//! helper scheduler/native owner. Preview never approves or selects a voice.
use super::*;
use super::super::my_voice_runtime::{authority, with_runtime};
use super::super::voice_lab_recording::AcceptedGuidedRecording;
use super::super::my_voice_reference::{select_references, synthesis_audio_valid, SYNTHESIS_RATE};
use crate::engine::audio::meeting_output::prepare_native_voice_reference;
use crate::engine::audio::live_segment_writer::write_pcm16_wav;
use translateit_native_cuda::{InferenceInput, InferenceOutput};

const PREVIEW_TEXT: &str = "Good morning, everyone. Thank you for joining this meeting.";

fn synthesize(generation: u64, deadline: Instant, reference: Vec<f32>) -> Result<Vec<f32>, String> {
    with_runtime(generation, deadline, "voice_lab_quick_preview", |request| {
        let binding = request(InferenceInput::BindVoice { reference }, "preview_reference_binding_failed")?;
        let InferenceOutput::VoiceBound(binding) = binding else { return Err("preview_invalid_receipt".into()); };
        let output = request(InferenceInput::Voice { text: PREVIEW_TEXT.into(), binding }, "preview_native_synthesis_failed")?;
        let InferenceOutput::Voice(samples) = output else { return Err("preview_invalid_receipt".into()); };
        if !authority(generation, deadline) || !synthesis_audio_valid(&samples) {
            return Err("preview_audio_artifacts_or_cancelled".into());
        }
        Ok(samples)
    })
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
        let selected = select_references(accepted, 1, || authority(generation, deadline))?
            .pop().ok_or("preview_reference_unusable")?;
        let reference = prepare_native_voice_reference(&selected.samples)?;
        drop(selected);
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
