//! Sole owned Codex child and bounded JSONL transport. No shell, TCP listener,
//! external bearer token, automatic login, model turn, or audio session.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::protocol::{self, Inbound, MAX_FRAME_BYTES};
use super::realtime;

const EVENT_QUEUE_CAPACITY: usize = 32;
const WRITE_QUEUE_CAPACITY: usize = 1;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(8);
const WAIT_SLICE: Duration = Duration::from_millis(50);
type WireEvent = Result<Inbound, &'static str>;
type WriteReceipt = Result<(), &'static str>;

/// One writer, two strictly bounded classes. Diagnostic requests retain the
/// original 8-KiB method allowlist; only a sealed typed realtime frame can
/// select the separate larger JSONL budget.
#[derive(Clone, Copy)]
enum WriteScope {
    Diagnostic,
    Realtime(usize),
}
impl WriteScope {
    fn limit(self) -> usize {
        match self {
            Self::Diagnostic => protocol::MAX_REQUEST_BYTES + 1,
            Self::Realtime(n) if matches!(n, 1024 | 24_576 | 32_768) => n,
            _ => 0,
        }
    }
}


/// The same cancellable write completion contract is used by the owned
/// AppServer and the test-only in-memory fake. No second request policy.
fn submit_with_receipt_scoped(
    writer: &SyncSender<WriteCommand>,
    body: Vec<u8>,
    scope: WriteScope,
    deadline: Instant,
    mut check_live: impl FnMut() -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    check_live()?;
    let (completed, receipt) = mpsc::sync_channel(1);
    match writer.try_send(WriteCommand { body, scope, completed }) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => return Err("codex_gateway:writer_queue_full"),
        Err(TrySendError::Disconnected(_)) => {
            return Err("codex_gateway:writer_disconnected");
        }
    }
    loop {
        check_live()?;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("codex_gateway:write_timeout");
        }
        match receipt.recv_timeout(remaining.min(WAIT_SLICE)) {
            Ok(result) => return result,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err("codex_gateway:write_receipt_lost");
            }
        }
    }
}

fn submit_with_receipt(
    writer: &SyncSender<WriteCommand>,
    body: Vec<u8>,
    deadline: Instant,
    check_live: impl FnMut() -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    submit_with_receipt_scoped(
        writer, body, WriteScope::Diagnostic, deadline, check_live,
    )
}

/// Exact ID and account-notification dispatch for both the native owner and
/// the scripted fake. No response payload is printed to logs or Debug.
fn recv_expected_response(
    receiver: &Receiver<WireEvent>,
    expected_id: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
    account_dirty: &mut bool,
) -> Result<Option<Value>, &'static str> {
    if cancelled.load(Ordering::Acquire) {
        return Err("codex_gateway:cancelled");
    }
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err("codex_gateway:rpc_timeout");
    }
    match receiver.recv_timeout(remaining.min(WAIT_SLICE)) {
        Ok(Ok(Inbound::Response { id, result })) if id == expected_id => {
            if cancelled.load(Ordering::Acquire) {
                Err("codex_gateway:cancelled")
            } else {
                Ok(Some(result))
            }
        }
        Ok(Ok(Inbound::Response { .. })) => {
            Err("codex_gateway:unexpected_response_id")
        }
        Ok(Ok(Inbound::Notification { method, .. })) => {
            if account_notification(&method)? {
                *account_dirty = true;
            }
            Ok(None)
        }
        Ok(Err(reason)) => Err(reason),
        Err(RecvTimeoutError::Timeout) => Ok(None),
        Err(RecvTimeoutError::Disconnected) => {
            Err("codex_gateway:reader_disconnected")
        }
    }
}

fn recv_idle_notification(
    receiver: &Receiver<WireEvent>,
    cancelled: &AtomicBool,
) -> Result<bool, &'static str> {
    if cancelled.load(Ordering::Acquire) {
        return Err("codex_gateway:cancelled");
    }
    match receiver.recv_timeout(Duration::from_millis(100)) {
        Ok(Ok(Inbound::Notification { method, .. })) => {
            account_notification(&method)
        }
        Ok(Ok(Inbound::Response { .. })) => Err("codex_gateway:unsolicited_response"),
        Ok(Err(reason)) => Err(reason),
        Err(RecvTimeoutError::Timeout) => Ok(false),
        Err(RecvTimeoutError::Disconnected) => {
            Err("codex_gateway:reader_disconnected")
        }
    }
}

fn cleanup_proven(
    child_reaped: bool,
    writer_released: bool,
    reader_released: bool,
    cleanup_failure: Option<&'static str>,
) -> bool {
    child_reaped && writer_released && reader_released && cleanup_failure.is_none()
}

/// An account or provider-policy event invalidates previously observed
/// ChatGPT/provider/usage claims before the controller dequeues that event.
/// This includes the current upstream modelProvider authentication recovery
/// notifications; future config/changed also fails closed.
fn affects_account_authority(method: &str) -> bool {
    method.starts_with("account/")
        || method.starts_with("modelProvider/")
        || matches!(method, "config/changed" | "configRequirements/changed")
}

