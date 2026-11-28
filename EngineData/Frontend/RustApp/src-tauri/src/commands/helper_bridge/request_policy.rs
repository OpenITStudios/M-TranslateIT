use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use crate::engine::audio::finalized_utterance::wake_incoming_consumer;

use crate::engine::runtime_state::{
    latest_runtime_session_state, runtime_generation_is_authoritative, runtime_generation_matches_epoch,
    RuntimeSessionSnapshot, APPLICATION_MEETING_OWNER_ID,
};

use super::super::helper_bridge_runtime::HelperTaskPriority;
use super::clean_helper_text;


static MEETING_OUTBOUND_PIPELINE_GENERATION: AtomicU64 = AtomicU64::new(0);

pub(super) fn live_outbound_generation_is_authoritative(generation: u64) -> bool {
    if !runtime_generation_is_authoritative(generation) {
        return false;
    }
    latest_runtime_session_state()
        .snapshot
        .map(|snapshot| {
            snapshot.owner_id == APPLICATION_MEETING_OWNER_ID
                && snapshot.generation == generation
                && snapshot.authority_active
                && snapshot.phase == "live"
        })
        .unwrap_or(false)
}

pub(super) fn meeting_generation(payload: &Value) -> Option<u64> {
    payload.get("meeting_generation").and_then(Value::as_u64)
}

pub(super) fn meeting_session_id(payload: &Value) -> Option<String> {
    payload
        .get("meeting_session_id")
        .and_then(Value::as_str)
        .map(|value| clean_helper_text(value, 96))
        .filter(|value| !value.is_empty())
}

pub(super) fn meeting_lane(payload: &Value) -> Option<String> {
    payload
        .get("meeting_lane")
        .and_then(Value::as_str)
        .map(|value| clean_helper_text(value, 32).to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "you" | "incoming"))
}

