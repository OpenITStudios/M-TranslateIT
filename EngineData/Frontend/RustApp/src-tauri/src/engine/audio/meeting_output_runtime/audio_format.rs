use std::fs;
use std::io::Read;
use std::path::Path;

use super::{
    MAX_DELIVERY_DEADLINE_MS, MAX_MEETING_OUTPUT_WAV_BYTES, MAX_OUTPUT_CHANNELS,
    MAX_OUTPUT_SAMPLE_RATE_HZ, MIN_DELIVERY_DEADLINE_MS, MIN_OUTPUT_SAMPLE_RATE_HZ,
};
use super::super::duration_ms;

// Keep resampled f32 delivery under 128 MiB even when device rate/channels
// expand a valid, bounded input WAV. This is a memory ceiling, not truncation.
const MAX_PREPARED_OUTPUT_SAMPLES: u128 = 32 * 1024 * 1024;

pub(super) struct DecodedWav {
    pub(super) sample_rate_hz: u32,
    pub(super) channels: u16,
    pub(super) samples: Vec<f32>,
}

pub(super) fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let chunk = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| "meeting_output:wav_truncated".to_string())?;
    Ok(u16::from_le_bytes([chunk[0], chunk[1]]))
}

pub(super) fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let chunk = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| "meeting_output:wav_truncated".to_string())?;
    Ok(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
}

pub(super) fn decode_wav_bytes(bytes: &[u8]) -> Result<DecodedWav, String> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("meeting_output:wav_invalid_riff".to_string());
    }

    // The RIFF length defines which chunks belong to this audio container.
    // Bytes appended outside that boundary must not override fmt/data.
    let riff_end = (read_u32(bytes, 4)? as usize)
        .checked_add(8)
        .ok_or_else(|| "meeting_output:wav_chunk_overflow".to_string())?;
    if riff_end < 12 {
        return Err("meeting_output:wav_riff_size_invalid".to_string());
    }
    if riff_end > bytes.len() {
        return Err("meeting_output:wav_truncated".to_string());
    }
    if riff_end < bytes.len() {
        return Err("meeting_output:wav_riff_size_mismatch".to_string());
    }

    let mut offset = 12usize;
    let mut format: Option<(u16, u16, u32, u16, u16)> = None;
    let mut data: Option<&[u8]> = None;

    while offset < riff_end {
        if riff_end - offset < 8 {
            return Err("meeting_output:wav_truncated".to_string());
        }
        let id = &bytes[offset..offset + 4];
        let size = read_u32(bytes, offset + 4)? as usize;
        let start = offset + 8;
        let end = start
            .checked_add(size)
            .ok_or_else(|| "meeting_output:wav_chunk_overflow".to_string())?;
        let next_offset = end
            .checked_add(size & 1)
            .ok_or_else(|| "meeting_output:wav_chunk_overflow".to_string())?;
        if next_offset > riff_end {
            return Err("meeting_output:wav_truncated".to_string());
        }

        if id == b"fmt " {
            if size < 16 {
                return Err("meeting_output:wav_fmt_invalid".to_string());
            }
            let mut audio_format = read_u16(bytes, start)?;
            let channels = read_u16(bytes, start + 2)?;
            let sample_rate = read_u32(bytes, start + 4)?;
            let block_align = read_u16(bytes, start + 12)?;
            let bits_per_sample = read_u16(bytes, start + 14)?;
            if audio_format == 0xfffe {
                // WAVEFORMATEXTENSIBLE requires a complete 22-byte extension
                // and its canonical PCM/IEEE-float GUID, not just a type WORD.
                if size < 40 {
                    return Err("meeting_output:wav_fmt_invalid".to_string());
                }
                let extension_size = usize::from(read_u16(bytes, start + 16)?);
                if extension_size < 22 || extension_size > size - 18 {
                    return Err("meeting_output:wav_fmt_invalid".to_string());
                }
                const KS_SUBFORMAT_SUFFIX: [u8; 14] = [
                    0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71,
                ];
                if &bytes[start + 26..start + 40] != KS_SUBFORMAT_SUFFIX.as_slice() {
                    return Err("meeting_output:wav_subformat_unsupported".to_string());
                }
                audio_format = read_u16(bytes, start + 24)?;
            }
            format = Some((
                audio_format,
                channels,
                sample_rate,
                block_align,
                bits_per_sample,
            ));
        } else if id == b"data" {
            data = Some(&bytes[start..end]);
        }

        offset = next_offset;
    }

    let (audio_format, channels, sample_rate_hz, block_align, bits_per_sample) =
        format.ok_or_else(|| "meeting_output:wav_fmt_missing".to_string())?;
    let data = data.ok_or_else(|| "meeting_output:wav_data_missing".to_string())?;

    if channels == 0 || channels > MAX_OUTPUT_CHANNELS {
        return Err("meeting_output:wav_channels_unsupported".to_string());
    }
    if !(MIN_OUTPUT_SAMPLE_RATE_HZ..=MAX_OUTPUT_SAMPLE_RATE_HZ).contains(&sample_rate_hz) {
        return Err("meeting_output:wav_sample_rate_unsupported".to_string());
    }
    if block_align == 0 || data.len() % usize::from(block_align) != 0 {
        return Err("meeting_output:wav_frame_alignment_invalid".to_string());
    }

    let bytes_per_sample = usize::from(bits_per_sample.div_ceil(8));
    if bytes_per_sample == 0 || usize::from(block_align) != bytes_per_sample * usize::from(channels)
    {
        return Err("meeting_output:wav_sample_layout_invalid".to_string());
    }

    let mut samples = Vec::with_capacity(data.len() / bytes_per_sample);
    match (audio_format, bits_per_sample) {
        (1, 8) => {
            samples.extend(data.iter().map(|value| (f32::from(*value) - 128.0) / 128.0));
        }
        (1, 16) => {
            for chunk in data.chunks_exact(2) {
                samples.push(f32::from(i16::from_le_bytes([chunk[0], chunk[1]])) / 32768.0);
            }
        }
        (1, 24) => {
            for chunk in data.chunks_exact(3) {
                let raw =
                    i32::from(chunk[0]) | (i32::from(chunk[1]) << 8) | (i32::from(chunk[2]) << 16);
                let signed = if raw & 0x0080_0000 != 0 {
                    raw | !0x00ff_ffff
                } else {
                    raw
                };
                samples.push(signed as f32 / 8_388_608.0);
            }
        }
        (1, 32) => {
            for chunk in data.chunks_exact(4) {
                let value = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                samples.push(value as f32 / 2_147_483_648.0);
            }
        }
        (3, 32) => {
            for chunk in data.chunks_exact(4) {
                let value = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                samples.push(if value.is_finite() {
                    value.clamp(-1.0, 1.0)
                } else {
                    0.0
                });
            }
        }
        _ => return Err("meeting_output:wav_encoding_unsupported".to_string()),
    }

    if samples.is_empty() {
        return Err("meeting_output:wav_empty".to_string());
    }
    Ok(DecodedWav {
        sample_rate_hz,
        channels,
        samples,
    })
}

