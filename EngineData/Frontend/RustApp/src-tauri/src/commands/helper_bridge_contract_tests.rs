use serde_json::{json, Value};

use crate::engine::runtime_state::{
    begin_application_meeting_session, begin_mic_test_session,
    clear_runtime_session_if_generation, clear_runtime_session_state,
    commit_application_meeting_session_live,
};

use super::helper_bridge::{
    cancel_helper_bridge_meeting_session, finish_orphaned_meeting_helper_without_session,
    meeting_helper_orphan_cleanup_pending, send_helper_worker_task,
    start_helper_bridge,
    shutdown_helper_bridge_for_app_exit, HelperBridgeWorkerResponse,
};
use super::helper_bridge_runtime::{
    clear_active_request, runtime, set_blocked, stop_child,
    HelperBridgeRuntime,
};

fn reset_bridge_runtime() {
    if let Ok(mut bridge) = runtime().lock() {
        assert!(stop_child(&mut bridge), "tests must not drop an unjoined helper owner");
        *bridge = HelperBridgeRuntime::default();
    }
}

fn response_blocker(response: &HelperBridgeWorkerResponse) -> String {
    response.worker_response_json
        .get("blocker")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_default()
}

#[test]
fn bridge_priority_and_stale_guards_follow_production_payload_contract() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    let _ = clear_runtime_session_state();
    reset_bridge_runtime();

    let text = send_helper_worker_task(
        "translate",
        json!({
            "text": "halo",
            "source_language": "id",
            "target_language": "en"
        }),
    );
    assert_eq!(text.scheduler_priority, "text");
    #[cfg(windows)]
    assert_eq!(response_blocker(&text), "native_inference:owner_not_admitted");
    #[cfg(not(windows))]
    assert_eq!(response_blocker(&text), "native_inference:platform_unsupported");

    reset_bridge_runtime();
    let diagnostic = send_helper_worker_task("status", json!({}));
    assert_eq!(diagnostic.scheduler_priority, "diagnostic");
    assert_eq!(response_blocker(&diagnostic), if super::helper_bridge::native_voice_candidate_enabled() {
        "native_inference:lifecycle_not_ready"
    } else {
        "helper_bridge:not_running"
    });

    reset_bridge_runtime();
    let incomplete_incoming = send_helper_worker_task(
        "translate",
        json!({
            "text": "hello",
            "meeting_lane": "incoming"
        }),
    );
    assert_eq!(
        incomplete_incoming.scheduler_priority, "text",
        "incoming lane without a Meeting session id must not gain MeetingIncoming priority"
    );

    reset_bridge_runtime();
    let start_prepare = send_helper_worker_task(
        "status",
        json!({
            "meeting_start_prepare": true
        }),
    );
    assert_eq!(start_prepare.scheduler_priority, "meeting_outbound");
    assert_eq!(response_blocker(&start_prepare), if super::helper_bridge::native_voice_candidate_enabled() {
        "native_inference:scope_revoked"
    } else {
        "helper_bridge:not_running"
    });

    let meeting = begin_application_meeting_session();
    assert!(meeting.blocker.is_empty());
    let snapshot = meeting.snapshot.as_ref().expect("Meeting authority claim");
    let generation = snapshot.generation;
    let session_id = snapshot.session_id.clone();

    reset_bridge_runtime();
    let outbound = send_helper_worker_task(
        "transcribe",
        json!({
            "audio_path": "unused-test.wav",
            "meeting_session_id": session_id.clone(),
            "meeting_lane": "you",
            "meeting_generation": generation
        }),
    );
    assert_eq!(outbound.scheduler_priority, "meeting_outbound");
    #[cfg(windows)]
    assert_eq!(response_blocker(&outbound), "native_inference:scope_invalid");
    #[cfg(not(windows))]
    assert_eq!(response_blocker(&outbound), "native_inference:platform_unsupported");

    let live = commit_application_meeting_session_live(
        generation,
        false,
        "bridge contract test commits Meeting Live without opening capture",
    );
    assert!(live.blocker.is_empty());

    reset_bridge_runtime();
    let incoming = send_helper_worker_task(
        "transcribe",
        json!({
            "audio_path": "unused-test.wav",
            "meeting_session_id": session_id.clone(),
            "meeting_lane": "INCOMING"
        }),
    );
    assert_eq!(incoming.scheduler_priority, "meeting_incoming");
    #[cfg(windows)]
    assert_eq!(response_blocker(&incoming), "native_inference:scope_invalid");
    #[cfg(not(windows))]
    assert_eq!(response_blocker(&incoming), "native_inference:platform_unsupported");

    let cleared = clear_runtime_session_if_generation(generation);
    assert_eq!(cleared.blocker, "runtime_session:cleared");

    reset_bridge_runtime();
    let stale_outbound = send_helper_worker_task(
        "translate",
        json!({
            "text": "halo",
            "meeting_session_id": session_id.clone(),
            "meeting_lane": "you",
            "meeting_generation": generation
        }),
    );
    assert_eq!(stale_outbound.scheduler_priority, "meeting_outbound");
    assert_eq!(stale_outbound.state, "stale_generation");
    assert_eq!(
        response_blocker(&stale_outbound),
        "helper_scheduler:meeting_generation_not_authoritative"
    );
    assert_eq!(
        stale_outbound.runtime_claim,
        "helper_scheduler_request_not_executed"
    );

    reset_bridge_runtime();
    let stale_incoming = send_helper_worker_task(
        "translate",
        json!({
            "text": "hello",
            "meeting_session_id": session_id,
            "meeting_lane": "incoming"
        }),
    );
    assert_eq!(stale_incoming.scheduler_priority, "meeting_incoming");
    assert_eq!(stale_incoming.state, "stale_session");
    assert_eq!(
        response_blocker(&stale_incoming),
        "helper_scheduler:incoming_meeting_session_not_eligible"
    );
    assert_eq!(
        stale_incoming.runtime_claim,
        "helper_scheduler_request_not_executed"
    );

    reset_bridge_runtime();
    let _ = clear_runtime_session_state();
}

