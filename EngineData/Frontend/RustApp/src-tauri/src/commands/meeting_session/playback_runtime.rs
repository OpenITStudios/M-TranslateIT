use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Condvar, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::engine::audio::meeting_output::{
    deliver_meeting_output_native_pcm_for_source,
    MAX_DELIVERY_DEADLINE_MS,
};
use crate::engine::runtime_state::{runtime_generation_is_authoritative, runtime_generation_matches_epoch};
use crate::engine::audio::{audio_thread_finished_within, AUDIO_THREAD_STOP_WAIT_BUDGET};
use crate::engine::audio::original_voice_pcm;
use crate::engine::audio::meeting_sound_capture::meeting_sound_capture_status;

use super::super::virtual_mic_route::get_bound_virtual_mic_output_device;
use super::committed_turns::{
    begin_committed_playback, update_committed_turn_delivery_state,
    update_committed_turn_outbound_timing,
};
use super::session_state::{
    elapsed_millis, record_first_playback_timing, set_outbound_playback_activity,
    set_outbound_timing, update_incoming_status, update_outbound_status, OutboundTimingContext,
};
use super::suppression::{
    begin_self_output_suppression, disable_optional_incoming_for_outbound,
};
use super::{generation_is_live, incoming_session_is_eligible};

const PLAYBACK_QUEUE_CAPACITY: usize = 1;

pub(super) struct PreparedPlaybackJob {
    pub(super) generation: u64,
    pub(super) source_epoch: u64,
    pub(super) session_id: String,
    pub(super) event_sequence: u64,
    pub(super) utterance_id: u64,
    pub(super) audio: Vec<f32>,
    pub(super) timing: OutboundTimingContext,
    pub(super) prepared_at: Instant,
}

struct MeetingPlaybackRuntime {
    generation: u64,
    session_id: String,
    sender: Option<SyncSender<PreparedPlaybackJob>>,
    thread: Option<JoinHandle<()>>,
    // A panicked JoinHandle is consumed on join(); retain this ownership
    // failure for all subsequent Retry Stop/Close decisions.
    join_failed: bool,
}

pub(super) struct MeetingPlaybackCleanupResult {
    pub(super) ok: bool,
    pub(super) message: String,
}

static MEETING_PLAYBACK_RUNTIME: OnceLock<Mutex<Option<MeetingPlaybackRuntime>>> = OnceLock::new();

// Notification only: the existing sync_channel remains the single queue.
type PlaybackWake = (Mutex<()>, Condvar);
static PLAYBACK_WAKE: OnceLock<PlaybackWake> = OnceLock::new();

fn playback_wake() -> &'static PlaybackWake {
    PLAYBACK_WAKE.get_or_init(|| (Mutex::new(()), Condvar::new()))
}

fn notify_playback_waiters(wake: &PlaybackWake) {
    let _guard = wake.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    wake.1.notify_all();
}

// Called after the atomic generation/source epoch changes. Never lock the
// runtime store here: its Start/Stop owner can hold it while joining playback.
pub(crate) fn wake_playback_waiters() {
    if let Some(wake) = PLAYBACK_WAKE.get() {
        notify_playback_waiters(wake);
    }
}

struct WakeOnPlaybackExit;
impl Drop for WakeOnPlaybackExit {
    fn drop(&mut self) { wake_playback_waiters(); }
}

fn playback_runtime_store() -> &'static Mutex<Option<MeetingPlaybackRuntime>> {
    MEETING_PLAYBACK_RUNTIME.get_or_init(|| Mutex::new(None))
}

pub(super) fn cleanup_job(job: &mut PreparedPlaybackJob) {
    drop(std::mem::take(&mut job.audio));
}

fn mark_interrupted(job: &PreparedPlaybackJob) {
    let _ = update_committed_turn_delivery_state(
        &job.session_id,
        job.generation,
        job.utterance_id,
        "interrupted",
    );
}

