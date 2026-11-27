//! Native Quick Preview. Reuse the VoiceLab build gate and the single
//! helper scheduler/native owner. Preview never approves or selects a voice.
use super::*;
use super::super::helper_bridge_runtime::{
    acquire_helper_task_permit, clear_active_request, runtime, set_blocked,
    HelperTaskPriority, NativeInferenceOwner,
};
use super::super::voice_lab_recording::AcceptedGuidedRecording;
use crate::engine::audio::meeting_output::{decode_native_voice_source, prepare_native_voice_reference};
use crate::engine::audio::live_segment_writer::write_pcm16_wav;
use std::io::Read;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::rc::Rc;
use translateit_native_cuda::{InferenceInput, InferenceOutput};

const RATE: usize = 32_000;
const PREVIEW_RATE: usize = 44_100;
const MAX_REFERENCE_BYTES: u64 = 2 * 1024 * 1024;
const PREVIEW_TEXT: &str = "Good morning, everyone. Thank you for joining this meeting.";

#[derive(Debug)]
struct SignalRank {
    clipping: f64,
    edge: Option<f64>,
    silence: f64,
    dc: f64,
    duration_delta: u64,
    line_id: u32,
}

impl SignalRank {
    fn cmp(&self, other: &Self, edges: bool) -> std::cmp::Ordering {
        self.clipping.total_cmp(&other.clipping)
            .then_with(|| if edges {
                self.edge.unwrap_or(f64::INFINITY).total_cmp(&other.edge.unwrap_or(f64::INFINITY))
            } else { std::cmp::Ordering::Equal })
            .then_with(|| self.silence.total_cmp(&other.silence))
            .then_with(|| self.dc.total_cmp(&other.dc))
            .then_with(|| self.duration_delta.cmp(&other.duration_delta))
            .then_with(|| self.line_id.cmp(&other.line_id))
    }
}

// Same gross signal thresholds and reference ordering as the previous
// Quick Preview. Quiet-edge RMS is an energy proxy, not speaker similarity.
fn signal_rank(line_id: u32, samples: &[f32]) -> Option<SignalRank> {
    if !(RATE * 3..=RATE * 10).contains(&samples.len()) { return None; }
    let mut silent = 0;
    let mut clipped = 0;
    let mut active_sq = 0.0;
    let mut sum = 0.0;
    for &sample in samples {
        if !sample.is_finite() || sample.abs() > 1.0 { return None; }
        let x = f64::from(sample) * 32768.0;
        sum += x;
        if x.abs() <= 128.0 { silent += 1; } else { active_sq += x*x; }
        if x.abs() >= 32760.0 { clipped += 1; }
    }
    let n = samples.len();
    let silence = silent as f64 / n as f64;
    let clipping = clipped as f64 / n as f64;
    let dc = (sum / n as f64).abs();
    let active_rms = if silent == n { 0.0 } else { (active_sq / (n - silent) as f64).sqrt() };
    if silence >= 0.90 || clipping >= 0.05 || dc >= 2048.0 || active_rms < 256.0 {
        return None;
    }
    let energy: Vec<f64> = samples.chunks(640).map(|frame| {
        (frame.iter().map(|x| (f64::from(*x)*32768.0).powi(2)).sum::<f64>() / frame.len() as f64).sqrt()
    }).collect();
    let peak = energy.iter().copied().fold(0.0, f64::max);
    let leading = energy.iter().take_while(|x| **x < peak*0.35).count();
    let trailing = energy.iter().rev().take_while(|x| **x < peak*0.35).count();
    let mut edges = Vec::new();
    if leading >= 8 { edges.extend_from_slice(&energy[..leading]); }
    if trailing >= 8 { edges.extend_from_slice(&energy[energy.len()-trailing..]); }
    edges.sort_by(f64::total_cmp);
    let edge = if edges.is_empty() { None } else {
        let middle = edges.len()/2;
        let median = if edges.len()%2 == 0 { (edges[middle-1]+edges[middle])/2.0 } else { edges[middle] };
        Some(median/active_rms)
    };
    Some(SignalRank { clipping, edge, silence, dc,
        duration_delta: (n as u64 * 1000 / RATE as u64).abs_diff(5000), line_id })
}