/// Deterministic notification admission shared by request and idle loops.
/// The actual read thread advances a revision before queueing a notification.
fn account_notification(method: &str) -> Result<bool, &'static str> {
    realtime::refuse_unadmitted_event(method)?;
    // An account-only child does not own agent turns. Never silently ignore
    // unexpected work on the diagnostic process, even if it is unrelated
    // to the current read-only account RPC.
    if method.starts_with("thread/") || method.starts_with("turn/")
        || method.starts_with("item/")
    {
        return Err("codex_gateway:agent_activity_in_diagnostics");
    }
    Ok(affects_account_authority(method))
}

fn owned_realtime_inbound_allowed(
    event: &Inbound,
    last_issued_id: u64,
) -> Result<(), &'static str> {
    match event {
        Inbound::Response { id, .. } if *id > 0 && *id <= last_issued_id => Ok(()),
        Inbound::Response { .. } => Err("codex_gateway:voice_response_id_unowned"),
        Inbound::Notification { method, .. } if affects_account_authority(method) => {
            Err("codex_gateway:voice_account_revision_changed")
        }
        Inbound::Notification { method, params } if method.starts_with("thread/realtime/") =>
            realtime::reject_unsafe_realtime_item(method, params),
        Inbound::Notification { method, .. } if method.starts_with("turn/")
            || method.starts_with("item/") =>
            Err("codex_gateway:voice_agent_turn_observed"),
        Inbound::Notification { .. } => Err("codex_gateway:voice_notification_unexpected"),
    }
}

/// The existing RealtimeRpc owner verifies each of the three upstream
/// signalling receipts. The JSONL reader can deliver them in any order.
/// SDP is private; never log or Debug the answer.
fn observe_realtime_start_signal(
    rpc: &mut super::realtime_rpc::RealtimeRpc,
    start_id: u64,
    answer: &mut Option<String>,
    event: Inbound,
) -> Result<(), &'static str> {
    let result = match event {
        Inbound::Response { id, result } if id == start_id =>
            rpc.ack_start(id, &result),
        Inbound::Notification { method, params } if method == "thread/realtime/started" =>
            rpc.observe_started(&params),
        Inbound::Notification { method, params } if method == "thread/realtime/sdp" => {
            rpc.observe_answer(&params)?;
            let sdp = params.get("sdp").and_then(Value::as_str)
                .ok_or("codex_gateway:voice_sdp_missing")?;
            *answer = Some(sdp.to_owned());
            Ok(())
        }
        _ => Err("codex_gateway:voice_signalling_event_unexpected"),
    };
    if result.is_err() { rpc.revoke(); }
    result
}

struct WriteCommand {
    body: Vec<u8>,
    scope: WriteScope,
    completed: SyncSender<WriteReceipt>,
}

/// Sole write owner. Blocking OS-pipe writes are confined to this owned
/// thread, never the request/Stop controller. Each write has a bounded payload,
/// one acknowledgement and an explicit shutdown/disconnect path.
fn write_jsonl(
    mut stdin: impl Write,
    receiver: Receiver<WriteCommand>,
    failed: Arc<AtomicBool>,
) {
    while let Ok(command) = receiver.recv() {
        if command.body.is_empty()
            || command.scope.limit() == 0
            || command.body.len() > command.scope.limit()
            || command.body.last() != Some(&b'\n')
        {
            failed.store(true, Ordering::Release);
            let _ = command.completed.try_send(Err("codex_gateway:writer_frame_invalid"));
            break;
        }
        // Do not expose command.body or raw IO errors in any logging.
        let written = stdin.write_all(&command.body)
            .and_then(|()| stdin.flush())
            .map_err(|_| "codex_gateway:stdin_write_failed");
        if written.is_err() {
            failed.store(true, Ordering::Release);
        }
        let stop_after_failure = written.is_err();
        let _ = command.completed.try_send(written);
        if stop_after_failure {
            break;
        }
    }
}


fn acceptable_binary(path: &Path) -> bool {
    path.is_absolute()
        && path.extension().and_then(OsStr::to_str)
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
        && std::fs::symlink_metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
}

/// Conservative candidates only. Never execute a .cmd shim, Node.js loader or
/// scan an unbounded filesystem tree. Standard npm global installations expose
/// codex.cmd on PATH but keep the actual binary inside the installed package.
fn native_candidates_in_bin(directory: &Path) -> [PathBuf; 3] {
    let package = directory.join("node_modules").join("@openai").join("codex");
    [
        directory.join("codex.exe"),
        package.join("node_modules").join("@openai").join("codex-win32-x64")
            .join("vendor").join("x86_64-pc-windows-msvc").join("bin").join("codex.exe"),
        package.join("vendor").join("x86_64-pc-windows-msvc")
            .join("codex").join("codex.exe"),
    ]
}

/// Explicit native executable first, then a reviewed installed-CLI layout in
/// absolute PATH directories. Preserve the user's other Codex CLI processes.
fn codex_binary() -> Result<PathBuf, &'static str> {
    for key in ["TRANSLATEIT_CODEX_EXECUTABLE", "CODEX_CLI_PATH"] {
        if let Some(raw) = std::env::var_os(key) {
            let path = PathBuf::from(raw);
            return acceptable_binary(&path)
                .then_some(path)
                .ok_or("codex_gateway:executable_untrusted");
        }
    }
    if let Some(path_env) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path_env) {
            if !directory.is_absolute() {
                continue;
            }
            for candidate in native_candidates_in_bin(&directory) {
                if acceptable_binary(&candidate) {
                    return Ok(candidate);
                }
            }
        }
    }
    Err("codex_gateway:native_executable_missing")
}

