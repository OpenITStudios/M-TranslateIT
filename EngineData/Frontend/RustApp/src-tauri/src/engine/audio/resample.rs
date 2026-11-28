//! Whole-utterance fixed-rate mono conversion. No device or inference owner.
use rubato::audioadapter_buffers::direct::SequentialSlice;
use rubato::{Fft, FixedSync, Resampler, WindowFunction};

use super::evidence::safe_sample;

const RESAMPLE_CHUNK_FRAMES: usize = 1024;

// The owner fixes the rates for its lifetime. Rubato process_all resets
// before each clip, flushes the tail and trims its startup delay.
pub(super) struct MonoFft {
    resampler: Fft<f32>,
    source_rate: u32,
    target_rate: u32,
}

impl MonoFft {
    pub(super) fn new(source_rate: u32, target_rate: u32) -> Result<Self, String> {
        if source_rate == 0 || target_rate == 0 {
            return Err("take_has_no_audio".to_string());
        }
        let resampler = Fft::<f32>::new_custom(
            source_rate as usize,
            target_rate as usize,
            RESAMPLE_CHUNK_FRAMES,
            1,
            1,
            WindowFunction::BlackmanHarris2,
            FixedSync::Both,
        ).map_err(|error| format!("resampler_create_failed:{error}"))?;
        Ok(Self { resampler, source_rate, target_rate })
    }

    pub(super) fn process(&mut self, samples: &[f32]) -> Result<Vec<f32>, String> {
        if samples.is_empty() {
            return Err("take_has_no_audio".to_string());
        }
        let expected_frames = (samples.len() as u128 * u128::from(self.target_rate)
            / u128::from(self.source_rate)) as usize;
        let input = SequentialSlice::new(samples, 1, samples.len())
            .map_err(|error| format!("resample_failed:{error}"))?;
        let mut output = self.resampler.process_all(&input, samples.len(), None)
            .map_err(|error| format!("resample_failed:{error}"))?
            .take_data();
        if expected_frames == 0 || output.len() < expected_frames {
            return Err("resample_output_incomplete".to_string());
        }
        output.truncate(expected_frames);
        output.iter_mut().for_each(|sample| *sample = safe_sample(*sample));
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    #[test]
    fn repeated_clips_keep_duration_and_do_not_share_filter_history() {
        for rate in [8_000_u32, 44_100, 48_000, 96_000, 192_000] {
            let mut reused = MonoFft::new(rate, 16_000).unwrap();
            let long = vec![0.5; rate as usize / 2];
            let short: Vec<f32> = (0..rate as usize / 3 + 1)
                .map(|i| 0.2 * (TAU * 440.0 * i as f32 / rate as f32).sin())
                .collect();
            let _ = reused.process(&long).unwrap();
            let output = reused.process(&short).unwrap();
            let fresh = MonoFft::new(rate, 16_000).unwrap().process(&short).unwrap();
            assert_eq!(output.len(), short.len() * 16_000 / rate as usize);
            assert_eq!(output, fresh);
            let silence = reused.process(&vec![0.0; rate as usize / 4]).unwrap();
            assert!(silence.iter().all(|s| s.abs() <= 1e-7));
        }
    }

    #[test]
    fn downsampling_preserves_speech_band_and_rejects_alias_band() {
        let mut fft = MonoFft::new(48_000, 16_000).unwrap();
        let tone = |hz: f32| (0..48_000).map(|i|
            0.25 * (TAU * hz * i as f32 / 48_000.0).sin()).collect::<Vec<_>>();
        let pass = fft.process(&tone(1_000.0)).unwrap();
        let stop = fft.process(&tone(12_000.0)).unwrap();
        let rms = |samples: &[f32]| (samples.iter().map(|s| s*s).sum::<f32>()
            / samples.len() as f32).sqrt();
        // Ignore filter transients at clip edges. These are signal-level
        // assertions, not an ASR accuracy benchmark.
        assert!(rms(&pass[2_000..14_000]) > 0.15);
        assert!(rms(&stop[2_000..14_000]) < 0.01);
    }
}