fn sequence_is_monotonic(last_sequence: u64, candidate: u64) -> bool {
    candidate > last_sequence
}

fn process_playback_job(mut job: PreparedPlaybackJob, last_sequence: &mut u64) {
    if !sequence_is_monotonic(*last_sequence, job.event_sequence) {
        cleanup_job(&mut job);
        let _ = update_committed_turn_delivery_state(
            &job.session_id,
            job.generation,
            job.utterance_id,
            "output_failed",
        );
        update_outbound_status(
            job.generation,
            &job.session_id,
            "attention_needed",
            job.event_sequence,
            false,
            false,
            "meeting_playback:sequence_not_monotonic",
            "Prepared Meeting playback was rejected because output ordering was no longer monotonic.",
        );
        return;
    }
    if !generation_is_live(job.generation)
        || !original_voice_pcm::translated_epoch_matches(job.generation, job.source_epoch)
        || !super::local_translated_meeting_route_admitted()
    {
        cleanup_job(&mut job);
        mark_interrupted(&job);
        return;
    }

    let mut timing = OutboundTimingContext {
        finalized_at: job.timing.finalized_at,
        metrics: job.timing.metrics.clone(),
    };
    timing.metrics.playback_queue_ms = Some(elapsed_millis(job.prepared_at, Instant::now()));
    set_outbound_timing(
        job.generation,
        &job.session_id,
        job.event_sequence,
        &timing.metrics,
    );
    let _ = update_committed_turn_outbound_timing(
        &job.session_id,
        job.generation,
        job.utterance_id,
        &timing.metrics,
    );

    let bound_output_device = match get_bound_virtual_mic_output_device(job.generation) {
        Ok(device) => device,
        Err(blocker) => {
            cleanup_job(&mut job);
            let _ = update_committed_turn_delivery_state(
                &job.session_id,
                job.generation,
                job.utterance_id,
                "output_failed",
            );
            update_outbound_status(
                job.generation,
                &job.session_id,
                "attention_needed",
                job.event_sequence,
                false,
                false,
                &blocker,
                "Prepared voice output could not resolve the authoritative Meeting microphone route.",
            );
            return;
        }
    };

    if !generation_is_live(job.generation)
        || !original_voice_pcm::translated_epoch_matches(job.generation, job.source_epoch)
        || !super::local_translated_meeting_route_admitted()
    {
        cleanup_job(&mut job);
        mark_interrupted(&job);
        return;
    }

    // Monotonic queue order is not sufficient authorization to speak.
    if !begin_committed_playback(
        &job.session_id,
        job.generation,
        job.event_sequence,
        job.utterance_id,
    ) {
        cleanup_job(&mut job);
        update_outbound_status(
            job.generation,
            &job.session_id,
            "attention_needed",
            job.event_sequence,
            false,
            false,
            "meeting_playback:source_not_queued",
            "Prepared voice was rejected because its committed source is no longer queued for delivery.",
        );
        return;
    }
    *last_sequence = job.event_sequence;

    let suppression_guard = match begin_self_output_suppression(&job.session_id) {
        Some(guard) => Some(guard),
        None => {
            let incoming_cleanup = disable_optional_incoming_for_outbound(&job.session_id);
            update_outbound_status(
                job.generation,
                &job.session_id,
                "queued",
                job.event_sequence,
                false,
                true,
                "",
                &format!(
                    "Optional incoming protection became unavailable and incoming was disabled before required outbound delivery. {incoming_cleanup}"
                ),
            );
            None
        }
    };

    set_outbound_playback_activity(
        job.generation,
        &job.session_id,
        job.event_sequence,
        true,
    );

    if !original_voice_pcm::translated_epoch_matches(job.generation, job.source_epoch)
        || !super::local_translated_meeting_route_admitted()
    {
        cleanup_job(&mut job);
        mark_interrupted(&job);
        set_outbound_playback_activity(job.generation, &job.session_id, job.event_sequence, false);
        drop(suppression_guard);
        return;
    }
    let delivery_started_at = Instant::now();
    let route = deliver_meeting_output_native_pcm_for_source(
        std::mem::take(&mut job.audio), Some(bound_output_device.as_str()),
        job.generation, job.source_epoch,
    );
    record_first_playback_timing(
        &mut timing,
        delivery_started_at,
        route.first_playback_at,
        route.first_playback_unix_ms,
    );
    timing.metrics.output_stream_build_ms = route.stream_build_ms;
    timing.metrics.output_stream_start_to_first_audio_ms =
        route.stream_start_to_first_audio_ms;
    set_outbound_timing(
        job.generation,
        &job.session_id,
        job.event_sequence,
        &timing.metrics,
    );
    let _ = update_committed_turn_outbound_timing(
        &job.session_id,
        job.generation,
        job.utterance_id,
        &timing.metrics,
    );

    set_outbound_playback_activity(
        job.generation,
        &job.session_id,
        job.event_sequence,
        false,
    );
    drop(suppression_guard);

    if incoming_session_is_eligible(&job.session_id)
        && meeting_sound_capture_status().stream_active
    {
        update_incoming_status(
            &job.session_id,
            "listening",
            false,
            "",
            "Incoming Meeting Sound resumed from a fresh speech boundary after TranslateIT TTS playback ended.",
        );
    }

    cleanup_job(&mut job);

    if !generation_is_live(job.generation)
        || !original_voice_pcm::translated_epoch_matches(job.generation, job.source_epoch)
        || !super::local_translated_meeting_route_admitted()
    {
        mark_interrupted(&job);        return;
    }

    if !route.ok || !route.execution_attempted {
        let blocker = if route.blocker.is_empty() {
            "meeting_outbound:meeting_route_delivery_failed".to_string()
        } else {
            route.blocker
        };
        let _ = update_committed_turn_delivery_state(
            &job.session_id,
            job.generation,
            job.utterance_id,
            "output_failed",
        );
        update_outbound_status(
            job.generation,
            &job.session_id,
            "attention_needed",
            job.event_sequence,
            false,
            false,
            &blocker,
            "Translated voice could not be safely delivered to the Meeting microphone route. Playback was not retried.",
        );
        return;
    }

    let _ = update_committed_turn_delivery_state(
        &job.session_id,
        job.generation,
        job.utterance_id,
        "output_complete",
    );
}