#[test]
fn request_cleanup_preserves_replacement_and_clears_exact_identity() {
    let mut bridge = HelperBridgeRuntime::default();
    bridge.active_task = Some("translate".to_string());
    bridge.active_request_id = Some("request-new".to_string());
    bridge.active_meeting_generation = Some(44);
    bridge.active_meeting_session_id = Some("session-new".to_string());
    bridge.active_meeting_lane = Some("you".to_string());

    clear_active_request(&mut bridge, "request-old");
    assert_eq!(bridge.active_task.as_deref(), Some("translate"));
    assert_eq!(bridge.active_request_id.as_deref(), Some("request-new"));
    assert_eq!(bridge.active_meeting_generation, Some(44));
    assert_eq!(
        bridge.active_meeting_session_id.as_deref(),
        Some("session-new")
    );
    assert_eq!(bridge.active_meeting_lane.as_deref(), Some("you"));

    clear_active_request(&mut bridge, "request-new");
    assert!(bridge.active_task.is_none());
    assert!(bridge.active_request_id.is_none());
    assert!(bridge.active_meeting_generation.is_none());
    assert!(bridge.active_meeting_session_id.is_none());
    assert!(bridge.active_meeting_lane.is_none());
}

#[test]
fn helper_hard_cancel_is_session_scoped_and_cleans_matching_metadata() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    reset_bridge_runtime();
    {
        let mut bridge = runtime().lock().expect("helper bridge test lock");
        bridge.state = "ready".to_string();
        bridge.generation_token = 41;
        bridge.active_task = Some("translate".to_string());
        bridge.active_request_id = Some("request-live".to_string());
        bridge.active_meeting_generation = Some(77);
        bridge.active_meeting_session_id = Some("session-live".to_string());
        bridge.active_meeting_lane = Some("you".to_string());
    }

    let stale_cancel = cancel_helper_bridge_meeting_session("session-stale");
    assert!(stale_cancel.ok);
    {
        let bridge = runtime().lock().expect("helper bridge test lock");
        assert_eq!(bridge.generation_token, 41);
        assert_eq!(bridge.active_task.as_deref(), Some("translate"));
        assert_eq!(bridge.active_request_id.as_deref(), Some("request-live"));
        assert_eq!(bridge.active_meeting_generation, Some(77));
        assert_eq!(
            bridge.active_meeting_session_id.as_deref(),
            Some("session-live")
        );
        assert_eq!(bridge.active_meeting_lane.as_deref(), Some("you"));
    }

    let matching_cancel = cancel_helper_bridge_meeting_session("session-live");
    assert!(matching_cancel.ok);
    assert_eq!(matching_cancel.state, "stopped");
    {
        let bridge = runtime().lock().expect("helper bridge test lock");
        assert_eq!(bridge.generation_token, 42);
        assert!(bridge.active_task.is_none());
        assert!(bridge.active_request_id.is_none());
        assert!(bridge.active_meeting_generation.is_none());
        assert!(bridge.active_meeting_session_id.is_none());
        assert!(bridge.active_meeting_lane.is_none());
        assert_eq!(
            bridge.last_error.as_deref(),
            Some("helper_bridge:meeting_session_hard_cancelled")
        );
    }
    reset_bridge_runtime();
}


