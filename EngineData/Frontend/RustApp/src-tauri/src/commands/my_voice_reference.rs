//! Shared native My Voice reference selection and gross synthesis checks.
//! No training, speaker approval, model lifecycle, or profile mutation.
use super::voice_lab_recording::AcceptedGuidedRecording;
use crate::engine::audio::meeting_output::decode_native_voice_source;
use std::fs;
use std::io::Read;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::rc::Rc;
use sha2::{Digest, Sha256};

const RATE: usize = 32_000;
pub(super) const SYNTHESIS_RATE: usize = 44_100;
const MAX_REFERENCE_BYTES: u64 = 2 * 1024 * 1024;

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

fn read_reference(path: &std::path::Path) -> Result<(Vec<f32>, [u8; 32]), String> {
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
    let samples = decode_native_voice_source(&bytes)?;
    Ok((samples, Sha256::digest(&bytes).into()))
}

#[derive(Debug)]
pub(super) struct SelectedReference {
    pub(super) line_id: u32,
    pub(super) wav_sha256: [u8; 32],
    pub(super) samples: Vec<f32>,
}

struct Reference { rank: SignalRank, selected: SelectedReference }

// Two bounded rankings are necessary: quiet-edge energy is comparable only
// if every eligible take has a measurable edge. Retain at most 2 * limit
// distinct PCM buffers, plus the take currently being inspected.
struct Shortlist {
    limit: usize,
    best: Vec<Rc<Reference>>,
    best_edge: Vec<Rc<Reference>>,
    all_edges: bool,
}

impl Shortlist {
    fn new(limit: usize) -> Self {
        Self { limit, best: Vec::new(), best_edge: Vec::new(), all_edges: true }
    }

    fn keep_best(list: &mut Vec<Rc<Reference>>, candidate: &Rc<Reference>, limit: usize, edges: bool) {
        let position = list.partition_point(|old| old.rank.cmp(&candidate.rank, edges).is_lt());
        if position < limit {
            list.insert(position, Rc::clone(candidate));
            list.truncate(limit);
        }
    }

    fn push(&mut self, line_id: u32, samples: Vec<f32>, wav_sha256: [u8; 32]) {
        let Some(rank) = signal_rank(line_id, &samples) else { return; };
        self.all_edges &= rank.edge.is_some();
        let candidate = Rc::new(Reference { rank, selected: SelectedReference { line_id, wav_sha256, samples } });
        Self::keep_best(&mut self.best, &candidate, self.limit, false);
        // Once any eligible reference lacks an edge, that ranking can never
        // win. Release it immediately, including candidates unique to it.
        if self.all_edges {
            Self::keep_best(&mut self.best_edge, &candidate, self.limit, true);
        } else {
            self.best_edge.clear();
        }
    }

    fn finish(self) -> Result<Vec<SelectedReference>, String> {
        let Self { best, best_edge, all_edges, .. } = self;
        let selected = if all_edges { drop(best); best_edge } else { drop(best_edge); best };
        if selected.is_empty() { return Err("preview_reference_unusable".into()); }
        selected.into_iter().map(|candidate| {
            Rc::try_unwrap(candidate).map(|reference| reference.selected)
                .map_err(|_| "preview_reference_candidate_invalid".to_string())
        }).collect()
    }
}

/// Select one to three accepted takes without training or retaining the
/// whole dataset in memory. Return the original line identity and owned
/// canonical 32 kHz PCM and SHA-256 of the exact bytes decoded; only chosen
/// references need resampling for TTS.
/// Signal ranking is not a speaker-similarity or approval decision.
pub(super) fn select_references(
    accepted: Vec<AcceptedGuidedRecording>, limit: usize, active: impl Fn() -> bool,
) -> Result<Vec<SelectedReference>, String> {
    if accepted.is_empty() || accepted.len() > 128 || !(1..=3).contains(&limit) {
        return Err("preview_reference_candidate_invalid".into());
    }
    let mut shortlist = Shortlist::new(limit);
    for take in accepted {
        if !active() { return Err("preview_cancelled_or_timed_out".into()); }
        let Ok((samples, wav_sha256)) = read_reference(&take.path) else { continue; };
        shortlist.push(take.line_id, samples, wav_sha256);
    }
    if !active() { return Err("preview_cancelled_or_timed_out".into()); }
    shortlist.finish()
}