fn emit(
    tx: &SyncSender<WireEvent>,
    overflow: &AtomicBool,
    event: WireEvent,
) -> bool {
    match tx.try_send(event) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) => {
            overflow.store(true, Ordering::Release);
            false
        }
        Err(TrySendError::Disconnected(_)) => false,
    }
}

/// Enforce the bound BEFORE allocating untrusted stdout bytes. Do not decode
/// stderr or include Codex response bodies in application diagnostics.
fn read_jsonl(
    mut stdout: impl Read,
    tx: SyncSender<WireEvent>,
    overflow: Arc<AtomicBool>,
    account_revision: Arc<AtomicU64>,
) {
    let mut block = [0u8; 4096];
    let mut line = Vec::with_capacity(4096);
    loop {
        let size = match stdout.read(&mut block) {
            Ok(0) => {
                let _ = emit(&tx, &overflow, Err("codex_gateway:stdout_closed"));
                break;
            }
            Ok(size) => size,
            Err(_) => {
                let _ = emit(&tx, &overflow, Err("codex_gateway:stdout_read_failed"));
                break;
            }
        };
        for byte in &block[..size] {
            if *byte == b'\n' {
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                let parsed = protocol::parse_frame(&line);
                line.clear();
                if let Ok(Inbound::Notification { method, .. }) = &parsed {
                    if affects_account_authority(method) {
                        // Atomic revision is updated before the event enters
                        // the bounded queue. UI snapshots therefore invalidate
                        // even when the controller has not dequeued the event.
                        if account_revision.fetch_update(
                            Ordering::AcqRel,
                            Ordering::Acquire,
                            |revision| revision.checked_add(1),
                        ).is_err() {
                            overflow.store(true, Ordering::Release);
                            return;
                        }
                    }
                }
                if !emit(&tx, &overflow, parsed) {
                    return;
                }
            } else {
                if line.len() >= MAX_FRAME_BYTES {
                    let _ = emit(&tx, &overflow, Err("codex_gateway:frame_too_large"));
                    return;
                }
                line.push(*byte);
            }
        }
    }
}

pub(super) struct AppServer {
    child: Child,
    writer_tx: Option<SyncSender<WriteCommand>>,
    receiver: Receiver<WireEvent>,
    overflow: Arc<AtomicBool>,
    writer_failed: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
    writer: Option<JoinHandle<()>>,
    next_id: u64,
    child_reaped: bool,
    account_dirty: bool,
    account_revision: Arc<AtomicU64>,
    cleanup_failure: Option<&'static str>,
}