// The caller opens the path once. Size checks and decoded bytes come from
// the same handle; a later pathname swap cannot redirect the WAV. Modification
// timestamps are a low-cost best-effort same-size rewrite signal, not a
// cryptographic integrity check or an atomic snapshot.
fn wav_changed_during_read(
    before: &fs::Metadata,
    after: &fs::Metadata,
    bytes_read: u64,
) -> bool {
    if before.len() != after.len() || bytes_read != after.len() {
        return true;
    }
    match (before.modified(), after.modified()) {
        (Ok(first), Ok(last)) => first != last,
        // If the filesystem does not expose comparable write times, do not
        // claim an unchanged complete WAV from this read.
        _ => true,
    }
}

pub(super) fn read_bounded_open_wav(mut file: fs::File) -> Result<Vec<u8>, String> {
    let initial = file.metadata().map_err(|_| "meeting_output:wav_read_failed".to_string())?;
    if !initial.is_file() {
        return Err("meeting_output:wav_missing".to_string());
    }
    if initial.len() > MAX_MEETING_OUTPUT_WAV_BYTES {
        return Err("meeting_output:wav_too_large".to_string());
    }
    let mut bytes = Vec::new();
    {
        let mut bounded = (&mut file).take(MAX_MEETING_OUTPUT_WAV_BYTES + 1);
        bounded.read_to_end(&mut bytes)
            .map_err(|_| "meeting_output:wav_read_failed".to_string())?;
    }
    if bytes.len() as u64 > MAX_MEETING_OUTPUT_WAV_BYTES {
        return Err("meeting_output:wav_too_large".to_string());
    }
    let final_metadata = file.metadata()
        .map_err(|_| "meeting_output:wav_read_failed".to_string())?;
    if wav_changed_during_read(&initial, &final_metadata, bytes.len() as u64) {
        return Err("meeting_output:wav_changed_during_read".to_string());
    }
    Ok(bytes)
}