// Same gross artifact rejection as the former preview. This cannot measure
// perceptual fidelity, identity similarity, or intelligibility.
pub(super) fn synthesis_audio_valid(samples: &[f32]) -> bool {
    if !(SYNTHESIS_RATE/10..=SYNTHESIS_RATE*30).contains(&samples.len()) { return false; }
    let (mut clipped, mut silent, mut run, mut longest) = (0,0,0,0);
    for &s in samples {
        if !s.is_finite() || s.abs() > 1.0 { return false; }
        if s.abs() >= 0.98 { clipped += 1; }
        if s.abs() <= 128.0/32768.0 {
            silent += 1; run += 1; longest = longest.max(run);
        } else { run = 0; }
    }
    clipped * 20 < samples.len() && silent * 10 < samples.len() * 9 && longest < SYNTHESIS_RATE*3
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
    fn bounded_shortlist_preserves_preview_winner_and_tie_order() {
        let mut one = Shortlist::new(1);
        let mut three = Shortlist::new(3);
        for (id, seconds) in [(4, 6), (3, 5), (1, 5), (2, 5), (5, 4)] {
            one.push(id, clean(RATE, seconds), [id as u8; 32]);
            three.push(id, clean(RATE, seconds), [id as u8; 32]);
            assert!(three.best.len() <= 3 && three.best_edge.len() <= 3);
        }
        let winner = one.finish().unwrap();
        assert_eq!(winner[0].line_id, 1);
        assert_eq!(winner[0].wav_sha256, [1; 32]);
        assert_eq!(three.finish().unwrap().into_iter().map(|reference| reference.line_id).collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn missing_edge_discards_edge_ranking_for_all_eligible_takes() {
        let mut shortlist = Shortlist::new(3);
        let mut edge = clean(RATE, 5);
        edge[..RATE / 5].fill(0.0);
        shortlist.push(1, edge, [1; 32]);
        assert!(shortlist.all_edges && !shortlist.best_edge.is_empty());
        // A rejected take must not affect comparability.
        shortlist.push(9, vec![0.0; RATE * 5], [9; 32]);
        assert!(shortlist.all_edges);
        shortlist.push(2, clean(RATE, 5), [2; 32]);
        assert!(!shortlist.all_edges && shortlist.best_edge.is_empty());
        assert_eq!(shortlist.finish().unwrap().into_iter().map(|reference| reference.line_id).collect::<Vec<_>>(), vec![2, 1]);
    }

    #[test]
    fn invalid_or_cancelled_selection_fails_closed() {
        let take = || AcceptedGuidedRecording { line_id: 1, text: "fixture", path: std::path::PathBuf::new() };
        assert!(select_references(vec![take()], 0, || true).is_err());
        assert!(select_references(vec![take()], 4, || true).is_err());
        assert_eq!(select_references(vec![take()], 1, || false).unwrap_err(), "preview_cancelled_or_timed_out");
        assert!(Shortlist::new(1).finish().is_err());
    }

    #[test]
    fn preview_rejects_gross_artifacts_without_claiming_similarity() {
        assert!(synthesis_audio_valid(&clean(SYNTHESIS_RATE, 5)));
        assert!(!synthesis_audio_valid(&vec![0.0; SYNTHESIS_RATE]));
        assert!(!synthesis_audio_valid(&vec![1.0; SYNTHESIS_RATE]));
        let mut dropout = clean(SYNTHESIS_RATE, 5);
        dropout[..SYNTHESIS_RATE*3].fill(0.0);
        assert!(!synthesis_audio_valid(&dropout));
    }
}
