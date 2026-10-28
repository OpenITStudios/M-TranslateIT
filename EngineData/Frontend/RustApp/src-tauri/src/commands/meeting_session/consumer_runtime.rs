use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::engine::audio::finalized_utterance::{
    clear_finalized_incoming_utterance_producer_for_session,
    clear_finalized_outbound_utterance_producer_for_generation,
    try_take_finalized_incoming_utterance, wait_take_finalized_incoming_utterance,
    wait_take_finalized_incoming_utterance_for, wait_take_finalized_outbound_utterance,
    IncomingTimedWait,
};
use crate::engine::audio::live_segment_writer::{
    remove_finalized_meeting_utterance_wav,
    write_finalized_incoming_utterance_wav_with_noise_suppression,
    write_finalized_outbound_utterance_wav_with_noise_suppression,
};
use crate::engine::runtime_state::runtime_generation_is_authoritative;
use crate::engine::audio::original_voice_pcm;
use crate::engine;

use super::incoming_deferred::clear_deferred_incoming_queue_for_session;
use super::incoming_pipeline::{
    drain_due_deferred_incoming, process_authoritative_finalized_incoming_wav,
    IncomingAudioProcessResult,
};
use super::session_state::{
    elapsed_millis, set_outbound_timing, timing_context_from_utterance, update_incoming_status,
    update_outbound_status,
};
use super::{
    generation_is_live, incoming_session_is_eligible,
    process_authoritative_finalized_outbound_wav,
};

// Only wait on a short interval when deferred incoming work exists. Normal
// idle Meeting Sound retains the existing event-driven blocking wait.
const DEFERRED_INCOMING_RETRY_WAIT: Duration = Duration::from_millis(500);
// Stop must never block indefinitely on inference/driver work. If the old
// thread is still running at this deadline, its JoinHandle stays in the
// canonical store and Start remains blocked until Retry Stop proves exit.
const CONSUMER_STOP_WAIT_BUDGET: Duration = Duration::from_millis(500);
const CONSUMER_STOP_POLL: Duration = Duration::from_millis(10);

fn consumer_thread_finished_within(thread: &JoinHandle<()>, budget: Duration) -> bool {
    let started = Instant::now();
    while !thread.is_finished() {
        let remaining = budget.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return false;
        }
        thread::sleep(remaining.min(CONSUMER_STOP_POLL));
    }
    true
}

struct MeetingOutboundConsumerRuntime {
    generation: u64,
    session_id: String,
    thread: Option<JoinHandle<()>>,
    join_failed: bool,
}

struct MeetingIncomingConsumerRuntime {
    session_id: String,
    thread: Option<JoinHandle<()>>,
    join_failed: bool,
}

pub(super) struct MeetingConsumerCleanupResult {
    pub(super) ok: bool,
    pub(super) message: String,
}

static MEETING_OUTBOUND_CONSUMER: OnceLock<Mutex<Option<MeetingOutboundConsumerRuntime>>> =
    OnceLock::new();
static MEETING_INCOMING_CONSUMER: OnceLock<Mutex<Option<MeetingIncomingConsumerRuntime>>> =
    OnceLock::new();

fn outbound_consumer_store() -> &'static Mutex<Option<MeetingOutboundConsumerRuntime>> {
    MEETING_OUTBOUND_CONSUMER.get_or_init(|| Mutex::new(None))
}

fn incoming_consumer_store() -> &'static Mutex<Option<MeetingIncomingConsumerRuntime>> {
    MEETING_INCOMING_CONSUMER.get_or_init(|| Mutex::new(None))
}

pub(super) fn ensure_meeting_outbound_consumer(generation: u64, session_id: &str) -> Result<(), String> {
    if !runtime_generation_is_authoritative(generation) {
        return Err("meeting_outbound:generation_not_authoritative".to_string());
    }
    {
        let store = outbound_consumer_store()
            .lock().map_err(|_| "meeting_outbound:consumer_state_lock_failed".to_string())?;
        if let Some(active) = store.as_ref() {
            return if active.generation == generation && active.session_id == session_id {
                Ok(())
            } else { Err("meeting_outbound:another_runtime_active".to_string()) };
        }
    }
    start_meeting_outbound_consumer(generation, session_id)
}