pub(super) fn decode_wav_file(path: &Path) -> Result<DecodedWav, String> {
    let file = fs::File::open(path).map_err(|_| "meeting_output:wav_missing".to_string())?;
    let bytes = read_bounded_open_wav(file)?;
    decode_wav_bytes(&bytes)
}

pub(super) fn prepare_output_samples(
    source: &DecodedWav,
    target_rate_hz: u32,
    target_channels: u16,
) -> Result<Vec<f32>, String> {
    let source_channels = usize::from(source.channels);
    if source_channels == 0
        || source.sample_rate_hz == 0
        || target_channels == 0
        || target_rate_hz == 0
    {
        return Ok(Vec::new());
    }
    let source_frames = source.samples.len() / source_channels;
    if source_frames == 0 {
        return Ok(Vec::new());
    }

    // Calculate the full expanded output size before allocating the device
    // buffer. Even a small 8 kHz WAV can expand at 192 kHz / 8 channels.
    let target_frames = (source_frames as u128 * u128::from(target_rate_hz))
        .div_ceil(u128::from(source.sample_rate_hz));
    let output_samples = target_frames * u128::from(target_channels);
    if output_samples > MAX_PREPARED_OUTPUT_SAMPLES {
        return Err("meeting_output:prepared_audio_too_large".to_string());
    }
    let target_frames = target_frames as usize;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_samples as usize)
        .map_err(|_| "meeting_output:prepared_audio_allocation_failed".to_string())?;

    // Preserve the existing f32 downmix and linear interpolation, but avoid
    // allocating a full mono copy of every source frame. Adjacent output
    // frames usually reuse the same two source averages, especially when
    // upsampling, so retain only that pair in a tiny rolling cache.
    let frame_mono = |frame: usize| {
        let start = frame * source_channels;
        let sum = source.samples[start..start + source_channels]
            .iter().copied().sum::<f32>();
        sum / source_channels as f32
    };
    let mut cached_lower = None;
    let mut lower_mono = 0.0_f32;
    let mut upper_mono = 0.0_f32;
    for target_index in 0..target_frames {
        let source_position =
            target_index as f64 * source.sample_rate_hz as f64 / target_rate_hz as f64;
        let lower = source_position.floor() as usize;
        let upper = (lower + 1).min(source_frames - 1);
        if cached_lower != Some(lower) {
            // When progressing by one source frame, the old upper value is
            // exactly the next lower value. A downsample jump recalculates it.
            lower_mono = if cached_lower.is_some_and(|previous| previous + 1 == lower) {
                upper_mono
            } else {
                frame_mono(lower)
            };
            upper_mono = if upper == lower { lower_mono } else { frame_mono(upper) };
            cached_lower = Some(lower);
        }
        let fraction = (source_position - lower as f64) as f32;
        let value = lower_mono + (upper_mono - lower_mono) * fraction;
        for _ in 0..target_channels {
            output.push(value.clamp(-1.0, 1.0));
        }
    }
    Ok(output)
}

pub(super) fn delivery_deadline_ms(sample_count: usize, channels: u16, sample_rate_hz: u32) -> u64 {
    let frames = sample_count / usize::from(channels.max(1));
    u64::from(duration_ms(frames, sample_rate_hz))
        .saturating_mul(2)
        .saturating_add(2_000)
        .clamp(MIN_DELIVERY_DEADLINE_MS, MAX_DELIVERY_DEADLINE_MS)
}

