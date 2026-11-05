//! One-flight, fail-closed correlation evidence for a *future* Codex Voice
//! adapter. No upstream event currently supplies a source-utterance/response
//! link. A provisional pair is NOT a translation commit or permission for TTS.
//! This state is local to the existing Codex gateway, not a second Meeting owner.

use super::realtime::{Observation, RealtimeObserver};
use serde_json::Value;

const MAX_CANDIDATE_BYTES: usize = 8 * 1024;
const MAX_UTTERANCE_SAMPLES: u64 = 24_000 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SourceWindow {
    pub generation: u64,
    pub source_epoch: u64,
    pub sequence: u64,
    pub start_sample: u64,
    pub end_sample: u64,
}

/// Evidence for further validation only; cannot be passed to the existing
/// Meeting translation or GPT-SoVITS APIs. Assistant speech has no guaranteed
/// causal link to a source window under the current upstream event contract.
#[derive(PartialEq, Eq)]
pub(super) struct UnverifiedPair {
    pub source: SourceWindow,
    pub source_item_id: String,
    pub source_text: String,
    pub assistant_item_id: String,
    pub assistant_text: String,
}

// An assert/panic must not print private source or translated meeting text.
impl std::fmt::Debug for UnverifiedPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnverifiedPair")
            .field("source", &self.source)
            .field("source_text", &"<redacted>")
            .field("assistant_text", &"<redacted>")
            .finish()
    }
}

#[derive(Default)]
pub(super) struct CorrelationGate {
    active: Option<SourceWindow>,
    source: Option<(String, String)>,
    retired: bool,
}

impl CorrelationGate {
    pub(super) fn begin(&mut self, window: SourceWindow) -> Result<(), &'static str> {
        if self.retired {
            return Err("codex_correlation:retired");
        }
        if self.active.is_some() {
            self.revoke();
            return Err("codex_correlation:overlapping_source");
        }
        let duration = window.end_sample.checked_sub(window.start_sample);
        if window.generation == 0
            || window.source_epoch == 0
            || window.sequence == 0
            || !matches!(duration, Some(1..=MAX_UTTERANCE_SAMPLES))
        {
            self.revoke();
            return Err("codex_correlation:source_window_invalid");
        }
        self.active = Some(window);
        Ok(())
    }

    pub(super) fn observe(
        &mut self,
        observation: Observation,
    ) -> Result<Option<UnverifiedPair>, &'static str> {
        if self.retired {
            return Err("codex_correlation:retired");
        }
        match observation {
            Observation::Ignored | Observation::CloudAudioIgnored => Ok(None),
            Observation::Closed => {
                self.revoke();
                Err("codex_correlation:session_closed")
            }
            Observation::SourceTranscript { item_id, text } => {
                if self.active.is_none() || self.source.is_some()
                    || !valid_candidate(&item_id, &text)
                {
                    self.revoke();
                    return Err("codex_correlation:source_ambiguous");
                }
                self.source = Some((item_id, text));
                Ok(None)
            }
            Observation::UnverifiedAssistantText { item_id, text } => {
                let Some(window) = self.active else {
                    self.revoke();
                    return Err("codex_correlation:assistant_without_window");
                };
                let Some((source_id, source_text)) = self.source.take() else {
                    self.revoke();
                    return Err("codex_correlation:assistant_without_source");
                };
                if item_id == source_id || !valid_candidate(&item_id, &text) {
                    self.revoke();
                    return Err("codex_correlation:assistant_ambiguous");
                }
                let pair = UnverifiedPair {
                    source: window,
                    source_item_id: source_id,
                    source_text,
                    assistant_item_id: item_id,
                    assistant_text: text,
                };
                // The current upstream lacks authoritative reply-to/source
                // identity. Retire this gate after one provisional observation
                // rather than guessing how a late answer relates to the next
                // utterance in a long session.
                self.revoke();
                Ok(Some(pair))
            }
        }
    }

    /// Compose the existing role-bearing realtime observer with the one-flight
    /// correlation gate; NEVER promote this provisional pair to GPT-SoVITS.
    /// An invalid provider event retires both boundaries immediately.
    pub(super) fn observe_event(
        &mut self,
        observer: &mut RealtimeObserver,
        method: &str,
        params: &Value,
    ) -> Result<Option<UnverifiedPair>, &'static str> {
        // A prior answer has already exhausted this one-flight boundary.
        // Revoke the event observer before examining any late/duplicate
        // provider item, so stale text cannot be paired with another window.
        if self.retired {
            observer.revoke();
            return Err("codex_correlation:retired");
        }
        let observation = match observer.observe(method, params) {
            Ok(value) => value,
            Err(blocker) => {
                self.revoke();
                return Err(blocker);
            }
        };
        let result = self.observe(observation);
        if result.is_err() {
            observer.revoke();
        }
        result
    }

    pub(super) fn revoke(&mut self) {
        self.active = None;
        self.source = None;
        self.retired = true;
    }

    pub(super) fn is_retired(&self) -> bool {
        self.retired
    }
}