pub(super) fn start_meeting_outbound_consumer(generation: u64, session_id: &str) -> Result<(), String> {
    let store = outbound_consumer_store();
    let mut guard = store
        .lock()
        .map_err(|_| "meeting_outbound:consumer_state_lock_failed".to_string())?;
    if guard.is_some() {
        return Err("meeting_outbound:consumer_already_active".to_string());
    }
    if !runtime_generation_is_authoritative(generation) {
        return Err("meeting_outbound:generation_not_authoritative".to_string());
    }

    let thread_session_id = session_id.to_string();
    let thread_session_for_runtime = thread_session_id.clone();
    let noise_suppression_enabled =
        engine::load_settings().audio.noise_suppression != "off";
    let handle = thread::Builder::new()
        .name("translateit-meeting-outbound".to_string())
        .spawn(move || {
            while let Some(utterance) = wait_take_finalized_outbound_utterance(generation) {
                let queue_ms = elapsed_millis(utterance.enqueued_at, Instant::now());
                if utterance.generation != Some(generation)
                    || utterance.lane != "you"
                    || utterance.session_id != thread_session_id
                    || !generation_is_live(generation)
                    || original_voice_pcm::is_original_active(generation)
                {
                    continue;
                }

                let audio_prepare_started_at = Instant::now();
                let write = write_finalized_outbound_utterance_wav_with_noise_suppression(
                    &utterance,
                    noise_suppression_enabled,
                );
                let audio_prepare_ms = elapsed_millis(audio_prepare_started_at, Instant::now());
                let timing = timing_context_from_utterance(&utterance, queue_ms, audio_prepare_ms);
                if !write.ok {
                    update_outbound_status(
                        generation,
                        &thread_session_id,
                        "attention_needed",
                        utterance.sequence,
                        false,
                        false,
                        &write.blocker,
                        "Finalized speech could not be written to its temporary ASR WAV. No AI/output stage consumed it.",
                    );
                    set_outbound_timing(
                        generation,
                        &thread_session_id,
                        utterance.sequence,
                        &timing.metrics,
                    );
                    continue;
                }

                let Some(audio_path) = write.audio_path else {
                    update_outbound_status(
                        generation,
                        &thread_session_id,
                        "attention_needed",
                        utterance.sequence,
                        false,
                        false,
                        "meeting_outbound:finalized_audio_path_missing",
                        "Finalized speech writer returned no temporary audio path. No AI/output stage consumed it.",
                    );
                    set_outbound_timing(
                        generation,
                        &thread_session_id,
                        utterance.sequence,
                        &timing.metrics,
                    );
                    continue;
                };

                if !generation_is_live(generation) {
                    remove_finalized_meeting_utterance_wav(&audio_path);
                    break;
                }

                let _ = process_authoritative_finalized_outbound_wav(
                    generation,
                    &utterance.session_id,
                    utterance.sequence,
                    utterance.utterance_id,
                    utterance.speech_duration_ms,
                    audio_path.clone(),
                    timing,
                );
                remove_finalized_meeting_utterance_wav(&audio_path);

                if !runtime_generation_is_authoritative(generation) {
                    break;
                }
            }
        })
        .map_err(|error| format!("meeting_outbound:consumer_spawn_failed:{error}"))?;

    *guard = Some(MeetingOutboundConsumerRuntime {
        generation,
        session_id: thread_session_for_runtime,
        thread: Some(handle),
        join_failed: false,
    });
    // Revoked while spawning: retain ownership for canonical Stop teardown.
    if !runtime_generation_is_authoritative(generation) {
        return Err("meeting_outbound:generation_not_authoritative".to_string());
    }
    Ok(())
}

