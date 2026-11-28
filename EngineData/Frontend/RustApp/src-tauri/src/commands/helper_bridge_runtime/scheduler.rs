use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::engine::runtime_state::runtime_generation_matches_epoch;

pub(crate) const SCHEDULER_SCOPE_REVOKED: &str = "helper_scheduler:meeting_scope_revoked";
pub(crate) const SCHEDULER_DEFERRED_EXPIRED: &str = "helper_scheduler:deferred_incoming_expired";

pub const HELPER_SCHEDULER_TOTAL_ADMISSION_CAP: u32 = 8;
pub const HELPER_SCHEDULER_MEETING_OUTBOUND_WAIT_MS: u64 = 120_000;
pub const HELPER_SCHEDULER_MEETING_INCOMING_WAIT_MS: u64 = 30_000;
pub const HELPER_SCHEDULER_TEXT_WAIT_MS: u64 = 15_000;
pub const HELPER_SCHEDULER_DIAGNOSTIC_WAIT_MS: u64 = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperTaskPriority {
    MeetingOutbound,
    MeetingIncoming,
    Text,
    Diagnostic,
}

impl HelperTaskPriority {
    pub fn label(self) -> &'static str {
        match self {
            Self::MeetingOutbound => "meeting_outbound",
            Self::MeetingIncoming => "meeting_incoming",
            Self::Text => "text",
            Self::Diagnostic => "diagnostic",
        }
    }

    fn scheduler_wait_deadline_ms(self) -> u64 {
        match self {
            Self::MeetingOutbound => HELPER_SCHEDULER_MEETING_OUTBOUND_WAIT_MS,
            Self::MeetingIncoming => HELPER_SCHEDULER_MEETING_INCOMING_WAIT_MS,
            Self::Text => HELPER_SCHEDULER_TEXT_WAIT_MS,
            Self::Diagnostic => HELPER_SCHEDULER_DIAGNOSTIC_WAIT_MS,
        }
    }

    fn scheduler_admission_limit(self) -> u32 {
        match self {
            Self::MeetingOutbound => HELPER_SCHEDULER_TOTAL_ADMISSION_CAP,
            Self::MeetingIncoming => HELPER_SCHEDULER_TOTAL_ADMISSION_CAP.saturating_sub(1),
            Self::Text => HELPER_SCHEDULER_TOTAL_ADMISSION_CAP.saturating_sub(2),
            Self::Diagnostic => HELPER_SCHEDULER_TOTAL_ADMISSION_CAP.saturating_sub(3),
        }
    }
}

#[derive(Debug, Default)]
struct HelperSchedulerState {
    active: bool,
    waiting_meeting_outbound: u32,
    waiting_meeting_incoming: u32,
    waiting_text: u32,
    waiting_diagnostic: u32,
    next_request_sequence: u64,
}

static HELPER_SCHEDULER: OnceLock<(Mutex<HelperSchedulerState>, Condvar)> = OnceLock::new();

fn scheduler() -> &'static (Mutex<HelperSchedulerState>, Condvar) {
    HELPER_SCHEDULER.get_or_init(|| (Mutex::new(HelperSchedulerState::default()), Condvar::new()))
}

// Revocation updates the atomic epoch before this notification. Synchronize
// with the existing scheduler mutex to prevent a lost wake between a
// waiter's predicate check and Condvar wait. No runtime-state lock is ever
// taken inside the scheduler; its scope check is atomic-only.
pub(crate) fn wake_revoked_scheduler_waiters() {
    if let Some((lock, wake)) = HELPER_SCHEDULER.get() {
        let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        wake.notify_all();
    }
}

pub struct HelperTaskPermit {
    request_id: String,
    scheduler_wait_ms: u64,
}

impl HelperTaskPermit {
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    /// Time spent waiting for the single worker permit, excluding model execution.
    pub fn scheduler_wait_ms(&self) -> u64 {
        self.scheduler_wait_ms
    }
}

