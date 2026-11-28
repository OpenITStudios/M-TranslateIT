pub(crate) mod device_selection;
pub mod evidence;
pub mod finalized_utterance;
pub mod guided_take;
pub mod input;
pub mod live_audio_buffer;
pub mod live_capture;
pub mod live_segment_writer;
pub mod meeting_output;
#[cfg(windows)]
pub(crate) mod native_capture_transport;
#[cfg(windows)]
pub(crate) mod native_capture_pipe;
#[cfg(windows)]
pub(crate) mod native_capture_pacing;
pub mod original_voice_pcm;
pub mod meeting_sound_capture;
// Bounded callback-to-CPU transport; capture and finalizer retain their owners.
mod pcm_handoff;
mod resample;
pub(crate) mod silero;
#[cfg(test)]
mod pcm_handoff_replay;
#[cfg(test)]
mod audio_fixture_replay_tests;
pub mod vad;

#[cfg(test)]
mod audio_contract_tests;
#[cfg(test)]
mod meeting_output_public_contract_tests;

use serde::{Deserialize, Serialize};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) const AUDIO_THREAD_STOP_WAIT_BUDGET: Duration = Duration::from_millis(500);
const AUDIO_THREAD_STOP_POLL: Duration = Duration::from_millis(10);

// Callers retain their own resource owner and handle until exit is confirmed.
// No join, detach, authority change or cleanup belongs in this shared wait.
pub(crate) fn audio_thread_finished_within(thread: &JoinHandle<()>, budget: Duration) -> bool {
    let started = Instant::now();
    while !thread.is_finished() {
        let remaining = budget.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return false;
        }
        thread::sleep(remaining.min(AUDIO_THREAD_STOP_POLL));
    }
    true
}

pub const TARGET_SAMPLE_RATE_HZ: u32 = 16_000;
pub const TARGET_CHANNELS: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFrame {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

// Single shared sample-count -> milliseconds conversion for the audio engine.
// Truncation to u32 ms is intentional and identical to the former per-file copies.
pub(crate) fn duration_ms(sample_count: usize, sample_rate_hz: u32) -> u32 {
    if sample_rate_hz == 0 {
        return 0;
    }
    ((sample_count as u64 * 1_000) / sample_rate_hz as u64) as u32
}