pub(super) fn ensure_meeting_playback_runtime(generation: u64, session_id: &str) -> Result<(), String> {
    start_or_reuse_meeting_playback_runtime(generation, session_id, true)
}

pub(super) fn start_meeting_playback_runtime(
    generation: u64,
    session_id: &str,
) -> Result<(), String> {
    start_or_reuse_meeting_playback_runtime(generation, session_id, false)
}

fn start_or_reuse_meeting_playback_runtime(
    generation: u64, session_id: &str, reuse_existing: bool,
) -> Result<(), String> {
    if !super::local_translated_meeting_route_admitted() {
        return Err("meeting_playback:codex_voice_not_admitted".to_string());
    }
    let store = playback_runtime_store();
    let mut guard = store
        .lock()
        .map_err(|_| "meeting_playback:runtime_state_lock_failed".to_string())?;
    if guard.is_some() && !reuse_existing {
        return Err("meeting_playback:runtime_already_active".to_string());
    }
    if !runtime_generation_is_authoritative(generation) {
        return Err("meeting_playback:generation_not_authoritative".to_string());
    }
    if let Some(active) = guard.as_ref() {
        if active.generation != generation || active.session_id != session_id {
            return Err("meeting_playback:another_runtime_active".to_string());
        }
        if active.join_failed || active.sender.is_none()
            || active.thread.as_ref().is_none_or(|thread| thread.is_finished())
        {
            return Err("meeting_playback:runtime_not_running".to_string());
        }
        return Ok(());
    }

    let (sender, receiver) =
        mpsc::sync_channel::<PreparedPlaybackJob>(PLAYBACK_QUEUE_CAPACITY);
    let thread_session_id = session_id.to_string();
    let runtime_session_id = thread_session_id.clone();
    let handle = thread::Builder::new()
        .name("translateit-meeting-playback".to_string())
        .spawn(move || {
            // Local drop order closes the receiver before the exit wake,
            // including unwinding: a blocked sender observes disconnection.
            let _wake_on_exit = WakeOnPlaybackExit;
            let receiver = receiver;
            let mut last_sequence = 0u64;
            while let Ok(mut job) = receiver.recv() {
                wake_playback_waiters();
                if job.generation != generation || job.session_id != thread_session_id {
                    cleanup_job(&mut job);
                    mark_interrupted(&job);
                    continue;
                }
                process_playback_job(job, &mut last_sequence);
            }
        })
        .map_err(|error| format!("meeting_playback:runtime_spawn_failed:{error}"))?;

    *guard = Some(MeetingPlaybackRuntime {
        generation,
        session_id: runtime_session_id,
        sender: Some(sender),
        thread: Some(handle),
        join_failed: false,
    });
    // Stop takes this same owner lock. Keep any raced spawned thread visible
    // so its canonical Stop cleanup can release and join it.
    if !runtime_generation_is_authoritative(generation) {
        return Err("meeting_playback:generation_not_authoritative".to_string());
    }
    Ok(())
}

