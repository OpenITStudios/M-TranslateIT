//! Canonical local speech presence. Model state belongs to one capture lane.
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ort::{session::Session, value::TensorRef};
use sha2::{Digest, Sha256};

use crate::engine::paths::ProjectPaths;

pub const WINDOW: usize = 512;
pub const MODEL_SHA256: &str = "4cbf549b8326f60f80f2536d9eefeb450a9abe83365a098031c89719f1be17d2";
const DLL_SHA256: &str = "aa7737cc5c987286f09f6f12304fa786ea697b11b4ca7ab59a027ff4ce343a9c";
// ort permits exactly one environment per process, including across lane restarts.
static ENVIRONMENT: Mutex<Option<ort::environment::Environment>> = Mutex::new(None);

fn verified(path: &Path, expected: &str, expected_size: u64) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| "silero:asset_missing".to_string())?;
    if !metadata.is_file() || metadata.len() != expected_size {
        return Err("silero:asset_size_mismatch".to_string());
    }
    let bytes = std::fs::read(path).map_err(|_| "silero:asset_missing".to_string())?;
    let actual = Sha256::digest(&bytes).iter().map(|v| format!("{v:02x}")).collect::<String>();
    if actual != expected { return Err("silero:asset_hash_mismatch".to_string()); }
    Ok(bytes)
}

// One native asset layout for both verified repository and packaged roots.
// A missing asset must block admission, not fall back to a Python environment.
fn native_ort_root(paths: &ProjectPaths) -> Result<PathBuf, String> {
    if !cfg!(windows) { return Err("silero:unsupported_platform".to_string()); }
    if !paths.packaged_context_initialized && !paths.development_root_verified {
        return Err("silero:unverified_runtime_root".to_string());
    }
    Ok(PathBuf::from(&paths.runtime_root).join("EngineData/Backend/RuntimeAssets/NativeOrt"))
}

// Shared with the held-out My Voice verifier. No second ORT environment,
// process, automatic provider choice or Python package lookup.
pub(crate) fn verified_ort_environment() -> Result<ort::environment::Environment, String> {
    let root = native_ort_root(&ProjectPaths::discover())?;
    verified_ort_environment_at(&root)
}

fn verified_ort_environment_at(root: &Path) -> Result<ort::environment::Environment, String> {
    let mut slot = ENVIRONMENT.lock().map_err(|_| "silero:environment_poisoned")?;
    if let Some(env) = slot.as_ref() { return Ok(env.clone()); }
    let dll = root.join("onnxruntime.dll");
    verified(&dll, DLL_SHA256, 19_288_928)?;
    let env = ort::init_from(&dll).map_err(|_| "silero:runtime_load_failed".to_string())?
        .with_name("translateit-speech-presence").with_telemetry(false)
        .build().map_err(|_| "silero:environment_failed".to_string())?;
    *slot = Some(env.clone());
    Ok(env)
}

const CONTEXT_SAMPLES: usize = 64;
const STATE_SAMPLES: usize = 128;

pub struct Silero {
    session: Session,
    context: [f32; CONTEXT_SAMPLES],
    h: [f32; STATE_SAMPLES],
    c: [f32; STATE_SAMPLES],
}

// The pinned graph has one probability and two recurrent state vectors.
// Validate all three outputs before changing either state or context.
fn checked_recurrent_output(
    probability: &[f32], h: &[f32], c: &[f32],
) -> Result<(f32, [f32; STATE_SAMPLES], [f32; STATE_SAMPLES]), String> {
    let [p] = probability else { return Err("silero:invalid_output".into()); };
    if !p.is_finite() || !(0.0..=1.0).contains(p) || h.len() != STATE_SAMPLES || c.len() != STATE_SAMPLES {
        return Err("silero:invalid_output".into());
    }
    let h = h.try_into().map_err(|_| "silero:invalid_output".to_string())?;
    let c = c.try_into().map_err(|_| "silero:invalid_output".to_string())?;
    Ok((*p, h, c))
}

impl Silero {
    pub fn load() -> Result<Self, String> {
        let root = native_ort_root(&ProjectPaths::discover())?;
        let model = verified(&root.join("silero_vad_v6.onnx"), MODEL_SHA256, 1_245_151)?;
        let env = verified_ort_environment_at(&root)?;
        let session = Session::builder(&env).and_then(|b| Ok(b.with_intra_threads(1)?))
            .and_then(|b| Ok(b.with_inter_threads(1)?))
            .and_then(|mut b| b.commit_from_memory(&model))
            .map_err(|_| "silero:model_load_failed".to_string())?;
        Ok(Self { session, context: [0.0; CONTEXT_SAMPLES], h: [0.0; STATE_SAMPLES], c: [0.0; STATE_SAMPLES] })
    }

