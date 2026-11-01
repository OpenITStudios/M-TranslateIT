//! Diagnostic-only Codex gateway foundation. This owner does not create a
//! realtime session, request inference, select a Meeting engine, or read audio.
//! The existing Local Meeting/voice worker remains untouched until a separately
//! verified audio-to-translated-text capability can be admitted.

mod compatibility;
mod correlation;
mod protocol;
mod realtime;
mod realtime_rpc;
mod transport;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;
use compatibility::DiagnosticCompatibility;

use crate::engine::runtime_state::latest_runtime_session_state;
use transport::AppServer;

const CLEANUP_SETTLEMENT_LIMIT: Duration = Duration::from_secs(3);
const CLEANUP_SETTLEMENT_SLICE: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Serialize)]
pub struct CodexGatewayStatus {
    pub state: &'static str,
    pub blocker: &'static str,
    pub process_owned: bool,
    pub chatgpt_authenticated: bool,
    pub included_usage_allowed: bool,
    pub app_server_version: Option<String>,
    pub diagnostic_protocol_compatible: bool,
    pub realtime_audio_verified: bool,
    pub meeting_route_enabled: bool,
    pub generation: u64,
    pub runtime_claim: &'static str,
}

impl CodexGatewayStatus {
    fn idle(generation: u64) -> Self {
        Self {
            state: "disconnected",
            blocker: "",
            process_owned: false,
            chatgpt_authenticated: false,
            included_usage_allowed: false,
            app_server_version: None,
            diagnostic_protocol_compatible: false,
            realtime_audio_verified: false,
            meeting_route_enabled: false,
            generation,
            runtime_claim: "codex_gateway:local_connection_only",
        }
    }

    fn blocked(generation: u64, blocker: &'static str) -> Self {
        Self {
            state: "blocked",
            blocker,
            ..Self::idle(generation)
        }
    }

