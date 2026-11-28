use serde_json::{json, Value};

use crate::engine::runtime_state::runtime_generation_is_authoritative;

use super::functional_readiness::{
    functional_asr_output, functional_translation_output,
    invalidate_required_outbound_ai_readiness,
};
use super::{
    get_helper_bridge_status, meeting_asr_compute_admitted,
    meeting_translation_compute_admitted, send_worker_task, worker_text,
    HelperBridgeWorkerResponse, NATIVE_INFERENCE_CLAIM, NATIVE_STATUS_CLAIM,
};

const REQUIRED_OUTBOUND_FUNCTIONAL_ID_FIXTURE: &str = "selamat pagi";

fn same_probe_inference(response: &HelperBridgeWorkerResponse, generation: u64,
                        native_device: &str) -> bool {
    response.ok && response.generation_token == generation
        && response.runtime_claim == NATIVE_INFERENCE_CLAIM
        && response.worker_response_json.get("cuda_pci_id").and_then(Value::as_str) == Some(native_device)
}

// A successful native bind already opened the complete resident stack.
// It is preparation evidence, not a claim that ASR or TTS has run.
fn native_binding_compute_admitted(response: &HelperBridgeWorkerResponse) -> bool {
    let body = &response.worker_response_json;
    response.ok && response.task == "voice_actor_preflight"
        && response.runtime_claim == NATIVE_INFERENCE_CLAIM
        && body.get("stage").and_then(Value::as_str) == Some("voice_actor_preflight")
        && body.get("device").and_then(Value::as_str) == Some("cuda")
        && body.get("model_id").and_then(Value::as_str) == Some("zonos3-mini-q8_0")
        && body.get("actor_token").and_then(Value::as_str)
            .is_some_and(|token| !token.is_empty() && token.len() <= 512)
}

