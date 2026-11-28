use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use super::super::voice_lab::{
    GuidedDatasetManifest, GuidedEvaluationLineContract, GuidedTakeContract, VoiceLabStoragePaths,
    VOICE_LAB_SCHEMA_VERSION,
};

pub(super) const MAX_EVALUATION_WAV_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FROZEN_TAKE_WAV_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DATASET_MANIFEST_BYTES: u64 = 64 * 1024;

// Read metadata used by routine My Voice status with a hard allocation cap.
// Checking byte length AFTER fs::read can allocate an arbitrarily large local
// file on each poll. A pre-open path check alone does not bound reads if the
// file grows; cap the actual read from the opened handle as well.
fn read_bounded_regular_file(path: &Path, min_bytes: u64, max_bytes: u64) -> Option<Vec<u8>> {
    if min_bytes == 0 || max_bytes < min_bytes { return None; }
    let info = fs::symlink_metadata(path).ok()?;
    if !info.is_file() || info.file_type().is_symlink()
        || info.len() < min_bytes || info.len() > max_bytes
    {
        return None;
    }
    let file = File::open(path).ok()?;
    let opened = file.metadata().ok()?;
    if !opened.is_file() || opened.len() != info.len() { return None; }
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1)).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 == info.len()).then_some(bytes)
}

pub(super) const HELD_OUT_LINES: &[(u32, &str)] = &[
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VoiceLabReferenceCriterion {
    OnlyEligible,
    LowerClippingFraction,
    LowerRelativeEdgeEnergy,
    LowerSilenceFraction,
    LowerDcOffset,
    HigherSpeakerConsistency,
    CloserToTargetDuration,
    LineIdTiebreak,
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceLabReferenceSelection {
    pub line_id: u32,
    pub duration_ms: u64,
    pub eligible_take_count: usize,
    pub criterion: VoiceLabReferenceCriterion,
    pub quiet_edge_comparable: bool,
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
    let Some(bytes) = read_bounded_regular_file(
        &dataset_manifest, 1, MAX_DATASET_MANIFEST_BYTES
    ) else {
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