    /// A cleanup failure is not an ordinary connection error. It remains
    /// latched even after the last owner thread terminates: no new Codex
    /// child may be started in this application lifetime.
    fn cleanup_incomplete(generation: u64, blocker: &'static str, process_owned: bool) -> Self {
        Self {
            state: "cleanup_incomplete",
            blocker,
            process_owned,
            generation,
            runtime_claim: "codex_gateway:cleanup_not_verified",
            ..Self::idle(generation)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum CleanupDisposition {
    Complete,
    Retry,
    Incomplete(&'static str),
}

/// One classification for both the actual child and synthetic P7 tests.
/// A latched join/child error is fatal immediately; only a healthy but still
/// settling owned thread may receive a bounded re-check.
fn cleanup_disposition(
    outcome: Result<bool, &'static str>,
    deadline_reached: bool,
) -> CleanupDisposition {
    match outcome {
        Ok(true) => CleanupDisposition::Complete,
        Ok(false) if !deadline_reached => CleanupDisposition::Retry,
        Ok(false) => CleanupDisposition::Incomplete("codex_gateway:owned_cleanup_deadline"),
        Err(reason) => CleanupDisposition::Incomplete(reason),
    }
}

struct GatewayOwner {
    generation: u64,
    status: CodexGatewayStatus,
    cancellation: Option<Arc<AtomicBool>>,
    thread: Option<JoinHandle<()>>,
}

impl Default for GatewayOwner {
    fn default() -> Self {
        Self {
            generation: 0,
            status: CodexGatewayStatus::idle(0),
            cancellation: None,
            thread: None,
        }
    }
}

static OWNER: OnceLock<Mutex<GatewayOwner>> = OnceLock::new();

fn owner() -> &'static Mutex<GatewayOwner> {
    OWNER.get_or_init(|| Mutex::new(GatewayOwner::default()))
}

fn reconcile(owner: &mut GatewayOwner) {
    if owner.thread.as_ref().is_some_and(JoinHandle::is_finished) {
        let finished = owner.thread.take().expect("completed handle present").join();
        owner.cancellation = None;
        if finished.is_err() {
            owner.status = CodexGatewayStatus::cleanup_incomplete(
                owner.generation,
                "codex_gateway:owner_thread_failed",
                false,
            );
        } else {
            // A joined owner is no longer holding its child handle, but a
            // prior cleanup failure remains a non-reconnectable uncertainty.
            owner.status.process_owned = false;
        }
    }
}

fn snapshot() -> CodexGatewayStatus {
    match owner().try_lock() {
        Ok(mut state) => {
            reconcile(&mut state);
            state.status.clone()
        }
        Err(_) => CodexGatewayStatus::blocked(0, "codex_gateway:owner_busy_or_unavailable"),
    }
}

fn update(generation: u64, status: CodexGatewayStatus, final_state: bool) {
    if let Ok(mut state) = owner().lock() {
        if state.generation != generation {
            return;
        }
        if !final_state && state.cancellation.as_ref()
            .is_some_and(|stop| stop.load(Ordering::Acquire))
        {
            return; // A late successful handshake cannot undo Stop.
        }
        state.status = status;
    }
}

fn refresh_account(
    generation: u64,
    server: &mut AppServer,
    cancellation: &AtomicBool,
    diagnostic: &DiagnosticCompatibility,
) -> Result<(), &'static str> {
    // A workspace/account change can race a response. At most one re-read
    // is allowed; never claim included usage from an unstable account pair.
    for _ in 0..2 {
        server.take_account_dirty();
        // Initialize alone does not acknowledge explicitGatewayOauth.
        // Prove the same owned server supports the read-only gateway status
        // method before account/rate-limit introspection, and recheck it after
        // account/workspace changes. No browser login or model call is sent.
        let gateway = server.request("account/gatewayOAuth/read", json!({}), cancellation)?;
        compatibility::verify_gateway_oauth_read(&gateway)?;
        let account = server.request("account/read", json!({}), cancellation)?;
        protocol::chatgpt_account(&account)?;
        let quota = server.request(
            "account/rateLimits/read",
            json!({"supportsLunaReserve": false, "excludeResetCreditDetails": true}),
            cancellation,
        )?;
        let included = protocol::ordinary_usage(&quota)?;
        if server.take_account_dirty() {
            continue;
        }
        update(generation, CodexGatewayStatus {
            state: match included {
                Some(true) => "authenticated",
                Some(false) => "quota_blocked",
                None => "usage_unverified",
            },
            blocker: match included {
                Some(true) => "",
                Some(false) => "codex_gateway:included_usage_unavailable",
                None => "codex_gateway:usage_permission_unknown",
            },
            process_owned: true,
            chatgpt_authenticated: true,
            included_usage_allowed: included == Some(true),
            app_server_version: Some(diagnostic.cli_version.clone()),
            diagnostic_protocol_compatible: true,
            // Login, protocol shape and ordinary usage are NOT Voice proof.
            realtime_audio_verified: false,
            meeting_route_enabled: false,
            generation,
            runtime_claim: "codex_gateway:account_probe_only",
        }, false);
        return Ok(());
    }
    Err("codex_gateway:account_changed_during_probe")
}

fn run_owned(generation: u64, cancellation: Arc<AtomicBool>) {
    let result = match AppServer::spawn() {
        Ok(mut server) => {
            let operation = (|| -> Result<(), &'static str> {
                let initialized = server.request(
                    "initialize",
                    json!({
                        "clientInfo": {
                            "name": "translateit",
                            "title": "TranslateIT",
                            "version": env!("CARGO_PKG_VERSION")
                        },
                        "capabilities": {
                            "experimentalApi": false,
                            "explicitGatewayOauth": true
                        }
                    }),
                    &cancellation,
                )?;
                // The upstream InitializeResponse has no serverVersion field.
                // Extract bounded released build information from userAgent
                // and check the documented platform/path response shape.
                let diagnostic = compatibility::verify_initialize(&initialized)?;
                server.initialized(&cancellation)?;

                // Verify explicit gateway login policy before account reads;
                // never assume an older server honored the initialize flag.
                refresh_account(generation, &mut server, &cancellation, &diagnostic)?;

                while !cancellation.load(Ordering::Acquire) {
                    if server.poll(&cancellation)? {
                        refresh_account(generation, &mut server, &cancellation, &diagnostic)?;
                    }
                }
                Ok(())
            })();

            // Keep the exact AppServer owner in this thread while teardown
            // is attempted. A permanently latched writer/reader join failure
            // must never cause an infinite retry loop. A bounded number of
            // healthy-but-not-yet-joined polls is allowed.
            let cleanup_deadline = Instant::now() + CLEANUP_SETTLEMENT_LIMIT;
            loop {
                let outcome = server.stop_and_reap();
                let expired = Instant::now() >= cleanup_deadline;
                match cleanup_disposition(outcome, expired) {
                    CleanupDisposition::Complete => break,
                    CleanupDisposition::Retry => {
                        update(
                            generation,
                            CodexGatewayStatus::cleanup_incomplete(
                                generation, "codex_gateway:owned_cleanup_pending", true,
                            ),
                            true,
                        );
                        thread::sleep(CLEANUP_SETTLEMENT_SLICE);
                    }
                    CleanupDisposition::Incomplete(reason) => {
                        // No optimistic Idle/Blocked transition after an
                        // unverified teardown, including when cancelled.
                        // The AppServer's Drop still owns its exact child
                        // handle and any unjoined read/write threads. A
                        // stalled OS handle may hold this owner thread here;
                        // Start and App Exit remain blocked in either case.
                        update(
                            generation,
                            CodexGatewayStatus::cleanup_incomplete(
                                generation, reason, true,
                            ),
                            true,
                        );
                        return;
                    }
                }
            }
            operation
        }
        Err(error) => Err(error),
    };

    let terminal = if cancellation.load(Ordering::Acquire) {
        CodexGatewayStatus::idle(generation)
    } else {
        CodexGatewayStatus::blocked(
            generation,
            result.err().unwrap_or("codex_gateway:unexpected_owner_exit"),
        )
    };
    update(generation, terminal, true);
}

fn busy_audio_owner() -> bool {
    let runtime = latest_runtime_session_state();
    runtime.has_active_session
        || runtime.snapshot.is_some()
        || super::meeting_session::meeting_stop_cleanup_pending()
}

fn start() -> CodexGatewayStatus {
    if !cfg!(windows) {
        return CodexGatewayStatus::blocked(0, "codex_gateway:windows_only");
    }
    // Gateway connection probing does not own microphone resources, but an
    // in-progress Meeting/Retry Stop must not be obscured by a new child.
    if busy_audio_owner() {
        return CodexGatewayStatus::blocked(0, "codex_gateway:meeting_active");
    }
    let Ok(mut state) = owner().try_lock() else {
        return CodexGatewayStatus::blocked(0, "codex_gateway:owner_busy_or_unavailable");
    };
    reconcile(&mut state);
    if state.status.state == "cleanup_incomplete" || state.thread.is_some() {
        // Terminal uncertainty cannot be cleared by Connect/Disconnect or
        // by a completed owner thread. No second Codex process is spawned.
        return state.status.clone();
    }
    let Some(generation) = state.generation.checked_add(1) else {
        return CodexGatewayStatus::blocked(0, "codex_gateway:generation_exhausted");
    };
    let cancellation = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&cancellation);
    state.generation = generation;
    state.status = CodexGatewayStatus {
        state: "connecting",
        blocker: "",
        process_owned: false,
        generation,
        ..CodexGatewayStatus::idle(generation)
    };
    let worker = thread::Builder::new()
        .name("translateit-codex-owner".to_string())
        .spawn(move || run_owned(generation, worker_stop));
    match worker {
        Ok(worker) => {
            state.thread = Some(worker);
            state.cancellation = Some(cancellation);
        }
        Err(_) => {
            state.status = CodexGatewayStatus::blocked(
                generation, "codex_gateway:owner_spawn_failed",
            );
        }
    }
    state.status.clone()
}