#[test]
fn exit_is_nonblocking_when_startup_owns_the_helper_mutex() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    reset_bridge_runtime();
    let held = runtime().lock().expect("test owns helper mutex");
    assert!(!shutdown_helper_bridge_for_app_exit());
    drop(held);
    reset_bridge_runtime();
}

#[test]
fn meeting_stop_defers_if_canonical_helper_mutex_is_busy_without_clearing_owner() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    reset_bridge_runtime();
    {
        let mut bridge = runtime().lock().expect("hold helper mutex during Stop");
        bridge.state = "ready".to_string();
        bridge.generation_token = 52;
        bridge.active_task = Some("translate".to_string());
        bridge.active_request_id = Some("meeting-request".to_string());
        bridge.active_meeting_session_id = Some("meeting-session-a".to_string());
        // A same-thread call would deadlock if cancellation used lock().
        let result = cancel_helper_bridge_meeting_session("meeting-session-a");
        assert!(!result.ok);
        assert_eq!(result.state, "helper_busy");
        assert_eq!(result.runtime_claim, "helper_cancel_deferred_owned_mutex");
        assert_eq!(result.generation_token, 0);
        assert_eq!(bridge.generation_token, 52);
        assert_eq!(bridge.active_request_id.as_deref(), Some("meeting-request"));
        assert_eq!(bridge.active_meeting_session_id.as_deref(), Some("meeting-session-a"));
        assert!(bridge.pending_meeting_cleanup_session_id.is_none());
    }
    let resumed = cancel_helper_bridge_meeting_session("meeting-session-a");
    assert!(resumed.ok);
    assert_eq!(resumed.state, "stopped");
    assert_eq!(resumed.generation_token, 53);
    reset_bridge_runtime();
}

#[test]
fn retry_stop_releases_only_exact_pending_meeting() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    reset_bridge_runtime();
    {
        let mut bridge = runtime().lock().unwrap();
        bridge.state = "blocked".into();
        bridge.generation_token = 72;
        bridge.last_error = Some("helper_bridge:cleanup_incomplete".into());
        bridge.pending_meeting_cleanup_session_id = Some("meeting-session-a".into());
    }
    let wrong = cancel_helper_bridge_meeting_session("meeting-session-b");
    assert!(!wrong.ok);
    assert_eq!(wrong.state, "cleanup_owner_unverified");
    assert_eq!(runtime().lock().unwrap().pending_meeting_cleanup_session_id.as_deref(), Some("meeting-session-a"));
    let right = cancel_helper_bridge_meeting_session("meeting-session-a");
    assert!(right.ok);
    assert!(runtime().lock().unwrap().pending_meeting_cleanup_session_id.is_none());
    reset_bridge_runtime();
}

#[test]
fn unattributed_incomplete_helper_cleanup_cannot_be_claimed_by_meeting_stop() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    reset_bridge_runtime();
    {
        let mut bridge = runtime().lock().expect("native helper mutex");
        bridge.generation_token = 92;
        bridge.state = "blocked".to_string();
        bridge.last_error = Some("helper_bridge:cleanup_incomplete".to_string());
        assert!(bridge.pending_meeting_cleanup_session_id.is_none());
    }
    let denied = cancel_helper_bridge_meeting_session("meeting-session-a");
    assert!(!denied.ok);
    assert_eq!(denied.state, "cleanup_owner_unverified");
    {
        let bridge = runtime().lock().expect("helper not stolen by Meeting A");
        assert_eq!(bridge.generation_token, 92);
        assert_eq!(bridge.last_error.as_deref(), Some("helper_bridge:cleanup_incomplete"));
        assert!(bridge.pending_meeting_cleanup_session_id.is_none());
    }
    reset_bridge_runtime();
}


