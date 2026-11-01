//! Test-only in-memory Codex App Server fixture for the P1 account gateway.
//!
//! Exercises the production JSONL reader/writer and RPC wait/dispatch helpers.
//! No codex.exe, sockets, credentials, real audio, token usage or Windows
//! microphone is used. Run only in the later authorized integrated test phase.

use super::{
    account_notification, cleanup_proven, read_jsonl, recv_expected_response,
    recv_idle_notification, submit_with_receipt, write_jsonl, WriteCommand,
    WireEvent, EVENT_QUEUE_CAPACITY, WRITE_QUEUE_CAPACITY,
};
use super::super::protocol::{self, Inbound};
use super::super::compatibility;
use super::super::{cleanup_disposition, CleanupDisposition, CodexGatewayStatus};

use serde_json::{json, Value};
use std::io::{self, Cursor, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const FIXTURE_DEADLINE: Duration = Duration::from_secs(3);

struct ScriptedStdin {
    frames: SyncSender<Vec<u8>>,
}

impl Write for ScriptedStdin {
    fn write(&mut self, source: &[u8]) -> io::Result<usize> {
        self.frames.send(source.to_vec())
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "test reader closed"))?;
        Ok(source.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

struct ScriptedStdout {
    chunks: Receiver<Vec<u8>>,
    current: Cursor<Vec<u8>>,
}

impl Read for ScriptedStdout {
    fn read(&mut self, destination: &mut [u8]) -> io::Result<usize> {
        loop {
            if (self.current.position() as usize) < self.current.get_ref().len() {
                return self.current.read(destination);
            }
            match self.chunks.recv() {
                Ok(chunk) => self.current = Cursor::new(chunk),
                Err(_) => return Ok(0),
            }
        }
    }
}

/// The test owns the faux upstream reply/event scripts. Its send order is
/// deliberately deterministic. The fake never executes a model request.
struct FakeCodexAppServer {
    outgoing_requests: Receiver<Vec<u8>>,
    responses: SyncSender<Vec<u8>>,
}

impl FakeCodexAppServer {
    fn next_request(&self) -> Value {
        let bytes = self.outgoing_requests.recv_timeout(FIXTURE_DEADLINE)
            .expect("fake did not receive a bounded outbound RPC");
        serde_json::from_slice(&bytes).expect("outgoing JSONL must be valid")
    }

    fn send_bytes(&self, bytes: &[u8]) {
        self.responses.send(bytes.to_vec())
            .expect("fake response channel must be available");
    }

    fn send_response(&self, id: u64, result: Value) {
        let mut frame = serde_json::to_vec(&json!({
            "id": id,
            "result": result,
        })).unwrap();
        frame.push(b'\n');
        self.send_bytes(&frame);
    }

    fn send_rpc_method_not_found(&self, id: u64) {
        let mut frame = serde_json::to_vec(&json!({
            "id": id,
            "error": {
                "code": -32601,
                "message": "account gateway read method unavailable"
            }
        })).unwrap();
        frame.push(b'\n');
        self.send_bytes(&frame);
    }

    fn send_notification(&self, method: &str) {
        let mut frame = serde_json::to_vec(&json!({
            "method": method,
            "params": {},
        })).unwrap();
        frame.push(b'\n');
        self.send_bytes(&frame);
    }
}

/// A fake control client using the REAL production writer receipt and response
/// dispatch functions. It does not construct an application AppServer child.
struct FakeControl {
    writer: Option<SyncSender<WriteCommand>>,
    replies: Receiver<WireEvent>,
    overflow: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    writer_thread: Option<JoinHandle<()>>,
    reader_thread: Option<JoinHandle<()>>,
    request_id: u64,
    account_dirty: bool,
    account_revision: Arc<AtomicU64>,
}

impl FakeControl {
    fn request(
        &mut self,
        method: &str,
        cancelled: &AtomicBool,
        deadline: Instant,
    ) -> Result<Value, &'static str> {
        self.request_id = self.request_id.checked_add(1)
            .ok_or("codex_gateway:request_id_exhausted")?;
        let frame = protocol::request(self.request_id, method, json!({}))?;
        let writer = self.writer.as_ref()
            .ok_or("codex_gateway:writer_unavailable")?.clone();
        let check = || {
            if cancelled.load(Ordering::Acquire) {
                Err("codex_gateway:cancelled")
            } else if self.overflow.load(Ordering::Acquire) {
                Err("codex_gateway:event_queue_overflow")
            } else if self.failed.load(Ordering::Acquire) {
                Err("codex_gateway:writer_failed")
            } else {
                Ok(())
            }
        };
        submit_with_receipt(&writer, frame, deadline, check)?;
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err("codex_gateway:cancelled");
            }
            if self.overflow.load(Ordering::Acquire) {
                return Err("codex_gateway:event_queue_overflow");
            }
            if let Some(result) = recv_expected_response(
                &self.replies,
                self.request_id,
                deadline,
                cancelled,
                &mut self.account_dirty,
            )? {
                return Ok(result);
            }
        }
    }

    fn poll(&mut self, cancelled: &AtomicBool) -> Result<bool, &'static str> {
        let dirty = recv_idle_notification(&self.replies, cancelled)?;
        self.account_dirty |= dirty;
        Ok(dirty)
    }

    fn shutdown(mut self, fake: FakeCodexAppServer) -> bool {
        self.writer.take();
        // This closes the fake stdout and breaks pending input writes.
        drop(fake);
        let writer_ok = self.writer_thread.take()
            .expect("one owned test writer").join().is_ok();
        let reader_ok = self.reader_thread.take()
            .expect("one owned test reader").join().is_ok();
        cleanup_proven(true, writer_ok, reader_ok, None)
    }
}