pub(super) fn run_required_outbound_ai_probe(
    meeting_generation: Option<u64>,
) -> Result<(u64, String), &'static str> {
    if meeting_generation
        .map(|generation| generation == 0 || !runtime_generation_is_authoritative(generation))
        .unwrap_or(false)
    {
        invalidate_required_outbound_ai_readiness();
        return Err("Meeting authority");
    }
    let meeting_start_prepare = meeting_generation.is_some();

    // Refresh cheap capability truth first so a newly approved/rebuilt My Voice is
    // visible to explicit setup checks and to the generation-bound Meeting probe.
    let refreshed = send_worker_task(
        "status",
        json!({
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    if !refreshed.ok {
        invalidate_required_outbound_ai_readiness();
        return Err("local translation runtime");
    }
    if refreshed.runtime_claim != NATIVE_STATUS_CLAIM {
        invalidate_required_outbound_ai_readiness();
        return Err("native CUDA capability");
    }
    let helper = get_helper_bridge_status();
    if helper.state != "ready" || !helper.provider_ready || helper.generation_token == 0
        || refreshed.generation_token != helper.generation_token
    {
        invalidate_required_outbound_ai_readiness();
        return Err("local translation runtime");
    }
    let generation_token = helper.generation_token;

    let preparation = super::transport::bind_native_voice(json!({
        "meeting_start_prepare": meeting_start_prepare,
        "meeting_generation": meeting_generation,
    }));
    let Some(native_device) = preparation.worker_response_json.get("cuda_pci_id").and_then(Value::as_str)
        .filter(|pci| !pci.is_empty()) else {
        invalidate_required_outbound_ai_readiness();
        return Err("native CUDA identity");
    };
    let preparation_valid = native_binding_compute_admitted(&preparation);
    if !same_probe_inference(&preparation, generation_token, native_device)
        || !preparation_valid
        || refreshed.worker_response_json.get("cuda_pci_id").and_then(Value::as_str)
            .is_some_and(|observed| observed != native_device)
    {
        invalidate_required_outbound_ai_readiness();
        return Err("native selected voice");
    }

    let translation = send_worker_task(
        "translate",
        json!({
            "text": REQUIRED_OUTBOUND_FUNCTIONAL_ID_FIXTURE,
            "source_language": "id",
            "target_language": "en",
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    if !same_probe_inference(&translation, generation_token, native_device)
        || !meeting_translation_compute_admitted(&translation) {
        invalidate_required_outbound_ai_readiness();
        return Err("CUDA Indonesian to English translation");
    }
    let Some(translated_fixture) = functional_translation_output(&translation) else {
        invalidate_required_outbound_ai_readiness();
        return Err("Indonesian to English translation");
    };

    let actor_preflight = &preparation;
    if !actor_preflight.ok || actor_preflight.generation_token != generation_token
        || actor_preflight.task != "voice_actor_preflight"
        || actor_preflight.worker_response_json.get("stage").and_then(Value::as_str) != Some("voice_actor_preflight")
    {
        invalidate_required_outbound_ai_readiness();
        return Err("My Voice");
    }
    let Some(actor_token) = worker_text(&actor_preflight.worker_response_json, "actor_token") else {
        invalidate_required_outbound_ai_readiness();
        return Err("My Voice");
    };

    let (voice, samples) = super::transport::synthesize_native_voice(json!({
        "text": translated_fixture,
        "expected_actor_token": actor_token.clone(),
        "meeting_start_prepare": meeting_start_prepare,
        "meeting_generation": meeting_generation,
    }));
    if !same_probe_inference(&voice, generation_token, native_device)
        || !super::native_voice_pcm_receipt_matches(&voice, &actor_token, samples.as_deref()) {
        invalidate_required_outbound_ai_readiness();
        return Err("native voice synthesis");
    }
    let asr_inference = super::transport::transcribe_native_voice_probe(json!({
        "language": "en",
        "meeting_start_prepare": meeting_start_prepare,
        "meeting_generation": meeting_generation,
    }), samples.expect("checked PCM receipt"));
    if !same_probe_inference(&asr_inference, generation_token, native_device)
        || !functional_asr_output(&asr_inference)
        || !meeting_asr_compute_admitted(&asr_inference)
    {
        invalidate_required_outbound_ai_readiness();
        return Err("speech recognition");
    }

    // Synthesis already proves the exact actor token remained authoritative and the
    // ASR round-trip above proves the generated audio is consumable. Avoid a second
    // full worker status scan here; it repeats dependency/model/actor inspection on
    // every functional probe without adding stronger evidence than these executed
    // stages. The host bridge generation and Meeting authority are still checked.
    let current = get_helper_bridge_status();
    if current.state != "ready"
        || !current.provider_ready
        || current.generation_token != generation_token
        || meeting_generation
            .map(|generation| !runtime_generation_is_authoritative(generation))
            .unwrap_or(false)
    {
        invalidate_required_outbound_ai_readiness();
        return Err("local translation runtime");
    }

    Ok((generation_token, actor_token))
}


#[cfg(test)]
mod tests {
    use super::same_probe_inference;
    use serde_json::json;

    #[test]
    fn native_binding_receipt_requires_exact_model_stage_and_token() {
        let mut receipt = super::super::HelperBridgeWorkerResponse {
            ok: true, state: "ready".into(), task: "voice_actor_preflight".into(),
            request_id: "fixture".into(), scheduler_priority: "meeting_outbound".into(),
            message: String::new(), generation_token: 7,
            runtime_claim: super::NATIVE_INFERENCE_CLAIM.into(),
            worker_response_json: json!({"stage":"voice_actor_preflight","device":"cuda",
                "model_id":"zonos3-mini-q8_0","actor_token":"bound","cuda_pci_id":"0000:01:00.0"}),
        };
        assert!(super::native_binding_compute_admitted(&receipt));
        for (key, bad) in [("stage", json!("asr_preload")), ("device", json!("cpu")),
            ("model_id", json!("other")), ("actor_token", json!(""))] {
            let original = receipt.worker_response_json[key].clone();
            receipt.worker_response_json[key] = bad;
            assert!(!super::native_binding_compute_admitted(&receipt));
            receipt.worker_response_json[key] = original;
        }
        receipt.runtime_claim = "helper_process_ready_ping_verified".into();
        assert!(!super::native_binding_compute_admitted(&receipt));
    }

    #[test]
    fn functional_probe_rejects_engine_device_and_generation_changes() {
        let mut receipt = super::super::HelperBridgeWorkerResponse {
            ok: true, state: "ready".into(), task: "translate".into(),
            request_id: "fixture".into(), scheduler_priority: "meeting_outbound".into(),
            message: String::new(), generation_token: 7,
            runtime_claim: super::NATIVE_INFERENCE_CLAIM.into(),
            worker_response_json: json!({"cuda_pci_id":"0000:01:00.0"}),
        };
        assert!(same_probe_inference(&receipt, 7, "0000:01:00.0"));
        assert!(!same_probe_inference(&receipt, 8, "0000:01:00.0"));
        assert!(!same_probe_inference(&receipt, 7, "0000:02:00.0"));

        receipt.runtime_claim = "helper_process_ready_ping_verified".into();
        assert!(!same_probe_inference(&receipt, 7, "0000:01:00.0"));

    }
}