pub(super) fn meeting_start_prepare(payload: &Value) -> bool {
    payload
        .get("meeting_start_prepare")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(crate) fn meeting_outbound_pipeline_active() -> bool {
    runtime_generation_matches_epoch(MEETING_OUTBOUND_PIPELINE_GENERATION.load(Ordering::Acquire))
}

pub(super) fn mark_meeting_outbound_pipeline(generation: u64) {
    // A delayed older request must never overwrite a newer Meeting's mark.
    let _ = MEETING_OUTBOUND_PIPELINE_GENERATION.fetch_update(
        Ordering::AcqRel, Ordering::Acquire,
        |current| (current <= generation && runtime_generation_matches_epoch(generation))
            .then_some(generation),
    );
}

// Transport releases the marker at TTS completion/failure. The serialized
// utterance owner also guarantees release when it exits between worker stages,
// e.g. after a source change, invalid receipt or failed transcript commit.
pub(crate) struct OutboundPipelineRelease(u64);

impl OutboundPipelineRelease {
    pub(crate) fn new(generation: u64) -> Self { Self(generation) }
}

impl Drop for OutboundPipelineRelease {
    fn drop(&mut self) { clear_meeting_outbound_pipeline(self.0); }
}

pub(super) fn clear_meeting_outbound_pipeline(generation: u64) {
    if generation != 0 && MEETING_OUTBOUND_PIPELINE_GENERATION.compare_exchange(
        generation, 0, Ordering::AcqRel, Ordering::Acquire,
    ).is_ok() {
        wake_incoming_consumer();
    }
}

pub(super) fn clear_any_meeting_outbound_pipeline() {
    if MEETING_OUTBOUND_PIPELINE_GENERATION.swap(0, Ordering::AcqRel) != 0 {
        wake_incoming_consumer();
    }
}

// A stale Stop request must not release a newer Meeting's outbound admission
// protection. Stop has already revoked authority but still owns the snapshot.
fn matching_session_pipeline_generation(
    session_id: &str,
    snapshot: &RuntimeSessionSnapshot,
) -> Option<u64> {
    (snapshot.owner_id == APPLICATION_MEETING_OWNER_ID
        && !session_id.is_empty()
        && snapshot.session_id == session_id
        && snapshot.generation != 0)
        .then_some(snapshot.generation)
}

pub(super) fn clear_meeting_outbound_pipeline_for_session(session_id: &str) {
    let snapshot = latest_runtime_session_state().snapshot;
    if let Some(generation) = snapshot.as_ref().and_then(|snapshot| {
        matching_session_pipeline_generation(session_id, snapshot)
    }) {
        // Generation-scoped CAS also protects a newer pipeline that claimed
        // the marker between snapshot lookup and this clear.
        clear_meeting_outbound_pipeline(generation);
    }
}

pub(super) fn incoming_session_is_eligible(session_id: &str) -> bool {
    incoming_session_generation(session_id).is_some()
}

pub(super) fn incoming_session_generation(session_id: &str) -> Option<u64> {
    latest_runtime_session_state()
        .snapshot
        .filter(|snapshot| {
            snapshot.owner_id == APPLICATION_MEETING_OWNER_ID
                && snapshot.session_id == session_id
                && snapshot.authority_active
                && snapshot.phase == "live"
        })
        .map(|snapshot| snapshot.generation)
}

// Scheduling metadata is parsed once per request. It is not a live
// authority receipt: the transport still checks current session/generation.
pub(super) struct RequestAdmission {
    pub(super) priority: HelperTaskPriority,
    pub(super) generation: Option<u64>,
    pub(super) session_id: Option<String>,
    pub(super) lane: Option<String>,
}

impl RequestAdmission {
    pub(super) fn from_payload(task: &str, payload: &Value) -> Self {
        let generation = meeting_generation(payload);
        let session_id = meeting_session_id(payload);
        let lane = meeting_lane(payload);
        let priority = if generation.is_some() || meeting_start_prepare(payload) {
            HelperTaskPriority::MeetingOutbound
        } else if lane.as_deref() == Some("incoming") && session_id.is_some() {
            HelperTaskPriority::MeetingIncoming
        } else if task == "translate" {
            HelperTaskPriority::Text
        } else {
            HelperTaskPriority::Diagnostic
        };
        Self { priority, generation, session_id, lane }
    }

    pub(super) fn outbound_generation(&self) -> Option<u64> {
        if self.priority == HelperTaskPriority::MeetingOutbound && self.lane.as_deref() == Some("you") {
            self.generation
        } else {
            None
        }
    }
}

pub(super) fn task_priority(task: &str, payload: &Value) -> HelperTaskPriority {
    RequestAdmission::from_payload(task, payload).priority
}

pub(super) fn inject_request_metadata(
    payload: &mut Value,
    task: &str,
    request_id: &str,
    priority: HelperTaskPriority,
) {
    if !payload.is_object() {
        *payload = json!({});
    }
    if let Some(object) = payload.as_object_mut() {
        object.insert("command".to_string(), json!(task));
        object.insert("request_id".to_string(), json!(request_id));
        object.insert("scheduler_priority".to_string(), json!(priority.label()));
    }
}

#[cfg(test)]
mod scoped_cancellation_tests {
    use super::{matching_session_pipeline_generation, RequestAdmission, HelperTaskPriority};
    use serde_json::json;
    use crate::engine::runtime_state::{
        RuntimeSessionSnapshot, APPLICATION_MEETING_OWNER_ID, MIC_TEST_OWNER_ID,
    };

    fn snapshot(session: &str, generation: u64, owner: &str) -> RuntimeSessionSnapshot {
        RuntimeSessionSnapshot {
            started_unix_ms: 0,
            owner_id: owner.to_string(),
            session_id: session.to_string(),
            generation,
            authority_active: false,
            phase: "stopping".to_string(),
            live_capture_stream_active: false,
            safe_to_stop: true,
            note: String::new(),
        }
    }

    #[test]
    fn admission_preserves_priority_and_outbound_generation_contract() {
        let cases = [
            ("translate", json!({}), HelperTaskPriority::Text, None),
            ("status", json!({}), HelperTaskPriority::Diagnostic, None),
            ("translate", json!({"meeting_lane":" INCOMING ","meeting_session_id":" s "}), HelperTaskPriority::MeetingIncoming, None),
            ("translate", json!({"meeting_lane":"incoming","meeting_session_id":""}), HelperTaskPriority::Text, None),
            ("transcribe", json!({"meeting_lane":"you","meeting_generation":7,"meeting_session_id":"s"}), HelperTaskPriority::MeetingOutbound, Some(7)),
            ("status", json!({"meeting_start_prepare":true}), HelperTaskPriority::MeetingOutbound, None),
            ("translate", json!({"meeting_lane":"incoming","meeting_generation":8,"meeting_session_id":"s"}), HelperTaskPriority::MeetingOutbound, None),
        ];
        for (task, payload, priority, outbound) in cases {
            let admission = RequestAdmission::from_payload(task, &payload);
            assert_eq!(admission.priority, priority);
            assert_eq!(admission.outbound_generation(), outbound);
        }
        let incoming = RequestAdmission::from_payload("translate",
            &json!({"meeting_lane":" INCOMING ","meeting_session_id":" s "}));
        assert_eq!(incoming.lane.as_deref(), Some("incoming"));
        assert_eq!(incoming.session_id.as_deref(), Some("s"));
    }

    #[test]
    fn utterance_exit_releases_priority_and_stale_claim_cannot_replace_current() {
        use super::{clear_any_meeting_outbound_pipeline, mark_meeting_outbound_pipeline,
            meeting_outbound_pipeline_active, OutboundPipelineRelease, MEETING_OUTBOUND_PIPELINE_GENERATION};
        use std::sync::atomic::Ordering;
        use crate::engine::runtime_state::{begin_application_meeting_session,
            clear_runtime_session_if_generation, clear_runtime_session_state};
        let _serial = crate::engine::runtime_state::runtime_test_guard();
        let _ = clear_runtime_session_state();
        clear_any_meeting_outbound_pipeline();
        let old = begin_application_meeting_session().snapshot.unwrap().generation;
        {
            let _release = OutboundPipelineRelease::new(old);
            mark_meeting_outbound_pipeline(old);
            assert!(meeting_outbound_pipeline_active());
            // Simulate an early return after successful ASR, before TTS.
        }
        assert!(!meeting_outbound_pipeline_active());
        assert_eq!(MEETING_OUTBOUND_PIPELINE_GENERATION.load(Ordering::Acquire), 0);

        let _ = clear_runtime_session_if_generation(old);
        let current = begin_application_meeting_session().snapshot.unwrap().generation;
        {
            let _release = OutboundPipelineRelease::new(current);
            mark_meeting_outbound_pipeline(current);
            mark_meeting_outbound_pipeline(old);
            drop(OutboundPipelineRelease::new(old));
            assert_eq!(MEETING_OUTBOUND_PIPELINE_GENERATION.load(Ordering::Acquire), current);
            assert!(meeting_outbound_pipeline_active());
        }
        assert!(!meeting_outbound_pipeline_active());
        let _ = clear_runtime_session_if_generation(current);
        clear_any_meeting_outbound_pipeline();
    }

    #[test]
    fn cancellation_only_clears_a_matching_meeting_generation() {
        let current = snapshot("meeting-new", 42, APPLICATION_MEETING_OWNER_ID);
        assert_eq!(
            matching_session_pipeline_generation("meeting-new", &current),
            Some(42),
        );
        assert_eq!(matching_session_pipeline_generation("meeting-old", &current), None);
        assert_eq!(matching_session_pipeline_generation("", &current), None);
        assert_eq!(matching_session_pipeline_generation(
            "meeting-new", &snapshot("meeting-new", 42, MIC_TEST_OWNER_ID)
        ), None);
        assert_eq!(matching_session_pipeline_generation(
            "meeting-new", &snapshot("meeting-new", 0, APPLICATION_MEETING_OWNER_ID)
        ), None);
    }
}
