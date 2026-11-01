//! Diagnostic-only Codex gateway foundation. This owner does not create a
//! realtime session, request inference, select a Meeting engine, or read audio.
//! The existing Local Meeting/voice worker remains untouched until a separately
//! verified audio-to-translated-text capability can be admitted.

mod protocol;
mod realtime;
mod transport;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;

use crate::engine::runtime_state::latest_runtime_session_state;
use transport::AppServer;

#[derive(Debug, Clone, Serialize)]
pub struct CodexGatewayStatus {
    pub state: &'static str,
    pub blocker: &'static str,
    pub process_owned: bool,
    pub chatgpt_authenticated: bool,
    pub included_usage_allowed: bool,
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
            owner.status = CodexGatewayStatus::blocked(
                owner.generation, "codex_gateway:owner_thread_failed",
            );
        } else {
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
) -> Result<(), &'static str> {
    // A workspace/account change can race a response. At most one re-read
    // is allowed; never claim included usage from an unstable account pair.
    for _ in 0..2 {
        server.take_account_dirty();
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
            // Login and quota are not proof of ChatGPT Voice entitlement.
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
                if !initialized.is_object() {
                    return Err("codex_gateway:initialize_invalid");
                }
                server.initialized(&cancellation)?;

                // Account introspection only; no browser login or AI turn.
                refresh_account(generation, &mut server, &cancellation)?;

                while !cancellation.load(Ordering::Acquire) {
                    if server.poll(&cancellation)? {
                        refresh_account(generation, &mut server, &cancellation)?;
                    }
                }
                Ok(())
            })();

            // Do not lose an owned process/stdout reader after a Stop or
            // failure. This thread remains the owner until cleanup is proven.
            loop {
                match server.stop_and_reap() {
                    Ok(true) => break,
                    Ok(false) | Err(_) => {
                        update(generation, CodexGatewayStatus {
                            state: "cleanup_incomplete",
                            blocker: "codex_gateway:owned_cleanup_pending",
                            process_owned: true,
                            generation,
                            ..CodexGatewayStatus::idle(generation)
                        }, true);
                        thread::sleep(Duration::from_millis(100));
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
    if state.thread.is_some() {
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
    fn unknown_or_rejected_probe_never_enables_inference() {
        let status = CodexGatewayStatus::blocked(4, "codex_gateway:non_chatgpt_provider");
        assert!(!status.chatgpt_authenticated);
        assert!(!status.included_usage_allowed);
        assert!(!status.meeting_route_enabled);
        assert_eq!(status.generation, 4);
    }
}