impl Drop for HelperTaskPermit {
    fn drop(&mut self) {
        let (lock, wake) = scheduler();
        if let Ok(mut state) = lock.lock() {
            state.active = false;
            // All sleeping callers register under this same mutex. A new
            // caller arriving after unlock sees the available permit itself.
            if scheduler_total_admitted(&state) != 0 {
                wake.notify_all();
            }
        }
    }
}

fn scheduler_waiting_increment(state: &mut HelperSchedulerState, priority: HelperTaskPriority) {
    match priority {
        HelperTaskPriority::MeetingOutbound => {
            state.waiting_meeting_outbound = state.waiting_meeting_outbound.saturating_add(1)
        }
        HelperTaskPriority::MeetingIncoming => {
            state.waiting_meeting_incoming = state.waiting_meeting_incoming.saturating_add(1)
        }
        HelperTaskPriority::Text => state.waiting_text = state.waiting_text.saturating_add(1),
        HelperTaskPriority::Diagnostic => {
            state.waiting_diagnostic = state.waiting_diagnostic.saturating_add(1)
        }
    }
}

fn scheduler_waiting_decrement(state: &mut HelperSchedulerState, priority: HelperTaskPriority) {
    match priority {
        HelperTaskPriority::MeetingOutbound => {
            state.waiting_meeting_outbound = state.waiting_meeting_outbound.saturating_sub(1)
        }
        HelperTaskPriority::MeetingIncoming => {
            state.waiting_meeting_incoming = state.waiting_meeting_incoming.saturating_sub(1)
        }
        HelperTaskPriority::Text => state.waiting_text = state.waiting_text.saturating_sub(1),
        HelperTaskPriority::Diagnostic => {
            state.waiting_diagnostic = state.waiting_diagnostic.saturating_sub(1)
        }
    }
}

fn scheduler_total_admitted(state: &HelperSchedulerState) -> u32 {
    let active = if state.active { 1_u32 } else { 0_u32 };
    active
        .saturating_add(state.waiting_meeting_outbound)
        .saturating_add(state.waiting_meeting_incoming)
        .saturating_add(state.waiting_text)
        .saturating_add(state.waiting_diagnostic)
}

fn scheduler_can_admit(state: &HelperSchedulerState, priority: HelperTaskPriority) -> bool {
    scheduler_total_admitted(state) < priority.scheduler_admission_limit()
}

fn scheduler_wait_deadline_error(priority: HelperTaskPriority, wait_deadline: Duration) -> String {
    format!(
        "helper_scheduler:wait_deadline_exceeded:{}:{}ms",
        priority.label(),
        wait_deadline.as_millis()
    )
}

fn scheduler_can_enter(state: &HelperSchedulerState, priority: HelperTaskPriority) -> bool {
    if state.active {
        return false;
    }
    match priority {
        HelperTaskPriority::MeetingOutbound => true,
        HelperTaskPriority::MeetingIncoming => state.waiting_meeting_outbound == 0,
        HelperTaskPriority::Text => {
            state.waiting_meeting_outbound == 0 && state.waiting_meeting_incoming == 0
        }
        HelperTaskPriority::Diagnostic => {
            state.waiting_meeting_outbound == 0
                && state.waiting_meeting_incoming == 0
                && state.waiting_text == 0
        }
    }
}

fn acquire_helper_task_permit_with_wait_deadline(
    priority: HelperTaskPriority,
    wait_deadline: Duration,
) -> Result<HelperTaskPermit, String> {
    acquire_scoped_permit_with_deadline(priority, wait_deadline, None, None)
}