impl AppServer {
    pub(super) fn spawn(account_revision: Arc<AtomicU64>)
        -> Result<Self, &'static str>
    {
        let binary = codex_binary()?;
        let mut command = Command::new(binary);
        command.args(["app-server", "--listen", "stdio://"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // Neither this process nor its child may silently use an inherited
        // paid API key. The later Voice admission must separately verify
        // entitlement/provider identity: account/read alone cannot do so.
        for key in [
            "OPENAI_API_KEY", "CODEX_API_KEY", "AZURE_OPENAI_API_KEY",
            "OPENAI_BASE_URL", "OPENAI_API_BASE", "OPENAI_ORG_ID",
            "OPENAI_PROJECT_ID", "OPENAI_MODEL",
        ] {
            command.env_remove(key);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = command.spawn().map_err(|_| "codex_gateway:spawn_failed")?;
        let Some(stdin) = child.stdin.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("codex_gateway:stdin_unavailable");
        };
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("codex_gateway:stdout_unavailable");
        };
        let (writer_tx, writer_rx) = mpsc::sync_channel(WRITE_QUEUE_CAPACITY);
        let writer_failed = Arc::new(AtomicBool::new(false));
        let write_fault = Arc::clone(&writer_failed);
        let writer = match thread::Builder::new()
            .name("translateit-codex-jsonl-writer".to_string())
            .spawn(move || write_jsonl(stdin, writer_rx, write_fault))
        {
            Ok(writer) => writer,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("codex_gateway:writer_spawn_failed");
            }
        };
        let (tx, receiver) = mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
        let overflow = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&overflow);
        let reader_revision = Arc::clone(&account_revision);
        let reader = match thread::Builder::new()
            .name("translateit-codex-jsonl-reader".to_string())
            .spawn(move || read_jsonl(stdout, tx, observed, reader_revision))
        {
            Ok(reader) => reader,
            Err(_) => {
                // Close the idle writer's one-slot queue, then reap only the
                // child that this gateway created. No unrelated Codex process.
                drop(writer_tx);
                let _ = child.kill();
                let _ = child.wait();
                let _ = writer.join();
                return Err("codex_gateway:reader_spawn_failed");
            }
        };
        Ok(Self {
            child, writer_tx: Some(writer_tx), receiver, overflow,
            writer_failed, reader: Some(reader), writer: Some(writer),
            next_id: 0, child_reaped: false, account_dirty: false,
            account_revision,
            cleanup_failure: None,
        })
    }

    fn check_live(&mut self, cancelled: &AtomicBool) -> Result<(), &'static str> {
        if cancelled.load(Ordering::Acquire) {
            return Err("codex_gateway:cancelled");
        }
        if self.overflow.load(Ordering::Acquire) {
            return Err("codex_gateway:event_queue_overflow");
        }
        if self.writer_failed.load(Ordering::Acquire) {
            return Err("codex_gateway:writer_failed");
        }
        if self.writer.as_ref().is_some_and(JoinHandle::is_finished) {
            return Err("codex_gateway:writer_exited");
        }
        if self.child.try_wait().map_err(|_| "codex_gateway:child_probe_failed")?.is_some() {
            return Err("codex_gateway:child_exited");
        }
        Ok(())
    }

    /// One owned writer and one in-flight control request. The source-only
    /// fake exercises the same cancellation and deadline helper.
    fn write_frame(
        &mut self,
        body: Vec<u8>,
        cancelled: &AtomicBool,
        deadline: Instant,
    ) -> Result<(), &'static str> {
        let writer = self.writer_tx.as_ref()
            .ok_or("codex_gateway:writer_unavailable")?.clone();
        submit_with_receipt(&writer, body, deadline, || self.check_live(cancelled))
    }

    pub(super) fn request(
        &mut self,
        method: &str,
        params: Value,
        cancelled: &AtomicBool,
    ) -> Result<Value, &'static str> {
        self.check_live(cancelled)?;
        self.next_id = self.next_id.checked_add(1)
            .ok_or("codex_gateway:request_id_exhausted")?;
        let current_id = self.next_id;
        let body = protocol::request(current_id, method, params)?;
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        self.write_frame(body, cancelled, deadline)?;
        loop {
            self.check_live(cancelled)?;
            if let Some(result) = recv_expected_response(
                &self.receiver,
                current_id,
                deadline,
                cancelled,
                &mut self.account_dirty,
            )? {
                return Ok(result);
            }
        }
    }

    /// One request ID owner across diagnostic and future realtime messages.
    /// This is unreachable from production without a VerifiedMediaAuthority.
    pub(super) fn next_realtime_request_id(
        &mut self,
        pinned_account_revision: u64,
        _admission: &super::realtime_rpc::VerifiedMediaAuthority,
        cancelled: &AtomicBool,
    ) -> Result<u64, &'static str> {
        self.check_live(cancelled)?;
        if self.account_dirty || self.account_revision() != pinned_account_revision {
            return Err("codex_gateway:voice_account_revision_changed");
        }
        self.next_id.checked_add(1).ok_or("codex_gateway:request_id_exhausted")
    }

    /// Submit one typed V3 JSONL frame through the *existing* writer queue.
    /// This does not wait for the model RPC response: the same owned reader
    /// must observe that ACK separately to avoid blocking 200-ms audio ticks.
    /// No production path can construct the sealed admission argument.
    pub(super) fn queue_realtime_frame(
        &mut self,
        frame: super::realtime_rpc::RpcFrame,
        pinned_account_revision: u64,
        _admission: &super::realtime_rpc::VerifiedMediaAuthority,
        cancelled: &AtomicBool,
    ) -> Result<(), &'static str> {
        let next = self.next_realtime_request_id(
            pinned_account_revision, _admission, cancelled,
        )?;
        if frame.id != next {
            return Err("codex_gateway:voice_rpc_id_out_of_order");
        }
        let max_bytes = frame.validate_for_transport()?;
        self.next_id = next;
        let writer = self.writer_tx.as_ref()
            .ok_or("codex_gateway:writer_unavailable")?.clone();
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        submit_with_receipt_scoped(
            &writer, frame.bytes, WriteScope::Realtime(max_bytes), deadline,
            || self.check_live(cancelled),
        )?;
        if self.account_dirty || self.account_revision() != pinned_account_revision {
            return Err("codex_gateway:voice_account_revision_changed");
        }
        Ok(())
    }

    /// Receive exact typed events on the SAME AppServer child/reader.
    /// On account/provider change, never return stale voice text or ACKs.
    /// Per-thread/turn/utterance IDs remain checked by RealtimeRpc and the
    /// existing RealtimeObserver + CorrelationGate, not this pipe layer.
    pub(super) fn next_realtime_event(
        &mut self,
        pinned_account_revision: u64,
        _admission: &super::realtime_rpc::VerifiedMediaAuthority,
        cancelled: &AtomicBool,
    ) -> Result<Option<Inbound>, &'static str> {
        self.check_live(cancelled)?;
        if self.account_dirty || self.account_revision() != pinned_account_revision {
            return Err("codex_gateway:voice_account_revision_changed");
        }
        let received = match self.receiver.recv_timeout(WAIT_SLICE) {
            Ok(Ok(value)) => value,
            Ok(Err(reason)) => return Err(reason),
            Err(RecvTimeoutError::Timeout) => return Ok(None),
            Err(RecvTimeoutError::Disconnected) => return Err("codex_gateway:reader_disconnected"),
        };
        // A separate reader thread updates revision BEFORE queueing a changed
        // account notification. Refuse data regardless of delivery order.
        if cancelled.load(Ordering::Acquire)
            || self.account_revision() != pinned_account_revision
        {
            return Err("codex_gateway:voice_account_revision_changed");
        }
        if let Err(blocker) = owned_realtime_inbound_allowed(&received, self.next_id) {
            if blocker == "codex_gateway:voice_account_revision_changed" {
                self.account_dirty = true;
            }
            return Err(blocker);
        }
        Ok(Some(received))
    }

    /// Source-only SDP handshake on the SAME owned AppServer child/reader.
    /// Caller holds a sealed VerifiedMediaAuthority: no production constructor.
    /// This cannot authorize WebRTC media, model output or Meeting playback.
    pub(super) fn exchange_realtime_offer(
        &mut self,
        rpc: &mut super::realtime_rpc::RealtimeRpc,
        offer_sdp: &str,
        account_revision: u64,
        admission: &super::realtime_rpc::VerifiedMediaAuthority,
        cancelled: &AtomicBool,
    ) -> Result<String, &'static str> {
        let operation = (|| {
            let start_id = self.next_realtime_request_id(
                account_revision, admission, cancelled,
            )?;
            let frame = rpc.start(start_id, offer_sdp, cancelled)?;
            self.queue_realtime_frame(frame, account_revision, admission, cancelled)?;
            let deadline = Instant::now() + RESPONSE_TIMEOUT;
            let mut answer = None;
            loop {
                if cancelled.load(Ordering::Acquire) {
                    return Err("codex_gateway:cancelled");
                }
                if Instant::now() >= deadline {
                    return Err("codex_gateway:voice_signalling_timeout");
                }
                if let Some(event) = self.next_realtime_event(
                    account_revision, admission, cancelled,
                )? {
                    observe_realtime_start_signal(rpc, start_id, &mut answer, event)?;
                }
                if rpc.phase() == super::realtime_rpc::Phase::AwaitMedia {
                    return answer.ok_or("codex_gateway:voice_signalling_missing_answer");
                }
            }
        })();
        if operation.is_err() { rpc.revoke(); }
        operation
    }

    /// The read thread advances this revision BEFORE queueing account or
    /// provider events, including while the controller awaits an RPC.
    pub(super) fn account_revision(&self) -> u64 {
        self.account_revision.load(Ordering::Acquire)
    }

    /// Account notifications arriving while an RPC response is in flight
    /// invalidate that account/usage snapshot; refresh in a bounded probe.
    pub(super) fn take_account_dirty(&mut self) -> bool {
        std::mem::take(&mut self.account_dirty)
    }

    pub(super) fn initialized(&mut self, cancelled: &AtomicBool) -> Result<(), &'static str> {
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        self.write_frame(
            protocol::initialized_notification().to_vec(),
            cancelled,
            deadline,
        )
    }

    /// No automatic model requests. Only account changes invalidate the probe.
    pub(super) fn poll(
        &mut self,
        cancelled: &AtomicBool,
    ) -> Result<bool, &'static str> {
        self.check_live(cancelled)?;
        let changed = recv_idle_notification(&self.receiver, cancelled)?;
        self.account_dirty |= changed;
        Ok(changed)
    }

    /// A false result retains this exact owner. Completion requires the
    /// child to be reaped and BOTH JSONL threads to have finished and joined.
    pub(super) fn stop_and_reap(&mut self) -> Result<bool, &'static str> {
        if let Some(failure) = self.cleanup_failure {
            return Err(failure);
        }
        // The channel close wakes an idle writer without injecting a stop
        // frame into a potentially stalled upstream pipe.
        self.writer_tx.take();
        if !self.child_reaped {
            let still_running = self.child.try_wait()
                .map_err(|_| "codex_gateway:child_probe_failed")?.is_none();
            if still_running && self.child.kill().is_err() {
                // Process may have exited between try_wait and kill.
                if self.child.try_wait()
                    .map_err(|_| "codex_gateway:child_probe_failed")?.is_none()
                {
                    return Err("codex_gateway:child_stop_failed");
                }
            }
            self.child.wait().map_err(|_| "codex_gateway:child_reap_failed")?;
            self.child_reaped = true;
        }
        if self.writer.as_ref().is_some_and(JoinHandle::is_finished) {
            let writer = self.writer.take().expect("checked writer present");
            if writer.join().is_err() {
                self.cleanup_failure = Some("codex_gateway:writer_join_failed");
                return Err("codex_gateway:writer_join_failed");
            }
        }
        if self.reader.as_ref().is_some_and(JoinHandle::is_finished) {
            let reader = self.reader.take().expect("checked reader present");
            if reader.join().is_err() {
                self.cleanup_failure = Some("codex_gateway:reader_join_failed");
                return Err("codex_gateway:reader_join_failed");
            }
        }
        Ok(cleanup_proven(
            self.child_reaped,
            self.writer.is_none(),
            self.reader.is_none(),
            self.cleanup_failure,
        ))
    }
}