pub(super) fn stop_meeting_outbound_consumer(generation: u64) -> MeetingConsumerCleanupResult {
    let producer_cleared = clear_finalized_outbound_utterance_producer_for_generation(generation);
    let mut guard = match outbound_consumer_store().try_lock() {
        Ok(guard) => guard,
        Err(_) => return MeetingConsumerCleanupResult {
            ok: false,
            message: "Meeting outbound consumer owner is busy or unavailable during Stop.".to_string(),
        },
    };
    let Some(active) = guard.as_mut().filter(|value| value.generation == generation) else {
        return MeetingConsumerCleanupResult {
            ok: producer_cleared,
            message: if producer_cleared {
                "No matching Meeting outbound consumer required cleanup.".to_string()
            } else {
                "Meeting outbound producer cleanup could not be verified.".to_string()
            },
        };
    };
    if active.join_failed {
        return MeetingConsumerCleanupResult {
            ok: false,
            message: format!("Meeting outbound consumer for {} generation {generation} had a failed join; the owner remains retained.", active.session_id),
        };
    }
    if active.thread.as_ref().is_some_and(|handle| {
        !consumer_thread_finished_within(handle, CONSUMER_STOP_WAIT_BUDGET)
    }) {
        return MeetingConsumerCleanupResult {
            ok: false,
            message: format!("Meeting outbound consumer for {} generation {generation} has not exited. Retry Stop; a new consumer remains blocked.", active.session_id),
        };
    }
    if active.thread.take().is_some_and(|handle| handle.join().is_err()) {
        // A panicked join consumes its handle; retain the original owner.
        active.join_failed = true;
        return MeetingConsumerCleanupResult {
            ok: false,
            message: format!("Meeting outbound consumer for {} generation {generation} could not be joined; the owner remains retained.", active.session_id),
        };
    }
    if !producer_cleared {
        // The thread may already be joined, but the scoped Silero producer
        // still has not verified release. Retain the same generation for
        // Retry Stop to repeat producer cleanup without joining twice.
        return MeetingConsumerCleanupResult {
            ok: false,
            message: format!("Meeting outbound producer cleanup could not be verified for {} generation {generation}; the owner remains retained.", active.session_id),
        };
    }
    let owned = guard.take().expect("verified outbound owner");
    MeetingConsumerCleanupResult {
        ok: true,
        message: format!("Meeting outbound consumer stopped for {} generation {generation}.", owned.session_id),
    }
}

