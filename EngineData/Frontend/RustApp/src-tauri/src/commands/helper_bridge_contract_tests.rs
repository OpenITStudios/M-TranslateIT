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
    clear_active_request, runtime, set_blocked, stop_child, validate_worker_response_contract,
    HelperBridgeRuntime,
};

fn reset_bridge_runtime() {
    if let Ok(mut bridge) = runtime().lock() {
        assert!(stop_child(&mut bridge), "tests must not drop an unjoined helper owner");
        *bridge = HelperBridgeRuntime::default();
    }
}

fn response_blocker(response: &HelperBridgeWorkerResponse) -> String {
    serde_json::from_str::<Value>(&response.worker_response_json)
        .ok()
        .and_then(|value| {
            value
                .get("blocker")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
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
    assert_eq!(response_blocker(&text), "helper_bridge:not_running");

    reset_bridge_runtime();
    let diagnostic = send_helper_worker_task("status", json!({}));
    assert_eq!(diagnostic.scheduler_priority, "diagnostic");
    assert_eq!(response_blocker(&diagnostic), "helper_bridge:not_running");

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
    assert_eq!(response_blocker(&start_prepare), "helper_bridge:not_running");

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
    assert_eq!(response_blocker(&outbound), "helper_bridge:not_running");

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
    assert_eq!(response_blocker(&incoming), "helper_bridge:not_running");

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
fn incomplete_stderr_cleanup_prevents_clean_exit_until_retry_finishes() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    reset_bridge_runtime();

    let (sender, receiver) = std::sync::mpsc::channel::<()>();
    {
        let mut bridge = runtime().lock().expect("helper mutex");
        bridge.state = "ready".to_string();
        bridge.stderr_logger = Some(std::thread::spawn(move || {
            let _ = receiver.recv();
        }));
    }

    assert!(!shutdown_helper_bridge_for_app_exit());
    {
        let bridge = runtime().lock().expect("helper mutex after failed exit");
        assert_eq!(bridge.state, "blocked");
        assert_eq!(bridge.last_error.as_deref(), Some("helper_bridge:cleanup_incomplete"));
        assert!(bridge.stderr_logger.is_some(), "live stderr owner must be retained");
    }

    sender.send(()).expect("release stderr reader");
    assert!(shutdown_helper_bridge_for_app_exit());
    {
        let bridge = runtime().lock().expect("helper mutex after clean exit");
        assert_eq!(bridge.state, "stopped");
        assert!(bridge.child.is_none());
        assert!(bridge.stderr_logger.is_none());
        assert!(bridge.last_error.is_none());
    }
    reset_bridge_runtime();
}

#[test]
fn meeting_retry_stop_cannot_claim_success_until_stderr_owner_is_reaped() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    reset_bridge_runtime();

    let (sender, receiver) = std::sync::mpsc::channel::<()>();
    {
        let mut bridge = runtime().lock().expect("helper mutex");
        bridge.state = "ready".to_string();
        bridge.active_meeting_session_id = Some("test-session".to_string());
        bridge.active_task = Some("translate".to_string());
        bridge.stderr_logger = Some(std::thread::spawn(move || {
            let _ = receiver.recv();
        }));
    }
    let incomplete = cancel_helper_bridge_meeting_session("test-session");
    assert!(!incomplete.ok);
    assert_eq!(incomplete.state, "blocked");
    {
        let bridge = runtime().lock().expect("helper mutex");
        assert_eq!(bridge.last_error.as_deref(), Some("helper_bridge:cleanup_incomplete"));
        assert!(bridge.stderr_logger.is_some());
    }

    sender.send(()).expect("release stderr reader");
    let resumed = cancel_helper_bridge_meeting_session("test-session");
    assert!(resumed.ok);
    assert_eq!(resumed.state, "stopped");
    {
        let bridge = runtime().lock().expect("helper mutex");
        assert!(bridge.stderr_logger.is_none());
        assert!(bridge.child.is_none());
    }
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
fn retry_stop_reaps_only_the_exact_session_that_owned_incomplete_cleanup() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    reset_bridge_runtime();
    let (release, gate) = std::sync::mpsc::channel::<()>();
    {
        let mut bridge = runtime().lock().expect("helper mutex");
        bridge.state = "ready".to_string();
        bridge.generation_token = 71;
        bridge.active_task = Some("voice_actor_synthesize".to_string());
        bridge.active_request_id = Some("meeting-a-request".to_string());
        bridge.active_meeting_session_id = Some("meeting-session-a".to_string());
        bridge.stderr_logger = Some(std::thread::spawn(move || {
            let _ = gate.recv();
        }));
    }
    let first = cancel_helper_bridge_meeting_session("meeting-session-a");
    assert!(!first.ok);
    assert_eq!(first.state, "blocked");
    {
        let bridge = runtime().lock().expect("helper after incomplete Stop");
        assert_eq!(bridge.pending_meeting_cleanup_session_id.as_deref(), Some("meeting-session-a"));
        assert!(bridge.active_meeting_session_id.is_none());
        assert!(bridge.stderr_logger.is_some());
        assert_eq!(bridge.last_error.as_deref(), Some("helper_bridge:cleanup_incomplete"));
    }

    let wrong = cancel_helper_bridge_meeting_session("meeting-session-b");
    assert!(!wrong.ok);
    assert_eq!(wrong.state, "cleanup_owner_unverified");
    assert_eq!(wrong.runtime_claim, "helper_cancel_rejected_unattributed_or_other_session_cleanup");
    {
        let bridge = runtime().lock().expect("helper still owned by session A");
        assert!(bridge.stderr_logger.is_some(), "wrong session must not reap A's reader");
        assert_eq!(bridge.pending_meeting_cleanup_session_id.as_deref(), Some("meeting-session-a"));
        assert_eq!(bridge.generation_token, 72);
    }

    release.send(()).expect("release matching reader");
    let right = cancel_helper_bridge_meeting_session("meeting-session-a");
    assert!(right.ok);
    assert_eq!(right.state, "stopped");
    {
        let bridge = runtime().lock().expect("helper after successful retry");
        assert!(bridge.stderr_logger.is_none());
        assert!(bridge.pending_meeting_cleanup_session_id.is_none());
        assert!(bridge.active_meeting_session_id.is_none());
        assert_eq!(bridge.last_error.as_deref(), Some("helper_bridge:cleanup_completed"));
    }
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
fn orphaned_meeting_helper_blocks_native_close_until_no_snapshot_retry_stop() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    let _ = clear_runtime_session_state();
    reset_bridge_runtime();

    let (release, gate) = std::sync::mpsc::channel::<()>();
    {
        let mut owner = runtime().lock().expect("orphan helper owner");
        owner.state = "blocked".to_string();
        owner.last_error = Some("helper_bridge:cleanup_incomplete".to_string());
        owner.pending_meeting_cleanup_session_id = Some("orphan-meeting".to_string());
        owner.stderr_logger = Some(std::thread::spawn(move || {
            let _ = gate.recv();
        }));
    }

    assert!(meeting_helper_orphan_cleanup_pending());
    assert!(super::meeting_session::meeting_stop_cleanup_pending());
    assert!(!shutdown_helper_bridge_for_app_exit(),
        "native ExitRequested may not certify Meeting audio cleanup on a helper reap");
    assert_eq!(
        finish_orphaned_meeting_helper_without_session().unwrap_err(),
        "helper_orphan:cleanup_incomplete",
    );
    {
        let owner = runtime().lock().expect("retained owner after failed cleanup");
        assert_eq!(owner.pending_meeting_cleanup_session_id.as_deref(), Some("orphan-meeting"));
        assert!(owner.stderr_logger.is_some());
    }

    release.send(()).expect("finish orphan stderr reader");
    assert!(finish_orphaned_meeting_helper_without_session().is_ok());
    assert!(!meeting_helper_orphan_cleanup_pending());
    {
        let owner = runtime().lock().expect("verified orphan helper release");
        assert!(owner.stderr_logger.is_none());
        assert!(owner.child.is_none());
        assert!(owner.pending_meeting_cleanup_session_id.is_none());
        assert_eq!(owner.last_error.as_deref(),
            Some("helper_bridge:orphan_meeting_cleanup_verified"));
    }
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
        assert!(owner.child.is_none());
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

#[test]
fn worker_response_identity_and_stage_match_the_requested_task() {
    assert!(validate_worker_response_contract(
        "ping",
        "helper-17",
        &json!({"ok": true, "stage": "ping", "request_id": "helper-17"}),
    ).is_ok());
    assert!(validate_worker_response_contract(
        "status",
        "helper-18",
        &json!({"ok": true, "stage": "local_realtime_worker_preflight", "request_id": "helper-18"}),
    ).is_ok());
    assert!(validate_worker_response_contract(
        "translate",
        "helper-19",
        &json!({"ok": false, "stage": "worker_error", "request_id": "helper-19"}),
    ).is_ok());
    assert!(validate_worker_response_contract(
        "status",
        "helper-20",
        &json!({"ok": false, "stage": "status", "request_id": "helper-20"}),
    ).is_ok());
}

#[test]
fn worker_response_mismatched_identity_and_stage_fail_closed() {
    for response in [
        json!({"ok": true, "stage": "ping"}),
        json!({"ok": true, "stage": "ping", "request_id": "helper-old"}),
    ] {
        assert_eq!(
            validate_worker_response_contract("ping", "helper-current", &response),
            Err("worker:response_request_id_mismatch".to_string()),
        );
    }
    assert_eq!(
        validate_worker_response_contract(
            "ping", "helper-current",
            &json!({"ok": true, "stage": "translate", "request_id": "helper-current"}),
        ),
        Err("worker:response_stage_mismatch".to_string()),
    );
    assert_eq!(
        validate_worker_response_contract(
            "ping", "helper-current",
            &json!({"ok": true, "stage": "worker_error", "request_id": "helper-current"}),
        ),
        Err("worker:response_stage_mismatch".to_string()),
    );
    assert_eq!(
        validate_worker_response_contract(
            "ping", "helper-current",
            &json!({"stage": "ping", "request_id": "helper-current"}),
        ),
        Err("worker:response_ok_must_be_bool".to_string()),
    );
    assert_eq!(
        validate_worker_response_contract(
            "ping", "helper-current",
            &json!(["not", "an", "object"]),
        ),
        Err("worker:response_must_be_object".to_string()),
    );
}
