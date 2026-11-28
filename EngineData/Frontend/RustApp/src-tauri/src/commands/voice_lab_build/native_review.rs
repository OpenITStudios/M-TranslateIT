//! Native reference-based review package in the existing Candidate workspace.
//! No model weights, no selected-profile mutation, no implicit approval.
//! Stage under the existing authorized build; publish the small manifest
//! only under the canonical build completion lock in the future caller.
use super::evaluation::HELD_OUT_LINES;
use super::native_evaluation::{NativeEvaluation, word_error_rate};
use super::super::my_voice_reference::synthesis_audio_valid;
use super::super::my_voice_runtime::authority;
use super::super::voice_lab::VoiceLabStoragePaths;
use crate::engine::audio::meeting_output::{decode_native_voice_source, decode_native_review_pcm};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::{Read, Write}, path::{Path, PathBuf}, time::Instant};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

const MANIFEST: &str = "native-review.json";
const PENDING: &str = "native-review.pending";
const REFERENCE: &str = "reference.wav";
const MAX_MANIFEST: u64 = 128 * 1024;
const MAX_REFERENCE: u64 = 2 * 1024 * 1024;
const MAX_SAMPLE: u64 = 44 + 44_100 * 30 * 2;

fn hash(bytes: &[u8]) -> [u8; 32] { Sha256::digest(bytes).into() }

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn valid_digest(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ModelIdentity {
    // Exact reviewed DLL closure includes the enforced GGUF pins and ABI.
    dll_pins_sha256: [u8; 32],
    speaker_sha256: String,
    speaker_bytes: u64,
}

impl ModelIdentity {
    fn current() -> Result<Self, String> {
        let pins = option_env!("TRANSLATEIT_NATIVE_DLL_PINS").filter(|p| !p.is_empty())
            .ok_or("native_review:runtime_pin_required")?;
        let speaker_sha256 = option_env!("TRANSLATEIT_SPEAKER_ONNX_SHA256")
            .filter(|p| valid_digest(p)).ok_or("native_review:verifier_pin_required")?;
        let speaker_bytes = option_env!("TRANSLATEIT_SPEAKER_ONNX_BYTES")
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| (1..=512 * 1024 * 1024).contains(v))
            .ok_or("native_review:verifier_pin_required")?;
        Ok(Self { dll_pins_sha256: hash(pins.as_bytes()), speaker_sha256: speaker_sha256.into(), speaker_bytes })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ReviewSample {
    line_id: u32,
    exact_text: String,
    wav_sha256: [u8; 32],
    intelligibility_text: String,
    intelligibility_wer: f64,
    speaker_similarity: f64,
    anchor_similarity: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    schema: u32,
    models: ModelIdentity,
    reference_line_id: u32,
    reference_sha256: [u8; 32],
    anchors: Vec<(u32, [u8; 32])>,
    peer_scores: Vec<(u32, f64)>,
    samples: Vec<ReviewSample>,
}

impl Manifest {
    fn valid(&self) -> bool {
        self.schema == 1 && (2..=3).contains(&self.anchors.len())
            && self.anchors.iter().any(|a| *a == (self.reference_line_id, self.reference_sha256))
            && self.anchors.iter().enumerate().all(|(i, (id, digest))| {
                *id != 0 && !self.anchors[..i].iter().any(|(a, d)| a == id || d == digest)
            })
            && self.peer_scores.len() == self.anchors.len()
            && self.peer_scores.iter().zip(&self.anchors).all(|((id, score), (anchor, _))| {
                id == anchor && (-1.0..=1.0).contains(score)
            })
            && self.samples.len() == HELD_OUT_LINES.len()
            && self.samples.iter().zip(HELD_OUT_LINES).all(|(s, &(id, text))| {
                s.line_id == id && s.exact_text == text
                    && s.intelligibility_text.len() <= 16_000
                    && word_error_rate(text, &s.intelligibility_text).is_ok_and(|wer| (wer - s.intelligibility_wer).abs() <= 1e-12)
                    && (-1.0..=1.0).contains(&s.speaker_similarity)
                    && (-1.0..=1.0).contains(&s.anchor_similarity)
            })
    }
}

fn read_exact(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    let file = fs::OpenOptions::new().read(true).share_mode(1).custom_flags(0x00200000)
        .open(path).map_err(|_| "native_review:file_unavailable")?;
    let meta = file.metadata().map_err(|_| "native_review:file_unavailable")?;
    if !meta.is_file() || meta.file_attributes() & 0x400 != 0 || !(1..=cap).contains(&meta.len()) {
        return Err("native_review:file_invalid".into());
    }
    let mut bytes = Vec::new();
    file.take(cap+1).read_to_end(&mut bytes).map_err(|_| "native_review:read_failed")?;
    if bytes.len() as u64 != meta.len() { return Err("native_review:file_changed".into()); }
    Ok(bytes)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = fs::OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|_| "native_review:stage_exists_or_unavailable")?;
    f.write_all(bytes).and_then(|_| f.sync_all()).map_err(|_| "native_review:stage_write_failed".into())
}

fn sample_path(root: &Path, line_id: u32) -> PathBuf {
    root.join(format!("held_out_{line_id}.wav"))
}

fn checked_root(root: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(root).map_err(|_| "native_review:root_unavailable")?;
    if !meta.is_dir() || meta.file_attributes() & 0x400 != 0 {
        return Err("native_review:root_invalid".into());
    }
    Ok(())
}

pub(super) struct StagedReview {
    root: PathBuf,
    pub(super) review_id: String,
}

impl StagedReview {
    /// Call only inside commit_voice_lab_review_if_active after staging and
    /// verifying all files. The existing lifecycle lock serializes Stop.
    pub(super) fn publish(self) -> Result<(), String> {
        checked_root(&self.root)?;
        if self.root.join(MANIFEST).exists() {
            return Err("native_review:already_published".into());
        }
        let pending = read_exact(&self.root.join(PENDING), MAX_MANIFEST)?;
        if hex(&hash(&pending)) != self.review_id {
            return Err("native_review:pending_changed".into());
        }
        fs::rename(self.root.join(PENDING), self.root.join(MANIFEST))
            .map_err(|_| "native_review:publish_failed".into())
    }
}

/// Requires an exclusive authorized build and an empty existing Candidate
/// workspace. A partial write is not reviewable and never changes MyVoice.
/// The established build cleanup owns removal; do not delete an old actor.
pub(super) fn stage(
    paths: &VoiceLabStoragePaths, generation: u64, deadline: Instant,
    evaluation: NativeEvaluation,
) -> Result<StagedReview, String> {
    if !authority(generation, deadline) { return Err("native_review:inactive".into()); }
    let NativeEvaluation { selected, reference_peer_scores, anchor_identities } = evaluation;
    if hash(&selected.reference_wav) != selected.reference_sha256
        || selected.reference_wav.len() as u64 > MAX_REFERENCE {
        return Err("native_review:reference_changed".into());
    }
    let source = decode_native_voice_source(&selected.reference_wav)?;
    if !(32_000*3..=32_000*10).contains(&source.len()) { return Err("native_review:reference_invalid".into()); }
    drop(source);
    let manifest = Manifest {
        schema: 1, models: ModelIdentity::current()?, reference_line_id: selected.reference_line_id,
        reference_sha256: selected.reference_sha256, anchors: anchor_identities,
        peer_scores: reference_peer_scores,
        samples: selected.samples.iter().map(|s| ReviewSample {
            line_id: s.line_id, exact_text: s.exact_text.into(), wav_sha256: hash(&s.wav),
            intelligibility_text: s.intelligibility_text.clone(), intelligibility_wer: s.intelligibility_wer,
            speaker_similarity: s.speaker_similarity, anchor_similarity: s.anchor_similarity,
        }).collect(),
    };
    if !manifest.valid() { return Err("native_review:invalid_manifest".into()); }
    let bytes = serde_json::to_vec(&manifest).map_err(|_| "native_review:serialize_failed")?;
    if bytes.len() as u64 > MAX_MANIFEST { return Err("native_review:manifest_too_large".into()); }
    let root = paths.candidate_actor_dir.clone();
    checked_root(&paths.cache_root)?;
    fs::create_dir(&root).map_err(|_| "native_review:candidate_workspace_busy")?;
    checked_root(&root)?;
    write_new(&root.join(REFERENCE), &selected.reference_wav)?;
    for sample in selected.samples {
        if !authority(generation, deadline) { return Err("native_review:cancelled".into()); }
        if sample.wav.len() as u64 > MAX_SAMPLE
            || !synthesis_audio_valid(&decode_native_review_pcm(&sample.wav)?) {
            return Err("native_review:sample_invalid".into());
        }
        write_new(&sample_path(&root, sample.line_id), &sample.wav)?;
    }
    // Recheck the actual staged bytes, not only the in-memory payload.
    verify_files(&root, &manifest)?;
    if !authority(generation, deadline) { return Err("native_review:cancelled".into()); }
    write_new(&root.join(PENDING), &bytes)?;
    Ok(StagedReview { root, review_id: hex(&hash(&bytes)) })
}

fn verified_bytes(path: &Path, cap: u64, digest: &[u8; 32]) -> Result<Vec<u8>, String> {
    let bytes = read_exact(path, cap)?;
    if hash(&bytes) != *digest { return Err("native_review:identity_mismatch".into()); }
    Ok(bytes)
}

fn verify_files(root: &Path, manifest: &Manifest) -> Result<(), String> {
    verified_bytes(&root.join(REFERENCE), MAX_REFERENCE, &manifest.reference_sha256)?;
    for sample in &manifest.samples {
        verified_bytes(&sample_path(root, sample.line_id), MAX_SAMPLE, &sample.wav_sha256)?;
    }
    Ok(())
}

fn load(root: &Path, review_id: &str) -> Result<Manifest, String> {
    checked_root(root)?;
    let bytes = read_exact(&root.join(MANIFEST), MAX_MANIFEST)?;
    if hex(&hash(&bytes)) != review_id { return Err("native_review:review_changed".into()); }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|_| "native_review:invalid_manifest")?;
    if !manifest.valid() || manifest.models != ModelIdentity::current()? {
        return Err("native_review:identity_mismatch".into());
    }
    Ok(manifest)
}