fn request_stop() -> CodexGatewayStatus {
    let Ok(mut state) = owner().lock() else {
        return CodexGatewayStatus::blocked(0, "codex_gateway:owner_busy_or_unavailable");
    };
    reconcile(&mut state);
    if state.status.state == "cleanup_incomplete" {
        return state.status.clone();
    }
    if let Some(stop) = &state.cancellation {
        stop.store(true, Ordering::Release);
        state.status = CodexGatewayStatus {
            state: "stopping",
            blocker: "",
            process_owned: true,
            generation: state.generation,
            ..CodexGatewayStatus::idle(state.generation)
        };
    }
    state.status.clone()
}

fn wait_for_settlement(deadline: Duration, expected_generation: u64) -> CodexGatewayStatus {
    let until = Instant::now() + deadline;
    loop {
        let status = snapshot();
        if status.generation != expected_generation
            || !matches!(status.state, "connecting" | "stopping")
            || Instant::now() >= until
        {
            return status;
        }
        thread::sleep(Duration::from_millis(40));
    }
}

#[tauri::command]
pub fn get_codex_gateway_status() -> CodexGatewayStatus {
    snapshot()
}

#[tauri::command]
pub async fn connect_codex_gateway() -> CodexGatewayStatus {
    tauri::async_runtime::spawn_blocking(|| {
        let status = start();
        if status.state != "connecting" {
            return status;
        }
        wait_for_settlement(Duration::from_secs(25), status.generation)
    })
    .await
    .unwrap_or_else(|_| CodexGatewayStatus::blocked(0, "codex_gateway:connect_task_failed"))
}

