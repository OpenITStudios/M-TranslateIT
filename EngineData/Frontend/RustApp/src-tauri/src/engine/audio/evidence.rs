use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioEvidenceReport {
    pub reason: String,
    pub rms: f32,
    pub peak: f32,
    pub mean_abs: f32,
    pub peak_to_rms_ratio: f32,
    pub speech_to_noise_gap: f32,
    pub voiced_frame_ratio: f32,
    pub zero_crossing_rate: f32,
    pub frame_energy_concentration: f32,
    pub frame_active_ratio: f32,
    pub active_frame_ratio: f32,
    pub impulse_edge_ratio: f32,
    pub clipping_ratio: f32,
}

impl AudioEvidenceReport {
    pub fn from_samples(samples: &[f32]) -> Self {
        Self::from_samples_with_noise_floor(samples, 0.0, "Normal")
    }

    pub fn from_samples_with_noise_floor(
        samples: &[f32],
        noise_floor_rms: f32,
        sensitivity: &str,
    ) -> Self {
        if samples.is_empty() {
            return Self::empty_with_reason("rejected_silence");
        }

        // This function is called from realtime capture/VAD paths. Keep evidence
        // calculation allocation-free: sanitize samples as they are read instead of
        // first materializing a second normalized Vec, and retain only the top three
        // frame energies rather than sorting an allocated frame-energy vector.
        let noise_floor_rms = safe_metric(noise_floor_rms).clamp(0.0, 1.0);
        let len = samples.len() as f32;
        let mut sum_square = 0.0_f32;
        let mut sum_abs = 0.0_f32;
        let mut peak = 0.0_f32;
        for sample in samples {
            let value = safe_sample(*sample);
            sum_square += value * value;
            sum_abs += value.abs();
            peak = peak.max(value.abs());
        }

        let rms = safe_metric((sum_square / len).sqrt()).clamp(0.0, 1.0);
        let peak = safe_metric(peak).clamp(0.0, 1.0);
        let peak_to_rms_ratio = safe_metric(peak / rms.max(0.0001));
        let speech_to_noise_gap = safe_metric(rms - noise_floor_rms);
        let threshold = 0.0025_f32.max(noise_floor_rms * 1.45);
        let active_threshold = 0.006_f32
            .max(noise_floor_rms * 1.45)
            .min(0.012_f32.max(rms * 1.6));
        let edge_threshold = 0.006_f32.max(noise_floor_rms * 1.8).max(rms * 1.7);

        let mut voiced_count = 0usize;
        let mut clipping_count = 0usize;
        let mut active_count = 0usize;
        let mut zero_crossings = 0usize;
        let mut impulse_edges = 0usize;
        let mut previous: Option<f32> = None;
        for sample in samples {
            let value = safe_sample(*sample);
            if value.abs() >= threshold {
                voiced_count += 1;
            }
            if value.abs() >= 0.98 {
                clipping_count += 1;
            }
            if value.abs() >= active_threshold {
                active_count += 1;
            }
            if let Some(previous) = previous {
                if (previous >= 0.0 && value < 0.0) || (previous < 0.0 && value >= 0.0) {
                    zero_crossings += 1;
                }
                if (value - previous).abs() >= edge_threshold {
                    impulse_edges += 1;
                }
            }
            previous = Some(value);
        }

        let voiced_frame_ratio = safe_ratio(voiced_count, samples.len());
        let zero_crossing_rate = safe_ratio(zero_crossings, samples.len().saturating_sub(1));
        let impulse_edge_ratio = safe_ratio(impulse_edges, samples.len().saturating_sub(1));
        let (frame_energy_concentration, frame_active_ratio) =
            frame_metrics(samples, noise_floor_rms, rms);
        let clipping_ratio = safe_ratio(clipping_count, samples.len());
        let active_frame_ratio = safe_ratio(active_count, samples.len());

        let sensitivity = normalize_sensitivity(sensitivity);
        let (min_peak, min_rms, min_voiced) = sensitivity_thresholds(sensitivity);
        let strong_voiced_speech = sensitivity == "High"
            && voiced_frame_ratio >= 0.16
            && speech_to_noise_gap >= -0.0003
            && peak >= (min_peak * 0.80).max(0.0042);
        let reason = if peak < 0.002 && rms < 0.001 {
            "rejected_silence"
        } else if rms < min_rms && peak < min_peak && !strong_voiced_speech {
            "rejected_low_energy"
        } else if speech_to_noise_gap < 0.0012 && peak < min_peak {
            "rejected_low_snr"
        } else if voiced_frame_ratio < min_voiced
            && peak < (min_peak * 1.15).max(0.01)
            && !strong_voiced_speech
        {
            "rejected_unconfirmed_speech"
        } else if samples.len() >= 640
            && noise_like_impulse(
                peak_to_rms_ratio,
                voiced_frame_ratio,
                frame_energy_concentration,
                frame_active_ratio,
                impulse_edge_ratio,
                zero_crossing_rate,
            )
            && !strong_voiced_speech
        {
            "rejected_noise_like_impulse"
        } else {
            ""
        };

        Self {
            reason: reason.to_string(),
            rms,
            peak,
            mean_abs: safe_metric(sum_abs / len).clamp(0.0, 1.0),
            peak_to_rms_ratio,
            speech_to_noise_gap,
            voiced_frame_ratio,
            zero_crossing_rate,
            frame_energy_concentration,
            frame_active_ratio,
            active_frame_ratio,
            impulse_edge_ratio,
            clipping_ratio,
        }
    }

