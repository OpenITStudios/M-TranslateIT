//! Native Create/Improve evaluation core. The result is unapproved and
//! in-memory until the existing build owner stages a native review package.
use super::evaluation::HELD_OUT_LINES;
use super::native_speaker::{median_peer_cosines, SpeakerVerifier};
use super::super::my_voice_reference::{select_references, synthesis_audio_valid, SYNTHESIS_RATE};
use super::super::my_voice_runtime::{authority, with_runtime};
use super::super::voice_lab_recording::AcceptedGuidedRecording;
use crate::engine::audio::{guided_take::resample_mono_fft, meeting_output::{prepare_native_voice_reference, decode_native_review_pcm}, live_segment_writer::encode_pcm16_wav};
use std::time::Instant;
use translateit_native_cuda::{InferenceInput, InferenceOutput, Language};

pub(super) struct EvaluatedSample {
    pub(super) line_id: u32,
    pub(super) exact_text: &'static str,
    pub(super) wav: Vec<u8>,
    pub(super) intelligibility_text: String,
    pub(super) intelligibility_wer: f64,
    // Legacy-style conditioned-reference score, not directly comparable
    // across ASR/resampling changes without matched baseline evidence.
    pub(super) speaker_similarity: f64,
    // Mean across the identical original-voice anchor set for all candidates.
    pub(super) anchor_similarity: f64,
}

pub(super) struct EvaluatedCandidate {
    pub(super) reference_line_id: u32,
    pub(super) reference_sha256: [u8; 32],
    pub(super) reference_wav: Vec<u8>,
    pub(super) samples: Vec<EvaluatedSample>,
}

pub(super) struct NativeEvaluation {
    pub(super) selected: EvaluatedCandidate,
    pub(super) reference_peer_scores: Vec<(u32, f64)>,
    pub(super) anchor_identities: Vec<(u32, [u8; 32])>,
}

// English ASCII words and optional single internal apostrophe, matching the
// legacy tokenizer on ASCII input. Unicode full casefold parity is not claimed.
fn normalized_words(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let bytes = lower.as_bytes();
    let mut words = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_alphanumeric() { i += 1; continue; }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphanumeric() { i += 1; }
        if i+1 < bytes.len() && bytes[i] == b'\'' && bytes[i+1].is_ascii_alphanumeric() {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_alphanumeric() { i += 1; }
        }
        words.push(lower[start..i].to_string());
    }
    words
}

pub(super) fn word_error_rate(expected: &str, actual: &str) -> Result<f64, String> {
    let expected = normalized_words(expected);
    let actual = normalized_words(actual);
    if expected.is_empty() || actual.is_empty() {
        return Err("native_evaluation:empty_transcript".into());
    }
    let mut row: Vec<usize> = (0..=expected.len()).collect();
    for (i, word) in actual.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i+1;
        for (j, expected_word) in expected.iter().enumerate() {
            let above = row[j+1];
            row[j+1] = (diagonal + usize::from(word != expected_word))
                .min(above+1).min(row[j]+1);
            diagonal = above;
        }
    }
    Ok(row[expected.len()] as f64 / expected.len() as f64)
}

fn rank(candidate: &EvaluatedCandidate) -> (f64, f64, f64, f64, u32) {
    let n = candidate.samples.len() as f64;
    let mean_wer = candidate.samples.iter().map(|s| s.intelligibility_wer).sum::<f64>() / n;
    let max_wer = candidate.samples.iter().map(|s| s.intelligibility_wer).fold(0.0, f64::max);
    let mean_similarity = candidate.samples.iter().map(|s| s.anchor_similarity).sum::<f64>() / n;
    let min_similarity = candidate.samples.iter().map(|s| s.anchor_similarity).fold(1.0, f64::min);
    (mean_wer, max_wer, -mean_similarity, -min_similarity, candidate.reference_line_id)
}

fn preferred(candidate: &EvaluatedCandidate, old: &EvaluatedCandidate) -> bool {
    let a = rank(candidate);
    let b = rank(old);
    a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1))
        .then_with(|| a.2.total_cmp(&b.2)).then_with(|| a.3.total_cmp(&b.3))
        .then_with(|| a.4.cmp(&b.4)).is_lt()
}