#[test]
fn orphaned_native_helper_blocks_close_until_exact_orphan_cleanup() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    let _ = clear_runtime_session_state();
    reset_bridge_runtime();
    {
        let mut owner = runtime().lock().unwrap();
        owner.state = "blocked".into();
        owner.last_error = Some("helper_bridge:cleanup_incomplete".into());
        owner.pending_meeting_cleanup_session_id = Some("orphan-meeting".into());
    }
    assert!(meeting_helper_orphan_cleanup_pending());
    assert!(super::meeting_session::meeting_stop_cleanup_pending());
    assert!(!shutdown_helper_bridge_for_app_exit());
    assert!(finish_orphaned_meeting_helper_without_session().is_ok());
    assert!(!meeting_helper_orphan_cleanup_pending());
    assert!(shutdown_helper_bridge_for_app_exit());
    reset_bridge_runtime();
}

#[test]
fn no_snapshot_meeting_stop_does_not_terminate_standalone_worker() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    let _ = clear_runtime_session_state();
    reset_bridge_runtime();
    {
        let mut owner = runtime().lock().expect("text helper owner");
        owner.state = "ready".to_string();
        owner.generation_token = 31;
        owner.last_error = None;
    }
    assert!(!meeting_helper_orphan_cleanup_pending());
    assert!(finish_orphaned_meeting_helper_without_session().is_ok());
    let owner = runtime().lock().expect("standalone helper state untouched");
    assert_eq!(owner.state, "ready");
    assert_eq!(owner.generation_token, 31);
    drop(owner);
    reset_bridge_runtime();
}

#[test]
fn no_snapshot_orphan_cleanup_never_waits_for_busy_helper_lock() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    let _ = clear_runtime_session_state();
    reset_bridge_runtime();
    let owner = runtime().lock().expect("hold canonical helper mutex");
    assert_eq!(
        finish_orphaned_meeting_helper_without_session().unwrap_err(),
        "helper_orphan:owner_busy",
    );
    drop(owner);
    reset_bridge_runtime();
}

#[test]
fn public_worker_restart_cannot_erase_orphaned_meeting_attribution() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    let _ = clear_runtime_session_state();
    reset_bridge_runtime();
    {
        let mut owner = runtime().lock().expect("old Meeting cleanup owner");
        owner.state = "stopped".to_string();
        owner.generation_token = 24;
        owner.pending_meeting_cleanup_session_id = Some("old-meeting".to_string());
    }

    let denied = start_helper_bridge();
    assert!(!denied.ok);
    assert_eq!(denied.state, "cleanup_incomplete");
    {
        let owner = runtime().lock().expect("retained old helper owner");
        assert_eq!(owner.generation_token, 24);
        assert_eq!(owner.pending_meeting_cleanup_session_id.as_deref(), Some("old-meeting"));
    }
    assert!(finish_orphaned_meeting_helper_without_session().is_ok());
    reset_bridge_runtime();
}

#[test]
fn orphan_helper_cleanup_does_not_take_over_active_mic_test() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    let _ = clear_runtime_session_state();
    reset_bridge_runtime();
    let report = begin_mic_test_session();
    assert!(report.blocker.is_empty());
    let generation = report.snapshot.expect("mic test owner").generation;
    {
        let mut owner = runtime().lock().expect("meeting orphan pending");
        owner.pending_meeting_cleanup_session_id = Some("old-meeting".to_string());
    }
    assert_eq!(
        finish_orphaned_meeting_helper_without_session().unwrap_err(),
        "helper_orphan:runtime_session_not_verified_idle",
    );
    assert!(meeting_helper_orphan_cleanup_pending());
    assert_eq!(
        clear_runtime_session_if_generation(generation).blocker,
        "runtime_session:cleared",
    );
    assert!(finish_orphaned_meeting_helper_without_session().is_ok());
    reset_bridge_runtime();
}

#[test]
fn blocked_bridge_state_clears_all_active_request_metadata() {
    let mut bridge = HelperBridgeRuntime::default();
    bridge.active_task = Some("transcribe".to_string());
    bridge.active_request_id = Some("request-active".to_string());
    bridge.active_meeting_generation = Some(9);
    bridge.active_meeting_session_id = Some("session-active".to_string());
    bridge.active_meeting_lane = Some("incoming".to_string());

    let blocked = set_blocked(
        &mut bridge,
        "test helper bridge blocked state",
        "helper_bridge:test_blocked",
    );
    assert!(!blocked.ok);
    assert_eq!(blocked.state, "blocked");
    assert!(bridge.active_task.is_none());
    assert!(bridge.active_request_id.is_none());
    assert!(bridge.active_meeting_generation.is_none());
    assert!(bridge.active_meeting_session_id.is_none());
    assert!(bridge.active_meeting_lane.is_none());
    assert_eq!(bridge.last_error.as_deref(), Some("helper_bridge:test_blocked"));
}