// The active playback may take up to the native delivery deadline. Never let
// a stalled receiver pin the AI consumer indefinitely or evict existing audio.
fn enqueue_with_deadline<T>(
    sender: &SyncSender<T>,
    wake: &PlaybackWake,
    mut value: T,
    wait_budget: Duration,
    mut still_authoritative: impl FnMut() -> bool,
) -> Result<(), (T, &'static str)> {
    let started = Instant::now();
    // Atomic-only predicate. Notifiers acquire this same mutex to avoid
    // losing a slot/cancel wake between try_send and Condvar wait.
    let mut guard = match wake.0.lock() {
        Ok(guard) => guard,
        Err(_) => return Err((value, "meeting_playback:runtime_state_lock_failed")),
    };
    let mut waited = false;
    loop {
        if !still_authoritative() {
            return Err((value, "meeting_playback:generation_not_authoritative"));
        }
        // Preserve immediate zero-budget admission to an empty slot, but
        // never grant a waiting request after its deadline has elapsed.
        if waited && started.elapsed() >= wait_budget {
            return Err((value, "meeting_playback:enqueue_wait_deadline_exceeded"));
        }
        match sender.try_send(value) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Full(returned)) => {
                value = returned;
                let remaining = wait_budget.saturating_sub(started.elapsed());
                if remaining.is_zero() {
                    return Err((value, "meeting_playback:enqueue_wait_deadline_exceeded"));
                }
                waited = true;
                guard = match wake.1.wait_timeout(guard, remaining) {
                    Ok((guard, _)) => guard,
                    Err(_) => return Err((value, "meeting_playback:runtime_state_lock_failed")),
                };
            }
            Err(TrySendError::Disconnected(returned)) => {
                return Err((returned, "meeting_playback:runtime_disconnected"));
            }
        }
    }
}

