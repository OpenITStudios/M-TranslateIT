//! Independent My Voice quality verifier, not a TTS conditioning embedding.
//! Exact legacy forward3 graph exported offline; FP32 feature and graph parity
//! must be demonstrated before admitting an export as a build-time artifact.
//! Only use during an exclusive, authorized VoiceLab evaluation.
use crate::engine::audio::{guided_take::resample_mono_fft, silero::verified_ort_environment};
use crate::engine::paths::ProjectPaths;
use ort::{session::Session, value::Tensor};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

const RATE: usize = 16_000;
const FRAME: usize = 400;
const HOP: usize = 160;
const FFT: usize = 512;
const MEL: usize = 80;
const DIM: usize = 20_480;
const MAX_PCM: usize = RATE * 60;
const MAX_MODEL_BYTES: u64 = 512 * 1024 * 1024;
const PI: f32 = std::f32::consts::PI;

fn verified_model(path: &Path, digest: &str, size: u64) -> Result<Vec<u8>, String> {
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || size == 0 || size > MAX_MODEL_BYTES {
        return Err("native_speaker:invalid_build_pin".into());
    }
    let file = fs::OpenOptions::new().read(true).share_mode(1).custom_flags(0x00200000)
        .open(path).map_err(|_| "native_speaker:asset_missing")?;
    let meta = file.metadata().map_err(|_| "native_speaker:asset_missing")?;
    if !meta.is_file() || meta.file_attributes() & 0x400 != 0 || meta.len() != size {
        return Err("native_speaker:asset_identity_mismatch".into());
    }
    let mut bytes = Vec::new();
    file.take(size + 1).read_to_end(&mut bytes).map_err(|_| "native_speaker:asset_read_failed")?;
    if bytes.len() as u64 != size
        || Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect::<String>() != digest {
        return Err("native_speaker:asset_identity_mismatch".into());
    }
    Ok(bytes)
}

// Exact legacy fbank contract from pinned GPT-SoVITS sv.py + kaldi.py:
// normalized PCM, snip edges, 25/10 ms, per-frame DC removal, 0.97
// preemphasis, Povey window, 512-point power spectrum, 20..8000 Hz,
// 80 triangular mel filters, float epsilon floor, NO column-mean normalization.
// Rust FP32 parity against the actual legacy implementation is a held gate.
struct Fbank {
    window: [f32; FRAME],
    twiddles: [(f32, f32); FFT / 2],
    filters: Vec<Vec<(usize, f32)>>,
}

fn mel(hz: f32) -> f32 { 1127.0 * (1.0 + hz / 700.0).ln() }

impl Fbank {
    fn new() -> Self {
        let window = std::array::from_fn(|i| {
            (0.5 - 0.5 * (2.0 * PI * i as f32 / (FRAME - 1) as f32).cos()).powf(0.85)
        });
        let twiddles = std::array::from_fn(|i| {
            let angle = -2.0 * PI * i as f32 / FFT as f32;
            (angle.cos(), angle.sin())
        });
        let lo = mel(20.0);
        let delta = (mel(8000.0) - lo) / (MEL + 1) as f32;
        let filters = (0..MEL).map(|m| {
            let left = lo + m as f32 * delta;
            let center = left + delta;
            let right = center + delta;
            (0..FFT/2).filter_map(|k| {
                let freq = mel(k as f32 * RATE as f32 / FFT as f32);
                let w = ((freq - left) / delta).min((right - freq) / delta);
                (w > 0.0).then_some((k, w))
            }).collect()
        }).collect();
        Self { window, twiddles, filters }
    }