    pub fn empty() -> Self {
        Self::empty_with_reason("")
    }

    fn empty_with_reason(reason: &str) -> Self {
        Self {
            reason: reason.to_string(),
            rms: 0.0,
            peak: 0.0,
            mean_abs: 0.0,
            peak_to_rms_ratio: 0.0,
            speech_to_noise_gap: 0.0,
            voiced_frame_ratio: 0.0,
            zero_crossing_rate: 0.0,
            frame_energy_concentration: 0.0,
            frame_active_ratio: 0.0,
            active_frame_ratio: 0.0,
            impulse_edge_ratio: 0.0,
            clipping_ratio: 0.0,
        }
    }
}

// ASR noise suppression uses only RMS and clipping, not the full speech
// evidence report. Keep the same f32 accumulation, sanitization and clipping
// threshold, but calculate both metrics in a single pass.
pub(super) fn noise_suppression_threshold(samples: &[f32]) -> Option<f32> {
    if samples.len() < 320 {
        return None;
    }
    let mut sum_square = 0.0_f32;
    let mut clipping_count = 0usize;
    for &sample in samples {
        let value = safe_sample(sample);
        sum_square += value * value;
        if value.abs() >= 0.98 {
            clipping_count += 1;
        }
    }
    let rms = safe_metric((sum_square / samples.len() as f32).sqrt()).clamp(0.0, 1.0);
    let clipping_ratio = safe_ratio(clipping_count, samples.len());
    if rms < 0.012 || clipping_ratio > 0.025 {
        return None;
    }
    Some((rms * 0.08).clamp(0.0015, 0.006))
}

fn attenuate_low_level_sample(sample: f32, threshold: f32) -> f32 {
    let transition_end = threshold * 2.5;
    let value = safe_sample(sample);
    let level = value.abs();
    let gain = if level <= threshold {
        0.22
    } else if level < transition_end {
        0.22 + 0.78 * ((level - threshold) / (transition_end - threshold))
    } else {
        1.0
    };
    value * gain
}

pub(super) fn condition_asr_sample(sample: f32, threshold: Option<f32>) -> f32 {
    match threshold {
        Some(threshold) => attenuate_low_level_sample(sample, threshold),
        None => safe_sample(sample),
    }
}

/// Native cutover seam: condition only an already-admitted owned 16 kHz
/// mono buffer, once, after deferral is no longer possible. No allocation,
/// resampling, PCM16 quantization or file I/O. The legacy retry path still
/// borrows unmodified PCM and applies the same gain while writing WAV.
pub(crate) fn condition_asr_pcm(samples: &mut [f32], noise_suppression_enabled: bool) {
    let threshold = if noise_suppression_enabled {
        noise_suppression_threshold(samples)
    } else {
        None
    };
    for sample in samples {
        *sample = condition_asr_sample(*sample, threshold);
    }
}