/// Caller must own an authorized active build generation. No profile writes
/// or approval here. Require at least two distinct eligible original takes
/// for an independent anchor comparison; Quick Preview still needs only one.
pub(super) fn evaluate(
    generation: u64, deadline: Instant, accepted: Vec<AcceptedGuidedRecording>,
) -> Result<NativeEvaluation, String> {
    if !authority(generation, deadline) {
        return Err("native_evaluation:inactive_generation".into());
    }
    let references = select_references(accepted, 3, || authority(generation, deadline))?;
    if references.len() < 2 || references.iter().enumerate().any(|(i, r)| {
        references[..i].iter().any(|old| old.line_id == r.line_id || old.wav_sha256 == r.wav_sha256)
    }) {
        return Err("native_evaluation:distinct_references_required".into());
    }
    let mut verifier = SpeakerVerifier::open()?;
    let cancelled = || !authority(generation, deadline);
    let mut anchors = Vec::with_capacity(references.len());
    let mut prepared = Vec::with_capacity(references.len());
    let mut anchor_identities = Vec::with_capacity(references.len());
    for reference in references {
        if cancelled() { return Err("native_evaluation:cancelled".into()); }
        let pcm = prepare_native_voice_reference(&reference.samples)?;
        let embedding = verifier.embed(&pcm, 16_000, cancelled)?;
        anchors.push((reference.line_id, embedding));
        anchor_identities.push((reference.line_id, reference.wav_sha256));
        prepared.push((reference.line_id, reference.wav_sha256, reference.wav, pcm));
    }
    let reference_peer_scores = median_peer_cosines(&anchors)?;
    let selected = with_runtime(generation, deadline, "voice_lab_native_evaluation", |request| {
        let mut best: Option<EvaluatedCandidate> = None;
        'candidate: for (index, (reference_line_id, reference_sha256, reference_wav, pcm)) in prepared.into_iter().enumerate() {
            let output = request(InferenceInput::BindVoice { reference: pcm }, "native_evaluation:bind_failed")?;
            let InferenceOutput::VoiceBound(binding) = output else { return Err("native_evaluation:invalid_binding".into()); };
            let mut samples = Vec::with_capacity(HELD_OUT_LINES.len());
            for &(line_id, exact_text) in HELD_OUT_LINES {
                let output = request(InferenceInput::Voice { text: exact_text.into(), binding }, "native_evaluation:synthesis_failed")?;
                let InferenceOutput::Voice(pcm) = output else { return Err("native_evaluation:invalid_synthesis".into()); };
                if !synthesis_audio_valid(&pcm) || pcm.len() < SYNTHESIS_RATE*3/10 {
                    continue 'candidate;
                }
                // Evaluate the exact persisted/listened-to PCM16 payload, not
                // a pre-quantization buffer with a different identity.
                let wav = encode_pcm16_wav(SYNTHESIS_RATE as u32, 1, &pcm);
                drop(pcm);
                let review_pcm = decode_native_review_pcm(&wav)?;
                if !synthesis_audio_valid(&review_pcm) { continue 'candidate; }
                let asr_pcm = resample_mono_fft(&review_pcm, SYNTHESIS_RATE as u32, 16_000)?;
                drop(review_pcm);
                if !(4_800..=960_000).contains(&asr_pcm.len()) {
                    return Err("native_evaluation:invalid_asr_pcm".into());
                }
                let generated = verifier.embed(&asr_pcm, 16_000, cancelled)?;
                let speaker_similarity = generated.cosine(&anchors[index].1);
                let anchor_similarity = anchors.iter().map(|(_, a)| generated.cosine(a)).sum::<f64>() / anchors.len() as f64;
                let output = request(InferenceInput::Speech { pcm: asr_pcm, language: Language::English }, "native_evaluation:asr_failed")?;
                let InferenceOutput::Text(intelligibility_text) = output else { return Err("native_evaluation:invalid_asr".into()); };
                let intelligibility_wer = word_error_rate(exact_text, &intelligibility_text)?;
                samples.push(EvaluatedSample { line_id, exact_text, wav, intelligibility_text,
                    intelligibility_wer, speaker_similarity, anchor_similarity });
            }
            if samples.len() != HELD_OUT_LINES.len() || cancelled() {
                return Err("native_evaluation:incomplete".into());
            }
            let candidate = EvaluatedCandidate { reference_line_id, reference_sha256, reference_wav, samples };
            if best.as_ref().is_none_or(|old| preferred(&candidate, old)) {
                best = Some(candidate);
            }
        }
        best.ok_or_else(|| "native_evaluation:no_artifact_free_candidate".into())
    })?;
    Ok(NativeEvaluation { selected, reference_peer_scores, anchor_identities })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_wer_preserves_insertions_deletions_and_substitutions() {
        assert_eq!(normalized_words("Don't, ROCK'n'ROLL 15!"), vec!["don't", "rock'n", "roll", "15"]);
        assert_eq!(word_error_rate("one two", "ONE two.").unwrap(), 0.0);
        assert_eq!(word_error_rate("one two", "one three").unwrap(), 0.5);
        assert_eq!(word_error_rate("one two", "one").unwrap(), 0.5);
        assert_eq!(word_error_rate("one two", "one two three four five").unwrap(), 1.5);
        assert!(word_error_rate("one two", "...").is_err());
    }

    fn candidate(id: u32, wer: f64, similarity: f64) -> EvaluatedCandidate {
        EvaluatedCandidate { reference_line_id: id, reference_sha256: [id as u8;32], reference_wav: Vec::new(),
            samples: HELD_OUT_LINES.iter().map(|&(line_id, exact_text)| EvaluatedSample {
                line_id, exact_text, wav: Vec::new(), intelligibility_text: exact_text.into(),
                intelligibility_wer: wer, speaker_similarity: similarity, anchor_similarity: similarity,
            }).collect() }
    }

    #[test]
    fn rank_prefers_intelligibility_then_common_anchor_then_line_identity() {
        let old = candidate(2, 0.1, 0.8);
        assert!(preferred(&candidate(3, 0.0, 0.5), &old));
        assert!(!preferred(&candidate(1, 0.2, 0.99), &old));
        assert!(preferred(&candidate(3, 0.1, 0.9), &old));
        assert!(preferred(&candidate(1, 0.1, 0.8), &old));
    }
}
