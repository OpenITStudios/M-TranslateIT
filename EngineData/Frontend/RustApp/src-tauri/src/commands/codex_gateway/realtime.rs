//! Source-derived, offline Codex Realtime V1/V3 acceptance boundary.
//!
//! This module does NOT start Voice, send model requests, or authorize TTS.
//! A role-bearing assistant transcript is merely an uncorrelated candidate:
//! upstream events lack a canonical source-utterance sequence. Production
//! admission requires an independently verified audio/source binding and a
//! validated included-usage entitlement. Do not promote a candidate to speech.

use std::collections::VecDeque;
use serde_json::{json, Value};

const MAX_ID_BYTES: usize = 160;
const MAX_TEXT_BYTES: usize = 8 * 1024;
const MAX_TRACKED_ITEMS: usize = 256;
const MAX_AUDIO_FRAMES: usize = 4;
const SAMPLES_PER_FRAME: usize = 4_800; // 200 ms at 24 kHz
const AUDIO_RATE: u32 = 24_000;

#[derive(PartialEq, Eq)]
pub(super) enum Observation {
    Ignored,
    SourceTranscript { item_id: String, text: String },
    /// Role and source pairing are NOT established by this observation.
    /// Never forward this to GPT-SoVITS or Meeting committed turns.
    UnverifiedAssistantText { item_id: String, text: String },
    CloudAudioIgnored,
    Closed,
}