impl Drop for AppServer {
    fn drop(&mut self) {
        // Panic/error backstop. Normal lifecycle holds the AppServer until
        // stop_and_reap proves both owned threads finished.
        self.writer_tx.take();
        if !self.child_reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[cfg(test)]
mod fake_app_server_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jsonl_in_chunks_without_hidden_unbounded_buffer() {
        let input = std::io::Cursor::new(
            b"{\"id\":1,\"result\":{}}\r\n{\"method\":\"account/updated\"}\n".to_vec()
        );
        let (tx, rx) = mpsc::sync_channel(4);
        let overflow = Arc::new(AtomicBool::new(false));
        let revision = Arc::new(AtomicU64::new(0));
        read_jsonl(input, tx, Arc::clone(&overflow), Arc::clone(&revision));
        assert!(matches!(rx.recv().unwrap().unwrap(), Inbound::Response { id: 1, .. }));
        assert!(matches!(rx.recv().unwrap().unwrap(),
            Inbound::Notification { method, .. } if method == "account/updated"));
        assert!(!overflow.load(Ordering::Acquire));
        assert_eq!(revision.load(Ordering::Acquire), 1);
    }

    #[test]
    fn overflow_is_fail_closed_and_does_not_block_reader() {
        let (tx, rx) = mpsc::sync_channel(1);
        let overflow = Arc::new(AtomicBool::new(false));
        read_jsonl(std::io::Cursor::new(
            b"{\"method\":\"one\"}\n{\"method\":\"two\"}\n".to_vec()
        ), tx, Arc::clone(&overflow), Arc::new(AtomicU64::new(0)));
        assert!(overflow.load(Ordering::Acquire));
        assert!(matches!(rx.recv().unwrap().unwrap(),
            Inbound::Notification { method, .. } if method == "one"));
    }


    #[test]
    fn writer_ack_preserves_exact_jsonl_without_using_the_control_thread_for_io() {
        use std::sync::Mutex;

        struct CaptureWriter(Arc<Mutex<Vec<u8>>>);
        impl Write for CaptureWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }

        let recorded = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = mpsc::sync_channel(WRITE_QUEUE_CAPACITY);
        let (ack, done) = mpsc::sync_channel(1);
        assert!(tx.try_send(WriteCommand {
            body: b"{\"method\":\"initialized\"}\n".to_vec(),
            scope: WriteScope::Diagnostic,
            completed: ack,
        }).is_ok());
        drop(tx);
        let failed = Arc::new(AtomicBool::new(false));
        write_jsonl(CaptureWriter(Arc::clone(&recorded)), rx, Arc::clone(&failed));
        assert!(done.recv().unwrap().is_ok());
        assert_eq!(
            recorded.lock().unwrap().as_slice(),
            b"{\"method\":\"initialized\"}\n".as_slice()
        );
        assert!(!failed.load(Ordering::Acquire));
    }