    pub fn reset(&mut self) {
        self.context.fill(0.0);
        self.h.fill(0.0);
        self.c.fill(0.0);
    }

    pub fn probability(&mut self, pcm: &[f32; WINDOW]) -> Result<f32, String> {
        // Fixed-size contiguous input on this speech CPU owner's stack.
        // ORT's synchronous tensor views borrow these slices only for the run.
        // No Vec growth or recurrent-state clones for each 32 ms window.
        let mut input = [0.0_f32; CONTEXT_SAMPLES + WINDOW];
        input[..CONTEXT_SAMPLES].copy_from_slice(&self.context);
        input[CONTEXT_SAMPLES..].copy_from_slice(pcm);
        let (p, h, c) = {
            let input_tensor = TensorRef::from_array_view(([1usize, CONTEXT_SAMPLES + WINDOW], &input[..]))
                .map_err(|_| "silero:inference_failed".to_string())?;
            let h_tensor = TensorRef::from_array_view(([1usize, 1, STATE_SAMPLES], &self.h[..]))
                .map_err(|_| "silero:inference_failed".to_string())?;
            let c_tensor = TensorRef::from_array_view(([1usize, 1, STATE_SAMPLES], &self.c[..]))
                .map_err(|_| "silero:inference_failed".to_string())?;
            let outputs = self.session.run(ort::inputs![
                "input" => input_tensor, "h" => h_tensor, "c" => c_tensor,
            ]).map_err(|_| "silero:inference_failed".to_string())?;
            let (_, p) = outputs[0].try_extract_tensor::<f32>().map_err(|_| "silero:inference_failed".to_string())?;
            let (_, h) = outputs[1].try_extract_tensor::<f32>().map_err(|_| "silero:inference_failed".to_string())?;
            let (_, c) = outputs[2].try_extract_tensor::<f32>().map_err(|_| "silero:inference_failed".to_string())?;
            checked_recurrent_output(p, h, c)?
        };
        self.h = h;
        self.c = c;
        self.context.copy_from_slice(&pcm[WINDOW - CONTEXT_SAMPLES..]);
        Ok(p)
    }
}

/// Rational sample clock: callback splits never reset interpolation phase.
/// Native samples remain intact for the existing writer and provenance owner.
pub struct SpeechWindows {
    rate: u32,
    window_index: u64,
    consumed: u64,
    pending: VecDeque<f32>,
}

impl SpeechWindows {
    pub fn new(rate: u32) -> Result<Self, String> {
        if !(8_000..=192_000).contains(&rate) { return Err("silero:unsupported_rate".to_string()); }
        Ok(Self { rate, window_index: 0, consumed: 0, pending: VecDeque::new() })
    }

    pub fn reset(&mut self) {
        self.window_index = 0;
        self.consumed = 0;
        self.pending.clear();
    }

    pub fn push(&mut self, pcm: &[f32]) {
        self.pending.extend(pcm.iter().map(|s| if s.is_finite() { s.clamp(-1.0, 1.0) } else { 0.0 }));
    }

    /// Maximum native samples consumed by one rational 32 ms window.
    pub fn native_window_capacity(&self) -> usize {
        (WINDOW * self.rate as usize).div_ceil(16_000)
    }