fn fixture() -> (FakeCodexAppServer, FakeControl) {
    let (outbound_tx, outbound_rx) = mpsc::sync_channel(1);
    let (inbound_tx, inbound_rx) = mpsc::sync_channel(2);
    let (requests, writer_rx) = mpsc::sync_channel(WRITE_QUEUE_CAPACITY);
    let (event_tx, event_rx) = mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
    let failed = Arc::new(AtomicBool::new(false));
    let overflow = Arc::new(AtomicBool::new(false));
    let writer_failed = Arc::clone(&failed);
    let reader_overflow = Arc::clone(&overflow);
    let account_revision = Arc::new(AtomicU64::new(0));
    let reader_revision = Arc::clone(&account_revision);

    let writer_thread = thread::Builder::new()
        .name("translateit-fake-codex-writer".into())
        .spawn(move || {
            write_jsonl(
                ScriptedStdin { frames: outbound_tx },
                writer_rx,
                writer_failed,
            )
        }).expect("fake writer thread available");

    let reader_thread = thread::Builder::new()
        .name("translateit-fake-codex-reader".into())
        .spawn(move || {
            read_jsonl(
                ScriptedStdout {
                    chunks: inbound_rx,
                    current: Cursor::new(Vec::new()),
                },
                event_tx,
                reader_overflow,
                reader_revision,
            )
        }).expect("fake reader thread available");

    (
        FakeCodexAppServer {
            outgoing_requests: outbound_rx,
            responses: inbound_tx,
        },
        FakeControl {
            writer: Some(requests),
            replies: event_rx,
            failed,
            overflow,
            writer_thread: Some(writer_thread),
            reader_thread: Some(reader_thread),
            request_id: 0,
            account_dirty: false,
            account_revision,
        },
    )
}

#[test]
fn scripted_account_notification_before_response_invalidates_same_rpc_probe() {
    let (server, mut client) = fixture();
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancelled);
    let worker = thread::spawn(move || {
        let reply = client.request(
            "account/read", &worker_cancel, Instant::now() + FIXTURE_DEADLINE,
        );
        (client, reply)
    });

    let request = server.next_request();
    assert_eq!(request["method"], "account/read");
    let id = request["id"].as_u64().unwrap();
    server.send_notification("account/updated");
    server.send_response(id, json!({"account":{"type":"chatgpt"}}));

    let (client, reply) = worker.join().unwrap();
    assert_eq!(reply.unwrap()["account"]["type"], "chatgpt");
    assert!(client.account_dirty);
    assert!(client.shutdown(server));
}

#[test]
fn scripted_jsonl_fragments_keep_exact_request_identity() {
    let (server, mut client) = fixture();
    let worker = thread::spawn(move || {
        let reply = client.request(
            "account/read", &AtomicBool::new(false), Instant::now() + FIXTURE_DEADLINE,
        );
        (client, reply)
    });
    let outbound = server.next_request();
    let id = outbound["id"].as_u64().unwrap();
    let mut bytes = serde_json::to_vec(&json!({
        "id": id, "result": {"account": {"type": "chatgpt"}}
    })).unwrap();
    bytes.extend_from_slice(b"\r\n");
    server.send_bytes(&bytes[..9]);
    server.send_bytes(&bytes[9..bytes.len() - 2]);
    server.send_bytes(&bytes[bytes.len() - 2..]);

    let (client, reply) = worker.join().unwrap();
    assert_eq!(reply.unwrap()["account"]["type"], "chatgpt");
    assert!(client.shutdown(server));
}