    #[test]
    fn writer_queue_is_bounded_without_blocking_the_caller() {
        let (tx, _rx) = mpsc::sync_channel(WRITE_QUEUE_CAPACITY);
        let (first, _first_receipt) = mpsc::sync_channel(1);
        assert!(tx.try_send(WriteCommand {
            body: b"{\"method\":\"initialized\"}\n".to_vec(),
            scope: WriteScope::Diagnostic,
            completed: first,
        }).is_ok());
        let (second, _second_receipt) = mpsc::sync_channel(1);
        assert!(matches!(
            tx.try_send(WriteCommand {
                body: b"{\"method\":\"initialized\"}\n".to_vec(),
            scope: WriteScope::Diagnostic,
                completed: second,
            }),
            Err(TrySendError::Full(_))
        ));
    }

    #[test]
    fn realtime_start_signalling_accepts_ack_started_sdp_in_any_order() {
        use super::super::realtime_rpc::{Phase, RealtimeRpc};
        let offer = "v=0\r\no=local 1 2 IN IP4 127.0.0.1\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n";
        let answer = "v=0\r\no=remote 2 3 IN IP4 127.0.0.1\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n";
        for order in [[0, 1, 2], [2, 1, 0], [1, 0, 2]] {
            let cancelled = AtomicBool::new(false);
            let mut rpc = RealtimeRpc::new("thread_1", "voice_1", 7, 2).unwrap();
            rpc.start(10, offer, &cancelled).unwrap();
            let mut browser_answer = None;
            for item in order {
                let event = match item {
                    0 => Inbound::Response { id: 10, result: serde_json::json!({}) },
                    1 => Inbound::Notification {
                        method: "thread/realtime/started".into(),
                        params: serde_json::json!({
                            "threadId":"thread_1","version":"v3","realtimeSessionId":"session_1"
                        }),
                    },
                    _ => Inbound::Notification {
                        method: "thread/realtime/sdp".into(),
                        params: serde_json::json!({"threadId":"thread_1","sdp":answer}),
                    },
                };
                observe_realtime_start_signal(&mut rpc, 10, &mut browser_answer, event).unwrap();
            }
            assert_eq!(rpc.phase(), Phase::AwaitMedia);
            assert_eq!(browser_answer.as_deref(), Some(answer));
        }
    }

