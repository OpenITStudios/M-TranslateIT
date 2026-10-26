//! Owned CPU transport for the canonical finalized producer, never another capture.
use super::*;
use crate::engine::audio::pcm_handoff::{PcmDrain, PcmHandoff, PcmLane};
use crate::engine::audio::silero::{Silero, SpeechWindows};
use std::thread::{self, JoinHandle};

pub(super) struct SpeechIngress {
    pub session: String,
    pub generation: Option<u64>,
    pub lane: &'static str,
    rate: u32,
    queue: Arc<PcmHandoff>,
    thread: Option<JoinHandle<()>>,
    fault: Arc<Mutex<Option<String>>>,
    #[cfg(test)]
    submitted: AtomicU64,
    #[cfg(test)]
    processed: Arc<AtomicU64>,
    #[cfg(test)]
    observations: Arc<Mutex<Vec<serde_json::Value>>>,
}

impl SpeechIngress {
    pub fn start(sync: &'static FinalizedProducerSync, session: &str, generation: Option<u64>,
        lane: &'static str, rate: u32) -> Result<Self, String> {
        let mut model = Silero::load()?;
        let mut windows = SpeechWindows::new(rate)?;
        let queue = Arc::new(PcmHandoff::new(session,
            if lane == LANE_YOU { PcmLane::You } else { PcmLane::Incoming },
            generation.unwrap_or(1), rate as usize * 2).map_err(str::to_string)?);
        let for_worker = Arc::clone(&queue);
        let fault = Arc::new(Mutex::new(None));
        let worker_fault = Arc::clone(&fault);
        #[cfg(test)]
        let processed = Arc::new(AtomicU64::new(0));
        #[cfg(test)]
        let worker_processed = Arc::clone(&processed);
        let expected_session = session.to_string();
        #[cfg(test)]
        let observations = Arc::new(Mutex::new(Vec::new()));
        #[cfg(test)]
        let worker_observations = Arc::clone(&observations);
        let handle = thread::Builder::new().name(format!("translateit-{lane}-silero"))
            .spawn(move || {
                let mut run = || -> Result<(), String> {
                    let mut pcm = Vec::with_capacity(rate as usize / 5);
                    let mut loss_epoch = 0;
                    let mut audio_epoch = 1;
                    let mut observed_transport_loss = 0;
                    loop {
                        if for_worker.metrics().stopped { return Ok(()); }
                        let epoch = {
                            let mut guard = sync.state.lock().map_err(|_| "silero:producer_poisoned")?;
                            let Some(state) = guard.as_mut() else { return Ok(()) };
                            if state.session_id != expected_session || state.generation != generation {
                                return Ok(());
                            }
                            apply_capture_discontinuity(state);
                            state.capture_epoch
                        };
                        if epoch != audio_epoch {
                            model.reset(); windows.reset(); for_worker.invalidate(); audio_epoch = epoch;
                        }
                        let external_loss = sync.transport_loss.load(Ordering::Acquire);
                        if external_loss != observed_transport_loss {
                            for_worker.invalidate(); observed_transport_loss = external_loss;
                        }
                        match for_worker.drain_into(&mut pcm, rate as usize / 5, &mut loss_epoch) {
                            PcmDrain::Empty => { thread::park_timeout(Duration::from_millis(2)); continue; }
                            PcmDrain::Stopped => return Ok(()),
                            PcmDrain::Poisoned => return Err("silero:pcm_poisoned".to_string()),
                            PcmDrain::Discontinuity => {
                                model.reset(); windows.reset();
                                let mut guard = sync.state.lock().map_err(|_| "silero:producer_poisoned")?;
                                if let Some(state) = guard.as_mut() {
                                    state.capture_epoch = state.capture_epoch.saturating_add(1);
                                    state.input_sample_cursor = 0;
                                    reset_current_utterance(state); state.pending.clear();
                                    audio_epoch = state.capture_epoch;
                                }
                                continue;
                            }
                            PcmDrain::Samples(_) => {}
                        }
                        windows.push(&pcm);
                        while let Some((native, frame)) = windows.next() {
                            let probability = model.probability(&frame)?;
                            if for_worker.metrics().stopped { return Ok(()); }
                            let mut guard = sync.state.lock().map_err(|_| "silero:producer_poisoned")?;
                            let Some(state) = guard.as_mut() else { return Ok(()) };
                            let changed = apply_capture_discontinuity(state)
                                || state.capture_epoch != audio_epoch
                                || sync.transport_loss.load(Ordering::Acquire) != external_loss
                                || for_worker.metrics().discontinuity_epoch != loss_epoch;
                            if changed {
                                reset_current_utterance(state); state.pending.clear();
                                model.reset(); windows.reset(); for_worker.invalidate();
                                break;
                            }
                            state.input_sample_cursor = state.input_sample_cursor.saturating_add(native.len() as u64);
                            let evidence = AudioEvidenceReport::from_samples(&native);
                            let accepted = probability >= if state.in_utterance { 0.35 } else { 0.5 };
                            let clipping = evidence.clipping_ratio > state.profile.gate.max_clipping_ratio;
                            let gate = VadGateResult { accepted: accepted && !clipping,
                                reason: if clipping { "rejected_clipping" } else if accepted {
                                    "silero_speech" } else { "silero_silence" }.to_string(),
                                evidence: evidence.clone() };
                            #[cfg(test)]
                            if let Ok(mut observations) = worker_observations.lock() {
                                observations.push(serde_json::json!({"input_end_sample": state.input_sample_cursor,
                                    "probability": probability, "gate": gate}));
                            }
                            if ingest_observation(state, &native, &evidence, &gate) { sync.ready.notify_one(); }
                        }
                        #[cfg(test)]
                        worker_processed.fetch_add(pcm.len() as u64, Ordering::Release);
                    }
                };
                if let Err(error) = run() {
                    if let Ok(mut fault) = worker_fault.lock() { *fault = Some(error); }
                }
                for_worker.request_stop();
                if worker_fault.lock().map(|f| f.is_some()).unwrap_or(true) {
                    if let Ok(mut state) = sync.state.lock() { *state = None; sync.ready.notify_all(); }
                }
            }).map_err(|_| "silero:thread_spawn_failed".to_string())?;
        Ok(Self { session: session.to_string(), generation, lane, rate, queue,
            thread: Some(handle), fault,
            #[cfg(test)] submitted: AtomicU64::new(0),
            #[cfg(test)] processed,
            #[cfg(test)] observations })
    }

    pub fn push(&self, pcm: &[f32], rate: u32) {
        if rate != self.rate {
            if let Ok(mut fault) = self.fault.lock() { *fault = Some("silero:capture_rate_changed".to_string()); }
            self.queue.request_stop(); return;
        }
        if let crate::engine::audio::pcm_handoff::PcmPush::Accepted(_n) = self.queue.try_push(pcm) {
            #[cfg(test)]
            self.submitted.fetch_add(_n as u64, Ordering::Release);
        }
    }

    #[cfg(test)]
    pub fn observations(&self) -> Vec<serde_json::Value> { self.observations.lock().unwrap().clone() }

    pub fn fault(&self) -> Option<String> {
        self.fault.lock().map(|f| f.clone()).unwrap_or(Some("silero:fault_poisoned".to_string()))
    }

    pub fn dropped_callbacks(&self) -> u64 { self.queue.metrics().dropped_callbacks }

    pub fn stop(&mut self) -> bool {
        self.stop_with_budget(Duration::from_millis(750))
    }

    fn stop_with_budget(&mut self, budget: Duration) -> bool {
        self.queue.request_stop();
        if let Some(handle) = self.thread.as_ref() {
            handle.thread().unpark();
            let deadline = Instant::now() + budget;
            while !handle.is_finished() {
                if Instant::now() >= deadline { return false; }
                thread::sleep(Duration::from_millis(1));
            }
        }
        if let Some(handle) = self.thread.take() {
            let ok = handle.join().is_ok(); self.queue.stop(); return ok;
        }
        true
    }

    #[cfg(test)]
    pub fn wait_processed(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.processed.load(Ordering::Acquire) < self.submitted.load(Ordering::Acquire) {
            assert!(self.fault().is_none(), "{:?}", self.fault());
            assert!(Instant::now() < deadline, "canonical speech CPU drain timed out");
            thread::sleep(Duration::from_millis(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_timeout_keeps_the_cpu_lease_until_a_confirmed_join() {
        let (release, blocked) = std::sync::mpsc::sync_channel(1);
        let handle = thread::spawn(move || { blocked.recv().unwrap(); });
        let queue = Arc::new(PcmHandoff::new("stalled-speech", PcmLane::Incoming, 1, 16_000).unwrap());
        let mut cpu = SpeechIngress {
            session: "stalled-speech".to_string(), generation: None, lane: LANE_INCOMING,
            rate: 16_000, queue: Arc::clone(&queue), thread: Some(handle),
            fault: Arc::new(Mutex::new(None)), submitted: AtomicU64::new(0),
            processed: Arc::new(AtomicU64::new(0)), observations: Arc::new(Mutex::new(Vec::new())),
        };
        assert!(!cpu.stop_with_budget(Duration::ZERO));
        assert!(cpu.thread.is_some(), "timeout cannot release a live worker lease");
        assert!(queue.metrics().stopped, "admission must already be revoked");
        assert!(matches!(queue.try_push(&[0.1]), crate::engine::audio::pcm_handoff::PcmPush::Dropped(_)));
        release.send(()).unwrap();
        assert!(cpu.stop());
        assert!(cpu.thread.is_none());
        assert_eq!(queue.metrics().buffered_samples, 0);
    }
}