#[cfg(test)]
mod read_consistency_tests {
    use super::wav_changed_during_read;
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn same_length_modified_wav_is_not_accepted_as_unchanged() {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)
            .expect("system time after epoch").as_nanos();
        let path = std::env::temp_dir().join(format!(
            "translateit-wav-mtime-{}-{nonce}.wav", std::process::id(),
        ));
        fs::write(&path, b"same sized WAV fixture").expect("create temporary fixture");
        // set_modified needs write-attribute permission on Windows.
        let file = fs::OpenOptions::new().read(true).write(true)
            .open(&path).expect("open fixture with write attributes");
        let earlier = UNIX_EPOCH + Duration::from_secs(1_600_000_000);
        let later = earlier + Duration::from_secs(120);
        file.set_modified(earlier).expect("set deterministic original mtime");
        let before = file.metadata().expect("read first metadata");
        assert!(!wav_changed_during_read(&before, &before, before.len()));
        file.set_modified(later).expect("simulate in-place same-size update");
        let after = file.metadata().expect("read changed metadata");
        assert_eq!(before.len(), after.len());
        assert!(wav_changed_during_read(&before, &after, before.len()),
            "same-length write-time changes must fail closed");
        assert!(wav_changed_during_read(&before, &before, before.len() - 1),
            "partial read must fail despite equal metadata");
        drop(file);
        fs::remove_file(path).expect("cleanup temporary fixture");
    }
}

#[cfg(test)]
mod resampling_buffer_tests {
    use super::{prepare_output_samples, DecodedWav};

    // Reference implementation is deliberately test-only: it is the old
    // full-mono-buffer algorithm. Production keeps only two source averages.
    fn previous_resampling_output(
        source: &DecodedWav, target_rate_hz: u32, target_channels: u16,
    ) -> Vec<f32> {
        let source_channels = usize::from(source.channels);
        let mono: Vec<f32> = source.samples.chunks_exact(source_channels)
            .map(|frame| frame.iter().copied().sum::<f32>() / source_channels as f32)
            .collect();
        let target_frames = (mono.len() as u128 * u128::from(target_rate_hz))
            .div_ceil(u128::from(source.sample_rate_hz)) as usize;
        let mut output = Vec::new();
        for target_index in 0..target_frames {
            let position =
                target_index as f64 * source.sample_rate_hz as f64 / target_rate_hz as f64;
            let lower = position.floor() as usize;
            let upper = (lower + 1).min(mono.len() - 1);
            let fraction = (position - lower as f64) as f32;
            let value = mono[lower] + (mono[upper] - mono[lower]) * fraction;
            for _ in 0..target_channels {
                output.push(value.clamp(-1.0, 1.0));
            }
        }
        output
    }

    #[test]
    fn rolling_source_frame_averages_preserve_mono_and_multichannel_samples() {
        let cases = [
            (16_000_u32, 48_000_u32, 2_u16, 2_u16,
             vec![0.8, -0.4, 0.0, 0.6, -0.8, 0.2, 0.1, -0.1]),
            (48_000, 8_000, 2, 1,
             vec![0.1, 0.4, 0.3, -0.1, -0.3, 0.5, 0.6, 0.4,
                  -0.1, -0.5, 0.9, 0.1, -0.9, -0.3, 0.7, 0.5]),
            (22_050, 44_100, 1, 2, vec![-1.0, -0.5, 0.5, 1.0]),
            (44_100, 16_000, 4, 1,
             vec![0.1, 0.2, 0.3, 0.4, -0.5, 0.0, 0.5, 1.0,
                  -0.2, -0.1, 0.6, 0.7, 0.4, -0.4, -0.4, 0.4]),
            (16_000, 48_000, 1, 1, vec![0.5]),
        ];
        for (source_rate, target_rate, source_channels, target_channels, samples) in cases {
            let source = DecodedWav {
                sample_rate_hz: source_rate,
                channels: source_channels,
                samples,
            };
            let expected = previous_resampling_output(&source, target_rate, target_channels);
            let actual = prepare_output_samples(&source, target_rate, target_channels)
                .expect("bounded resampling fixture");
            assert_eq!(actual, expected, "rate {source_rate}->{target_rate}, channels {source_channels}->{target_channels}");
        }
    }

    #[test]
    fn resampling_empty_or_invalid_direction_stays_empty() {
        let source = DecodedWav {
            sample_rate_hz: 16_000,
            channels: 1,
            samples: Vec::new(),
        };
        assert!(prepare_output_samples(&source, 48_000, 2).unwrap().is_empty());
        let one_frame = DecodedWav { samples: vec![0.5], ..source };
        assert!(prepare_output_samples(&one_frame, 0, 2).unwrap().is_empty());
        assert!(prepare_output_samples(&one_frame, 48_000, 0).unwrap().is_empty());
    }
}