    fn transform(&self, frame: &[f32], real: &mut [f32; FFT], imag: &mut [f32; FFT]) {
        let mean = frame.iter().sum::<f32>() / FRAME as f32;
        let mut previous = frame[0] - mean;
        real.fill(0.0);
        imag.fill(0.0);
        for i in 0..FRAME {
            let current = frame[i] - mean;
            real[i] = (current - 0.97 * previous) * self.window[i];
            previous = current;
        }
        // Iterative radix-2 FFT; fixed scratch arrays, one plan per verifier.
        let mut j = 0;
        for i in 1..FFT {
            let mut bit = FFT >> 1;
            while j & bit != 0 { j ^= bit; bit >>= 1; }
            j ^= bit;
            if i < j { real.swap(i, j); }
        }
        let mut width = 2;
        while width <= FFT {
            let half = width/2;
            for start in (0..FFT).step_by(width) {
                for i in 0..half {
                    let (wr, wi) = self.twiddles[i * FFT/width];
                    let a = start+i;
                    let b = a+half;
                    let tr = wr*real[b] - wi*imag[b];
                    let ti = wr*imag[b] + wi*real[b];
                    real[b] = real[a] - tr;
                    imag[b] = imag[a] - ti;
                    real[a] += tr;
                    imag[a] += ti;
                }
            }
            width *= 2;
        }
    }

    fn compute(&self, pcm: &[f32], mut cancelled: impl FnMut() -> bool) -> Result<(usize, Vec<f32>), String> {
        if !(RATE*3/10..=MAX_PCM).contains(&pcm.len())
            || pcm.iter().any(|s| !s.is_finite() || s.abs() > 1.0)
            || !pcm.iter().any(|s| s.abs() >= 0.0001) {
            return Err("native_speaker:invalid_pcm".into());
        }
        let frames = 1 + (pcm.len() - FRAME) / HOP;
        let mut features = Vec::with_capacity(frames*MEL);
        let mut real = [0.0; FFT];
        let mut imag = [0.0; FFT];
        for frame in 0..frames {
            if cancelled() { return Err("native_speaker:cancelled".into()); }
            let start = frame*HOP;
            self.transform(&pcm[start..start+FRAME], &mut real, &mut imag);
            for k in 0..FFT/2 { real[k] = real[k]*real[k] + imag[k]*imag[k]; }
            for filter in &self.filters {
                let power = filter.iter().map(|&(k,w)| real[k]*w).sum::<f32>();
                features.push(power.max(f32::EPSILON).ln());
            }
        }
        if features.iter().any(|v| !v.is_finite()) { return Err("native_speaker:invalid_features".into()); }
        Ok((frames, features))
    }
}

// Only an observed, nonzero, finite forward3 output creates a scoring vector.
// Not serialized or kept in a user's selected profile.
pub(super) struct Embedding(Vec<f32>);

impl Embedding {
    fn new(values: &[f32]) -> Result<Self, String> {
        if values.len() != DIM || values.iter().any(|x| !x.is_finite()) {
            return Err("native_speaker:invalid_embedding".into());
        }
        let norm = values.iter().map(|&x| f64::from(x).powi(2)).sum::<f64>().sqrt();
        if !norm.is_finite() || norm <= 1e-8 { return Err("native_speaker:invalid_embedding".into()); }
        Ok(Self(values.iter().map(|&x| (f64::from(x)/norm) as f32).collect()))
    }
    pub(super) fn cosine(&self, other: &Self) -> f64 {
        self.0.iter().zip(&other.0).map(|(&a,&b)| f64::from(a)*f64::from(b)).sum::<f64>().clamp(-1.0,1.0)
    }
}

/// Evaluation-only CPU session: no extra GPU context or Meeting hot-path cost.
/// Instantiate once for all references and held-out lines, under the existing
/// exclusive VoiceLab owner. No external model path or score supplied by UI.
pub(super) struct SpeakerVerifier {
    session: Session,
    features: Fbank,
}