// These observations may contain private meeting content. Never expose it via
// automatically derived Debug when a validator rejects an event.
impl std::fmt::Debug for Observation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ignored => f.write_str("Ignored"),
            Self::CloudAudioIgnored => f.write_str("CloudAudioIgnored"),
            Self::Closed => f.write_str("Closed"),
            Self::SourceTranscript { .. } => f.write_str("SourceTranscript(<redacted>)"),
            Self::UnverifiedAssistantText { .. } => {
                f.write_str("UnverifiedAssistantText(<redacted>)")
            }
        }
    }
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_BYTES
        && value.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_BYTES
        && !value.chars().any(|ch| {
            (ch.is_control() && ch != '\n' && ch != '\t')
                || matches!(ch, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
}

/// Codex publishes RealtimeEvent::HandoffRequested as itemAdded with
/// item.type=handoff_request. Upstream can start an agent turn before
/// forwarding the event. This detects an unsafe session but cannot prevent
/// upstream agent execution; do NOT treat it as media/Voice authorization.
/// No private item text may enter logs or error strings.
pub(super) fn reject_unsafe_realtime_item(
    method: &str, params: &Value,
) -> Result<(), &'static str> {
    match method {
        "thread/realtime/itemAdded" => {
            let item = params.get("item").and_then(Value::as_object)
                .ok_or("codex_realtime:item_added_invalid")?;
            match item.get("type").and_then(Value::as_str) {
                Some("handoff_request" | "function_call") =>
                    Err("codex_realtime:agent_handoff_observed"),
                Some("bemItemPromoted") =>
                    Err("codex_realtime:agent_item_promoted"),
                // Only non-authoritative capture metadata or user input.
                Some("input_audio_buffer.speech_started" | "response.cancelled") => Ok(()),
                Some("message") if item.get("role").and_then(Value::as_str) == Some("user") =>
                    Ok(()),
                _ => Err("codex_realtime:item_added_unadmitted"),
            }
        }
        "thread/realtime/item/started" | "thread/realtime/item/completed"
            if params.pointer("/item/type").and_then(Value::as_str) == Some("bemItemPromoted") =>
                Err("codex_realtime:agent_item_promoted"),
        _ => Ok(()),
    }
}

/// An observer for *one* Codex realtime thread/session, not a new Meeting owner.
/// No transcript/source text is stored after the observation is returned.
pub(super) struct RealtimeObserver {
    thread_id: String,
    session_id: Option<String>,
    seen_ids: Vec<String>,
    revoked: bool,
}

impl RealtimeObserver {
    pub(super) fn new(thread_id: &str) -> Result<Self, &'static str> {
        if !safe_id(thread_id) {
            return Err("codex_realtime:thread_identity_invalid");
        }
        Ok(Self {
            thread_id: thread_id.to_string(),
            session_id: None,
            seen_ids: Vec::new(),
            revoked: false,
        })
    }

    pub(super) fn revoke(&mut self) {
        self.revoked = true;
        self.session_id = None;
        self.seen_ids.clear();
    }

    pub(super) fn observe(
        &mut self,
        method: &str,
        params: &Value,
    ) -> Result<Observation, &'static str> {
        let result = self.observe_inner(method, params);
        if result.is_err() {
            self.revoke();
        }
        result
    }

    fn observe_inner(
        &mut self,
        method: &str,
        params: &Value,
    ) -> Result<Observation, &'static str> {
        if self.revoked {
            return Err("codex_realtime:session_revoked");
        }
        if !method.starts_with("thread/realtime/") {
            return Err("codex_realtime:method_not_realtime");
        }
        let thread_id = params.get("threadId").and_then(Value::as_str)
            .ok_or("codex_realtime:thread_identity_missing")?;
        if thread_id != self.thread_id {
            return Err("codex_realtime:foreign_thread");
        }
        reject_unsafe_realtime_item(method, params)?;

        if method == "thread/realtime/started" {
            if self.session_id.is_some() {
                return Err("codex_realtime:duplicate_session_start");
            }
            // V1/V3 are the currently documented WebRTC candidates for
            // ChatGPT auth. Neither proves that output text-only is supported.
            let version = params.get("version").and_then(Value::as_str)
                .ok_or("codex_realtime:version_missing")?;
            if !matches!(version, "v1" | "v3") {
                return Err("codex_realtime:voice_version_unadmitted");
            }
            let session = params.get("realtimeSessionId").and_then(Value::as_str)
                .ok_or("codex_realtime:session_identity_missing")?;
            if !safe_id(session) {
                return Err("codex_realtime:session_identity_invalid");
            }
            self.session_id = Some(session.to_string());
            return Ok(Observation::Ignored);
        }

        let expected = self.session_id.as_deref()
            .ok_or("codex_realtime:session_not_started")?;
        match method {
            "thread/realtime/item/completed" => {
                let item = params.get("item").and_then(Value::as_object)
                    .ok_or("codex_realtime:item_invalid")?;
                let id = item.get("id").and_then(Value::as_str)
                    .ok_or("codex_realtime:item_id_missing")?;
                let session_id = item.get("realtimeSessionId").and_then(Value::as_str)
                    .ok_or("codex_realtime:item_session_missing")?;
                if !safe_id(id) || session_id != expected {
                    return Err("codex_realtime:item_identity_mismatch");
                }
                if self.seen_ids.iter().any(|seen| seen == id) {
                    return Err("codex_realtime:duplicate_item");
                }
                if self.seen_ids.len() >= MAX_TRACKED_ITEMS {
                    // Without an authoritative source-sequence binding, never
                    // evict old IDs and risk replaying stale assistant text.
                    return Err("codex_realtime:identity_window_exhausted");
                }
                self.seen_ids.push(id.to_string());
                match item.get("type").and_then(Value::as_str) {
                    Some("transcriptSegment") => {
                        let text = item.get("text").and_then(Value::as_str)
                            .ok_or("codex_realtime:final_text_missing")?;
                        if !safe_text(text) {
                            return Err("codex_realtime:final_text_invalid");
                        }
                        match item.get("role").and_then(Value::as_str) {
                            Some("user") => Ok(Observation::SourceTranscript {
                                item_id: id.to_string(),
                                text: text.to_string(),
                            }),
                            Some("assistant") => Ok(Observation::UnverifiedAssistantText {
                                item_id: id.to_string(),
                                text: text.to_string(),
                            }),
                            _ => Err("codex_realtime:final_role_invalid"),
                        }
                    }
                    Some("realtimeSessionClosed") => {
                        // An upstream realtime item distinguishes ended from failed.
                        // Only a successful outcome can be a clean session close.
                        // observe() revokes the session on every error.
                        match item.get("outcome").and_then(Value::as_str) {
                            Some("ended") => {
                                self.revoke();
                                Ok(Observation::Closed)
                            }
                            Some("failed") => Err("codex_realtime:session_failed"),
                            _ => Err("codex_realtime:session_outcome_invalid"),
                        }
                    }
                    Some("realtimeSessionStarted") => Ok(Observation::Ignored),
                    // Agent item promotion is incompatible with a tool-free
                    // translation-only session. This detects it after the
                    // fact, not before the Codex agent may have run.
                    Some("bemItemPromoted") => Err("codex_realtime:agent_item_promoted"),
                    _ => Err("codex_realtime:item_type_unrecognized"),
                }
            }
            // Flat transcript/done has no item ID or verified source pairing.
            // Item/delta is likewise provisional and MUST NOT feed TTS.
            "thread/realtime/item/started" => {
                let item = params.get("item").and_then(Value::as_object)
                    .ok_or("codex_realtime:started_item_invalid")?;
                if item.get("type").and_then(Value::as_str) == Some("bemItemPromoted") {
                    return Err("codex_realtime:agent_item_promoted");
                }
                Ok(Observation::Ignored)
            }
            "thread/realtime/transcript/delta"
            | "thread/realtime/item/transcript/delta"
            | "thread/realtime/itemAdded"
            | "thread/realtime/sdp" => Ok(Observation::Ignored),
            "thread/realtime/transcript/done" => {
                match params.get("role").and_then(Value::as_str) {
                    Some("user" | "assistant") => Ok(Observation::Ignored),
                    _ => Err("codex_realtime:flat_role_invalid"),
                }
            }
            "thread/realtime/outputAudio/delta" => Ok(Observation::CloudAudioIgnored),
            "thread/realtime/closed" => match params.get("reason").and_then(Value::as_str) {
                // Upstream reports "requested" only for an intentional Stop.
                Some("requested") => {
                    self.revoke();
                    Ok(Observation::Closed)
                }
                Some("error" | "transport_closed") =>
                    Err("codex_realtime:transport_closed_unverified"),
                _ => Err("codex_realtime:close_reason_invalid"),
            },
            "thread/realtime/error" => Err("codex_realtime:remote_error"),
            _ => Err("codex_realtime:unrecognized_event"),
        }
    }
}