fn valid_candidate(id: &str, text: &str) -> bool {
    !id.is_empty()
        && id.len() <= 160
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
        && !text.trim().is_empty()
        && text.len() <= MAX_CANDIDATE_BYTES
        && !text.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lease() -> SourceWindow {
        SourceWindow {
            generation: 11,
            source_epoch: 5,
            sequence: 8,
            start_sample: 240,
            end_sample: 24_240,
        }
    }

    fn source() -> Observation {
        Observation::SourceTranscript {
            item_id: "source_8".into(),
            text: "Tiga belas, bukan tiga puluh.".into(),
        }
    }

    fn assistant() -> Observation {
        Observation::UnverifiedAssistantText {
            item_id: "assistant_8".into(),
            text: "Thirteen, not thirty.".into(),
        }
    }

    #[test]
    fn one_source_one_assistant_yields_only_an_unverified_pair() {
        let mut gate = CorrelationGate::default();
        gate.begin(lease()).unwrap();
        assert_eq!(gate.observe(Observation::Ignored).unwrap(), None);
        assert_eq!(gate.observe(source()).unwrap(), None);
        let pair = gate.observe(assistant()).unwrap().expect("one candidate");
        assert_eq!(pair.source, lease());
        assert_eq!(pair.source_item_id, "source_8");
        assert_eq!(pair.assistant_item_id, "assistant_8");
        assert_eq!(pair.assistant_text, "Thirteen, not thirty.");
        assert!(gate.is_retired());
        let debug = format!("{pair:?}");
        assert!(!debug.contains("Thirteen, not thirty."));
        assert!(!debug.contains("Tiga belas, bukan tiga puluh."));
        assert_eq!(gate.observe(assistant()), Err("codex_correlation:retired"));
        assert_eq!(gate.begin(lease()), Err("codex_correlation:retired"));
    }

    #[test]
    fn completed_realtime_items_form_only_one_unverified_pair() {
        let mut observer = RealtimeObserver::new("thr_1").unwrap();
        let mut gate = CorrelationGate::default();
        assert_eq!(
            gate.observe_event(&mut observer, "thread/realtime/started",
                &json!({"threadId":"thr_1","realtimeSessionId":"sess_1","version":"v3"})),
            Ok(None)
        );
        gate.begin(lease()).unwrap();
        assert_eq!(
            gate.observe_event(&mut observer, "thread/realtime/outputAudio/delta",
                &json!({"threadId":"thr_1","audio":{"data":"private-audio"}})),
            Ok(None)
        );
        let source = json!({"threadId":"thr_1","item":{
            "id":"src_1","realtimeSessionId":"sess_1",
            "type":"transcriptSegment","role":"user","text":"Jangan hapus file."
        }});
        let reply = json!({"threadId":"thr_1","item":{
            "id":"out_1","realtimeSessionId":"sess_1",
            "type":"transcriptSegment","role":"assistant","text":"Do not delete the file."
        }});
        assert_eq!(
            gate.observe_event(&mut observer, "thread/realtime/item/completed", &source),
            Ok(None)
        );
        let candidate = gate.observe_event(
            &mut observer, "thread/realtime/item/completed", &reply
        ).unwrap().expect("provisional pair only");
        assert_eq!(candidate.source, lease());
        assert_eq!(candidate.source_item_id, "src_1");
        assert_eq!(candidate.assistant_item_id, "out_1");
        assert_eq!(candidate.assistant_text, "Do not delete the file.");
        assert!(!format!("{candidate:?}").contains("Do not delete the file."));
        assert!(gate.is_retired());
        assert_eq!(
            gate.observe_event(&mut observer, "thread/realtime/item/completed", &reply),
            Err("codex_correlation:retired")
        );
    }

    #[test]
    fn invalid_realtime_item_revokes_the_pairing_boundary() {
        let mut observer = RealtimeObserver::new("thr_1").unwrap();
        let mut gate = CorrelationGate::default();
        gate.begin(lease()).unwrap();
        assert_eq!(
            gate.observe_event(&mut observer, "thread/realtime/started",
                &json!({"threadId":"thr_1","realtimeSessionId":"sess_1","version":"v3"})),
            Ok(None)
        );
        assert_eq!(
            gate.observe_event(&mut observer, "thread/realtime/item/completed",
                &json!({"threadId":"foreign","item":{
                    "id":"bad","realtimeSessionId":"sess_1",
                    "type":"transcriptSegment","role":"assistant","text":"No"
                }})),
            Err("codex_realtime:foreign_thread")
        );
        assert!(gate.is_retired());
        assert_eq!(
            observer.observe("thread/realtime/outputAudio/delta",
                &json!({"threadId":"thr_1"})),
            Err("codex_realtime:session_revoked")
        );
    }

    #[test]
    fn assistant_before_source_cannot_be_guessed_into_pairing() {
        let mut gate = CorrelationGate::default();
        gate.begin(lease()).unwrap();
        assert_eq!(
            gate.observe(assistant()),
            Err("codex_correlation:assistant_without_source")
        );
        assert!(gate.is_retired());
    }

    #[test]
    fn repeated_source_or_overlapping_input_revokes_owner() {
        let mut duplicate = CorrelationGate::default();
        duplicate.begin(lease()).unwrap();
        duplicate.observe(source()).unwrap();
        assert_eq!(
            duplicate.observe(source()),
            Err("codex_correlation:source_ambiguous")
        );
        assert!(duplicate.is_retired());

        let mut overlapping = CorrelationGate::default();
        overlapping.begin(lease()).unwrap();
        assert_eq!(
            overlapping.begin(SourceWindow { sequence: 9, ..lease() }),
            Err("codex_correlation:overlapping_source")
        );
        assert!(overlapping.is_retired());
    }

    #[test]
    fn stop_or_invalid_source_range_cannot_emit_a_pair() {
        let mut stopped = CorrelationGate::default();
        stopped.begin(lease()).unwrap();
        stopped.observe(source()).unwrap();
        stopped.revoke();
        assert_eq!(stopped.observe(assistant()), Err("codex_correlation:retired"));

        let mut invalid = CorrelationGate::default();
        assert_eq!(
            invalid.begin(SourceWindow { end_sample: 240, ..lease() }),
            Err("codex_correlation:source_window_invalid")
        );
        assert!(invalid.is_retired());
    }

    #[test]
    fn provider_item_ids_are_distinct_and_candidate_text_is_bounded() {
        let mut same_id = CorrelationGate::default();
        same_id.begin(lease()).unwrap();
        same_id.observe(source()).unwrap();
        assert_eq!(
            same_id.observe(Observation::UnverifiedAssistantText {
                item_id: "source_8".into(),
                text: "Not safe.".into(),
            }),
            Err("codex_correlation:assistant_ambiguous")
        );

        let mut huge = CorrelationGate::default();
        huge.begin(lease()).unwrap();
        assert_eq!(
            huge.observe(Observation::SourceTranscript {
                item_id: "source_8".into(),
                text: "a".repeat(MAX_CANDIDATE_BYTES + 1),
            }),
            Err("codex_correlation:source_ambiguous")
        );
    }
}