fn acquire_scoped_permit_with_deadline(
    priority: HelperTaskPriority,
    wait_deadline: Duration,
    generation: Option<u64>,
    deferred_deadline: Option<Instant>,
) -> Result<HelperTaskPermit, String> {
    let wait_started = Instant::now();
    let (lock, wake) = scheduler();
    let mut state = lock
        .lock()
        .map_err(|_| "helper_scheduler:lock_poisoned".to_string())?;

    if generation.is_some_and(|generation| !runtime_generation_matches_epoch(generation)) {
        return Err(SCHEDULER_SCOPE_REVOKED.to_string());
    }
    if deferred_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err(SCHEDULER_DEFERRED_EXPIRED.to_string());
    }
    if !scheduler_can_admit(&state, priority) {
        return Err(format!(
            "helper_scheduler:admission_capacity_exceeded:{}:total={}:limit={}",
            priority.label(),
            scheduler_total_admitted(&state),
            priority.scheduler_admission_limit()
        ));
    }

    // Uncontended requests need no waiting counter updates. Both paths
    // share the same deadline, epoch and priority checks before claiming.
    let mut registered = false;
    loop {
        if generation.is_some_and(|generation| !runtime_generation_matches_epoch(generation)) {
            if registered {
                scheduler_waiting_decrement(&mut state, priority);
                wake.notify_all();
            }
            return Err(SCHEDULER_SCOPE_REVOKED.to_string());
        }
        let remaining = wait_deadline.saturating_sub(wait_started.elapsed());
        let deferred_remaining = deferred_deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
        let deferred_expired = deferred_remaining.is_some_and(|budget| budget.is_zero());
        if deferred_expired || remaining.is_zero() {
            if registered {
                scheduler_waiting_decrement(&mut state, priority);
                wake.notify_all();
            }
            return Err(if deferred_expired { SCHEDULER_DEFERRED_EXPIRED.to_string() }
                else { scheduler_wait_deadline_error(priority, wait_deadline) });
        }
        let remaining = deferred_remaining.map_or(remaining, |budget| remaining.min(budget));

        if scheduler_can_enter(&state, priority) {
            break;
        }
        if !registered {
            scheduler_waiting_increment(&mut state, priority);
            registered = true;
        }
        let (next_state, _) = match wake.wait_timeout(state, remaining) {
            Ok(value) => value,
            Err(poisoned) => {
                let (mut poisoned_state, _) = poisoned.into_inner();
                scheduler_waiting_decrement(&mut poisoned_state, priority);
                wake.notify_all();
                return Err("helper_scheduler:wait_lock_poisoned".to_string());
            }
        };
        state = next_state;
        // A wake or timeout is only a reason to recheck the same budget.
    }

    if registered {
        scheduler_waiting_decrement(&mut state, priority);
    }
    state.active = true;
    state.next_request_sequence = state.next_request_sequence.saturating_add(1);
    let sequence = state.next_request_sequence;
    drop(state);
    Ok(HelperTaskPermit {
        request_id: format!("helper-{}", sequence),
        scheduler_wait_ms: wait_started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
    })
}

// A UI observation may refresh capability only when no execution or waiter
// owns the scheduler. It never registers, waits, or jumps ahead of a waiter.
pub(crate) fn try_acquire_idle_diagnostic_permit() -> Result<Option<HelperTaskPermit>, &'static str> {
    let wait_started = Instant::now();
    let (lock, _) = scheduler();
    let mut state = match lock.try_lock() {
        Ok(state) => state,
        Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
        Err(std::sync::TryLockError::Poisoned(_)) => return Err("helper_scheduler:lock_poisoned"),
    };
    if scheduler_total_admitted(&state) != 0 {
        return Ok(None);
    }
    state.active = true;
    state.next_request_sequence = state.next_request_sequence.saturating_add(1);
    let sequence = state.next_request_sequence;
    drop(state);
    Ok(Some(HelperTaskPermit {
        request_id: format!("helper-{}", sequence),
        scheduler_wait_ms: wait_started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
    }))
}

pub fn acquire_helper_task_permit(
    priority: HelperTaskPriority,
) -> Result<HelperTaskPermit, String> {
    acquire_helper_task_permit_with_wait_deadline(
        priority,
        Duration::from_millis(priority.scheduler_wait_deadline_ms()),
    )
}

