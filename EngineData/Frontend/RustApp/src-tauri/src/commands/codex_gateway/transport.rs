//! Sole owned Codex child and bounded JSONL transport. No shell, TCP listener,
//! external bearer token, automatic login, model turn, or audio session.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::protocol::{self, Inbound, MAX_FRAME_BYTES};
use super::realtime;

const EVENT_QUEUE_CAPACITY: usize = 32;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(8);
const WAIT_SLICE: Duration = Duration::from_millis(50);
type WireEvent = Result<Inbound, &'static str>;

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
    stdin: ChildStdin,
    receiver: Receiver<WireEvent>,
    overflow: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
    next_id: u64,
    child_reaped: bool,
    account_dirty: bool,
}

impl AppServer {
    pub(super) fn spawn() -> Result<Self, &'static str> {
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
        let (tx, receiver) = mpsc::sync_channel(EVENT_QUEUE_CAPACITY);
        let overflow = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&overflow);
        let reader = match thread::Builder::new()
            .name("translateit-codex-jsonl-reader".to_string())
            .spawn(move || read_jsonl(stdout, tx, observed))
        {
            Ok(reader) => reader,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("codex_gateway:reader_spawn_failed");
            }
        };
        Ok(Self {
            child, stdin, receiver, overflow, reader: Some(reader),
            next_id: 0, child_reaped: false, account_dirty: false,
        })
    }

    fn check_live(&mut self, cancelled: &AtomicBool) -> Result<(), &'static str> {
        if cancelled.load(Ordering::Acquire) {
            return Err("codex_gateway:cancelled");
        }
        if self.overflow.load(Ordering::Acquire) {
            return Err("codex_gateway:event_queue_overflow");
        }
        if self.child.try_wait().map_err(|_| "codex_gateway:child_probe_failed")?.is_some() {
            return Err("codex_gateway:child_exited");
        }
        Ok(())
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
        self.stdin.write_all(&body).and_then(|()| self.stdin.flush())
            .map_err(|_| "codex_gateway:stdin_write_failed")?;
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        loop {
            self.check_live(cancelled)?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("codex_gateway:rpc_timeout");
            }
            match self.receiver.recv_timeout(remaining.min(WAIT_SLICE)) {
                Ok(Ok(Inbound::Response { id, result })) if id == current_id => return Ok(result),
                Ok(Ok(Inbound::Response { .. })) => {
                    return Err("codex_gateway:unexpected_response_id");
                }
                Ok(Ok(Inbound::Notification { method, .. })) => {
                    // While a request is in flight, an account switch cannot
                    // be considered the same authenticated owner. A realtime
                    // event is forbidden before explicit Voice admission.
                    realtime::refuse_unadmitted_event(&method)?;
                    if method.starts_with("account/") {
                        self.account_dirty = true;
                    }
                }
                Ok(Err(reason)) => return Err(reason),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("codex_gateway:reader_disconnected");
                }
            }
        }
    }

    /// Account notifications arriving while an RPC response is in flight
    /// invalidate that account/usage snapshot; refresh in a bounded probe.
    pub(super) fn take_account_dirty(&mut self) -> bool {
        std::mem::take(&mut self.account_dirty)
    }

    pub(super) fn initialized(&mut self, cancelled: &AtomicBool) -> Result<(), &'static str> {
        self.check_live(cancelled)?;
        self.stdin.write_all(protocol::initialized_notification())
            .and_then(|()| self.stdin.flush())
            .map_err(|_| "codex_gateway:initialized_write_failed")
    }

    /// No automatic model requests. Only account changes invalidate the probe.
    pub(super) fn poll(
        &mut self,
        cancelled: &AtomicBool,
    ) -> Result<bool, &'static str> {
        self.check_live(cancelled)?;
        match self.receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(Inbound::Notification { method, .. })) if method.starts_with("account/") => Ok(true),
            Ok(Ok(Inbound::Notification { method, .. })) => {
                realtime::refuse_unadmitted_event(&method)?;
                Ok(false)
            },
            Ok(Ok(Inbound::Response { .. })) => Err("codex_gateway:unsolicited_response"),
            Ok(Err(reason)) => Err(reason),
            Err(RecvTimeoutError::Timeout) => Ok(false),
            Err(RecvTimeoutError::Disconnected) => {
                Err("codex_gateway:reader_disconnected")
            }
        }
    }

    /// A false result retains this exact owner. The caller must not mark the
    /// gateway clean or release its generation until the stdout reader exited.
    pub(super) fn stop_and_reap(&mut self) -> Result<bool, &'static str> {
        if !self.child_reaped {
            let still_running = self.child.try_wait()
                .map_err(|_| "codex_gateway:child_probe_failed")?.is_none();
            if still_running {
                self.child.kill().map_err(|_| "codex_gateway:child_stop_failed")?;
            }
            self.child.wait().map_err(|_| "codex_gateway:child_reap_failed")?;
            self.child_reaped = true;
        }
        if self.reader.as_ref().is_some_and(JoinHandle::is_finished) {
            let reader = self.reader.take().expect("checked reader present");
            reader.join().map_err(|_| "codex_gateway:reader_join_failed")?;
            return Ok(true);
        }
        Ok(false)
    }
}

impl Drop for AppServer {
    fn drop(&mut self) {
        // Panic/error backstop. This can wait for an inherited stdout handle:
        // do not silently detach an owned JSONL reader or claim clean release.
        if !self.child_reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

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
        read_jsonl(input, tx, Arc::clone(&overflow));
        assert!(matches!(rx.recv().unwrap().unwrap(), Inbound::Response { id: 1, .. }));
        assert!(matches!(rx.recv().unwrap().unwrap(),
            Inbound::Notification { method, .. } if method == "account/updated"));
        assert!(!overflow.load(Ordering::Acquire));
    }

    #[test]
    fn overflow_is_fail_closed_and_does_not_block_reader() {
        let (tx, rx) = mpsc::sync_channel(1);
        let overflow = Arc::new(AtomicBool::new(false));
        read_jsonl(std::io::Cursor::new(
            b"{\"method\":\"one\"}\n{\"method\":\"two\"}\n".to_vec()
        ), tx, Arc::clone(&overflow));
        assert!(overflow.load(Ordering::Acquire));
        assert!(matches!(rx.recv().unwrap().unwrap(),
            Inbound::Notification { method, .. } if method == "one"));
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