/// Exact bytes for a specific review. No caller-provided paths.
pub(super) fn review_audio(paths: &VoiceLabStoragePaths, review_id: &str, line_id: u32) -> Result<Vec<u8>, String> {
    let root = &paths.candidate_actor_dir;
    let manifest = load(root, review_id)?;
    let sample = manifest.samples.iter().find(|s| s.line_id == line_id)
        .ok_or("native_review:line_missing")?;
    verified_bytes(&sample_path(root, line_id), MAX_SAMPLE, &sample.wav_sha256)
}

/// Before a future approval transaction, verify the complete review and
/// reference payload again. This is integrity only, not listening approval,
/// current-source equivalence, or permission to change Meeting's voice.
pub(super) fn verify_review(paths: &VoiceLabStoragePaths, review_id: &str) -> Result<(), String> {
    let root = &paths.candidate_actor_dir;
    let manifest = load(root, review_id)?;
    verify_files(root, &manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Manifest {
        Manifest { schema: 1, models: ModelIdentity { dll_pins_sha256: [1;32],
            speaker_sha256: "a".repeat(64), speaker_bytes: 100 },
            reference_line_id: 1, reference_sha256: [2;32],
            anchors: vec![(1,[2;32]),(2,[3;32])], peer_scores: vec![(1,0.8),(2,0.8)],
            samples: HELD_OUT_LINES.iter().map(|&(line_id, exact_text)| ReviewSample {
                line_id, exact_text: exact_text.into(), wav_sha256: [4;32],
                intelligibility_text: exact_text.into(), intelligibility_wer: 0.0,
                speaker_similarity: 0.9, anchor_similarity: 0.8,
            }).collect() }
    }

    #[test]
    fn review_identity_covers_reference_models_anchors_and_exact_audio() {
        let mut m = fixture();
        assert!(m.valid());
        let id = hash(&serde_json::to_vec(&m).unwrap());
        m.samples[0].wav_sha256 = [5;32];
        assert_ne!(id, hash(&serde_json::to_vec(&m).unwrap()));
        m.models.dll_pins_sha256 = [6;32];
        assert_ne!(id, hash(&serde_json::to_vec(&m).unwrap()));
        m.reference_sha256 = [7;32];
        assert!(!m.valid());
    }

    #[test]
    fn rejects_missing_duplicate_or_unscored_review_lines() {
        let mut m = fixture();
        m.samples.pop();
        assert!(!m.valid());
        let mut m = fixture();
        m.samples[1].line_id = m.samples[0].line_id;
        assert!(!m.valid());
        let mut m = fixture();
        m.samples[0].intelligibility_wer = 1.0;
        assert!(!m.valid());
        let mut m = fixture();
        m.anchors[1].1 = m.anchors[0].1;
        assert!(!m.valid());
    }
}
