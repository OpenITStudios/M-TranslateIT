//! Bounded PCM pacing primitive for the existing Meeting output worker.
//!
//! No timer thread, global queue, audio capture, source selector or broker
//! lifecycle is created here. The authoritative Meeting worker supplies its
//! existing generation/epoch predicate before every 20ms block.
use std::io::{self, ErrorKind};
use std::thread;
use std::time::{Duration, Instant};

const OUTPUT_RATE: u64 = 48_000;
const FRAMES_PER_PACKET: usize = 960;
const MAX_SOURCE_SECONDS: u64 = 120;
const MAX_LATENESS: Duration = Duration::from_millis(75);
const CANCEL_POLL: Duration = Duration::from_millis(5);

fn deadline_after_frames(start: Instant, frames: usize) -> Option<Instant> {
    let nanos = (frames as u128).checked_mul(1_000_000_000)? / u128::from(OUTPUT_RATE);
    start.checked_add(Duration::from_nanos(u64::try_from(nanos).ok()?))
}

fn interrupted() -> io::Error {
    io::Error::new(ErrorKind::Interrupted, "native_capture:generation_or_epoch_revoked")
}
fn late() -> io::Error {
    io::Error::new(ErrorKind::TimedOut, "native_capture:pcm_output_too_late")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacingSummary {
    pub frames_submitted: usize,
    pub chunks_submitted: usize,
}

/// This function is legal only on the existing Meeting playback/producer
/// worker thread. Never call from a WASAPI/CPAL callback. A failed block must
/// cause the Meeting owner to revoke/FLUSH its native session, not retry or
/// silently deliver Original microphone speech.
pub fn send_paced(
    generation: u64,
    source_epoch: u64,
    pcm16_mono_48k: &[i16],
    mut authorized: impl FnMut(u64, u64) -> bool,
    mut send: impl FnMut(&[i16]) -> io::Result<()>,
) -> io::Result<PacingSummary> {
    if generation == 0 || source_epoch == 0 {
        return Err(io::Error::new(ErrorKind::InvalidInput, "native_capture:missing_source_authority"));
    }
    if pcm16_mono_48k.is_empty()
        || pcm16_mono_48k.len() > (OUTPUT_RATE * MAX_SOURCE_SECONDS) as usize
    {
        return Err(io::Error::new(ErrorKind::InvalidInput, "native_capture:duration_invalid"));
    }
    let start = Instant::now();
    let mut summary = PacingSummary { frames_submitted: 0, chunks_submitted: 0 };
    for chunk in pcm16_mono_48k.chunks(FRAMES_PER_PACKET) {
        let deadline = deadline_after_frames(start, summary.frames_submitted)
            .ok_or_else(late)?;
        loop {
            if !authorized(generation, source_epoch) { return Err(interrupted()); }
            let now = Instant::now();
            if now >= deadline {
                if now.duration_since(deadline) > MAX_LATENESS {
                    return Err(late());
                }
                break;
            }
            // Short blocking slices leave the existing native Meeting generation
            // revocation observable. No separate timer/background output loop.
            thread::sleep((deadline - now).min(CANCEL_POLL));
        }
        if !authorized(generation, source_epoch) { return Err(interrupted()); }
        send(chunk)?;
        // A synchronous broker request may have accepted the last packet
        // while Stop/Mute/source-switch revoked the Meeting authority.
        // Never report a completed submission from a stale generation/epoch.
        // The caller must FLUSH/close the native owner on this failure; the
        // check cannot recall frames already consumed by another application.
        if !authorized(generation, source_epoch) { return Err(interrupted()); }
        summary.frames_submitted += chunk.len();
        summary.chunks_submitted += 1;
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunk_deadlines_are_monotonic_and_exact_for_48k() {
        let now = Instant::now();
        assert_eq!(deadline_after_frames(now, 960).unwrap() - now, Duration::from_millis(20));
        assert_eq!(deadline_after_frames(now, 4800).unwrap() - now, Duration::from_millis(100));
    }
    #[test]
    fn normal_short_audio_is_chunked_and_ordered() {
        let samples = vec![1i16; 1_921];
        let mut observed = Vec::new();
        let result = send_paced(7, 2, &samples, |generation, epoch| generation == 7 && epoch == 2, |block| {
            observed.push(block.len());
            Ok(())
        }).unwrap();
        assert_eq!(observed, vec![960, 960, 1]);
        assert_eq!(result.frames_submitted, 1_921);
        assert_eq!(result.chunks_submitted, 3);
    }
    #[test]
    fn revoked_during_last_packet_is_not_reported_as_success() {
        use std::cell::Cell;
        let live = Cell::new(true);
        let mut sent = 0usize;
        let result = send_paced(
            7, 2, &[2i16; FRAMES_PER_PACKET],
            |generation, epoch| generation == 7 && epoch == 2 && live.get(),
            |chunk| {
                sent += chunk.len();
                live.set(false); // Stop/Source Switch during a synchronous WRITE.
                Ok(())
            },
        );
        assert_eq!(sent, FRAMES_PER_PACKET);
        assert_eq!(result.unwrap_err().kind(), ErrorKind::Interrupted);
    }

    #[test]
    fn revoked_session_sends_no_audio() {
        let mut sent = 0;
        let err = send_paced(7, 2, &[3i16; 16], |_, _| false, |_| { sent += 1; Ok(()) }).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Interrupted);
        assert_eq!(sent, 0);
    }
}