pub(super) fn enqueue_meeting_playback(mut job: PreparedPlaybackJob) -> Result<(), String> {
    // Only the canonical source/turn owner may admit Local prepared audio.
    // Unverified Codex assistant text never enters this queue.
    if !super::local_translated_meeting_route_admitted() {
        cleanup_job(&mut job);
        return Err("meeting_playback:codex_voice_not_admitted".to_string());
    }
    let generation = job.generation;
    let source_epoch = job.source_epoch;
    if !generation_is_live(generation)
        || !original_voice_pcm::translated_epoch_matches(generation, source_epoch) {
        cleanup_job(&mut job);
        return Err("meeting_playback:generation_not_authoritative".to_string());
    }
    let sender = playback_runtime_store()
        .lock()
        .map_err(|_| "meeting_playback:runtime_state_lock_failed".to_string())
        .and_then(|guard| {
            guard
                .as_ref()
                .filter(|runtime| {
                    runtime.generation == job.generation && runtime.session_id == job.session_id
                })
                .and_then(|runtime| runtime.sender.as_ref().cloned())
                .ok_or_else(|| "meeting_playback:runtime_not_active".to_string())
        });
    let sender = match sender {
        Ok(sender) => sender,
        Err(blocker) => {
            cleanup_job(&mut job);
            return Err(blocker);
        }
    };

    match enqueue_with_deadline(
        &sender,
        playback_wake(),
        job,
        Duration::from_millis(MAX_DELIVERY_DEADLINE_MS),
        || runtime_generation_matches_epoch(generation)
            && original_voice_pcm::translated_epoch_matches(generation, source_epoch),
    ) {
        Ok(()) => Ok(()),
        Err((mut job, blocker)) => {
            cleanup_job(&mut job);
            Err(blocker.to_string())
        }
    }
}

pub(super) fn stop_meeting_playback_runtime(
    generation: u64,
) -> MeetingPlaybackCleanupResult {
    let mut guard = match playback_runtime_store().try_lock() {
        Ok(guard) => guard,
        Err(_) => return MeetingPlaybackCleanupResult {
            ok: false,
            message: "Meeting playback runtime owner is busy or unavailable during Stop.".to_string(),
        },
    };
    let Some(active) = guard.as_mut().filter(|runtime| runtime.generation == generation) else {
        return MeetingPlaybackCleanupResult {
            ok: true,
            message: "No matching Meeting playback runtime required cleanup.".to_string(),
        };
    };

    // Close producer admission first. A still-running thread and its exact
    // owner remain stored until bounded Stop can confirm its completion.
    active.sender.take();
    wake_playback_waiters();
    if active.join_failed {
        return MeetingPlaybackCleanupResult {
            ok: false,
            message: format!(
                "Meeting playback runtime for {} generation {generation} previously failed to join. Owner remains retained.",
                active.session_id,
            ),
        };
    }
    if active.thread.as_ref().is_some_and(|thread| {
        !audio_thread_finished_within(thread, AUDIO_THREAD_STOP_WAIT_BUDGET)
    }) {
        return MeetingPlaybackCleanupResult {
            ok: false,
            message: format!(
                "Meeting playback runtime for {} generation {generation} has not exited. Retry Stop; another playback owner remains blocked.",
                active.session_id,
            ),
        };
    }
    if !active.thread.take().is_some_and(|handle| handle.join().is_ok()) {
        active.join_failed = true;
        return MeetingPlaybackCleanupResult {
            ok: false,
            message: format!(
                "Meeting playback runtime for {} generation {generation} failed to join; its original owner remains retained.",
                active.session_id,
            ),
        };
    }
    let runtime = guard.take().expect("matching verified playback owner");
    MeetingPlaybackCleanupResult {
        ok: true,
        message: format!(
            "Meeting playback runtime stopped for {} generation {}.",
            runtime.session_id, generation,
        ),
    }
}

// Read-only orphan indicator. A contended/poisoned playback store must not
// become a false idle signal for Meeting Stop or Application Close.
pub(super) fn inactive_meeting_playback_may_need_stop() -> bool {
    playback_runtime_store().try_lock()
        .map(|owner| owner.is_some()).unwrap_or(true)
}