#[test]
fn scripted_wrong_reply_id_is_not_reused_by_another_request() {
    let (server, mut client) = fixture();
    let worker = thread::spawn(move || {
        let reply = client.request(
            "account/read", &AtomicBool::new(false), Instant::now() + FIXTURE_DEADLINE,
        );
        (client, reply)
    });
    let outbound = server.next_request();
    let correct = outbound["id"].as_u64().unwrap();
    server.send_response(correct + 1, json!({"account":{"type":"chatgpt"}}));

    let (client, reply) = worker.join().unwrap();
    assert_eq!(reply, Err("codex_gateway:unexpected_response_id"));
    assert!(client.shutdown(server));
}

#[test]
fn scripted_cancellation_rejects_a_late_success_and_does_not_replay() {
    let (server, mut client) = fixture();
    let cancellation = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancellation);
    let worker = thread::spawn(move || {
        let reply = client.request(
            "account/read", &worker_cancel, Instant::now() + FIXTURE_DEADLINE,
        );
        (client, reply)
    });
    let outbound = server.next_request();
    cancellation.store(true, Ordering::Release);
    server.send_response(
        outbound["id"].as_u64().unwrap(),
        json!({"account":{"type":"chatgpt"}}),
    );
    let (client, reply) = worker.join().unwrap();
    assert_eq!(reply, Err("codex_gateway:cancelled"));
    assert_eq!(client.request_id, 1, "no implicit request retry after cancellation");
    assert!(client.shutdown(server));
}

#[test]
fn unsolicited_realtime_notification_is_rejected_by_the_real_idle_dispatch() {
    let (server, mut client) = fixture();
    server.send_notification("thread/realtime/started");
    let mut observation = None;
    for _ in 0..10 {
        match client.poll(&AtomicBool::new(false)) {
            Ok(false) => {}
            result => {
                observation = Some(result);
                break;
            }
        }
    }
    assert_eq!(
        observation,
        Some(Err("codex_realtime:unsolicited_without_voice_authority")),
    );
    assert!(client.shutdown(server));
}

#[test]
fn scripted_reply_after_deadline_cannot_complete_the_next_rpc() {
    let (server, mut client) = fixture();
    let worker = thread::spawn(move || {
        let reply = client.request(
            "account/read",
            &AtomicBool::new(false),
            Instant::now() + Duration::from_millis(300),
        );
        (client, reply)
    });

    let original = server.next_request();
    let id = original["id"].as_u64().unwrap();
    // No reply until the real production reply-wait helper times out.
    let (mut client, reply) = worker.join().unwrap();
    assert_eq!(reply, Err("codex_gateway:rpc_timeout"));
    server.send_response(id, json!({"account":{"type":"chatgpt"}}));
    assert_eq!(
        recv_expected_response(
            &client.replies,
            id + 1,
            Instant::now() + FIXTURE_DEADLINE,
            &AtomicBool::new(false),
            &mut client.account_dirty,
        ),
        Err("codex_gateway:unexpected_response_id"),
    );
    assert_eq!(client.request_id, 1, "a timed-out RPC is never replayed");
    assert!(client.shutdown(server));
}

#[test]
fn scripted_idle_account_change_and_unsolicited_reply_are_distinct() {
    let (server, mut client) = fixture();
    server.send_notification("account/updated");
    let mut changed = false;
    for _ in 0..10 {
        if client.poll(&AtomicBool::new(false)).unwrap() {
            changed = true;
            break;
        }
    }
    assert!(changed, "fake account change must become visible before expiry");
    assert!(client.account_dirty);
    server.send_response(999, json!({}));
    assert_eq!(
        client.poll(&AtomicBool::new(false)),
        Err("codex_gateway:unsolicited_response")
    );
    assert!(client.shutdown(server));
}

struct PausedStdin {
    started: SyncSender<()>,
    release: Receiver<()>,
}

