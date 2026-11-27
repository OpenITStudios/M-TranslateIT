//! One execution thread for the existing scheduler's already-admitted request.
//! This module does not select priority, retry, load a fallback or queue waiters.
use super::{Error, Language, RequestControl, Runtime, MAX_OUTPUT_BYTES};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Instant,
};

/// Transfer the existing owned buffer, not a cloned PCM vector or a WAV path.
pub enum InferenceInput {
    Speech { pcm: Vec<f32>, language: Language },
    Translation { source: String, from: Language, to: Language },
    // Candidate: authorized, 16 kHz mono F32 reference. Never a model path.
    Voice { normalized: String, reference: Vec<f32> },
}

#[derive(Debug, PartialEq)]
pub enum InferenceOutput {
    Text(String),
    Voice(Vec<f32>),
}

struct RequestState {
    cancelled: AtomicBool,
    done: AtomicBool,
}

struct Request {
    input: InferenceInput,
    state: Arc<RequestState>,
    deadline: Instant,
    reply: SyncSender<Result<InferenceOutput, Error>>,
}

/// Keep the existing scheduler permit until wait returns (or this receipt is
/// dropped). Never wait or drop an unfinished receipt under the lifecycle lock.
#[must_use]
pub struct PendingInference {
    reply: Option<Receiver<Result<InferenceOutput, Error>>>,
    state: Arc<RequestState>,
    deadline: Instant,
}

impl PendingInference {
    pub fn wait(self) -> Result<String, Error> {
        match self.wait_output()? {
            InferenceOutput::Text(text) => Ok(text),
            InferenceOutput::Voice(_) => Err(Error::InvalidReceipt),
        }
    }

    pub fn wait_output(mut self) -> Result<InferenceOutput, Error> {
        let reply = self.reply.take().expect("one owned inference reply");
        let result = reply.recv().map_err(|_| Error::OwnerThreadFailed)??;
        // A stop/deadline after native completion but before host consumption
        // still rejects the text. Host generation/session checks remain required.
        super::check_control(&RequestControl {
            cancelled: &self.state.cancelled, deadline: self.deadline,
        })?;
        Ok(result)
    }
}

impl Drop for PendingInference {
    fn drop(&mut self) {
        if let Some(reply) = self.reply.take() {
            self.state.cancelled.store(true, Ordering::Release);
            // A dropped receipt must not silently detach native work. The
            // existing permit owner can unwind only after this receive ends.
            let _ = reply.recv();
        }
        // Keep the slot occupied through receipt consumption, not just
        // native completion. A late Stop can still revoke this reply.
        self.state.done.store(true, Ordering::Release);
    }
}

/// The host retains this owner under its existing lifecycle authority. It is
/// not a second scheduler: one admitted request may occupy the handoff slot.
pub struct RuntimeOwner {
    device_id: String,
    sender: Option<SyncSender<Request>>,
    thread: Option<JoinHandle<()>>,
    active: Option<Arc<RequestState>>,
    join_failed: bool,
}

impl RuntimeOwner {
    /// Model load and its rollback occur entirely on the owner thread.
    /// Open is synchronous; the host must retain its startup ownership until
    /// it returns. It is not a cancellable GPU-load or UI-thread operation.
    ///
    /// # Safety
    /// Supply reviewed build pins, a trusted parent and system dependency policy.
    pub unsafe fn open(dll: &Path, pins: &str, asr: &Path, projector: &Path, mt: &Path,
                       pci_id: Option<&str>) -> Result<Self, Error> {
        unsafe { Self::open_with_voice(dll, pins, asr, projector, mt, pci_id, None) }
    }

    /// Optional candidate voice is opened with the same resident pair, on this
    /// same execution thread; no separate scheduler, process or runtime.
    /// # Safety
    /// Same trusted asset and build-pin preconditions as open.
    pub unsafe fn open_with_voice(dll: &Path, pins: &str, asr: &Path, projector: &Path, mt: &Path,
                                  pci_id: Option<&str>, voice: Option<(&Path, &Path)>) -> Result<Self, Error> {
        let voice = voice.map(|(model, projector)| (model.to_owned(), projector.to_owned()));
        let (dll, pins, asr, projector, mt, pci_id) =
            (dll.to_owned(), pins.to_owned(), asr.to_owned(), projector.to_owned(),
             mt.to_owned(), pci_id.map(str::to_owned));
        // A one-slot handoff, guarded by the one in-flight request below.
        // No queued second job, no priority policy, no polling or extra timer.
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = thread::Builder::new().name("translateit-inference".into()).spawn(move || {
            let mut runtime = match unsafe { Runtime::open_with_voice(&dll, &pins, &asr, &projector, &mt,
                pci_id.as_deref(), voice.as_ref().map(|(model, projector)| (model.as_path(), projector.as_path()))) } {
                Ok(runtime) => runtime,
                Err(error) => { let _ = ready_tx.send(Err(error)); return; }
            };
            let mut output = vec![0u8; MAX_OUTPUT_BYTES];
            if ready_tx.send(Ok(runtime.device_id().to_owned())).is_err() { return; }
            drop(ready_tx);
            while let Ok(request) = requests.recv() {
                let Request { input, state, deadline, reply } = request;
                let control = RequestControl { cancelled: &state.cancelled, deadline };
                let result = match input {
                    InferenceInput::Speech { pcm, language } =>
                        runtime.transcribe(&pcm, language, &control, &mut output)
                            .map(|text| InferenceOutput::Text(text.to_owned())),
                    InferenceInput::Translation { source, from, to } =>
                        runtime.translate(&source, from, to, &control, &mut output)
                            .map(|text| InferenceOutput::Text(text.to_owned())),
                    InferenceInput::Voice { normalized, reference } =>
                        runtime.synthesize(&normalized, &reference, &control).map(InferenceOutput::Voice),
                };
                // Input is released before the result; the only text copy is
                // the owned Rust result leaving reusable native scratch.
                let _ = reply.send(result);
            }
            // Runtime drops on this exact thread, after all admitted work.
        }).map_err(|_| Error::OwnerUnavailable)?;

        match ready_rx.recv() {
            Ok(Ok(device_id)) => Ok(Self {
                device_id,
                sender: Some(sender), thread: Some(thread), active: None, join_failed: false,
            }),
            result => {
                drop(sender);
                if thread.join().is_err() { return Err(Error::OwnerThreadFailed); }
                match result {
                    Ok(Err(error)) => Err(error),
                    _ => Err(Error::OwnerThreadFailed),
                }
            }
        }
    }