    /// Reuse the CPU owner's native scratch buffer without changing sample
    /// order, interpolation, lookahead, or rational clock advancement.
    pub fn next_into(&mut self, native: &mut Vec<f32>) -> Option<[f32; WINDOW]> {
        native.clear();
        let target_start = self.window_index * WINDOW as u64;
        let end = (target_start + WINDOW as u64) * u64::from(self.rate) / 16_000;
        let count = (end - self.consumed) as usize;
        // One lookahead sample supports interpolation at rates below 16 kHz.
        if self.pending.len() <= count { return None; }
        let mut output = [0.0; WINDOW];
        for (i, value) in output.iter_mut().enumerate() {
            let numerator = (target_start + i as u64) * u64::from(self.rate);
            let left = (numerator / 16_000 - self.consumed) as usize;
            let fraction = (numerator % 16_000) as f32 / 16_000.0;
            *value = self.pending[left] * (1.0 - fraction) + self.pending[left + 1] * fraction;
        }
        native.extend(self.pending.drain(..count));
        self.consumed = end;
        self.window_index += 1;
        Some(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rational_clock_is_invariant_to_callback_partition_and_reset() {
        for rate in [8_000, 16_000, 44_100, 48_000, 96_000, 192_000] {
            let input = (0..rate).map(|i| (i % 211) as f32 / 211.0).collect::<Vec<_>>();
            let collect = |chunk| {
                let mut windows = SpeechWindows::new(rate).unwrap();
                let mut result = Vec::new();
                let mut native = Vec::with_capacity(windows.native_window_capacity());
                let capacity = native.capacity();
                for pcm in input.chunks(chunk) {
                    windows.push(pcm);
                    while let Some(frame) = windows.next_into(&mut native) {
                        assert_eq!(native.capacity(), capacity);
                        result.push((native.clone(), frame));
                    }
                    assert!(native.is_empty());
                }
                windows.reset();
                assert!(windows.next_into(&mut native).is_none());
                assert!(native.is_empty());
                result
            };
            let whole = collect(input.len());
            assert_eq!(whole, collect(137));
            let retained = whole.iter().flat_map(|w| w.0.iter().copied()).collect::<Vec<_>>();
            assert_eq!(retained, input[..retained.len()]);
        }
    }

    #[test]
    fn native_window_reuse_preserves_rational_interpolation_and_lookahead() {
        // Independent reference for each sample position, including below
        // 16 kHz, fractional native windows and callback partition boundaries.
        for rate in [8_000, 16_000, 44_100, 48_000, 96_000, 192_000] {
            let input = (0..rate + 1)
                .map(|i| if i % 37 == 0 { f32::NAN } else { (i % 257) as f32 / 128.0 - 1.0 })
                .collect::<Vec<_>>();
            let sanitized = input.iter().map(|s| {
                if s.is_finite() { s.clamp(-1.0, 1.0) } else { 0.0 }
            }).collect::<Vec<_>>();
            let mut windows = SpeechWindows::new(rate).unwrap();
            let mut native = Vec::with_capacity(windows.native_window_capacity());
            let capacity = native.capacity();
            let mut index = 0u64;
            let mut previous_end = 0usize;
            for chunk in input.chunks(137) {
                windows.push(chunk);
                while let Some(frame) = windows.next_into(&mut native) {
                    let target_start = index * WINDOW as u64;
                    let end = ((target_start + WINDOW as u64) * u64::from(rate) / 16_000) as usize;
                    assert_eq!(native.as_slice(), &sanitized[previous_end..end]);
                    assert_eq!(native.capacity(), capacity);
                    for (i, value) in frame.iter().enumerate() {
                        let numerator = (target_start + i as u64) * u64::from(rate);
                        let left = (numerator / 16_000) as usize;
                        let fraction = (numerator % 16_000) as f32 / 16_000.0;
                        assert_eq!(*value, sanitized[left] * (1.0 - fraction) + sanitized[left + 1] * fraction);
                    }
                    previous_end = end;
                    index += 1;
                }
            }
            assert!(index > 0);
            windows.reset();
            assert!(windows.next_into(&mut native).is_none());
            assert!(native.is_empty());
        }
    }

    #[cfg(windows)]
    #[test]
    fn native_ort_path_uses_verified_runtime_root_only() {
        let mut paths = ProjectPaths::discover();
        paths.runtime_root = "C:/TranslateIT".into();
        paths.packaged_context_initialized = false;
        paths.development_root_verified = false;
        assert!(native_ort_root(&paths).is_err());
        paths.development_root_verified = true;
        let expected = PathBuf::from("C:/TranslateIT").join("EngineData/Backend/RuntimeAssets/NativeOrt");
        assert_eq!(native_ort_root(&paths).unwrap(), expected);
        paths.development_root_verified = false;
        paths.packaged_context_initialized = true;
        assert_eq!(native_ort_root(&paths).unwrap(), expected);
    }

    #[test]
    fn recurrent_output_preserves_exact_state_and_rejects_invalid_lengths() {
        let h = std::array::from_fn::<_, STATE_SAMPLES, _>(|index| index as f32 / 128.0);
        let c = std::array::from_fn::<_, STATE_SAMPLES, _>(|index| -(index as f32) / 128.0);
        assert_eq!(checked_recurrent_output(&[0.0], &h, &c).unwrap(), (0.0, h, c));
        assert_eq!(checked_recurrent_output(&[1.0], &h, &c).unwrap(), (1.0, h, c));
        for p in [&[][..], &[0.5, 0.5][..], &[f32::NAN][..], &[-0.1][..], &[1.1][..]] {
            assert!(checked_recurrent_output(p, &h, &c).is_err());
        }
        assert!(checked_recurrent_output(&[0.5], &h[..127], &c).is_err());
        assert!(checked_recurrent_output(&[0.5], &h, &c[..127]).is_err());
    }

    #[test]
    fn model_integrity_failure_is_explicit() {
        assert!(verified(Path::new("definitely-not-a-vad-model"), MODEL_SHA256, 1_245_151).is_err());
    }
}