impl Write for PausedStdin {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let _ = self.started.send(());
        self.release.recv()
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "fake stopped"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[test]
fn simulated_stalled_writer_times_out_and_stop_remains_unverified_until_join() {
    let (writer_tx, writer_rx) = mpsc::sync_channel(WRITE_QUEUE_CAPACITY);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let failed = Arc::new(AtomicBool::new(false));
    let writer_failed = Arc::clone(&failed);
    let writer = thread::spawn(move || write_jsonl(
        PausedStdin { started: entered_tx, release: release_rx },
        writer_rx,
        writer_failed,
    ));
    let cancelled = Arc::new(AtomicBool::new(false));
    let waiter_cancel = Arc::clone(&cancelled);
    let waiter_writer = writer_tx.clone();
    let waiter = thread::spawn(move || {
        submit_with_receipt(
            &waiter_writer,
            protocol::request(1, "account/read", json!({})).unwrap(),
            Instant::now() + Duration::from_millis(300),
            || {
                if waiter_cancel.load(Ordering::Acquire) {
                    Err("codex_gateway:cancelled")
                } else {
                    Ok(())
                }
            },
        )
    });

    entered_rx.recv_timeout(FIXTURE_DEADLINE)
        .expect("owned writer entered a simulated blocked pipe");
    assert!(!writer.is_finished());
    assert!(!cleanup_proven(true, false, true, None));
    assert_eq!(waiter.join().unwrap(), Err("codex_gateway:write_timeout"));

    cancelled.store(true, Ordering::Release);
    release_tx.send(()).expect("release simulated pipe");
    drop(writer_tx);
    assert!(writer.join().is_ok());
    assert!(cleanup_proven(true, true, true, None));
    assert!(!failed.load(Ordering::Acquire));
}

#[test]
fn cancellation_ends_a_blocked_writer_wait_before_pipe_release() {
    let (writer_tx, writer_rx) = mpsc::sync_channel(WRITE_QUEUE_CAPACITY);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let fault = Arc::new(AtomicBool::new(false));
    let failed = Arc::clone(&fault);
    let writer = thread::spawn(move || write_jsonl(
        PausedStdin { started: entered_tx, release: release_rx },
        writer_rx,
        failed,
    ));

    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancelled);
    let tx = writer_tx.clone();
    let worker = thread::spawn(move || {
        submit_with_receipt(
            &tx,
            protocol::request(1, "account/read", json!({})).unwrap(),
            Instant::now() + FIXTURE_DEADLINE,
            || if worker_cancel.load(Ordering::Acquire) {
                Err("codex_gateway:cancelled")
            } else {
                Ok(())
            },
        )
    });
    entered_rx.recv_timeout(FIXTURE_DEADLINE)
        .expect("writer must enter simulated blocked pipe");
    cancelled.store(true, Ordering::Release);
    assert_eq!(worker.join().unwrap(), Err("codex_gateway:cancelled"));
    assert!(!cleanup_proven(true, false, true, None));

    // The real AppServer would kill its owned child, releasing the pipe.
    release_tx.send(()).expect("fixture releases its stalled writer");
    drop(writer_tx);
    assert!(writer.join().is_ok());
    assert!(cleanup_proven(true, true, true, None));
    assert!(!fault.load(Ordering::Acquire));
}

#[test]
fn cleanup_uncertainty_never_promotes_a_clean_stop_marker() {
    assert!(!cleanup_proven(false, true, true, None));
    assert!(!cleanup_proven(true, false, true, None));
    assert!(!cleanup_proven(true, true, false, None));
    assert!(!cleanup_proven(true, true, true, Some("codex_gateway:writer_join_failed")));
    assert!(!cleanup_proven(true, true, true, Some("codex_gateway:reader_join_failed")));
    assert!(cleanup_proven(true, true, true, None));
}

#[test]
fn latched_join_failure_is_terminal_and_does_not_retry_cleanup_forever() {
    let failed = cleanup_disposition(
        Err("codex_gateway:writer_join_failed"), false,
    );
    assert_eq!(
        failed,
        CleanupDisposition::Incomplete("codex_gateway:writer_join_failed"),
    );
    assert_ne!(failed, CleanupDisposition::Retry);
    let still_settling = cleanup_disposition(Ok(false), false);
    assert_eq!(still_settling, CleanupDisposition::Retry);
    let timed_out = cleanup_disposition(Ok(false), true);
    assert_eq!(
        timed_out,
        CleanupDisposition::Incomplete("codex_gateway:owned_cleanup_deadline"),
    );
    assert_ne!(timed_out, CleanupDisposition::Complete);
}

#[test]
fn unverified_owner_exit_does_not_rearm_codex_voice_or_child_start() {
    let status = CodexGatewayStatus::cleanup_incomplete(
        11, "codex_gateway:reader_join_failed", false,
    );
    assert_eq!(status.state, "cleanup_incomplete");
    assert_eq!(status.generation, 11);
    assert_eq!(status.blocker, "codex_gateway:reader_join_failed");
    assert_eq!(status.runtime_claim, "codex_gateway:cleanup_not_verified");
    assert!(!status.process_owned);
    assert!(!status.chatgpt_authenticated);
    assert!(!status.included_usage_allowed);
    assert!(!status.realtime_audio_verified);
    assert!(!status.meeting_route_enabled);
}