    #[test]
    fn realtime_start_signalling_rejects_wrong_id_and_unrelated_events() {
        use super::super::realtime_rpc::{Phase, RealtimeRpc};
        let offer = "v=0\r\no=local 1 2 IN IP4 127.0.0.1\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n";
        let cancelled = AtomicBool::new(false);
        for event in [
            Inbound::Response { id: 11, result: serde_json::json!({}) },
            Inbound::Notification {
                method:"thread/realtime/error".into(),
                params: serde_json::json!({"message":"private"}),
            }
        ] {
            let mut rpc = RealtimeRpc::new("thread_1", "voice_1", 7, 2).unwrap();
            rpc.start(10, offer, &cancelled).unwrap();
            assert_eq!(
                observe_realtime_start_signal(&mut rpc, 10, &mut None, event),
                Err("codex_gateway:voice_signalling_event_unexpected")
            );
            assert_eq!(rpc.phase(), Phase::Revoked);
        }
    }

    #[test]
    fn separate_writer_limits_preserve_diagnostics_and_realtime_pcm() {
        use std::sync::Mutex;
        struct Capture(Arc<Mutex<Vec<u8>>>);
        impl Write for Capture {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        let mut audio = serde_json::to_vec(&serde_json::json!({
            "id": 1,
            "method": "thread/realtime/appendAudio",
            "params": { "audio": {"data":"A".repeat(12_800)} }
        })).unwrap();
        audio.push(b'\n');
        assert!(audio.len() > protocol::MAX_REQUEST_BYTES);
        assert!(audio.len() < 24_576);

        let owned = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = mpsc::sync_channel(1);
        let (done, receipt) = mpsc::sync_channel(1);
        tx.try_send(WriteCommand {
            body: audio.clone(),
            scope: WriteScope::Realtime(24_576),
            completed: done,
        }).unwrap();
        drop(tx);
        let failed = Arc::new(AtomicBool::new(false));
        write_jsonl(Capture(Arc::clone(&owned)), rx, Arc::clone(&failed));
        assert_eq!(receipt.recv().unwrap(), Ok(()));
        assert_eq!(&*owned.lock().unwrap(), &audio);
        assert!(!failed.load(Ordering::Acquire));

        let (tx, rx) = mpsc::sync_channel(1);
        let (done, receipt) = mpsc::sync_channel(1);
        tx.try_send(WriteCommand {
            body: audio,
            scope: WriteScope::Diagnostic,
            completed: done,
        }).unwrap();
        drop(tx);
        let rejected = Arc::new(AtomicBool::new(false));
        write_jsonl(Capture(Arc::new(Mutex::new(Vec::new()))), rx, Arc::clone(&rejected));
        assert_eq!(receipt.recv().unwrap(), Err("codex_gateway:writer_frame_invalid"));
        assert!(rejected.load(Ordering::Acquire));
    }

    #[test]
    fn owned_realtime_dispatch_rejects_unowned_response_and_unrelated_events() {
        let reply = Inbound::Response { id: 7, result: serde_json::json!({}) };
        assert_eq!(owned_realtime_inbound_allowed(&reply, 7), Ok(()));
        assert_eq!(
            owned_realtime_inbound_allowed(&reply, 6),
            Err("codex_gateway:voice_response_id_unowned")
        );
        let sdp = Inbound::Notification {
            method: "thread/realtime/sdp".into(),
            params: serde_json::json!({"threadId":"thread-1","sdp":"private"}),
        };
        assert_eq!(owned_realtime_inbound_allowed(&sdp, 7), Ok(()));
        let account = Inbound::Notification {
            method: "account/updated".into(), params: serde_json::Value::Null,
        };
        assert_eq!(
            owned_realtime_inbound_allowed(&account, 7),
            Err("codex_gateway:voice_account_revision_changed")
        );
        let unrelated = Inbound::Notification {
            method: "thread/started".into(), params: serde_json::Value::Null,
        };
        assert_eq!(
            owned_realtime_inbound_allowed(&unrelated, 7),
            Err("codex_gateway:voice_notification_unexpected")
        );
    }

    #[test]
    fn owned_voice_reader_refuses_handoff_before_delivering_a_candidate() {
        // Codex App Server turns HandoffRequested into itemAdded, after
        // upstream may already have launched a coding-agent turn.
        let handoff = Inbound::Notification {
            method: "thread/realtime/itemAdded".into(),
            params: serde_json::json!({"threadId":"thread_1","item":{
                "type":"handoff_request","handoff_id":"h1","item_id":"i1",
                "input_transcript":"private spoken command","active_transcript":[]
            }}),
        };
        assert_eq!(
            owned_realtime_inbound_allowed(&handoff, 10),
            Err("codex_realtime:agent_handoff_observed")
        );
        let agent = Inbound::Notification {
            method: "turn/started".into(),
            params: serde_json::json!({"threadId":"thread_1"}),
        };
        assert_eq!(
            owned_realtime_inbound_allowed(&agent, 10),
            Err("codex_gateway:voice_agent_turn_observed")
        );
        let benign = Inbound::Notification {
            method: "thread/realtime/itemAdded".into(),
            params: serde_json::json!({"threadId":"thread_1","item":{
                "type":"input_audio_buffer.speech_started","item_id":"i2"
            }}),
        };
        assert_eq!(owned_realtime_inbound_allowed(&benign, 10), Ok(()));
        let promoted = Inbound::Notification {
            method: "thread/realtime/item/started".into(),
            params: serde_json::json!({"threadId":"thread_1","item":{
                "type":"bemItemPromoted","id":"i3"
            }}),
        };
        assert_eq!(
            owned_realtime_inbound_allowed(&promoted, 10),
            Err("codex_realtime:agent_item_promoted")
        );
    }

    #[test]
    fn owned_voice_reader_rejects_agent_or_unknown_started_items_before_observation() {
        for (kind, blocker) in [
            ("handoff_request", "codex_realtime:agent_handoff_observed"),
            ("function_call", "codex_realtime:agent_handoff_observed"),
            ("bemItemPromoted", "codex_realtime:agent_item_promoted"),
            ("commandExecution", "codex_realtime:started_item_unadmitted"),
        ] {
            let event = Inbound::Notification {
                method: "thread/realtime/item/started".into(),
                params: serde_json::json!({"threadId":"thread_1","item":{
                    "id":"item_1", "realtimeSessionId":"sess_1", "type":kind
                }}),
            };
            assert_eq!(owned_realtime_inbound_allowed(&event, 10), Err(blocker));
        }
        let benign = Inbound::Notification {
            method: "thread/realtime/item/started".into(),
            params: serde_json::json!({"threadId":"thread_1","item":{
                "id":"item_2", "realtimeSessionId":"sess_1", "type":"transcriptSegment",
                "role":"user", "text":"source only"
            }}),
        };
        assert_eq!(owned_realtime_inbound_allowed(&benign, 10), Ok(()));
    }

    #[test]
    fn notification_dispatch_retains_only_account_changes_and_denies_voice() {
        assert_eq!(account_notification("account/updated"), Ok(true));
        assert_eq!(account_notification("account/rateLimits/updated"), Ok(true));
        assert_eq!(
            account_notification("thread/started"),
            Err("codex_gateway:agent_activity_in_diagnostics")
        );
        assert_eq!(
            account_notification("thread/realtime/transcript/done"),
            Err("codex_realtime:unsolicited_without_voice_authority")
        );
        assert_eq!(
            account_notification("turn/started"),
            Err("codex_gateway:agent_activity_in_diagnostics")
        );
    }

    #[test]
    fn reader_revision_invalidates_account_and_provider_before_dispatch() {
        let (tx, rx) = mpsc::sync_channel(8);
        let overflow = Arc::new(AtomicBool::new(false));
        let revision = Arc::new(AtomicU64::new(7));
        let stream = br#"{"method":"account/updated","params":{}}
{"method":"modelProvider/authRecoveryStarted","params":{}}
{"method":"config/changed","params":{}}
{"method":"thread/started","params":{}}
"#;
        read_jsonl(
            std::io::Cursor::new(stream.to_vec()), tx,
            Arc::clone(&overflow), Arc::clone(&revision),
        );
        assert_eq!(revision.load(Ordering::Acquire), 10);
        assert!(!overflow.load(Ordering::Acquire));
        for _ in 0..4 {
            assert!(matches!(rx.recv().unwrap().unwrap(), Inbound::Notification { .. }));
        }
        assert_eq!(account_notification("modelProvider/authRecoveryCompleted"), Ok(true));
        assert_eq!(account_notification("config/changed"), Ok(true));
        assert_eq!(
            account_notification("thread/started"),
            Err("codex_gateway:agent_activity_in_diagnostics")
        );
        assert_eq!(
            account_notification("thread/realtime/started"),
            Err("codex_realtime:unsolicited_without_voice_authority")
        );
    }

    #[test]
    fn writer_failure_stays_redacted_and_poisoned() {
        struct BrokenWriter;
        impl Write for BrokenWriter {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("private diagnostic payload"))
            }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }

        let (tx, rx) = mpsc::sync_channel(1);
        let (ack, done) = mpsc::sync_channel(1);
        assert!(tx.try_send(WriteCommand {
            body: b"{\"method\":\"initialized\"}\n".to_vec(),
            scope: WriteScope::Diagnostic,
            completed: ack,
        }).is_ok());
        drop(tx);
        let failed = Arc::new(AtomicBool::new(false));
        write_jsonl(BrokenWriter, rx, Arc::clone(&failed));
        assert_eq!(done.recv().unwrap(), Err("codex_gateway:stdin_write_failed"));
        assert!(failed.load(Ordering::Acquire));
    }

    #[test]
    fn rejects_non_native_and_relative_binary_paths() {
        assert!(!acceptable_binary(Path::new("codex.exe")));
        assert!(!acceptable_binary(Path::new("codex.cmd")));
        assert!(!acceptable_binary(Path::new("codex.sh")));
    }

    #[test]
    fn npm_candidate_paths_are_finite_and_native_only() {
        let candidates = native_candidates_in_bin(Path::new("C:\\npm-root"));
        assert_eq!(candidates.len(), 3);
        for candidate in candidates {
            assert_eq!(candidate.file_name().and_then(OsStr::to_str), Some("codex.exe"));
        }
    }
}