#[tauri::command]
pub async fn disconnect_codex_gateway() -> CodexGatewayStatus {
    tauri::async_runtime::spawn_blocking(|| {
        let status = request_stop();
        if status.state != "stopping" {
            return status;
        }
        wait_for_settlement(Duration::from_secs(3), status.generation)
    })
    .await
    .unwrap_or_else(|_| CodexGatewayStatus::blocked(0, "codex_gateway:disconnect_task_failed"))
}

/// Called only after native Meeting resources have been released. Unconfirmed
/// child/reader cleanup blocks a clean-shutdown marker; it never kills Codex
/// CLI processes which belong to another application.
pub fn shutdown_for_app_exit() -> bool {
    let status = request_stop();
    if status.state == "blocked" && status.blocker == "codex_gateway:owner_busy_or_unavailable" {
        return false;
    }
    let final_status = wait_for_settlement(Duration::from_secs(2), status.generation);
    matches!(final_status.state, "disconnected" | "blocked")
        && !final_status.process_owned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_never_claims_voice_or_meeting_readiness_from_login() {
        for state in ["authenticated", "quota_blocked", "usage_unverified"] {
            let candidate = CodexGatewayStatus {
                state,
                blocker: "",
                process_owned: true,
                chatgpt_authenticated: true,
                included_usage_allowed: true,
                generation: 7,
                ..CodexGatewayStatus::idle(7)
            };
            assert!(!candidate.realtime_audio_verified);
            assert!(!candidate.meeting_route_enabled);
        }
    }

    #[test]
    fn source_derived_realtime_items_produce_no_authorized_tts_result() {
        use super::correlation::{CorrelationGate, SourceWindow};
        use super::realtime::RealtimeObserver;

        let mut upstream = RealtimeObserver::new("thread_p3").unwrap();
        upstream.observe(
            "thread/realtime/started",
            &serde_json::json!({
                "threadId": "thread_p3",
                "realtimeSessionId": "voice_p3",
                "version": "v3"
            }),
        ).unwrap();
        let mut evidence = CorrelationGate::default();
        evidence.begin(SourceWindow {
            generation: 10,
            source_epoch: 4,
            sequence: 9,
            start_sample: 48_000,
            end_sample: 72_000,
        }).unwrap();
        let source = upstream.observe(
            "thread/realtime/item/completed",
            &serde_json::json!({
                "threadId": "thread_p3",
                "item": {"id": "in_9", "realtimeSessionId": "voice_p3",
                         "type": "transcriptSegment",
                         "role": "user", "text": "Berapa harga sekarang?"}
            }),
        ).unwrap();
        assert!(evidence.observe(source).unwrap().is_none());
        let assistant = upstream.observe(
            "thread/realtime/item/completed",
            &serde_json::json!({
                "threadId": "thread_p3",
                "item": {"id": "out_9", "realtimeSessionId": "voice_p3",
                         "type": "transcriptSegment",
                         "role": "assistant", "text": "What is the current price?"}
            }),
        ).unwrap();
        let pair = evidence.observe(assistant).unwrap().unwrap();
        assert_eq!(pair.source.sequence, 9);
        assert_eq!(pair.source_item_id, "in_9");
        assert_eq!(pair.assistant_item_id, "out_9");
        // One-off proximity is not an upstream-corroborated causal link:
        // no TTS/Meeting commit method accepts this UnverifiedPair.
        assert!(evidence.is_retired());
        assert!(!CodexGatewayStatus::idle(10).meeting_route_enabled);
    }

    #[test]
    fn offline_realtime_rpc_is_not_admitted_by_account_probe_transport() {
        use super::realtime_rpc::{RealtimeRpc, Phase};
        use std::sync::atomic::AtomicBool;

        let mut wire = RealtimeRpc::new("thread_test", "voice_test", 9, 2).unwrap();
        let frame = wire.start(
            10,
            "v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n",
            &AtomicBool::new(false),
        ).unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&frame.bytes).unwrap();
        // P1 has no path to send this experimental request. No Voice RPC,
        // cloud inference, billing or Meeting source can be activated.
        assert_eq!(
            protocol::request(frame.id, frame.method, payload["params"].clone()),
            Err("codex_gateway:request_not_allowed")
        );
        assert_eq!(wire.phase(), Phase::StartPending);
        assert!(!CodexGatewayStatus::idle(9).meeting_route_enabled);
    }

    #[test]
    fn cleanup_disposition_is_bounded_and_fatal_errors_are_not_retried() {
        assert_eq!(cleanup_disposition(Ok(true), false), CleanupDisposition::Complete);
        assert_eq!(cleanup_disposition(Ok(false), false), CleanupDisposition::Retry);
        assert_eq!(
            cleanup_disposition(Ok(false), true),
            CleanupDisposition::Incomplete("codex_gateway:owned_cleanup_deadline")
        );
        assert_eq!(
            cleanup_disposition(Err("codex_gateway:writer_join_failed"), false),
            CleanupDisposition::Incomplete("codex_gateway:writer_join_failed")
        );
        assert_eq!(
            cleanup_disposition(Err("codex_gateway:child_reap_failed"), false),
            CleanupDisposition::Incomplete("codex_gateway:child_reap_failed")
        );
    }

    #[test]
    fn finished_owner_cannot_clear_a_latched_cleanup_failure() {
        let worker = thread::spawn(|| {});
        while !worker.is_finished() {
            thread::yield_now();
        }
        let mut state = GatewayOwner {
            generation: 21,
            status: CodexGatewayStatus::cleanup_incomplete(
                21, "codex_gateway:writer_join_failed", true,
            ),
            cancellation: Some(Arc::new(AtomicBool::new(true))),
            thread: Some(worker),
        };
        reconcile(&mut state);
        assert_eq!(state.status.state, "cleanup_incomplete");
        assert_eq!(state.status.blocker, "codex_gateway:writer_join_failed");
        assert_eq!(state.status.generation, 21);
        assert!(!state.status.process_owned);
        assert!(state.thread.is_none());
        assert!(state.cancellation.is_none());
    }

    #[test]
    fn unresolved_cleanup_remains_distinct_from_a_clean_disconnection() {
        let awaiting = CodexGatewayStatus::cleanup_incomplete(
            13, "codex_gateway:owned_cleanup_pending", true
        );
        assert_eq!(awaiting.state, "cleanup_incomplete");
        assert!(awaiting.process_owned);
        assert!(!awaiting.realtime_audio_verified);
        assert!(!awaiting.meeting_route_enabled);
        let joined_but_unverified = CodexGatewayStatus::cleanup_incomplete(
            13, "codex_gateway:writer_join_failed", false
        );
        assert_eq!(joined_but_unverified.state, "cleanup_incomplete");
        assert!(!joined_but_unverified.process_owned);
        assert_ne!(joined_but_unverified.state, "disconnected");
        assert_eq!(
            joined_but_unverified.runtime_claim,
            "codex_gateway:cleanup_not_verified"
        );
    }

    #[test]
    fn versioned_diagnostic_status_never_enables_voice_or_meeting() {
        let diagnostic = CodexGatewayStatus {
            state: "authenticated",
            blocker: "",
            process_owned: true,
            chatgpt_authenticated: true,
            included_usage_allowed: true,
            app_server_version: Some("0.110.0".to_string()),
            diagnostic_protocol_compatible: true,
            generation: 14,
            ..CodexGatewayStatus::idle(14)
        };
        assert!(!diagnostic.realtime_audio_verified);
        assert!(!diagnostic.meeting_route_enabled);
        assert_eq!(diagnostic.app_server_version.as_deref(), Some("0.110.0"));
        assert!(!CodexGatewayStatus::idle(14).diagnostic_protocol_compatible);
    }

    #[test]
    fn unknown_or_rejected_probe_never_enables_inference() {
        let status = CodexGatewayStatus::blocked(4, "codex_gateway:non_chatgpt_provider");
        assert!(!status.chatgpt_authenticated);
        assert!(!status.included_usage_allowed);
        assert!(!status.meeting_route_enabled);
        assert_eq!(status.generation, 4);
    }
}