impl SpeakerVerifier {
    pub(super) fn open() -> Result<Self, String> {
        let paths = ProjectPaths::discover();
        if !paths.packaged_context_initialized && !paths.development_root_verified {
            return Err("native_speaker:unverified_runtime_root".into());
        }
        // The exact ONNX bytes are NOT known until the offline export and
        // parity gate. Never accept a runtime-supplied digest or unpinned file.
        let digest = option_env!("TRANSLATEIT_SPEAKER_ONNX_SHA256")
            .ok_or("native_speaker:reviewed_export_required")?;
        let size = option_env!("TRANSLATEIT_SPEAKER_ONNX_BYTES")
            .and_then(|v| v.parse::<u64>().ok()).ok_or("native_speaker:reviewed_export_required")?;
        let path = Path::new(&paths.voice_runtime_dir).join("ModelData/SpeakerVerifier/eres2netv2-forward3-f32.onnx");
        let bytes = verified_model(&path, digest, size)?;
        let env = verified_ort_environment()?;
        // Existing pinned CPU ORT runtime. No CUDA EP, Python or new library.
        let session = Session::builder(&env).and_then(|b| Ok(b.with_intra_threads(2)?))
            .and_then(|b| Ok(b.with_inter_threads(1)?))
            .and_then(|mut b| b.commit_from_memory(&bytes))
            .map_err(|_| "native_speaker:model_load_failed")?;
        Ok(Self { session, features: Fbank::new() })
    }

    pub(super) fn embed(&mut self, pcm: &[f32], rate: u32, mut cancelled: impl FnMut() -> bool) -> Result<Embedding, String> {
        if !matches!(rate, 16_000 | 32_000 | 44_100 | 48_000)
            || pcm.len() < rate as usize*3/10 || pcm.len() > rate as usize*60
            || pcm.iter().any(|s| !s.is_finite() || s.abs() > 1.0) {
            return Err("native_speaker:invalid_source_pcm".into());
        }
        if cancelled() { return Err("native_speaker:cancelled".into()); }
        let converted;
        let samples = if rate == RATE as u32 { pcm } else {
            converted = resample_mono_fft(pcm, rate, RATE as u32)?;
            converted.as_slice()
        };
        let (frames, features) = self.features.compute(samples, &mut cancelled)?;
        if cancelled() { return Err("native_speaker:cancelled".into()); }
        let result = self.run_features(frames, features)?;
        if cancelled() { return Err("native_speaker:cancelled".into()); }
        Ok(result)
    }

    fn run_features(&mut self, frames: usize, features: Vec<f32>) -> Result<Embedding, String> {
        // Synchronous ORT run; bounded audio, reject late output. Do not
        // claim kernel preemption. The existing build owner must retain it.
        let input = Tensor::from_array(([1usize, frames, MEL], features))
            .map_err(|_| "native_speaker:input_tensor_failed")?;
        let output = self.session.run(ort::inputs!["x" => input])
            .map_err(|_| "native_speaker:inference_failed")?;
        let (shape, data) = output.get("embedding").ok_or("native_speaker:output_missing")?
            .try_extract_tensor::<f32>()
            .map_err(|_| "native_speaker:output_invalid")?;
        if shape.len() != 2 || shape[0] != 1 || shape[1] != DIM as i64 {
            return Err("native_speaker:output_invalid".into());
        }
        Embedding::new(data)
    }

}

