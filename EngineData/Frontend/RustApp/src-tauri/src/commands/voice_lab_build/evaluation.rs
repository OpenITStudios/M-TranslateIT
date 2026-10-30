use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use super::super::voice_lab::{
    GuidedDatasetManifest, GuidedEvaluationLineContract, GuidedTakeContract, VoiceLabStoragePaths,
    archived_frozen_review_matches,
    VOICE_ACTOR_ENGINE, VOICE_ACTOR_ENGINE_REVISION, VOICE_LAB_SCHEMA_VERSION, sha256_digest,
};

pub(super) const MAX_EVALUATION_WAV_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FROZEN_TAKE_WAV_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DATASET_MANIFEST_BYTES: u64 = 64 * 1024;
fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}


fn sha256_matches(bytes: &[u8], expected: &str) -> bool {
    if !valid_sha256(expected) {
        return false;
    }
    let Some(digest) = sha256_digest(bytes) else {
        return false;
    };
    const HEX: &[u8; 16] = b"0123456789abcdef";
    digest.iter().enumerate().all(|(index, value)| {
        let byte = *value as usize;
        expected.as_bytes()[2 * index] == HEX[byte >> 4]
            && expected.as_bytes()[2 * index + 1] == HEX[byte & 15]
    })
}

// Hash the very same bounded bytes that are returned for playback. Never
// trust a manifest SHA-256 string by format alone.
pub(super) fn verified_held_out_wav(
    root: &Path,
    sample: &VoiceLabEvaluationSample,
) -> Option<Vec<u8>> {
    if !valid_sha256(&sample.sha256)
        || sample.wav_file != format!("held_out_{}.wav", sample.line_id)
    {
        return None;
    }
    let path = root.join(&sample.wav_file);
    let info = fs::symlink_metadata(&path).ok()?;
    if info.file_type().is_symlink()
        || !info.is_file()
        || info.len() < 44
        || info.len() > MAX_EVALUATION_WAV_BYTES
    {
        return None;
    }
    let file = File::open(path).ok()?;
    if file.metadata().ok()?.len() != info.len() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_EVALUATION_WAV_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 != info.len()
        || !sha256_matches(&bytes, &sample.sha256)
    {
        return None;
    }
    Some(bytes)
}

// The 1.4s UI status polling stays metadata-only; strong verification occurs
// when a sample is served and immediately before actor promotion.
pub(super) fn held_out_wavs_intact(
    paths: &VoiceLabStoragePaths,
    manifest: &EvaluationManifest,
) -> bool {
    let root = evaluation_dir(paths);
    manifest
        .samples
        .iter()
        .all(|sample| verified_held_out_wav(&root, sample).is_some())
}

pub(super) const EVALUATION_SELECTION_METHOD: &str =
    "held_out_artifacts_then_mean_wer_then_max_wer_then_similarity_tiebreak";