/// The current account-probe owner has no admitted realtime session.
/// Receiving any realtime notification in this context is a protocol fault.
pub(super) fn refuse_unadmitted_event(method: &str) -> Result<(), &'static str> {
    if method.starts_with("thread/realtime/") {
        Err("codex_realtime:unsolicited_without_voice_authority")
    } else {
        Ok(())
    }
}

struct RawFrame {
    start_sample: u64,
    pcm: Vec<i16>,
}

/// Bounded source-clock queue for a future audio adapter, not a CPAL callback.
/// Any clock gap, overflow or Stop invalidates the whole queue rather than
/// silently losing words. New sessions create a new queue.
#[derive(Default)]
pub(super) struct AudioFrameQueue {
    frames: VecDeque<RawFrame>,
    next_sample: Option<u64>,
    invalid: bool,
}

impl AudioFrameQueue {
    pub(super) fn push(
        &mut self, start_sample: u64, samples: &[i16]
    ) -> Result<(), &'static str> {
        if self.invalid {
            return Err("codex_realtime:audio_queue_revoked");
        }
        if samples.is_empty() || samples.len() > SAMPLES_PER_FRAME {
            self.revoke();
            return Err("codex_realtime:audio_frame_size_invalid");
        }
        if self.next_sample.is_some_and(|expected| expected != start_sample) {
            self.revoke();
            return Err("codex_realtime:audio_clock_gap");
        }
        if self.frames.len() >= MAX_AUDIO_FRAMES {
            self.revoke();
            return Err("codex_realtime:audio_backpressure");
        }
        let Some(end) = start_sample.checked_add(samples.len() as u64) else {
            self.revoke();
            return Err("codex_realtime:audio_clock_overflow");
        };
        self.next_sample = Some(end);
        self.frames.push_back(RawFrame {
            start_sample,
            pcm: samples.to_vec(),
        });
        Ok(())
    }

    /// Distinguish an empty queue from a revoked audio continuity lease.
    /// A revoked frame queue must never be treated as an ordinary idle tick.
    pub(super) fn pending_state(&self) -> Result<bool, &'static str> {
        if self.invalid {
            Err("codex_realtime:audio_queue_revoked")
        } else {
            Ok(!self.frames.is_empty())
        }
    }

    pub(super) fn pop(&mut self) -> Option<(u64, Value)> {
        if self.invalid {
            return None;
        }
        let frame = self.frames.pop_front()?;
        let mut bytes = Vec::with_capacity(frame.pcm.len() * 2);
        for sample in &frame.pcm {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        // Exactly the ThreadRealtimeAudioChunk wire fields; the owner must
        // still verify negotiated audio format and Voice entitlement.
        let audio = json!({
            "data": base64_pcm(&bytes),
            "sampleRate": AUDIO_RATE,
            "numChannels": 1,
            "samplesPerChannel": frame.pcm.len() as u32,
            "itemId": null,
        });
        Some((frame.start_sample, audio))
    }

    pub(super) fn revoke(&mut self) {
        self.frames.clear();
        self.next_sample = None;
        self.invalid = true;
    }
}