pub(super) fn start_meeting_incoming_consumer(session_id: &str) -> Result<(), String> {
    let store = incoming_consumer_store();
    let mut guard = store
        .lock()
        .map_err(|_| "meeting_incoming:consumer_state_lock_failed".to_string())?;
    if guard.is_some() {
        return Err("meeting_incoming:consumer_already_active".to_string());
    }
    // Check Live admission under the SAME consumer-store lock used by Stop.
    // If Stop completed before this detached activation arrived, reject it.
    // If Stop revokes authority after this check, its consumer cleanup must
    // wait for this guard and will see the newly stored thread owner.
    if !incoming_session_is_eligible(session_id) {
        return Err("meeting_incoming:session_no_longer_live".to_string());
    }

    let thread_session_id = session_id.to_string();
    let runtime_session_id = thread_session_id.clone();
    let noise_suppression_enabled =
        engine::load_settings().audio.noise_suppression != "off";
    let handle = thread::Builder::new()
        .name("translateit-meeting-incoming".to_string())
        .spawn(move || {
            loop {
                drain_due_deferred_incoming(&thread_session_id);

                let utterance = match try_take_finalized_incoming_utterance(&thread_session_id) {
                    Some(utterance) => utterance,
                    None if super::incoming_deferred::deferred_incoming_health_counts().0 > 0 => {
                        match wait_take_finalized_incoming_utterance_for(
                            &thread_session_id,
                            DEFERRED_INCOMING_RETRY_WAIT,
                        ) {
                            IncomingTimedWait::Utterance(utterance) => utterance,
                            IncomingTimedWait::TimedOut => continue,
                            IncomingTimedWait::Stopped => break,
                        }
                    }
                    None => {
                        let Some(utterance) =
                            wait_take_finalized_incoming_utterance(&thread_session_id)
                        else {
                            break;
                        };
                        utterance
                    }
                };

                if utterance.session_id != thread_session_id
                    || utterance.lane != "incoming"
                    || utterance.generation.is_some()
                    || !incoming_session_is_eligible(&thread_session_id)
                {
                    continue;
                }

                let write = write_finalized_incoming_utterance_wav_with_noise_suppression(
                    &utterance,
                    noise_suppression_enabled,
                );
                if !write.ok {
                    update_incoming_status(
                        &thread_session_id,
                        "degraded",
                        true,
                        &write.blocker,
                        "Finalized incoming speech could not be written to its temporary ASR WAV. Outbound remains available.",
                    );
                    continue;
                }
                let Some(audio_path) = write.audio_path else {
                    update_incoming_status(
                        &thread_session_id,
                        "degraded",
                        true,
                        "meeting_incoming:finalized_audio_path_missing",
                        "Finalized incoming speech writer returned no temporary audio path.",
                    );
                    continue;
                };

                if !incoming_session_is_eligible(&thread_session_id) {
                    remove_finalized_meeting_utterance_wav(&audio_path);
                    break;
                }
                let result = process_authoritative_finalized_incoming_wav(
                    &thread_session_id,
                    utterance.sequence,
                    utterance.utterance_id,
                    &audio_path,
                    None,
                );
                if result != IncomingAudioProcessResult::DeferredAsr {
                    remove_finalized_meeting_utterance_wav(&audio_path);
                }
            }
        })
        .map_err(|error| format!("meeting_incoming:consumer_spawn_failed:{error}"))?;

    *guard = Some(MeetingIncomingConsumerRuntime {
        session_id: runtime_session_id,
        thread: Some(handle),
        join_failed: false,
    });
    Ok(())
}

pub(super) fn stop_meeting_incoming_consumer(session_id: &str) -> MeetingConsumerCleanupResult {
    let producer_cleared = clear_finalized_incoming_utterance_producer_for_session(session_id);
    let deferred_cleanup = clear_deferred_incoming_queue_for_session(session_id);
    let mut guard = match incoming_consumer_store().try_lock() {
        Ok(guard) => guard,
        Err(_) => return MeetingConsumerCleanupResult {
            ok: false,
            message: "Meeting incoming consumer owner is busy or unavailable during Stop.".to_string(),
        },
    };
    let Some(active) = guard.as_mut().filter(|value| value.session_id == session_id) else {
        return MeetingConsumerCleanupResult {
            ok: producer_cleared && deferred_cleanup.is_ok(),
            message: if !producer_cleared {
                "Meeting incoming producer cleanup could not be verified.".to_string()
            } else {
                match deferred_cleanup {
                    Ok(()) => "No matching Meeting incoming consumer required cleanup.".to_string(),
                    Err(error) => format!("Incoming deferred cleanup failed: {error}"),
                }
            },
        };
    };
    if active.join_failed {
        return MeetingConsumerCleanupResult {
            ok: false,
            message: format!("Meeting incoming consumer for {session_id} had a failed join; ownership remains retained."),
        };
    }
    if active.thread.as_ref().is_some_and(|handle| {
        !consumer_thread_finished_within(handle, CONSUMER_STOP_WAIT_BUDGET)
    }) {
        return MeetingConsumerCleanupResult {
            ok: false,
            message: format!("Meeting incoming consumer for {session_id} has not exited. Retry Stop; a new consumer remains blocked."),
        };
    }
    if active.thread.take().is_some_and(|handle| handle.join().is_err()) {
        active.join_failed = true;
        return MeetingConsumerCleanupResult {
            ok: false,
            message: format!("Meeting incoming consumer for {session_id} could not be joined; ownership remains retained."),
        };
    }
    if !producer_cleared || deferred_cleanup.is_err() {
        // A finished thread is not proof its Silero producer or deferred WAV
        // queue was released. Keep the same session for idempotent Retry Stop.
        return MeetingConsumerCleanupResult {
            ok: false,
            message: if !producer_cleared {
                format!("Meeting incoming producer cleanup could not be verified for {session_id}; ownership remains retained.")
            } else {
                format!(
                    "Meeting incoming deferred cleanup could not be verified for {session_id}: {}",
                    deferred_cleanup.err().unwrap_or_else(|| "unverified".to_string())
                )
            },
        };
    }
    let _ = guard.take().expect("verified incoming owner");
    MeetingConsumerCleanupResult {
        ok: true,
        message: format!("Meeting incoming consumer stopped for session {session_id}."),
    }
}