#[test]
fn p10_account_revision_advances_before_fake_controller_dequeues_event() {
    let (server, mut client) = fixture();
    assert_eq!(client.account_revision.load(Ordering::Acquire), 0);
    server.send_notification("account/updated");
    let until = Instant::now() + FIXTURE_DEADLINE;
    while client.account_revision.load(Ordering::Acquire) == 0
        && Instant::now() < until
    {
        thread::yield_now();
    }
    assert_eq!(client.account_revision.load(Ordering::Acquire), 1);
    assert!(!client.account_dirty, "controller has not consumed the event");
    assert!(client.poll(&AtomicBool::new(false)).unwrap());
    assert!(client.account_dirty);
    assert!(client.shutdown(server));
}

#[test]
fn p10_provider_policy_event_forces_new_account_diagnostic_probe() {
    let (server, mut client) = fixture();
    server.send_notification("modelProvider/authRecoveryStarted");
    let until = Instant::now() + FIXTURE_DEADLINE;
    while client.account_revision.load(Ordering::Acquire) == 0
        && Instant::now() < until
    {
        thread::yield_now();
    }
    assert_eq!(client.account_revision.load(Ordering::Acquire), 1);
    assert!(!client.account_dirty);
    assert!(client.poll(&AtomicBool::new(false)).unwrap());
    assert!(client.account_dirty);
    assert!(client.shutdown(server));
}

#[test]
fn p9_scripted_gateway_capability_precedes_account_and_usage_reads() {
    let (server, mut client) = fixture();
    let worker = thread::spawn(move || {
        let deadline = Instant::now() + FIXTURE_DEADLINE;
        let cancelled = AtomicBool::new(false);
        let gateway = client.request(
            "account/gatewayOAuth/read", &cancelled, deadline,
        ).and_then(|result| {
            compatibility::verify_gateway_oauth_read(&result)?;
            Ok(())
        });
        let account = gateway.and_then(|_| {
            client.request("account/read", &cancelled, Instant::now() + FIXTURE_DEADLINE)
        });
        let quota = account.and_then(|result| {
            protocol::chatgpt_account(&result)?;
            client.request(
                "account/rateLimits/read",
                &cancelled,
                Instant::now() + FIXTURE_DEADLINE,
            )
        });
        (client, quota)
    });

    let gateway = server.next_request();
    assert_eq!(gateway["method"], "account/gatewayOAuth/read");
    assert!(gateway.get("params").is_none());
    server.send_response(gateway["id"].as_u64().unwrap(), json!({
        "providerId":"openai", "providerName":"OpenAI",
        "required":false, "status":null, "error":null
    }));

    let account = server.next_request();
    assert_eq!(account["method"], "account/read");
    server.send_response(account["id"].as_u64().unwrap(), json!({
        "account":{"type":"chatgpt"}
    }));
    let quota = server.next_request();
    assert_eq!(quota["method"], "account/rateLimits/read");
    server.send_response(quota["id"].as_u64().unwrap(), json!({
        "ordinaryUsageAllowed": true
    }));
    let (client, result) = worker.join().unwrap();
    assert_eq!(protocol::ordinary_usage(&result.unwrap()), Ok(Some(true)));
    assert!(client.shutdown(server));
}

#[test]
fn p9_unsupported_gateway_oauth_read_blocks_without_account_request() {
    let (server, mut client) = fixture();
    let worker = thread::spawn(move || {
        let reply = client.request(
            "account/gatewayOAuth/read",
            &AtomicBool::new(false),
            Instant::now() + FIXTURE_DEADLINE,
        );
        (client, reply)
    });
    let request = server.next_request();
    assert_eq!(request["method"], "account/gatewayOAuth/read");
    server.send_rpc_method_not_found(request["id"].as_u64().unwrap());
    let (client, reply) = worker.join().unwrap();
    assert_eq!(reply, Err("codex_gateway:rpc_rejected"));
    assert_eq!(client.request_id, 1, "no account or model request after feature rejection");
    assert!(client.shutdown(server));
}

#[test]
fn fake_transport_never_admits_voice_or_paid_inference() {
    assert_eq!(account_notification("account/updated"), Ok(true));
    assert_eq!(
        account_notification("thread/realtime/started"),
        Err("codex_realtime:unsolicited_without_voice_authority")
    );
    assert_eq!(
        protocol::request(1, "thread/realtime/start", json!({})),
        Err("codex_gateway:request_not_allowed")
    );
    assert!(matches!(
        protocol::parse_frame(
            br#"{"method":"account/updated","params":{}}"#
        ).unwrap(),
        Inbound::Notification { method, .. } if method == "account/updated"
    ));
}