const HELD_OUT_LINES: &[(u32, &str)] = &[
    (1001, "Please confirm the final schedule before we send the update to the client."),
    (1002, "The system should remain clear and natural during a longer technical discussion."),
    (1003, "I can review the latest results tomorrow morning and share my decision with the team."),
    (1004, "Do not restart the service until the backup is complete and the result is verified."),
    (1005, "Can everyone hear the translated voice clearly during this meeting?"),
    (1006, "The maintenance window starts at seven thirty in the evening on September twenty first."),
    (1007, "I said fifteen, not fifty, so please correct the invoice before approval."),
    (1008, "Before we continue, summarize the current status, the remaining risk, and the next action."),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceLabEvaluationSample {
    pub line_id: u32,
    pub exact_text: String,
    pub wav_file: String,
    pub sha256: String,
    pub speaker_similarity: f64,
    pub intelligibility_text: String,
    pub intelligibility_wer: f64,
    pub artifact_flags: Vec<String>,
}

// Comparison data is strictly advisory. It describes only checkpoints from
// the SAME completed training run, never a previous approved voice.
#[derive(Debug, Clone, Serialize)]
pub struct VoiceLabCandidateComparison {
    pub candidate_id: String,
    pub selected_for_review: bool,
    pub mean_intelligibility_wer: f64,
    pub maximum_intelligibility_wer: f64,
    pub mean_speaker_similarity: f64,
    pub minimum_speaker_similarity: f64,
    pub artifact_case_count: usize,
}

#[derive(Debug, Deserialize)]
struct CandidateEvidenceSample {
    line_id: u32,
    exact_text: String,
    sha256: String,
    intelligibility_text: String,
    intelligibility_wer: f64,
    speaker_similarity: f64,
    artifact_flags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CandidateEvidenceRow {
    candidate_id: String,
    samples: Vec<CandidateEvidenceSample>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct EvaluationManifest {
    schema_version: u32,
    engine: String,
    engine_revision: String,
    pub(super) selection_method: String,
    pub(super) selected_candidate_id: String,
    pub(super) review_id: String,
    pub(super) samples: Vec<VoiceLabEvaluationSample>,
    // Legacy reviews may not have a checkpoint comparison. A malformed
    // optional comparison must never break the original Review/approval.
    #[serde(default)]
    candidate_evidence: Option<serde_json::Value>,
}

// Only surface comparison rows when all 2–3 candidates describe the SAME
// eight authored held-out sentences and the selected row matches every
// current selected WAV hash, recognizer text, score and artifact flag.
// These are reported measurements, not independent verification of unselected
// audio files, identity, naturalness or the previously approved Meeting voice.
// The worker's review_id is SHA256 of Python compact sorted-key JSON containing
// engine_revision, selection_method, candidate_id and ordered selected WAV hashes.
// Reject a stale review ID rather than letting different audio reuse "Listened".
fn selected_review_fingerprint_matches(manifest: &EvaluationManifest) -> bool {
    if !valid_sha256(&manifest.review_id)
        || manifest.samples.len() != HELD_OUT_LINES.len()
        || manifest.samples.iter().any(|sample| !valid_sha256(&sample.sha256))
    {
        return false;
    }
    let audio = manifest.samples.iter()
        .map(|sample| (sample.line_id, sample.sha256.as_str()))
        .collect::<Vec<_>>();
    // BTreeMap is sorted even if serde_json's preserve_order feature is used
    // elsewhere. Tuple serialization yields Python's [line_id, sha256] arrays.
    let mut fingerprint = std::collections::BTreeMap::new();
    fingerprint.insert("audio", serde_json::json!(audio));
    fingerprint.insert("candidate_id", serde_json::json!(&manifest.selected_candidate_id));
    fingerprint.insert("engine_revision", serde_json::json!(&manifest.engine_revision));
    fingerprint.insert("selection_method", serde_json::json!(&manifest.selection_method));
    let Ok(bytes) = serde_json::to_vec(&fingerprint) else { return false; };
    sha256_matches(&bytes, &manifest.review_id)
}

pub(super) fn candidate_comparison(manifest: &EvaluationManifest) -> Vec<VoiceLabCandidateComparison> {
    let Some(raw) = manifest.candidate_evidence.clone() else {
        return Vec::new();
    };
    let Ok(candidates) = serde_json::from_value::<Vec<CandidateEvidenceRow>>(raw) else {
        return Vec::new();
    };
    if !(2..=3).contains(&candidates.len()) || manifest.samples.len() != HELD_OUT_LINES.len() {
        return Vec::new();
    }
    let mut identities = std::collections::BTreeSet::new();
    let mut selected_count = 0_usize;
    let mut summaries = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let id = candidate.candidate_id.as_str();
        if id.is_empty() || id.len() > 64
            || !id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
            || !identities.insert(candidate.candidate_id.clone())
            || candidate.samples.len() != HELD_OUT_LINES.len()
        {
            return Vec::new();
        }
        let selected_for_review = candidate.candidate_id == manifest.selected_candidate_id;
        if selected_for_review { selected_count += 1; }
        let mut seen = std::collections::BTreeSet::new();
        let mut wer_sum = 0.0_f64;
        let mut worst_wer = 0.0_f64;
        let mut similarity_sum = 0.0_f64;
        let mut minimum_similarity = f64::INFINITY;
        let mut flagged_count = 0_usize;
        for sample in &candidate.samples {
            let Some((_, canonical_text)) = HELD_OUT_LINES.iter()
                .find(|(line_id, _)| *line_id == sample.line_id)
            else { return Vec::new(); };
            if !seen.insert(sample.line_id)
                || sample.exact_text != *canonical_text
                || !valid_sha256(&sample.sha256)
                || sample.intelligibility_text.trim().is_empty()
                || !sample.intelligibility_wer.is_finite()
                || sample.intelligibility_wer < 0.0
                || !sample.speaker_similarity.is_finite()
                || !(-1.000001..=1.000001).contains(&sample.speaker_similarity)
                || sample.artifact_flags.iter().any(|flag| {
                    !matches!(flag.as_str(), "clipping" | "unexpected_silence")
                })
            {
                return Vec::new();
            }
            if selected_for_review {
                let Some(approved_review_sample) = manifest.samples.iter()
                    .find(|current| current.line_id == sample.line_id)
                else { return Vec::new(); };
                if approved_review_sample.exact_text != sample.exact_text
                    || approved_review_sample.sha256 != sample.sha256
                    || approved_review_sample.intelligibility_text != sample.intelligibility_text
                    || (approved_review_sample.intelligibility_wer - sample.intelligibility_wer).abs() > 0.000001
                    || (approved_review_sample.speaker_similarity - sample.speaker_similarity).abs() > 0.000001
                    || approved_review_sample.artifact_flags != sample.artifact_flags
                {
                    return Vec::new();
                }
            }
            wer_sum += sample.intelligibility_wer;
            worst_wer = worst_wer.max(sample.intelligibility_wer);
            similarity_sum += sample.speaker_similarity;
            minimum_similarity = minimum_similarity.min(sample.speaker_similarity);
            if !sample.artifact_flags.is_empty() { flagged_count += 1; }
        }
        if seen.len() != HELD_OUT_LINES.len() { return Vec::new(); }
        let count = candidate.samples.len() as f64;
        summaries.push(VoiceLabCandidateComparison {
            candidate_id: candidate.candidate_id,
            selected_for_review,
            mean_intelligibility_wer: wer_sum / count,
            maximum_intelligibility_wer: worst_wer,
            mean_speaker_similarity: similarity_sum / count,
            minimum_speaker_similarity: minimum_similarity,
            artifact_case_count: flagged_count,
        });
    }
    if selected_count != 1 { return Vec::new(); }
    summaries
}

pub(super) fn evaluation_dir(paths: &VoiceLabStoragePaths) -> PathBuf {
    paths.cache_root.join("Evaluation")
}

pub(super) fn held_out_contract() -> Vec<GuidedEvaluationLineContract> {
    HELD_OUT_LINES
        .iter()
        .map(|(line_id, exact_text)| GuidedEvaluationLineContract {
            line_id: *line_id,
            exact_text: (*exact_text).to_string(),
        })
        .collect()
}

// A review can outlive the accepted takes used to train it. Compare against
// the actual frozen build dataset, not a UI revision or only filenames/sizes.
// This stays in the existing evaluation owner and does not mutate recordings.
fn same_bounded_wav_bytes(source: &Path, frozen: &Path) -> bool {
    let (Ok(a), Ok(b)) = (fs::symlink_metadata(source), fs::symlink_metadata(frozen)) else {
        return false;
    };
    if !a.is_file() || !b.is_file() || a.file_type().is_symlink() || b.file_type().is_symlink()
        || a.len() < 44 || a.len() > MAX_FROZEN_TAKE_WAV_BYTES || a.len() != b.len()
    {
        return false;
    }
    let (Ok(mut a_file), Ok(mut b_file)) = (File::open(source), File::open(frozen)) else {
        return false;
    };
    let mut a_buffer = [0_u8; 8192];
    let mut b_buffer = [0_u8; 8192];
    loop {
        let Ok(count) = a_file.read(&mut a_buffer) else {
            return false;
        };
        if count == 0 {
            return matches!(b_file.read(&mut b_buffer), Ok(0));
        }
        if b_file.read_exact(&mut b_buffer[..count]).is_err()
            || a_buffer[..count] != b_buffer[..count]
        {
            return false;
        }
    }
}

pub(super) fn frozen_dataset_matches_current(
    takes_dir: &Path,
    dataset_dir: &Path,
    current_takes: &[GuidedTakeContract],
) -> bool {
    let dataset_manifest = dataset_dir.join("dataset.json");
    let Ok(info) = fs::symlink_metadata(&dataset_manifest) else {
        return false;
    };
    if !info.is_file() || info.file_type().is_symlink()
        || info.len() == 0 || info.len() > MAX_DATASET_MANIFEST_BYTES
    {
        return false;
    }
    let Ok(bytes) = fs::read(dataset_manifest) else {
        return false;
    };
    let Ok(frozen) = serde_json::from_slice::<GuidedDatasetManifest>(&bytes) else {
        return false;
    };
    if frozen.schema_version != VOICE_LAB_SCHEMA_VERSION
        || !frozen.authorized_voice_confirmed
        || frozen.held_out_lines != held_out_contract()
        || frozen.takes.is_empty()
        || frozen.takes != current_takes
    {
        return false;
    }
    frozen.takes.iter().all(|take| {
        same_bounded_wav_bytes(
            &takes_dir.join(&take.wav_file),
            &dataset_dir.join(&take.wav_file),
        )
    })
}

pub(super) fn evaluation_manifest(paths: &VoiceLabStoragePaths) -> Option<EvaluationManifest> {
    let root = evaluation_dir(paths);
    let bytes = fs::read(root.join("evaluation.json")).ok()?;
    if bytes.is_empty() || bytes.len() > 128 * 1024 {
        return None;
    }
    let manifest = serde_json::from_slice::<EvaluationManifest>(&bytes).ok()?;
    if manifest.schema_version != VOICE_LAB_SCHEMA_VERSION
        || manifest.engine != VOICE_ACTOR_ENGINE
        || manifest.engine_revision != VOICE_ACTOR_ENGINE_REVISION
        || manifest.selection_method != EVALUATION_SELECTION_METHOD
        || manifest.selected_candidate_id.trim().is_empty()
        || !valid_sha256(&manifest.review_id)
        || manifest.samples.len() != HELD_OUT_LINES.len()
    {
        return None;
    }
    for (expected_line_id, expected_text) in HELD_OUT_LINES {
        let matches = manifest
            .samples
            .iter()
            .filter(|sample| {
                sample.line_id == *expected_line_id
                    && sample.exact_text.trim() == expected_text.trim()
            })
            .count();
        if matches != 1 {
            return None;
        }
    }
    for sample in &manifest.samples {
        if !sample.speaker_similarity.is_finite()
            || !sample.intelligibility_wer.is_finite()
            || sample.intelligibility_wer < 0.0
            || sample.intelligibility_text.trim().is_empty()
            || sample.artifact_flags.iter().any(|flag| {
                !matches!(flag.as_str(), "clipping" | "unexpected_silence")
            })
            || !valid_sha256(&sample.sha256)
            || sample.wav_file != format!("held_out_{}.wav", sample.line_id)
            || Path::new(&sample.wav_file).file_name().and_then(|name| name.to_str())
                != Some(sample.wav_file.as_str())
        {
            return None;
        }
        let wav = root.join(&sample.wav_file);
        let metadata = fs::symlink_metadata(wav).ok()?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() < 44
            || metadata.len() > MAX_EVALUATION_WAV_BYTES
        {
            return None;
        }
    }
    // File-byte integrity is separately checked on playback/approval, but
    // metadata must first bind the review identity to the selected WAV hashes.
    if !selected_review_fingerprint_matches(&manifest) {
        return None;
    }
    Some(manifest)
}


pub(super) fn reviewable_evaluation(
    paths: &VoiceLabStoragePaths,
    current_takes: &[GuidedTakeContract],
) -> Option<EvaluationManifest> {
    let evaluation = evaluation_manifest(paths)?;
    let actor_bytes = fs::read(paths.candidate_actor_dir.join("actor.json")).ok()?;
    let actor_json = serde_json::from_slice::<serde_json::Value>(&actor_bytes).ok()?;
    let matches_training = match archived_frozen_review_matches(paths) {
        Ok(Some(matches)) => matches,
        Ok(None) => frozen_dataset_matches_current(
            &paths.takes_dir, &paths.build_dataset_dir, current_takes,
        ),
        Err(_) => false,
    };
    if !candidate_selection_matches_actor_json(&evaluation, &actor_json)
        || !matches_training
    {
        return None;
    }
    Some(evaluation)
}

pub(super) fn candidate_selection_matches_actor_json(
    manifest: &EvaluationManifest,
    actor_json: &serde_json::Value,
) -> bool {
    actor_json
        .get("candidate_selection")
        .and_then(|value| value.as_object())
        .and_then(|selection| {
            Some((
                selection.get("method")?.as_str()?,
                selection.get("candidate_id")?.as_str()?,
            ))
        })
        .map(|(method, candidate_id)| {
            method == manifest.selection_method && candidate_id == manifest.selected_candidate_id
        })
        .unwrap_or(false)
}


pub(super) fn evaluation_review_complete(
    manifest: &EvaluationManifest,
    reviewed_line_ids: &[u32],
) -> bool {
    use std::collections::BTreeSet;

    let expected = manifest
        .samples
        .iter()
        .map(|sample| sample.line_id)
        .collect::<BTreeSet<_>>();
    let reviewed = reviewed_line_ids.iter().copied().collect::<BTreeSet<_>>();
    reviewed.len() == reviewed_line_ids.len() && reviewed == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> EvaluationManifest {
        EvaluationManifest {
            schema_version: VOICE_LAB_SCHEMA_VERSION,
            engine: VOICE_ACTOR_ENGINE.to_string(),
            engine_revision: VOICE_ACTOR_ENGINE_REVISION.to_string(),
            selection_method: EVALUATION_SELECTION_METHOD.to_string(),
            selected_candidate_id: "s8-g15".to_string(),
            review_id: "a".repeat(64),
            candidate_evidence: None,
            samples: HELD_OUT_LINES
                .iter()
                .map(|(line_id, text)| VoiceLabEvaluationSample {
                    line_id: *line_id,
                    exact_text: (*text).to_string(),
                    wav_file: format!("held_out_{line_id}.wav"),
                    sha256: "b".repeat(64),
                    speaker_similarity: 0.9,
                    intelligibility_text: (*text).to_string(),
                    intelligibility_wer: 0.0,
                    artifact_flags: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn trained_review_requires_the_exact_frozen_recordings() {
        let root = std::env::temp_dir().join(format!(
            "translateit-frozen-take-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos(),
        ));
        let takes_dir = root.join("Takes");
        let dataset_dir = root.join("Dataset");
        fs::create_dir_all(&takes_dir).expect("takes");
        fs::create_dir_all(&dataset_dir).expect("dataset");
        let filename = "take_0001.wav";
        let take = GuidedTakeContract {
            line_id: 1,
            exact_text: "Good morning, everyone.".into(),
            wav_file: filename.into(),
        };
        let snapshot = GuidedDatasetManifest {
            schema_version: VOICE_LAB_SCHEMA_VERSION,
            authorized_voice_confirmed: true,
            takes: vec![take.clone()],
            held_out_lines: held_out_contract(),
        };
        fs::write(dataset_dir.join("dataset.json"), serde_json::to_vec(&snapshot).expect("json"))
            .expect("snapshot");
        let first = vec![1_u8; 64];
        fs::write(dataset_dir.join(filename), &first).expect("frozen audio");
        fs::write(takes_dir.join(filename), &first).expect("accepted audio");
        assert!(frozen_dataset_matches_current(&takes_dir, &dataset_dir, &[take.clone()]));

        // A replacement with exactly the same WAV length must still invalidate review.
        let second = vec![2_u8; 64];
        fs::write(takes_dir.join(filename), &second).expect("replace audio");
        assert!(!frozen_dataset_matches_current(&takes_dir, &dataset_dir, &[take.clone()]));
        fs::write(takes_dir.join(filename), &first).expect("restore audio");

        let added = GuidedTakeContract {
            line_id: 2,
            exact_text: "Thanks for joining today.".into(),
            wav_file: "take_0002.wav".into(),
        };
        assert!(!frozen_dataset_matches_current(
            &takes_dir,
            &dataset_dir,
            &[take.clone(), added],
        ));
        fs::remove_file(takes_dir.join(filename)).expect("remove accepted audio");
        assert!(!frozen_dataset_matches_current(&takes_dir, &dataset_dir, &[take]));
        fs::remove_dir_all(root).expect("cleanup");
    }


    #[cfg(target_os = "windows")]
    #[test]
    fn held_out_sha256_is_bound_to_exact_wav_bytes() {
        assert!(sha256_matches(
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        ));
        assert!(!sha256_matches(
            b"abd",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        ));

        let root = std::env::temp_dir().join(format!(
            "translateit-wav-integrity-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&root).expect("fixture directory");
        let mut bytes = vec![0x42_u8; 44];
        let digest = sha256_digest(&bytes).expect("native SHA-256");
        let sha256 = digest.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let sample = VoiceLabEvaluationSample {
            line_id: 1001,
            exact_text: "Test".into(),
            wav_file: "held_out_1001.wav".into(),
            sha256,
            speaker_similarity: 0.9,
            intelligibility_text: "Test".into(),
            intelligibility_wer: 0.0,
            artifact_flags: Vec::new(),
        };
        let path = root.join(&sample.wav_file);
        fs::write(&path, &bytes).expect("write fixture");
        assert_eq!(verified_held_out_wav(&root, &sample), Some(bytes.clone()));
        bytes[43] ^= 1;
        fs::write(&path, &bytes).expect("change same-length bytes");
        assert!(verified_held_out_wav(&root, &sample).is_none());
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn review_identity_is_specific_and_well_formed() {
        let manifest = manifest();
        assert_eq!(manifest.review_id.len(), 64);
        assert_ne!(manifest.review_id, "b".repeat(64));
        assert!(valid_sha256(&manifest.samples[0].sha256));
        assert!(!valid_sha256(&"G".repeat(64)));
        assert!(!valid_sha256(&"a".repeat(63)));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn review_id_must_bind_exact_selected_wav_hashes_and_order() {
        let mut report = manifest();
        // Independent Python canonical JSON fixture:
        // hashlib.sha256(json.dumps(fingerprint, sort_keys=True,
        //   separators=(",", ":")).encode()).hexdigest()
        report.review_id =
            "ddca43c82f3e8ab8955ad17f391169932fad1eb7d7f4bba0068c5eeb233d5da9".into();
        assert!(selected_review_fingerprint_matches(&report));
        report.samples[0].sha256 = "a".repeat(64);
        assert!(!selected_review_fingerprint_matches(&report));
        report.samples[0].sha256 = "b".repeat(64);
        report.samples.swap(0, 1);
        assert!(!selected_review_fingerprint_matches(&report));
        report.samples.swap(0, 1);
        report.selected_candidate_id = "s6-g10".into();
        assert!(!selected_review_fingerprint_matches(&report));
    }

    #[test]
    fn candidate_selection_binding_rejects_mismatched_actor() {
        let manifest = manifest();
        let matching = serde_json::json!({
            "candidate_selection": {
                "method": EVALUATION_SELECTION_METHOD,
                "candidate_id": "s8-g15"
            }
        });
        assert!(candidate_selection_matches_actor_json(&manifest, &matching));
        let stale = serde_json::json!({
            "candidate_selection": {
                "method": EVALUATION_SELECTION_METHOD,
                "candidate_id": "s6-g10"
            }
        });
        assert!(!candidate_selection_matches_actor_json(&manifest, &stale));
    }

    fn checkpoint_evidence(
        manifest: &EvaluationManifest,
        candidate_id: &str,
        similarity: f64,
        wer: f64,
    ) -> serde_json::Value {
        serde_json::json!({
            "candidate_id": candidate_id,
            "samples": manifest.samples.iter().map(|sample| serde_json::json!({
                "line_id": sample.line_id,
                "exact_text": &sample.exact_text,
                "sha256": if candidate_id == manifest.selected_candidate_id {
                    sample.sha256.as_str()
                } else { "c" },
                "intelligibility_text": &sample.intelligibility_text,
                "intelligibility_wer": wer,
                "speaker_similarity": similarity,
                "artifact_flags": [],
            })).collect::<Vec<_>>(),
        })
    }

    #[test]
    fn candidate_comparison_requires_complete_matched_eight_line_evidence() {
        let mut report = manifest();
        assert!(candidate_comparison(&report).is_empty(), "legacy review has no comparison");
        let selected = checkpoint_evidence(&report, "s8-g15", 0.9, 0.0);
        let mut alternative = checkpoint_evidence(&report, "s6-g10", 0.8, 0.25);
        for sample in alternative["samples"].as_array_mut().unwrap() {
            sample["sha256"] = serde_json::json!("c".repeat(64));
        }
        report.candidate_evidence = Some(serde_json::json!([selected, alternative]));
        let rows = candidate_comparison(&report);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].selected_for_review);
        assert!(!rows[1].selected_for_review);
        assert_eq!(rows[0].mean_intelligibility_wer, 0.0);
        assert_eq!(rows[1].maximum_intelligibility_wer, 0.25);
        assert_eq!(rows[1].minimum_speaker_similarity, 0.8);

        let valid = report.candidate_evidence.clone().unwrap();
        report.candidate_evidence.as_mut().unwrap()[0]["samples"][0]["sha256"] =
            serde_json::json!("a".repeat(64));
        assert!(candidate_comparison(&report).is_empty(), "selected WAV SHA drift is not comparable");
        report.candidate_evidence = Some(valid.clone());
        report.candidate_evidence.as_mut().unwrap()[1]["samples"][0]["exact_text"] =
            serde_json::json!("Unrelated sentence");
        assert!(candidate_comparison(&report).is_empty(), "different held-out text cannot compare");
        report.candidate_evidence = Some(valid.clone());
        report.candidate_evidence.as_mut().unwrap()[1]["samples"].as_array_mut().unwrap().pop();
        assert!(candidate_comparison(&report).is_empty(), "partial candidate is not comparable");
        report.candidate_evidence = Some(valid.clone());
        report.candidate_evidence.as_mut().unwrap()[1]["candidate_id"] =
            serde_json::json!("s8-g15");
        assert!(candidate_comparison(&report).is_empty(), "duplicate candidate ID");
        report.candidate_evidence = Some(serde_json::json!([valid[0].clone()]));
        assert!(candidate_comparison(&report).is_empty(), "one candidate has no comparison");
    }

    #[test]
    fn review_completion_requires_every_exact_held_out_line_once() {
        let manifest = manifest();
        let ids = HELD_OUT_LINES.iter().map(|(line_id, _)| *line_id).collect::<Vec<_>>();
        assert!(evaluation_review_complete(&manifest, &ids));
        assert!(!evaluation_review_complete(&manifest, &ids[..ids.len() - 1]));
        let mut duplicate = ids.clone();
        duplicate.push(ids[0]);
        assert!(!evaluation_review_complete(&manifest, &duplicate));
    }
}
