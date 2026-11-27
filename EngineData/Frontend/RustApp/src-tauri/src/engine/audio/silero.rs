//! Canonical local speech presence. Model state belongs to one capture lane.
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ort::{session::Session, value::Tensor};
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

fn packages() -> Result<PathBuf, String> {
    if !cfg!(windows) { return Err("silero:unsupported_platform".to_string()); }
    let paths = ProjectPaths::discover();
    if paths.packaged_context_initialized {
        // The private embeddable runtime vendors packages at its root (_pth '.').
        Ok(PathBuf::from(paths.python_runtime_dir))
    } else if paths.is_repository_development() {
        Ok(PathBuf::from(paths.worker_runtime_dir).join(".venv/Lib/site-packages"))
    } else { Err("silero:unverified_runtime_root".to_string()) }
}

// Shared with the held-out My Voice verifier. No second ORT environment,
// process, or automatic provider choice. Current verified DLL path is
// transitional; move it with VAD during the native payload cutover.
pub(crate) fn verified_ort_environment() -> Result<ort::environment::Environment, String> {
    let mut slot = ENVIRONMENT.lock().map_err(|_| "silero:environment_poisoned")?;
    if let Some(env) = slot.as_ref() { return Ok(env.clone()); }
    let dll = packages()?.join("onnxruntime/capi/onnxruntime.dll");
    verified(&dll, DLL_SHA256, 19_288_928)?;
    let env = ort::init_from(&dll).map_err(|_| "silero:runtime_load_failed".to_string())?
        .with_name("translateit-speech-presence").with_telemetry(false)
        .build().map_err(|_| "silero:environment_failed".to_string())?;
    *slot = Some(env.clone());
    Ok(env)
}

pub struct Silero {
    session: Session,
    context: [f32; 64],
    h: Vec<f32>,
    c: Vec<f32>,
}

impl Silero {
    pub fn load() -> Result<Self, String> {
        let packages = packages()?;
        let model = verified(&packages.join("faster_whisper/assets/silero_vad_v6.onnx"), MODEL_SHA256, 1_245_151)?;
        let env = verified_ort_environment()?;
        let session = Session::builder(&env).and_then(|b| Ok(b.with_intra_threads(1)?))
            .and_then(|b| Ok(b.with_inter_threads(1)?))
            .and_then(|mut b| b.commit_from_memory(&model))
            .map_err(|_| "silero:model_load_failed".to_string())?;
        Ok(Self { session, context: [0.0; 64], h: vec![0.0; 128], c: vec![0.0; 128] })
    }

    pub fn reset(&mut self) {
        self.context.fill(0.0);
        self.h.fill(0.0);
        self.c.fill(0.0);
    }

    pub fn probability(&mut self, pcm: &[f32; WINDOW]) -> Result<f32, String> {
        let mut run = || -> ort::Result<(f32, Vec<f32>, Vec<f32>)> {
            let mut input = self.context.to_vec();
            input.extend_from_slice(pcm);
            let outputs = self.session.run(ort::inputs![
                "input" => Tensor::from_array(([1usize, 576], input))?,
                "h" => Tensor::from_array(([1usize, 1, 128], self.h.clone()))?,
                "c" => Tensor::from_array(([1usize, 1, 128], self.c.clone()))?,
            ])?;
            let (_, p) = outputs[0].try_extract_tensor::<f32>()?;
            let (_, h) = outputs[1].try_extract_tensor::<f32>()?;
            let (_, c) = outputs[2].try_extract_tensor::<f32>()?;
            Ok((p[0], h.to_vec(), c.to_vec()))
        };
        let (p, h, c) = run().map_err(|_| "silero:inference_failed".to_string())?;
        if !p.is_finite() || !(0.0..=1.0).contains(&p) || h.len() != 128 || c.len() != 128 {
            return Err("silero:invalid_output".to_string());
        }
        self.h = h;
        self.c = c;
        self.context.copy_from_slice(&pcm[WINDOW - 64..]);
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

    #[test]
    fn model_integrity_failure_is_explicit() {
        assert!(verified(Path::new("definitely-not-a-vad-model"), MODEL_SHA256, 1_245_151).is_err());
    }
}