    pub fn device_id(&self) -> &str { &self.device_id }

    /// Resource observation, not functional inference or CUDA residency proof.
    pub fn is_available(&self) -> bool {
        !self.join_failed && self.sender.is_some()
            && self.thread.as_ref().is_some_and(|thread| !thread.is_finished())
    }

    /// Call only after existing scheduler admission and session/route checks.
    /// The caller retains its permit through PendingInference::wait.
    pub fn begin(&mut self, input: InferenceInput, deadline: Instant) -> Result<PendingInference, Error> {
        if !self.is_available() {
            return Err(Error::OwnerUnavailable);
        }
        let sender = self.sender.as_ref().ok_or(Error::OwnerUnavailable)?;
        if self.active.as_ref().is_some_and(|state| !state.done.load(Ordering::Acquire)) {
            return Err(Error::OwnerBusy);
        }
        if Instant::now() >= deadline { return Err(Error::DeadlineExceeded); }
        let state = Arc::new(RequestState {
            cancelled: AtomicBool::new(false), done: AtomicBool::new(false),
        });
        let (reply, receiver) = mpsc::sync_channel(1);
        sender.try_send(Request { input, state: Arc::clone(&state), deadline, reply })
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => Error::OwnerBusy,
                mpsc::TrySendError::Disconnected(_) => Error::OwnerUnavailable,
            })?;
        self.active = Some(Arc::clone(&state));
        Ok(PendingInference { reply: Some(receiver), state, deadline })
    }

    /// Only the existing host's exact session/request owner may call this.
    /// Signal even if native work completed but its receipt is not consumed.
    pub fn cancel_active(&self) {
        if let Some(state) = &self.active {
            state.cancelled.store(true, Ordering::Release);
        }
    }

    /// Nonblocking cleanup observation. A false result is not a clean Stop.
    /// The caller MUST retain this owner and retry, never replace or drop it.
    pub fn try_close(&mut self) -> Result<bool, Error> {
        self.cancel_active();
        drop(self.sender.take());
        if self.join_failed { return Err(Error::OwnerThreadFailed); }
        if self.thread.as_ref().is_some_and(|thread| !thread.is_finished()) {
            return Ok(false);
        }
        if self.thread.take().is_some_and(|thread| thread.join().is_err()) {
            self.join_failed = true;
            return Err(Error::OwnerThreadFailed);
        }
        self.active = None;
        Ok(true)
    }
}

impl Drop for RuntimeOwner {
    fn drop(&mut self) {
        self.cancel_active();
        drop(self.sender.take());
        // Safety backstop, not a bounded Stop path. The lifecycle must use
        // try_close and retain incomplete cleanup rather than blocking here.
        if let Some(thread) = self.thread.take() { let _ = thread.join(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn admitted_handoff_retains_blocked_owner_and_rejects_second_request() {
        // Mechanical lifecycle fixture only; no fake model/CUDA proof.
        let (sender, requests) = mpsc::sync_channel::<Request>(1);
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let thread = thread::spawn(move || {
            let request = requests.recv().unwrap();
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let cancelled = request.state.cancelled.load(Ordering::Acquire);
            request.reply.send(if cancelled { Err(Error::Cancelled) } else {
                Ok(InferenceOutput::Text("unexpected".to_string()))
            }).unwrap();
        });
        let mut owner = RuntimeOwner {
            device_id: "fixture-only".into(),
            sender: Some(sender), thread: Some(thread), active: None, join_failed: false,
        };
        let input = || InferenceInput::Translation {
            source: "Halo".into(), from: Language::Indonesian, to: Language::English,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        let pending = owner.begin(input(), deadline).unwrap();
        // Release the fixture on every assertion/unwind path before Drop joins.
        let entered = entered_rx.recv_timeout(Duration::from_secs(5));
        let second = owner.begin(input(), deadline);
        let close = owner.try_close();
        let available_after_close = owner.is_available();
        let retained = owner.thread.is_some();
        let _ = release_tx.send(());
        let result = pending.wait();
        assert!(entered.is_ok());
        assert!(matches!(second, Err(Error::OwnerBusy)));
        assert_eq!(close, Ok(false));
        assert!(!available_after_close);
        assert!(retained);
        assert_eq!(result, Err(Error::Cancelled));
        let deadline = Instant::now() + Duration::from_secs(5);
        while owner.thread.as_ref().is_some_and(|thread| !thread.is_finished()) {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        assert_eq!(owner.try_close(), Ok(true));
        assert!(owner.thread.is_none());
    }
}