// Only the canonical no-snapshot Stop may complete the old owned generation.
// Never reach into a current Meeting, Mic Test or voice recording. The
// Meeting lifecycle gate prevents a new Meeting Start during this transfer.
pub(super) fn finish_orphaned_meeting_playback_without_session() -> Result<(), String> {
    if !super::lifecycle::verified_idle_for_orphan_cleanup() {
        return Err("meeting_playback:runtime_session_not_verified_idle".to_string());
    }
    let old_generation = playback_runtime_store().try_lock()
        .map_err(|_| "meeting_playback:orphan_owner_busy".to_string())?
        .as_ref().map(|owner| owner.generation);
    if !super::lifecycle::verified_idle_for_orphan_cleanup() {
        return Err("meeting_playback:runtime_owner_changed".to_string());
    }
    if let Some(generation) = old_generation {
        let stopped = stop_meeting_playback_runtime(generation);
        if !stopped.ok {
            return Err(format!("meeting_playback:orphan_cleanup_incomplete:{}", stopped.message));
        }
    }
    if !super::lifecycle::verified_idle_for_orphan_cleanup() {
        return Err("meeting_playback:runtime_owner_changed_after_cleanup".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{enqueue_with_deadline, notify_playback_waiters, audio_thread_finished_within, sequence_is_monotonic, PlaybackWake, PLAYBACK_QUEUE_CAPACITY};
    use std::sync::{Condvar, Mutex};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn unconfirmed_playback_exit_does_not_force_blocking_join() {
        let (release, blocked) = mpsc::channel::<()>();
        let thread = thread::spawn(move || { let _ = blocked.recv(); });
        assert!(!audio_thread_finished_within(&thread, Duration::ZERO));
        release.send(()).expect("release playback fixture");
        assert!(audio_thread_finished_within(&thread, Duration::from_secs(2)));
        thread.join().expect("playback fixture joined");
    }

    #[test]
    fn admission_rejects_stall_cancellation_and_disconnect_without_eviction() {
        let wake: PlaybackWake = (Mutex::new(()), Condvar::new());
        let (sender, receiver) = mpsc::sync_channel(1);
        sender.try_send(1_u8).expect("fill pending slot");
        assert_eq!(enqueue_with_deadline(&sender, &wake, 2_u8, Duration::ZERO, || true),
            Err((2, "meeting_playback:enqueue_wait_deadline_exceeded")));
        assert_eq!(receiver.try_recv(), Ok(1));

        sender.try_send(3_u8).expect("refill pending slot");
        let authority = AtomicBool::new(true);
        thread::scope(|scope| {
            let (checked_tx, checked_rx) = mpsc::channel();
            let (result_tx, result_rx) = mpsc::channel();
            let (sender, wake, authority) = (&sender, &wake, &authority);
            scope.spawn(move || {
                let result = enqueue_with_deadline(sender, wake, 4_u8, Duration::from_secs(3), || {
                    let _ = checked_tx.send(());
                    authority.load(Ordering::Acquire)
                });
                let _ = result_tx.send(result);
            });
            checked_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            authority.store(false, Ordering::Release);
            notify_playback_waiters(wake);
            assert_eq!(result_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
                Err((4, "meeting_playback:generation_not_authoritative")));
        });
        assert_eq!(receiver.try_recv(), Ok(3));
        drop(receiver);
        assert_eq!(enqueue_with_deadline(&sender, &wake, 5_u8, Duration::ZERO, || true),
            Err((5, "meeting_playback:runtime_disconnected")));

        let (sender, receiver) = mpsc::sync_channel(1);
        enqueue_with_deadline(&sender, &wake, 6_u8, Duration::ZERO, || true).unwrap();
        assert_eq!(receiver.try_recv(), Ok(6));
    }

    #[test]
    fn playback_slot_release_wakes_waiting_producer_without_eviction() {
        let wake: PlaybackWake = (Mutex::new(()), Condvar::new());
        let (sender, receiver) = mpsc::sync_channel(1);
        sender.try_send(1_u8).unwrap();
        thread::scope(|scope| {
            let (checked_tx, checked_rx) = mpsc::channel();
            let (result_tx, result_rx) = mpsc::channel();
            let (sender, wake) = (&sender, &wake);
            scope.spawn(move || {
                let result = enqueue_with_deadline(sender, wake, 2_u8, Duration::from_secs(3), || {
                    let _ = checked_tx.send(());
                    true
                });
                let _ = result_tx.send(result);
            });
            checked_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            assert_eq!(receiver.try_recv(), Ok(1));
            notify_playback_waiters(wake);
            assert_eq!(result_rx.recv_timeout(Duration::from_secs(1)).unwrap(), Ok(()));
        });
        assert_eq!(receiver.try_recv(), Ok(2));
    }

    #[test]
    fn orphan_playback_owner_contention_blocks_clean_close() {
        use super::*;
        let _serial = crate::engine::runtime_state::runtime_test_guard();
        let held = playback_runtime_store().lock().expect("hold playback owner");
        assert!(inactive_meeting_playback_may_need_stop());
        drop(held);
    }

    #[test]
    fn panicked_playback_join_remains_owned_across_retry_stop() {
        use super::*;
        let _serial = crate::engine::runtime_state::runtime_test_guard();
        let generation = 987655_u64;
        let store = playback_runtime_store();
        let (tx, _rx) = mpsc::sync_channel::<PreparedPlaybackJob>(1);
        {
            let mut guard = store.lock().expect("playback fixture lock");
            assert!(guard.is_none(), "fixture starts with no playback owner");
            *guard = Some(MeetingPlaybackRuntime {
                generation,
                session_id: "orphan-playback-fixture".to_string(),
                sender: Some(tx),
                thread: Some(thread::spawn(|| panic!("simulated playback panic"))),
                join_failed: false,
            });
        }
        assert!(!stop_meeting_playback_runtime(generation).ok);
        {
            let guard = store.lock().expect("failed playback owner retained");
            let owner = guard.as_ref().expect("owner retained after failed join");
            assert!(owner.join_failed);
            assert!(owner.thread.is_none());
        }
        assert!(!stop_meeting_playback_runtime(generation).ok,
            "Retry Stop cannot turn an earlier join failure into success");
        let mut guard = store.lock().expect("fixture release");
        let _ = guard.take();
    }

    #[test]
    fn no_snapshot_stop_reaps_only_the_old_playback_generation() {
        use super::*;
        let _serial = crate::engine::runtime_state::runtime_test_guard();
        let _ = crate::engine::runtime_state::clear_runtime_session_state();
        let generation = 987652_u64;
        let (sender, receiver) = mpsc::sync_channel::<PreparedPlaybackJob>(1);
        let store = playback_runtime_store();
        {
            let mut guard = store.lock().expect("playback owner");
            assert!(guard.is_none());
            *guard = Some(MeetingPlaybackRuntime {
                generation,
                session_id: "old-meeting-playback".to_string(),
                sender: Some(sender),
                thread: Some(thread::spawn(move || drop(receiver))),
                join_failed: false,
            });
        }
        assert!(inactive_meeting_playback_may_need_stop());
        assert!(finish_orphaned_meeting_playback_without_session().is_ok());
        assert!(store.lock().expect("orphan owner released").is_none());
        assert!(!inactive_meeting_playback_may_need_stop());
    }

    #[test]
    fn bounded_playback_keeps_exactly_one_pending_slot() {
        assert_eq!(PLAYBACK_QUEUE_CAPACITY, 1);
    }

    #[test]
    fn playback_sequence_accepts_gaps_but_rejects_duplicate_or_older_output() {
        assert!(sequence_is_monotonic(0, 1));
        assert!(sequence_is_monotonic(10, 11));
        assert!(sequence_is_monotonic(10, 12));
        assert!(sequence_is_monotonic(u64::MAX - 1, u64::MAX));
        assert!(!sequence_is_monotonic(10, 10));
        assert!(!sequence_is_monotonic(10, 9));
    }
}