// Read only canonical stores; contention is an unverified release, not idle.
pub(super) fn inactive_meeting_consumers_may_need_stop() -> bool {
    let outbound = outbound_consumer_store().try_lock()
        .map(|owner| owner.is_some()).unwrap_or(true);
    let incoming = incoming_consumer_store().try_lock()
        .map(|owner| owner.is_some()).unwrap_or(true);
    outbound || incoming
}

// Only the existing no-snapshot Meeting Stop transaction may reclaim these
// old Meeting IDs. Meeting lifecycle gate prevents a new Meeting Start,
// and native session authority must remain verifiably idle throughout.
pub(super) fn finish_orphaned_meeting_consumers_without_session() -> Result<(), String> {
    if !super::lifecycle::verified_idle_for_orphan_cleanup() {
        return Err("meeting_consumers:runtime_session_not_verified_idle".to_string());
    }
    let outbound = outbound_consumer_store().try_lock()
        .map_err(|_| "meeting_outbound:orphan_owner_busy".to_string())?
        .as_ref().map(|owner| owner.generation);
    let incoming = incoming_consumer_store().try_lock()
        .map_err(|_| "meeting_incoming:orphan_owner_busy".to_string())?
        .as_ref().map(|owner| owner.session_id.clone());
    if !super::lifecycle::verified_idle_for_orphan_cleanup() {
        return Err("meeting_consumers:runtime_owner_changed".to_string());
    }
    let outbound_result = outbound.map(stop_meeting_outbound_consumer);
    let incoming_result = incoming.as_deref().map(stop_meeting_incoming_consumer);
    if !super::lifecycle::verified_idle_for_orphan_cleanup() {
        return Err("meeting_consumers:runtime_owner_changed_after_cleanup".to_string());
    }
    if outbound_result.as_ref().is_some_and(|result| !result.ok)
        || incoming_result.as_ref().is_some_and(|result| !result.ok)
    {
        return Err(format!(
            "meeting_consumers:orphan_cleanup_incomplete:outbound={};incoming={}",
            outbound_result.as_ref().map(|result| result.message.as_str()).unwrap_or("none"),
            incoming_result.as_ref().map(|result| result.message.as_str()).unwrap_or("none"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod bounded_consumer_stop_tests {
    use super::consumer_thread_finished_within;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn still_running_consumer_is_not_joined_or_detached_at_timeout() {
        let (release, blocked) = mpsc::channel::<()>();
        let thread = thread::spawn(move || {
            let _ = blocked.recv();
        });
        assert!(!consumer_thread_finished_within(&thread, Duration::ZERO));
        assert!(!thread.is_finished());
        release.send(()).expect("release owned fixture thread");
        assert!(consumer_thread_finished_within(
            &thread,
            Duration::from_secs(2),
        ));
        thread.join().expect("fixture thread exits normally");
    }

    #[test]
    fn orphan_consumer_owner_contention_remains_unverified() {
        use super::*;
        let _serial = crate::engine::runtime_state::runtime_test_guard();
        let held = outbound_consumer_store().lock().expect("test owns outbound consumer");
        assert!(inactive_meeting_consumers_may_need_stop());
        drop(held);
    }

    #[test]
    fn failed_outbound_join_cannot_be_forgotten_by_retry_stop() {
        use super::*;
        let _serial = crate::engine::runtime_state::runtime_test_guard();
        let generation = 987654_u64;
        let store = outbound_consumer_store();
        {
            let mut guard = store.lock().expect("consumer store");
            assert!(guard.is_none(), "fixture starts with no consumer");
            *guard = Some(MeetingOutboundConsumerRuntime {
                generation,
                session_id: "orphan-outbound-fixture".to_string(),
                thread: Some(thread::spawn(|| panic!("simulated consumer panic"))),
                join_failed: false,
            });
        }
        assert!(!stop_meeting_outbound_consumer(generation).ok);
        {
            let guard = store.lock().expect("failed join retains owner");
            let owner = guard.as_ref().expect("the same owner remains stored");
            assert!(owner.join_failed);
            assert!(owner.thread.is_none());
        }
        assert!(!stop_meeting_outbound_consumer(generation).ok,
            "a second Stop must not erase join failure");
        let mut guard = store.lock().expect("fixture release");
        let _ = guard.take();
    }

    #[test]
    fn no_snapshot_stop_reaps_only_the_old_outbound_consumer_generation() {
        use super::*;
        let _serial = crate::engine::runtime_state::runtime_test_guard();
        let _ = crate::engine::runtime_state::clear_runtime_session_state();
        let generation = 987653_u64;
        let store = outbound_consumer_store();
        {
            let mut guard = store.lock().expect("outbound store");
            assert!(guard.is_none());
            *guard = Some(MeetingOutboundConsumerRuntime {
                generation,
                session_id: "old-meeting-consumer".to_string(),
                thread: Some(thread::spawn(|| {})),
                join_failed: false,
            });
        }
        assert!(inactive_meeting_consumers_may_need_stop());
        assert!(finish_orphaned_meeting_consumers_without_session().is_ok());
        assert!(store.lock().expect("orphan owner released").is_none());
        assert!(!inactive_meeting_consumers_may_need_stop());
    }

    #[test]
    fn already_joined_outbound_consumer_can_retry_scoped_producer_cleanup() {
        use super::*;
        let _serial = crate::engine::runtime_state::runtime_test_guard();
        let _ = crate::engine::runtime_state::clear_runtime_session_state();
        let generation = 987651_u64;
        let store = outbound_consumer_store();
        {
            let mut guard = store.lock().expect("outbound retry fixture");
            assert!(guard.is_none());
            *guard = Some(MeetingOutboundConsumerRuntime {
                generation,
                session_id: "producer-retry-fixture".to_string(),
                thread: None, // join succeeded previously; scoped producer was unverified
                join_failed: false,
            });
        }
        assert!(inactive_meeting_consumers_may_need_stop());
        assert!(stop_meeting_outbound_consumer(generation).ok,
            "a successfully joined thread must not be joined again on Retry Stop");
        assert!(store.lock().expect("owner released after producer check").is_none());
    }

    #[test]
    fn live_mic_test_prevents_orphan_consumer_cleanup() {
        use super::*;
        let _serial = crate::engine::runtime_state::runtime_test_guard();
        let _ = crate::engine::runtime_state::clear_runtime_session_state();
        let claimed = crate::engine::runtime_state::begin_mic_test_session();
        assert!(claimed.blocker.is_empty());
        assert_eq!(
            finish_orphaned_meeting_consumers_without_session().unwrap_err(),
            "meeting_consumers:runtime_session_not_verified_idle",
        );
        let generation = claimed.snapshot.expect("mic test owner").generation;
        assert_eq!(
            crate::engine::runtime_state::clear_runtime_session_if_generation(generation).blocker,
            "runtime_session:cleared",
        );
    }

    #[test]
    fn already_exited_consumer_is_immediately_joinable() {
        let thread = thread::spawn(|| {});
        assert!(consumer_thread_finished_within(
            &thread,
            Duration::from_secs(2),
        ));
        thread.join().expect("fixture thread exits normally");
    }
}