// Score one normalized vector per distinct accepted take. Reuse cached
// embeddings for candidate ranking and old/new comparison; no per-pair
// inference and no persisted biometrics. Consistency remains advisory.
pub(super) fn median_peer_cosines(embeddings: &[(u32, Embedding)]) -> Result<Vec<(u32, f64)>, String> {
    if !(2..=128).contains(&embeddings.len()) {
        return Err("native_speaker:invalid_reference_count".into());
    }
    let mut seen = std::collections::HashSet::new();
    if embeddings.iter().any(|(id, _)| *id == 0 || !seen.insert(*id)) {
        return Err("native_speaker:duplicate_reference".into());
    }
    let mut scores = Vec::with_capacity(embeddings.len());
    for (i, (id, vector)) in embeddings.iter().enumerate() {
        let mut peers: Vec<_> = embeddings.iter().enumerate().filter(|(j, _)| i != *j)
            .map(|(_, (_, peer))| vector.cosine(peer)).collect();
        peers.sort_by(f64::total_cmp);
        let mid = peers.len()/2;
        let median = if peers.len()%2 == 0 { (peers[mid-1]+peers[mid])/2.0 } else { peers[mid] };
        scores.push((*id, median));
    }
    Ok(scores)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fbank_frames_floor_and_cancellation() {
        let samples: Vec<f32> = (0..RATE).map(|i| (2.0*PI*440.0*i as f32/RATE as f32).sin()*0.1).collect();
        let f = Fbank::new();
        let (frames, values) = f.compute(&samples, || false).unwrap();
        assert_eq!(frames, 98);
        assert_eq!(values.len(), 98*MEL);
        assert!(values.iter().all(|v| v.is_finite() && *v >= f32::EPSILON.ln()));
        assert!(f.compute(&samples, || true).is_err());
        assert!(f.compute(&[0.0; RATE], || false).is_err());
    }
    // Target-only fixture consumes offline exporter raw F32LE oracles, never
    // private recordings or test downloads. Not executed in source work.
    #[test]
    #[ignore = "requires reviewed model pins, verified target ORT and offline synthetic oracles"]
    fn exact_legacy_graph_and_rust_frontend_parity() {
        let root = std::path::PathBuf::from(std::env::var_os("TRANSLATEIT_TEST_SPEAKER_ORACLES")
            .expect("explicit offline oracle directory"));
        let mut verifier = SpeakerVerifier::open().expect("reviewed model and runtime");
        for length in [4800usize, 16000, 48000] {
            let frames = 1+(length-FRAME)/HOP;
            let total = length+frames*MEL+DIM;
            let path = root.join(format!("oracle-{length}.f32le"));
            let file = fs::File::open(path).unwrap();
            let mut raw = Vec::new();
            file.take((total*4+1) as u64).read_to_end(&mut raw).unwrap();
            assert_eq!(raw.len(), total*4);
            let values: Vec<f32> = raw.chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
            let pcm = &values[..length];
            let expected_fbank = &values[length..length+frames*MEL];
            let expected_embedding = Embedding::new(&values[length+frames*MEL..]).unwrap();
            let (actual_frames, actual_fbank) = verifier.features.compute(pcm, || false).unwrap();
            assert_eq!(actual_frames, frames);
            // Proposed conservative parity tolerance; a failure blocks export
            // admission. Do not loosen to make a target run pass.
            for (&actual, &expected) in actual_fbank.iter().zip(expected_fbank) {
                assert!((actual-expected).abs() <= 0.02 + 0.002*expected.abs(),
                    "legacy fbank mismatch: {actual} vs {expected}");
            }
            let graph = verifier.run_features(frames, expected_fbank.to_vec()).unwrap();
            assert!(graph.cosine(&expected_embedding) >= 0.9999);
            let actual = verifier.embed(pcm, RATE as u32, || false).unwrap();
            assert!(actual.cosine(&expected_embedding) >= 0.9999);
        }
    }

    #[test]
    fn speaker_cosine_rejects_missing_or_invalid_evidence() {
        let mut a = vec![0.0; DIM]; a[0] = 1.0;
        let mut b = vec![0.0; DIM]; b[1] = 1.0;
        let aa = Embedding::new(&a).unwrap();
        assert_eq!(aa.cosine(&aa), 1.0);
        assert_eq!(aa.cosine(&Embedding::new(&b).unwrap()), 0.0);
        assert!(Embedding::new(&[0.0; DIM]).is_err());
        assert!(Embedding::new(&[f32::NAN; DIM]).is_err());
        assert!(Embedding::new(&[1.0; 192]).is_err());
        let peers = [(1, Embedding::new(&a).unwrap()), (2, Embedding::new(&a).unwrap()),
            (3, Embedding::new(&b).unwrap())];
        assert_eq!(median_peer_cosines(&peers).unwrap(), vec![(1,0.5),(2,0.5),(3,0.0)]);
    }
}