// Callers establish exact session authority before queuing. The epoch is
// only a cancellation fence; the existing post-permit authority check stays.
pub(crate) fn acquire_scoped_helper_task_permit(
    priority: HelperTaskPriority, generation: Option<u64>, deferred_deadline: Option<Instant>,
) -> Result<HelperTaskPermit, String> {
    acquire_scoped_permit_with_deadline(
        priority, Duration::from_millis(priority.scheduler_wait_deadline_ms()), generation, deferred_deadline,
    )
}

#[cfg(test)]
mod scheduler_policy_tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;

    fn scheduler_test_guard() -> std::sync::MutexGuard<'static, ()> {
        // Bridge contract fixtures use the same process-wide scheduler.
        crate::engine::runtime_state::runtime_test_guard()
    }

    fn reset_scheduler() {
        let (lock, wake) = scheduler();
        let mut state = lock.lock().expect("scheduler test lock");
        *state = HelperSchedulerState::default();
        wake.notify_all();
    }

    #[test]
    fn scheduler_policy_preserves_priority_and_reserved_admission_headroom() {
        let _serial = scheduler_test_guard();
        assert!(
            HelperTaskPriority::MeetingOutbound.scheduler_wait_deadline_ms()
                > HelperTaskPriority::MeetingIncoming.scheduler_wait_deadline_ms()
        );
        assert!(
            HelperTaskPriority::MeetingIncoming.scheduler_wait_deadline_ms()
                > HelperTaskPriority::Text.scheduler_wait_deadline_ms()
        );
        assert!(
            HelperTaskPriority::Text.scheduler_wait_deadline_ms()
                > HelperTaskPriority::Diagnostic.scheduler_wait_deadline_ms()
        );

        let mut state = HelperSchedulerState::default();
        state.waiting_meeting_outbound = 1;
        assert!(scheduler_can_enter(
            &state,
            HelperTaskPriority::MeetingOutbound
        ));
        assert!(!scheduler_can_enter(
            &state,
            HelperTaskPriority::MeetingIncoming
        ));
        assert!(!scheduler_can_enter(&state, HelperTaskPriority::Text));
        assert!(!scheduler_can_enter(&state, HelperTaskPriority::Diagnostic));

        state.waiting_meeting_outbound = 0;
        state.waiting_meeting_incoming = 1;
        assert!(scheduler_can_enter(
            &state,
            HelperTaskPriority::MeetingIncoming
        ));
        assert!(!scheduler_can_enter(&state, HelperTaskPriority::Text));
        assert!(!scheduler_can_enter(&state, HelperTaskPriority::Diagnostic));

        state.waiting_meeting_incoming = 0;
        state.waiting_text = 1;
        assert!(scheduler_can_enter(&state, HelperTaskPriority::Text));
        assert!(!scheduler_can_enter(&state, HelperTaskPriority::Diagnostic));

        state = HelperSchedulerState::default();
        state.active = true;
        state.waiting_diagnostic = HelperTaskPriority::Diagnostic
            .scheduler_admission_limit()
            .saturating_sub(1);
        assert_eq!(
            scheduler_total_admitted(&state),
            HelperTaskPriority::Diagnostic.scheduler_admission_limit()
        );
        assert!(!scheduler_can_admit(&state, HelperTaskPriority::Diagnostic));
        assert!(scheduler_can_admit(&state, HelperTaskPriority::Text));
        assert!(scheduler_can_admit(
            &state,
            HelperTaskPriority::MeetingIncoming
        ));
        assert!(scheduler_can_admit(
            &state,
            HelperTaskPriority::MeetingOutbound
        ));
        assert_eq!(
            HelperTaskPriority::MeetingOutbound.scheduler_admission_limit(),
            HELPER_SCHEDULER_TOTAL_ADMISSION_CAP
        );
    }

    #[test]
    fn uncontended_permit_preserves_sequence_and_never_leaves_waiters() {
        let _serial = scheduler_test_guard();
        reset_scheduler();
        for (index, priority) in [
            HelperTaskPriority::MeetingOutbound, HelperTaskPriority::MeetingIncoming,
            HelperTaskPriority::Text, HelperTaskPriority::Diagnostic,
        ].into_iter().enumerate() {
            let permit = acquire_helper_task_permit_with_wait_deadline(priority, Duration::from_secs(1)).unwrap();
            assert_eq!(permit.request_id(), format!("helper-{}", index + 1));
            {
                let state = scheduler().0.lock().unwrap();
                assert!(state.active);
                assert_eq!(scheduler_total_admitted(&state), 1);
            }
            drop(permit);
        }
        reset_scheduler();
    }

    #[test]
    fn diagnostic_observation_never_registers_or_steals_a_waiter() {
        let _serial = scheduler_test_guard();
        reset_scheduler();
        let permit = acquire_helper_task_permit(HelperTaskPriority::MeetingOutbound).unwrap();
        assert!(try_acquire_idle_diagnostic_permit().unwrap().is_none());
        {
            let state = scheduler().0.lock().unwrap();
            assert_eq!(scheduler_total_admitted(&state), 1);
            assert_eq!(state.next_request_sequence, 1);
        }
        drop(permit);
        {
            let mut state = scheduler().0.lock().unwrap();
            // Contended lock must not block the observer.
            assert!(try_acquire_idle_diagnostic_permit().unwrap().is_none());
            state.waiting_meeting_outbound = 1;
        }
        assert!(try_acquire_idle_diagnostic_permit().unwrap().is_none());
        {
            let mut state = scheduler().0.lock().unwrap();
            assert_eq!(state.waiting_meeting_outbound, 1);
            assert_eq!(state.next_request_sequence, 1);
            state.waiting_meeting_outbound = 0;
            state.waiting_diagnostic = 1;
        }
        assert!(try_acquire_idle_diagnostic_permit().unwrap().is_none());
        {
            let mut state = scheduler().0.lock().unwrap();
            assert_eq!(state.waiting_diagnostic, 1);
            state.waiting_diagnostic = 0;
        }
        let observed = try_acquire_idle_diagnostic_permit().unwrap().expect("idle permit");
        assert_eq!(observed.request_id(), "helper-2");
        drop(observed);
        assert_eq!(scheduler_total_admitted(&scheduler().0.lock().unwrap()), 0);
        reset_scheduler();
    }

    #[test]
    fn expired_wait_cannot_claim_an_idle_scheduler() {
        let _serial = scheduler_test_guard();
        reset_scheduler();
        let error = acquire_helper_task_permit_with_wait_deadline(
            HelperTaskPriority::MeetingOutbound, Duration::ZERO,
        ).err().expect("expired request must not claim even a free slot");
        assert_eq!(error, "helper_scheduler:wait_deadline_exceeded:meeting_outbound:0ms");
        let (lock, _) = scheduler();
        let state = lock.lock().expect("scheduler state");
        assert_eq!(scheduler_total_admitted(&state), 0);
        assert_eq!(state.next_request_sequence, 0);
        drop(state);
        let next = acquire_helper_task_permit_with_wait_deadline(
            HelperTaskPriority::MeetingOutbound, Duration::from_secs(1),
        ).expect("next valid request can enter");
        assert_eq!(next.request_id(), "helper-1");
        drop(next);
        reset_scheduler();
    }

    #[test]
    fn deferred_deadline_caps_scheduler_wait_and_never_claims_expired_work() {
        let _serial = scheduler_test_guard();
        reset_scheduler();
        let expired = acquire_scoped_permit_with_deadline(
            HelperTaskPriority::MeetingIncoming, Duration::from_secs(1), None, Some(Instant::now()),
        );
        assert_eq!(expired.err().as_deref(), Some(SCHEDULER_DEFERRED_EXPIRED));
        let active = acquire_helper_task_permit(HelperTaskPriority::MeetingOutbound).unwrap();
        let deadline = Instant::now() + Duration::from_millis(25);
        let expired = acquire_scoped_permit_with_deadline(
            HelperTaskPriority::MeetingIncoming, Duration::from_secs(1), None, Some(deadline),
        );
        assert_eq!(expired.err().as_deref(), Some(SCHEDULER_DEFERRED_EXPIRED));
        {
            let state = scheduler().0.lock().unwrap();
            assert!(state.active);
            assert_eq!(state.waiting_meeting_incoming, 0);
            assert_eq!(state.next_request_sequence, 1);
        }
        drop(active);
        reset_scheduler();
    }

    #[test]
    fn scheduler_capacity_rejection_does_not_add_waiting_callers() {
        let _serial = scheduler_test_guard();
        reset_scheduler();
        {
            let (lock, _) = scheduler();
            let mut state = lock.lock().expect("scheduler test lock");
            state.active = true;
            state.waiting_diagnostic = HelperTaskPriority::Diagnostic
                .scheduler_admission_limit()
                .saturating_sub(1);
        }

        let error = match acquire_helper_task_permit_with_wait_deadline(
            HelperTaskPriority::Diagnostic,
            Duration::from_millis(1),
        ) {
            Ok(_) => panic!("diagnostic request should be rejected at its admission limit"),
            Err(error) => error,
        };
        assert!(error.starts_with("helper_scheduler:admission_capacity_exceeded:diagnostic:"));

        let (lock, _) = scheduler();
        let state = lock.lock().expect("scheduler test lock");
        assert_eq!(
            scheduler_total_admitted(&state),
            HelperTaskPriority::Diagnostic.scheduler_admission_limit()
        );
        drop(state);
        reset_scheduler();
    }

    #[test]
    fn scheduler_wait_deadline_cleans_counter_and_releases_lower_priority_blocking() {
        let _serial = scheduler_test_guard();
        reset_scheduler();
        let active = acquire_helper_task_permit_with_wait_deadline(
            HelperTaskPriority::MeetingOutbound,
            Duration::from_millis(50),
        )
        .expect("outbound scheduler permit");

        let waiter = thread::spawn(|| {
            acquire_helper_task_permit_with_wait_deadline(
                HelperTaskPriority::Text,
                Duration::from_millis(25),
            )
        });
        let error = match waiter.join().expect("scheduler waiter thread") {
            Ok(_) => panic!("text request should time out while worker remains active"),
            Err(error) => error,
        };
        assert_eq!(error, "helper_scheduler:wait_deadline_exceeded:text:25ms");

        {
            let (lock, _) = scheduler();
            let state = lock.lock().expect("scheduler test lock");
            assert!(state.active);
            assert_eq!(state.waiting_text, 0);
            assert_eq!(state.waiting_meeting_outbound, 0);
            assert_eq!(state.waiting_meeting_incoming, 0);
            assert_eq!(state.waiting_diagnostic, 0);
        }

        drop(active);
        {
            let (lock, _) = scheduler();
            let state = lock.lock().expect("scheduler test lock");
            assert!(!state.active);
        }
        reset_scheduler();
    }

    #[test]
    fn revoked_meeting_waiters_exit_while_active_permit_is_still_held() {
        use crate::engine::runtime_state::{
            begin_application_meeting_session, clear_runtime_session_state,
            clear_runtime_session_if_generation, revoke_runtime_session_authority,
        };
        let _serial = scheduler_test_guard();
        reset_scheduler();
        let _ = clear_runtime_session_state();
        let generation = begin_application_meeting_session().snapshot.expect("Meeting scope").generation;
        let active = acquire_helper_task_permit(HelperTaskPriority::Diagnostic).expect("active permit");
        let (tx, rx) = mpsc::channel();
        let mut handles = Vec::new();
        for priority in [HelperTaskPriority::MeetingOutbound, HelperTaskPriority::MeetingIncoming] {
            let tx = tx.clone();
            handles.push(thread::spawn(move || {
                let result = acquire_scoped_permit_with_deadline(priority, Duration::from_secs(5), Some(generation), None);
                let _ = tx.send(result.err());
            }));
        }
        drop(tx);
        let registered = Instant::now() + Duration::from_secs(1);
        loop {
            let state = scheduler().0.lock().unwrap();
            if state.waiting_meeting_outbound == 1 && state.waiting_meeting_incoming == 1 { break; }
            drop(state);
            assert!(Instant::now() < registered, "both scoped waiters registered");
            thread::yield_now();
        }
        let _ = revoke_runtime_session_authority(generation, "fixture scope revoked");
        let first = rx.recv_timeout(Duration::from_secs(1));
        let second = rx.recv_timeout(Duration::from_secs(1));
        let (still_active, waiting, sequence) = {
            let state = scheduler().0.lock().unwrap();
            (state.active, state.waiting_meeting_outbound + state.waiting_meeting_incoming, state.next_request_sequence)
        };
        drop(active);
        for handle in handles { handle.join().unwrap(); }
        assert_eq!(first.unwrap().as_deref(), Some(SCHEDULER_SCOPE_REVOKED));
        assert_eq!(second.unwrap().as_deref(), Some(SCHEDULER_SCOPE_REVOKED));
        assert!(still_active);
        assert_eq!((waiting, sequence), (0, 1));
        assert_eq!(acquire_scoped_helper_task_permit(HelperTaskPriority::MeetingOutbound, Some(generation), None).err().as_deref(),
            Some(SCHEDULER_SCOPE_REVOKED));
        let next = acquire_helper_task_permit(HelperTaskPriority::Text).unwrap();
        assert_eq!(next.request_id(), "helper-2");
        drop(next);
        let _ = clear_runtime_session_if_generation(generation);
        reset_scheduler();
    }

    #[test]
    fn scheduler_waiters_enter_in_priority_order_after_active_permit_releases() {
        let _serial = scheduler_test_guard();
        reset_scheduler();
        let active = acquire_helper_task_permit_with_wait_deadline(
            HelperTaskPriority::Diagnostic,
            Duration::from_millis(100),
        )
        .expect("seed active scheduler permit");

        let (entered_tx, entered_rx) = mpsc::channel::<&'static str>();
        let mut handles = Vec::new();
        for (priority, label) in [
            (HelperTaskPriority::Diagnostic, "diagnostic"),
            (HelperTaskPriority::Text, "text"),
            (HelperTaskPriority::MeetingIncoming, "meeting_incoming"),
            (HelperTaskPriority::MeetingOutbound, "meeting_outbound"),
        ] {
            let tx = entered_tx.clone();
            handles.push(thread::spawn(move || {
                let permit = acquire_helper_task_permit_with_wait_deadline(
                    priority,
                    Duration::from_secs(2),
                )
                .expect("queued scheduler permit");
                tx.send(label).expect("record scheduler entry");
                drop(permit);
            }));
        }
        drop(entered_tx);

        let registration_deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let all_registered = {
                let (lock, _) = scheduler();
                let state = lock.lock().expect("scheduler test lock");
                state.waiting_meeting_outbound == 1
                    && state.waiting_meeting_incoming == 1
                    && state.waiting_text == 1
                    && state.waiting_diagnostic == 1
            };
            if all_registered {
                break;
            }
            assert!(
                Instant::now() < registration_deadline,
                "all scheduler waiters must register before releasing the active permit"
            );
            thread::sleep(Duration::from_millis(2));
        }

        drop(active);
        let entered: Vec<&'static str> = (0..4)
            .map(|_| {
                entered_rx
                    .recv_timeout(Duration::from_secs(2))
                    .expect("scheduler entry order")
            })
            .collect();
        assert_eq!(
            entered,
            vec![
                "meeting_outbound",
                "meeting_incoming",
                "text",
                "diagnostic"
            ]
        );

        for handle in handles {
            handle.join().expect("scheduler waiter thread");
        }
        reset_scheduler();
    }
}