/// Stateful input-rate -> 24-kHz PCM16 converter for the future Codex
/// realtime input. Called by an owned non-callback consumer, never CPAL.
/// One prior sample preserves interpolation across microphone buffer seams.
pub(super) struct Pcm24kResampler {
    input_rate: u32,
    next_input: u64,
    next_output: u64,
    previous: Option<f32>,
    revoked: bool,
}

impl Pcm24kResampler {
    pub(super) fn new(input_rate: u32) -> Result<Self, &'static str> {
        if !(8_000..=96_000).contains(&input_rate) {
            return Err("codex_realtime:input_rate_unsupported");
        }
        Ok(Self { input_rate, next_input: 0, next_output: 0, previous: None, revoked: false })
    }

    pub(super) fn revoke(&mut self) {
        self.revoked = true;
        self.previous = None;
    }

    /// Convert one bounded mono-f32 microphone chunk and enqueue PCM16.
    /// A clock gap, invalid frame or rejected queue invalidates both owners.
    pub(super) fn push_mono_f32(
        &mut self,
        start_input: u64,
        samples: &[f32],
        queue: &mut AudioFrameQueue,
    ) -> Result<(), &'static str> {
        match self.convert(start_input, samples) {
            Ok((start_output, pcm)) => {
                if !pcm.is_empty() {
                    if let Err(blocker) = queue.push(start_output, &pcm) {
                        self.revoke();
                        queue.revoke();
                        return Err(blocker);
                    }
                }
                Ok(())
            }
            Err(blocker) => {
                self.revoke();
                queue.revoke();
                Err(blocker)
            }
        }
    }

    fn convert(
        &mut self,
        start_input: u64,
        samples: &[f32],
    ) -> Result<(u64, Vec<i16>), &'static str> {
        if self.revoked {
            return Err("codex_realtime:resampler_revoked");
        }
        if start_input != self.next_input {
            return Err("codex_realtime:input_clock_gap");
        }
        // 200-ms maximum source chunk. This ensures at most one queued
        // 4,800-sample output frame without silently dropping overflow.
        if samples.is_empty()
            || samples.len() > (self.input_rate / 5) as usize
            || samples.iter().any(|x| !x.is_finite())
        {
            return Err("codex_realtime:source_frame_invalid");
        }
        let end_input = start_input.checked_add(samples.len() as u64)
            .ok_or("codex_realtime:input_clock_overflow")?;
        let first_output = self.next_output;
        let capacity = ((samples.len() as u64 * AUDIO_RATE as u64) / self.input_rate as u64)
            .min(SAMPLES_PER_FRAME as u64) as usize + 1;
        let mut pcm = Vec::with_capacity(capacity);
        loop {
            let pos = self.next_output.checked_mul(self.input_rate as u64)
                .ok_or("codex_realtime:output_clock_overflow")?;
            let left = pos / AUDIO_RATE as u64;
            let rem = pos % AUDIO_RATE as u64;
            let right = left + u64::from(rem != 0);
            if right >= end_input { break; }
            if pcm.len() >= SAMPLES_PER_FRAME {
                return Err("codex_realtime:source_frame_overflow");
            }
            let at = |idx: u64| -> Result<f32, &'static str> {
                if idx >= start_input {
                    samples.get((idx - start_input) as usize).copied()
                        .ok_or("codex_realtime:source_history_missing")
                } else if idx.checked_add(1) == Some(start_input) {
                    self.previous.ok_or("codex_realtime:source_history_missing")
                } else {
                    Err("codex_realtime:source_history_missing")
                }
            };
            let lo = at(left)?;
            let hi = at(right)?;
            let fraction = rem as f32 / AUDIO_RATE as f32;
            let mixed = (lo + (hi - lo) * fraction).clamp(-1.0, 1.0);
            pcm.push((mixed * f32::from(i16::MAX)).round() as i16);
            self.next_output = self.next_output.checked_add(1)
                .ok_or("codex_realtime:output_clock_overflow")?;
        }
        self.next_input = end_input;
        self.previous = samples.last().copied();
        Ok((first_output, pcm))
    }
}

