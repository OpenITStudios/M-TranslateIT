use crate::engine::runtime_state::{
    latest_runtime_session_state, RuntimeSessionStateReport, APPLICATION_MEETING_OWNER_ID,
};

pub fn prepare_for_app_exit() -> bool {
    if super::super::voice_lab_recording::voice_lab_recording_blocks_app_exit()
        || super::super::voice_lab::current_voice_lab_build_snapshot().active
    {
        return false;
    }

    let runtime = latest_runtime_session_state();
    if runtime.has_active_session && runtime.snapshot.is_none() {
        return false;
    }

    if let Some(owner) = runtime
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.owner_id.as_str())
    {
        if owner != APPLICATION_MEETING_OWNER_ID {
            return false;
        }

        let result = super::super::meeting_session::stop_meeting_translation();
        if !result.ok || result.status.has_session {
            return false;
        }
    }

    finish_app_exit(
        &latest_runtime_session_state(),
        super::super::meeting_session::meeting_stop_cleanup_pending(),
        super::super::helper_bridge::shutdown_helper_bridge_for_app_exit,
        super::super::startup_recovery::mark_clean_shutdown,
    )
}

// Keep the irreversible shutdown steps behind the same verified-release gate.
fn finish_app_exit(
    runtime: &RuntimeSessionStateReport,
    cleanup_pending: bool,
    shutdown_helper: impl FnOnce() -> bool,
    mark_clean: impl FnOnce() -> bool,
) -> bool {
    if runtime.has_active_session
        || runtime.snapshot.is_some()
        || runtime.blocker != "runtime_session:no_active_session"
        || cleanup_pending
    {
        return false;
    }

    shutdown_helper() && mark_clean()
}

#[cfg(test)]
mod tests {
    use super::finish_app_exit;
    use crate::engine::runtime_state::{
        RuntimeSessionSnapshot, RuntimeSessionStateReport, APPLICATION_MEETING_OWNER_ID,
        MIC_TEST_OWNER_ID, VOICE_RECORDING_OWNER_ID,
    };
    use std::cell::RefCell;

    fn idle() -> RuntimeSessionStateReport {
        RuntimeSessionStateReport {
            has_active_session: false,
            snapshot: None,
            active_age_ms: None,
            ready_for_stop: false,
            blocker: "runtime_session:no_active_session".to_string(),
            note: String::new(),
        }
    }

    #[test]
    fn orphan_cleanup_blocks_exit_until_retry_stop_verifies_release() {
        let steps = RefCell::new(Vec::new());
        let finish = |pending| finish_app_exit(
            &idle(),
            pending,
            || {
                steps.borrow_mut().push("helper_stopped");
                true
            },
            || {
                steps.borrow_mut().push("marked_clean");
                true
            },
        );
        assert!(!finish(true), "no session does not prove orphan cleanup finished");
        assert!(steps.borrow().is_empty(), "preserve worker and recovery marker");
        assert!(finish(false), "successful Retry Stop permits clean shutdown");
        assert_eq!(*steps.borrow(), vec!["helper_stopped", "marked_clean"]);
    }

    #[test]
    fn stale_or_unverifiable_runtime_never_reaches_shutdown_steps() {
        let mut unavailable = idle();
        unavailable.blocker = "runtime_session:state_lock_failed".to_string();
        let mut active = idle();
        active.has_active_session = true;
        let mut retained = idle();
        retained.snapshot = Some(RuntimeSessionSnapshot {
            started_unix_ms: 0,
            owner_id: APPLICATION_MEETING_OWNER_ID.to_string(),
            session_id: "meeting_shutdown_test".to_string(),
            generation: 1,
            authority_active: false,
            phase: "cleanup_incomplete".to_string(),
            live_capture_stream_active: false,
            safe_to_stop: true,
            note: String::new(),
        });
        for runtime in [unavailable, active, retained] {
            assert!(!finish_app_exit(
                &runtime,
                false,
                || panic!("unreleased runtime must not stop helper"),
                || panic!("unreleased runtime must retain recovery marker"),
            ));
        }
    }

    #[test]
    fn helper_or_marker_failure_prevents_clean_exit() {
        assert!(!finish_app_exit(
            &idle(),
            false,
            || false,
            || panic!("failed helper shutdown must preserve recovery marker"),
        ));
        assert!(!finish_app_exit(&idle(), false, || true, || false));
    }

    #[test]
    fn shared_resource_owner_ids_remain_distinct_for_shutdown_routing() {
        assert_ne!(APPLICATION_MEETING_OWNER_ID, MIC_TEST_OWNER_ID);
        assert_ne!(APPLICATION_MEETING_OWNER_ID, VOICE_RECORDING_OWNER_ID);
        assert_ne!(MIC_TEST_OWNER_ID, VOICE_RECORDING_OWNER_ID);
    }
}