fn safe_sample(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

fn safe_metric(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn safe_ratio(count: usize, total: usize) -> f32 {
    if total == 0 {
        0.0
    } else {
        safe_metric(count as f32 / total as f32).clamp(0.0, 1.0)
    }
}

fn normalize_sensitivity(value: &str) -> &str {
    match value {
        "Low" | "High" => value,
        "Headset" => "High",
        _ => "Normal",
    }
}

fn sensitivity_thresholds(value: &str) -> (f32, f32, f32) {
    match value {
        "Low" => (0.0085, 0.0019, 0.040),
        "High" => (0.0040, 0.0008, 0.009),
        _ => (0.0055, 0.0010, 0.016),
    }
}

fn frame_metrics(samples: &[f32], noise_floor_rms: f32, rms: f32) -> (f32, f32) {
    let frame_size = 320;
    let frame_count = samples.len() / frame_size;
    if frame_count == 0 {
        return (0.0, 0.0);
    }

    let active_threshold = ((noise_floor_rms * 1.25).powi(2))
        .max((rms * 0.45).powi(2))
        .max(1e-7);
    let mut total_energy = 0.0_f32;
    let mut active_frames = 0usize;
    let mut top_three = [0.0_f32; 3];

    for frame in samples[..frame_count * frame_size].chunks_exact(frame_size) {
        let energy = safe_metric(
            frame
                .iter()
                .map(|value| {
                    let safe = safe_sample(*value);
                    safe * safe
                })
                .sum::<f32>()
                / frame_size as f32,
        );
        total_energy = safe_metric(total_energy + energy);
        if energy >= active_threshold {
            active_frames += 1;
        }
        retain_top_three(&mut top_three, energy);
    }

    let frame_energy_concentration = if total_energy > 0.0 {
        safe_metric(top_three.iter().sum::<f32>() / total_energy).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let frame_active_ratio = safe_ratio(active_frames, frame_count);
    (frame_energy_concentration, frame_active_ratio)
}

fn retain_top_three(top_three: &mut [f32; 3], value: f32) {
    if value >= top_three[0] {
        top_three[2] = top_three[1];
        top_three[1] = top_three[0];
        top_three[0] = value;
    } else if value >= top_three[1] {
        top_three[2] = top_three[1];
        top_three[1] = value;
    } else if value > top_three[2] {
        top_three[2] = value;
    }
}

fn noise_like_impulse(
    peak_to_rms_ratio: f32,
    voiced_ratio: f32,
    concentration: f32,
    active_ratio: f32,
    impulse_edge_ratio: f32,
    zcr: f32,
) -> bool {
    (peak_to_rms_ratio >= 7.0
        && voiced_ratio <= 0.10
        && concentration >= 0.70
        && active_ratio <= 0.35)
        || (concentration >= 0.85
            && active_ratio <= 0.28
            && peak_to_rms_ratio >= 5.5
            && voiced_ratio <= 0.10)
        || (impulse_edge_ratio >= 0.05 && voiced_ratio <= 0.10 && peak_to_rms_ratio >= 5.5)
        || (zcr <= 0.12 && voiced_ratio <= 0.10 && peak_to_rms_ratio >= 5.0 && active_ratio <= 0.35)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(left: f32, right: f32) {
        assert!((left - right).abs() <= 1e-6, "left={left}, right={right}");
    }

    #[test]
    fn evidence_sanitizes_nonfinite_and_out_of_range_samples_without_materializing_a_copy() {
        let raw = [f32::NAN, 2.0, -2.0, 0.25, -0.10, f32::INFINITY, -0.25, 0.10];
        let normalized = raw.iter().copied().map(safe_sample).collect::<Vec<_>>();
        let direct = AudioEvidenceReport::from_samples(&raw);
        let reference = AudioEvidenceReport::from_samples(&normalized);

        assert_eq!(direct.reason, reference.reason);
        assert_close(direct.rms, reference.rms);
        assert_close(direct.peak, reference.peak);
        assert_close(direct.mean_abs, reference.mean_abs);
        assert_close(direct.voiced_frame_ratio, reference.voiced_frame_ratio);
        assert_close(direct.zero_crossing_rate, reference.zero_crossing_rate);
        assert_close(direct.impulse_edge_ratio, reference.impulse_edge_ratio);
        assert_close(direct.clipping_ratio, reference.clipping_ratio);
    }

    #[test]
    fn top_three_frame_energy_is_bounded_and_deterministic() {
        let mut samples = vec![0.0_f32; 320 * 5];
        for (index, amplitude) in [0.1_f32, 0.4, 0.2, 0.8, 0.6].into_iter().enumerate() {
            for sample in &mut samples[index * 320..(index + 1) * 320] {
                *sample = amplitude;
            }
        }
        let report = AudioEvidenceReport::from_samples(&samples);
        assert!((0.0..=1.0).contains(&report.frame_energy_concentration));
        assert!((0.0..=1.0).contains(&report.frame_active_ratio));
        assert!(report.frame_energy_concentration > 0.5);
    }
}