// Deterministic Base64 for small, bounded PCM chunks. Avoid adding a second
// dependency or a dynamic codec to the Windows realtime boundary.
fn base64_pcm(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(ALPHABET[(first >> 2) as usize] as char);
        output.push(ALPHABET[(((first & 3) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() >= 2 {
            output.push(ALPHABET[(((second & 15) << 2) | (third >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() >= 3 {
            output.push(ALPHABET[(third & 63) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, session: &str, role: &str, text: &str) -> Value {
        json!({
            "threadId":"thr_a",
            "item":{"id":id, "realtimeSessionId":session,
                    "type":"transcriptSegment", "role":role, "text":text}
        })
    }
    fn started() -> Value {
        json!({"threadId":"thr_a","realtimeSessionId":"sess_a","version":"v3"})
    }
    fn observer() -> RealtimeObserver {
        let mut o = RealtimeObserver::new("thr_a").unwrap();
        o.observe("thread/realtime/started", &started()).unwrap();
        o
    }

    #[test]
    fn debug_representation_does_not_dump_private_spoken_text() {
        let candidate = Observation::UnverifiedAssistantText {
            item_id: "out_1".into(),
            text: "sensitive utterance".into(),
        };
        let debug = format!("{candidate:?}");
        assert!(!debug.contains("sensitive utterance"));
    }

    #[test]
    fn source_and_assistant_are_distinct_and_neither_is_a_tts_commit() {
        let mut o = observer();
        assert_eq!(
            o.observe("thread/realtime/item/completed", &item("i1","sess_a","user","Mulai sekarang.")).unwrap(),
            Observation::SourceTranscript { item_id:"i1".into(), text:"Mulai sekarang.".into() }
        );
        assert_eq!(
            o.observe("thread/realtime/item/completed", &item("i2","sess_a","assistant","Start now.")).unwrap(),
            Observation::UnverifiedAssistantText { item_id:"i2".into(), text:"Start now.".into() }
        );
        assert_eq!(
            o.observe("thread/realtime/transcript/done",
                &json!({"threadId":"thr_a","role":"assistant","text":"Not correlated."})).unwrap(),
            Observation::Ignored
        );
    }

    #[test]
    fn rejects_wrong_thread_session_duplicate_and_stale_events() {
        let mut wrong = observer();
        assert_eq!(
            wrong.observe("thread/realtime/item/completed", &item("i1","sess_else","assistant","Hello")),
            Err("codex_realtime:item_identity_mismatch")
        );
        assert_eq!(
            wrong.observe("thread/realtime/item/completed", &item("i2","sess_a","assistant","Hello")),
            Err("codex_realtime:session_revoked")
        );
        let mut duplicate = observer();
        duplicate.observe("thread/realtime/item/completed", &item("i1","sess_a","assistant","Hello")).unwrap();
        assert_eq!(
            duplicate.observe("thread/realtime/item/completed", &item("i1","sess_a","assistant","Hello")),
            Err("codex_realtime:duplicate_item")
        );
        let mut thread = observer();
        assert_eq!(
            thread.observe("thread/realtime/closed", &json!({"threadId":"foreign"})),
            Err("codex_realtime:foreign_thread")
        );
    }

    #[test]
    fn failed_or_missing_realtime_outcome_cannot_become_a_clean_close() {
        let closed = |outcome: Value| json!({
            "threadId": "thr_a",
            "item": {
                "id": "close_1",
                "realtimeSessionId": "sess_a",
                "type": "realtimeSessionClosed",
                "outcome": outcome
            }
        });

        let mut ended = observer();
        assert_eq!(
            ended.observe("thread/realtime/item/completed", &closed(json!("ended"))),
            Ok(Observation::Closed)
        );
        assert_eq!(
            ended.observe("thread/realtime/item/completed", &closed(json!("ended"))),
            Err("codex_realtime:session_revoked")
        );

        let mut failed = observer();
        assert_eq!(
            failed.observe("thread/realtime/item/completed", &closed(json!("failed"))),
            Err("codex_realtime:session_failed")
        );
        assert_eq!(
            failed.observe("thread/realtime/transcript/done",
                &json!({"threadId":"thr_a","role":"assistant","text":"late"})),
            Err("codex_realtime:session_revoked")
        );

        let mut invalid = observer();
        assert_eq!(
            invalid.observe("thread/realtime/item/completed", &closed(Value::Null)),
            Err("codex_realtime:session_outcome_invalid")
        );
    }

    #[test]
    fn ignores_cloud_audio_and_never_promotes_drafts() {
        let mut o = observer();
        assert_eq!(o.observe("thread/realtime/outputAudio/delta",
            &json!({"threadId":"thr_a","audio":{"data":"AQID"}})).unwrap(),
            Observation::CloudAudioIgnored);
        assert_eq!(o.observe("thread/realtime/item/transcript/delta",
            &json!({"threadId":"thr_a","itemId":"i1","delta":"not final"})).unwrap(),
            Observation::Ignored);
        assert_eq!(o.observe("thread/realtime/error", &json!({"threadId":"thr_a","message":"private"})),
            Err("codex_realtime:remote_error"));
        assert!(refuse_unadmitted_event("thread/realtime/started").is_err());
    }

    #[test]
    fn coding_agent_item_promoted_revokes_realtime_observer() {
        let promoted = json!({"threadId":"thr_a","item":{
            "id":"promoted_1", "realtimeSessionId":"sess_a",
            "type":"bemItemPromoted","turnId":"turn_1",
            "itemId":"coding_item_1","presentation":{"type":"wholeItem"}
        }});
        let mut completed = observer();
        assert_eq!(
            completed.observe("thread/realtime/item/completed", &promoted),
            Err("codex_realtime:agent_item_promoted")
        );
        assert_eq!(
            completed.observe("thread/realtime/item/completed",
                &item("late","sess_a","assistant","Unsafe late text")),
            Err("codex_realtime:session_revoked")
        );
        let mut started = observer();
        assert_eq!(
            started.observe("thread/realtime/item/started", &promoted),
            Err("codex_realtime:agent_item_promoted")
        );
    }

    #[test]
    fn upstream_handoff_item_added_retires_unverified_translation_before_next_item() {
        let handoff = json!({"threadId":"thr_a","item":{
            "type":"handoff_request","handoff_id":"h1",
            "item_id":"agent_1","input_transcript":"private spoken command",
            "active_transcript":[]
        }});
        let mut o = observer();
        assert_eq!(
            o.observe("thread/realtime/itemAdded", &handoff),
            Err("codex_realtime:agent_handoff_observed")
        );
        assert_eq!(
            o.observe("thread/realtime/item/completed",
                &item("late","sess_a","assistant","Unsafe late text")),
            Err("codex_realtime:session_revoked")
        );
        for raw_type in ["function_call", "bemItemPromoted", "commandExecution"] {
            let mut o = observer();
            let payload = json!({"threadId":"thr_a","item":{"type":raw_type}});
            assert!(o.observe("thread/realtime/itemAdded", &payload).is_err());
            assert_eq!(
                o.observe("thread/realtime/transcript/done",
                    &json!({"threadId":"thr_a","role":"assistant"})),
                Err("codex_realtime:session_revoked")
            );
        }
    }

    #[test]
    fn raw_item_added_never_grants_final_translation_or_tts_authority() {
        for raw_item in [
            json!({"type":"input_audio_buffer.speech_started","item_id":"voice"}),
            json!({"type":"response.cancelled","response_id":"r1"}),
            json!({"type":"message","role":"user","content":"spoken data"}),
        ] {
            let mut o = observer();
            assert_eq!(
                o.observe("thread/realtime/itemAdded",
                    &json!({"threadId":"thr_a","item":raw_item})),
                Ok(Observation::Ignored)
            );
        }
        for raw_item in [
            json!({"type":"message","role":"assistant","content":"unverified"}),
            json!({"type":"message","role":"system"}),
            json!({}),
        ] {
            let mut o = observer();
            assert_eq!(
                o.observe("thread/realtime/itemAdded",
                    &json!({"threadId":"thr_a","item":raw_item})),
                Err("codex_realtime:item_added_unadmitted")
            );
        }
        let mut o = observer();
        assert_eq!(
            o.observe("thread/realtime/itemAdded", &json!({"threadId":"thr_a"})),
            Err("codex_realtime:item_added_invalid")
        );
    }

    #[test]
    fn unrequested_transport_closure_never_claims_clean_stop() {
        let mut good = observer();
        assert_eq!(
            good.observe("thread/realtime/closed",
                &json!({"threadId":"thr_a","reason":"requested"})),
            Ok(Observation::Closed)
        );
        assert_eq!(
            good.observe("thread/realtime/closed",
                &json!({"threadId":"thr_a","reason":"requested"})),
            Err("codex_realtime:session_revoked")
        );
        for reason in ["error", "transport_closed"] {
            let mut bad = observer();
            assert_eq!(
                bad.observe("thread/realtime/closed",
                    &json!({"threadId":"thr_a","reason":reason})),
                Err("codex_realtime:transport_closed_unverified")
            );
            assert_eq!(
                bad.observe("thread/realtime/item/completed",
                    &item("late","sess_a","assistant","Late")),
                Err("codex_realtime:session_revoked")
            );
        }
        let mut absent = observer();
        assert_eq!(
            absent.observe("thread/realtime/closed", &json!({"threadId":"thr_a"})),
            Err("codex_realtime:close_reason_invalid")
        );
    }

    #[test]
    fn bounded_pcm_queue_preserves_clock_and_revokes_on_gap() {
        let mut q = AudioFrameQueue::default();
        assert_eq!(q.pending_state(), Ok(false));
        q.push(100, &[0,1]).unwrap();
        assert_eq!(q.pending_state(), Ok(true));
        q.push(102, &[-1]).unwrap();
        let (_, first) = q.pop().unwrap();
        assert_eq!(first["sampleRate"], AUDIO_RATE);
        assert_eq!(first["samplesPerChannel"], 2);
        assert_eq!(first["numChannels"], 1);
        assert_eq!(first["data"], "AAABAA==");
        let (_, second) = q.pop().unwrap();
        assert_eq!(second["data"], "//8=");
        assert_eq!(q.push(105, &[0]), Err("codex_realtime:audio_clock_gap"));
        assert_eq!(q.pending_state(), Err("codex_realtime:audio_queue_revoked"));
        assert!(q.pop().is_none());
    }

    #[test]
    fn streaming_pcm24k_conversion_is_partition_independent() {
        let data = [0.0, 1.0, 0.0, -1.0];
        let mut split = Pcm24kResampler::new(16_000).unwrap();
        let (s0, first) = split.convert(0, &data[..2]).unwrap();
        let (s1, second) = split.convert(2, &data[2..]).unwrap();
        let mut single = Pcm24kResampler::new(16_000).unwrap();
        let (_, entire) = single.convert(0, &data).unwrap();
        let combined: Vec<i16> = first.iter().chain(second.iter()).copied().collect();
        assert_eq!(combined, entire);
        // Linear interpolation needs the next source sample at the last
        // fractional position: four source samples yield five immediately.
        // The sixth output becomes available only with the fifth input.
        assert_eq!(combined.len(), 5);
        assert_eq!((s0, s1), (0, 2));
        assert_eq!(combined[0], 0);
        assert!(combined[1] > 0 && combined[4] < 0);
        let (next_start, next_pcm) = split.convert(4, &[0.5]).unwrap();
        let (_, full_tail) = single.convert(4, &[0.5]).unwrap();
        assert_eq!(next_start, 5);
        assert_eq!(next_pcm, full_tail);
        assert_eq!(next_pcm.len(), 2);
    }

    #[test]
    fn native_48k_mic_clock_and_invalid_input_fail_closed() {
        let mut converter = Pcm24kResampler::new(48_000).unwrap();
        let mut queue = AudioFrameQueue::default();
        converter.push_mono_f32(0, &[0.0, 0.25, 0.5, 0.75], &mut queue).unwrap();
        let (start, encoded) = queue.pop().unwrap();
        assert_eq!(start, 0);
        assert_eq!(encoded["sampleRate"], 24_000);
        assert_eq!(encoded["samplesPerChannel"], 2);
        assert_eq!(
            converter.push_mono_f32(9, &[0.0], &mut queue),
            Err("codex_realtime:input_clock_gap")
        );
        assert_eq!(queue.pending_state(), Err("codex_realtime:audio_queue_revoked"));

        let mut invalid = Pcm24kResampler::new(16_000).unwrap();
        let mut bad_queue = AudioFrameQueue::default();
        assert_eq!(
            invalid.push_mono_f32(0, &[f32::NAN], &mut bad_queue),
            Err("codex_realtime:source_frame_invalid")
        );
        assert_eq!(bad_queue.pending_state(), Err("codex_realtime:audio_queue_revoked"));
        assert_eq!(invalid.push_mono_f32(0, &[0.0], &mut bad_queue),
            Err("codex_realtime:resampler_revoked"));
        assert!(Pcm24kResampler::new(0).is_err());
    }

    #[test]
    fn overflow_is_not_silent_audio_loss() {
        let mut q = AudioFrameQueue::default();
        for i in 0..MAX_AUDIO_FRAMES {
            q.push((i * SAMPLES_PER_FRAME) as u64, &vec![0; SAMPLES_PER_FRAME]).unwrap();
        }
        assert_eq!(
            q.push((MAX_AUDIO_FRAMES * SAMPLES_PER_FRAME) as u64, &[0]),
            Err("codex_realtime:audio_backpressure")
        );
        assert!(q.pop().is_none());
        assert!(q.push(0, &[1]).is_err());
    }

    #[test]
    fn invalid_frame_and_sample_clock_overflow_fail_closed() {
        let mut invalid = AudioFrameQueue::default();
        assert_eq!(
            invalid.push(0, &[]),
            Err("codex_realtime:audio_frame_size_invalid")
        );
        assert!(invalid.push(0, &[1]).is_err());

        let mut overflow = AudioFrameQueue::default();
        assert_eq!(
            overflow.push(u64::MAX, &[1,2]),
            Err("codex_realtime:audio_clock_overflow")
        );
        assert!(overflow.pop().is_none());
    }
}