fn read_reference(path: &std::path::Path) -> Result<Vec<f32>, String> {
    let file = fs::OpenOptions::new().read(true).share_mode(1).custom_flags(0x00200000)
        .open(path).map_err(|_| "preview_reference_unavailable")?;
    let meta = file.metadata().map_err(|_| "preview_reference_unavailable")?;
    if !meta.is_file() || meta.file_attributes() & 0x400 != 0
        || !(44..=MAX_REFERENCE_BYTES).contains(&meta.len()) {
        return Err("preview_reference_invalid".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_REFERENCE_BYTES+1).read_to_end(&mut bytes)
        .map_err(|_| "preview_reference_unavailable")?;
    if bytes.len() as u64 != meta.len() { return Err("preview_reference_invalid".into()); }
    decode_native_voice_source(&bytes)
}

struct Reference { rank: SignalRank, samples: Vec<f32> }

fn select_reference(accepted: Vec<AcceptedGuidedRecording>, active: impl Fn() -> bool)
    -> Result<Vec<f32>, String>
{
    // Preserve the existing preview cap; no unbounded collection of PCM.
    if accepted.is_empty() || accepted.len() > 128 {
        return Err("preview_reference_candidate_invalid".into());
    }
    let mut best: Option<Rc<Reference>> = None;
    let mut best_edge: Option<Rc<Reference>> = None;
    let mut all_edges = true;
    for take in accepted {
        if !active() { return Err("preview_cancelled_or_timed_out".into()); }
        let Ok(samples) = read_reference(&take.path) else { continue; };
        let Some(rank) = signal_rank(take.line_id, &samples) else { continue; };
        all_edges &= rank.edge.is_some();
        let candidate = Rc::new(Reference { rank, samples });
        if best.as_ref().is_none_or(|old| candidate.rank.cmp(&old.rank, false).is_lt()) {
            best = Some(Rc::clone(&candidate));
        }
        if best_edge.as_ref().is_none_or(|old| candidate.rank.cmp(&old.rank, true).is_lt()) {
            best_edge = Some(candidate);
        }
    }
    let selected = if all_edges { best_edge } else { best };
    // At most two best candidates remain in memory, no recording is modified.
    let selected = selected.ok_or("preview_reference_unusable")?;
    prepare_native_voice_reference(&selected.samples)
}

// Same gross artifact rejection as the former preview. This cannot measure
// perceptual fidelity, identity similarity, or intelligibility.
fn preview_audio_valid(samples: &[f32]) -> bool {
    if !(PREVIEW_RATE/10..=PREVIEW_RATE*30).contains(&samples.len()) { return false; }
    let (mut clipped, mut silent, mut run, mut longest) = (0,0,0,0);
    for &s in samples {
        if !s.is_finite() || s.abs() > 1.0 { return false; }
        if s.abs() >= 0.98 { clipped += 1; }
        if s.abs() <= 128.0/32768.0 {
            silent += 1; run += 1; longest = longest.max(run);
        } else { run = 0; }
    }
    clipped * 20 < samples.len() && silent * 10 < samples.len() * 9 && longest < PREVIEW_RATE*3
}

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
        if !authority(generation, deadline) || !preview_audio_valid(&samples) {
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
        let reference = select_reference(accepted, || authority(generation, deadline))?;
        let samples = synthesize(generation, deadline, reference)?;
        if !authority(generation, deadline) { return Err("preview_cancelled_or_timed_out".into()); }
        // UI replay only. Meeting still uses direct F32 PCM.
        write_pcm16_wav(&file, PREVIEW_RATE as u32, 1, &samples)
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

#[cfg(test)]
mod tests {
    use super::*;
    fn clean(rate: usize, seconds: usize) -> Vec<f32> {
        (0..rate*seconds).map(|i| if i%64 < 32 { 0.15 } else { -0.15 }).collect()
    }
    #[test]
    fn reference_ranking_keeps_legacy_signal_gates() {
        assert!(signal_rank(1, &vec![0.0; RATE*5]).is_none());
        assert!(signal_rank(1, &vec![1.0; RATE*5]).is_none());
        assert!(signal_rank(1, &vec![0.2; RATE*5]).is_none());
        let a = signal_rank(2, &clean(RATE, 5)).unwrap();
        let b = signal_rank(1, &clean(RATE, 6)).unwrap();
        assert!(a.cmp(&b, false).is_lt());
        assert!(signal_rank(1, &clean(RATE, 2)).is_none());
    }
    #[test]
    fn preview_rejects_gross_artifacts_without_claiming_similarity() {
        assert!(preview_audio_valid(&clean(PREVIEW_RATE, 5)));
        assert!(!preview_audio_valid(&vec![0.0; PREVIEW_RATE]));
        assert!(!preview_audio_valid(&vec![1.0; PREVIEW_RATE]));
        let mut dropout = clean(PREVIEW_RATE, 5);
        dropout[..PREVIEW_RATE*3].fill(0.0);
        assert!(!preview_audio_valid(&dropout));
    }
}
