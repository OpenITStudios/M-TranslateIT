use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::engine::runtime_state::runtime_generation_is_authoritative;

use super::functional_readiness::{
    functional_asr_output, functional_translation_output, functional_voice_actor_output_path,
    invalidate_required_outbound_ai_readiness, worker_response_value,
};
use super::{get_helper_bridge_status, send_worker_task, worker_text};

const REQUIRED_OUTBOUND_FUNCTIONAL_ID_FIXTURE: &str = "selamat pagi";

fn remove_probe_output_paths(requested_path: &str, reported_path: Option<&str>) {
    if let Some(path) = reported_path.filter(|path| !path.trim().is_empty()) {
        let _ = fs::remove_file(path);
    }
    if reported_path.map(str::trim) != Some(requested_path.trim()) && !requested_path.trim().is_empty() {
        let _ = fs::remove_file(requested_path);
    }
}

pub(super) fn run_required_outbound_ai_probe(
    meeting_generation: Option<u64>,
    output_path: &str,
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
    let helper = get_helper_bridge_status();
    if helper.state != "ready" || !helper.provider_ready || helper.generation_token == 0 {
        invalidate_required_outbound_ai_readiness();
        return Err("local translation runtime");
    }
    let generation_token = helper.generation_token;

    let asr = send_worker_task(
        "asr_preload",
        json!({
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    if !asr.ok {
        invalidate_required_outbound_ai_readiness();
        return Err("speech recognition");
    }

    let translation = send_worker_task(
        "translate",
        json!({
            "text": REQUIRED_OUTBOUND_FUNCTIONAL_ID_FIXTURE,
            "source_language": "id",
            "target_language": "en",
            "max_new_tokens": 24,
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    let Some(translated_fixture) = functional_translation_output(&translation) else {
        invalidate_required_outbound_ai_readiness();
        return Err("Indonesian to English translation");
    };

    let actor_preflight = send_worker_task(
        "voice_actor_preflight",
        json!({
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    if !actor_preflight.ok {
        invalidate_required_outbound_ai_readiness();
        return Err("My Voice");
    }
    let Some(actor_token) = worker_text(&worker_response_value(&actor_preflight), "actor_token") else {
        invalidate_required_outbound_ai_readiness();
        return Err("My Voice");
    };

    let voice = send_worker_task(
        "voice_actor_synthesize",
        json!({
            "text": translated_fixture,
            "output_path": output_path,
            "expected_actor_token": actor_token.clone(),
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    let Some(functional_voice_path) =
        functional_voice_actor_output_path(&voice, &actor_token)
    else {
        remove_probe_output_paths(output_path, None);
        invalidate_required_outbound_ai_readiness();
        return Err("My Voice");
    };

    let asr_inference = send_worker_task(
        "transcribe",
        json!({
            "audio_path": functional_voice_path,
            "language": "en",
            "beam_size": 1,
            "vad_filter": false,
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    let reported_voice_path = worker_response_value(&voice)
        .get("output_path")
        .and_then(Value::as_str)
        .map(str::to_string);
    remove_probe_output_paths(output_path, reported_voice_path.as_deref());
    if !functional_asr_output(&asr_inference) {
        invalidate_required_outbound_ai_readiness();
        return Err("speech recognition");
    }

    // Synthesis already proves the exact actor token remained authoritative and the
    // ASR round-trip above proves the generated WAV is consumable. Avoid a second
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


/// Source-only Codex voice preparation: GPT-SoVITS without local ASR or MT.
/// No Codex Meeting Start caller or cloud-audio admission exists yet.
pub(super) fn run_codex_voice_only_probe(
    meeting_generation: u64,
) -> Result<(u64, String), &'static str> {
    if meeting_generation == 0 || !runtime_generation_is_authoritative(meeting_generation) {
        return Err("codex_voice:meeting_authority_missing");
    }
    let helper = get_helper_bridge_status();
    // provider_ready verifies the *entire Local* pipeline, not voice alone.
    if helper.state != "ready" || helper.generation_token == 0 {
        return Err("codex_voice:helper_not_running");
    }
    let helper_generation = helper.generation_token;
    let output_path = PathBuf::from(
        crate::engine::paths::ProjectPaths::discover().user_cache_dir
    ).join("helper_functional_readiness/codex_voice_only.wav");
    if !output_path.is_absolute() {
        return Err("codex_voice:output_root_unverified");
    }
    let requested = output_path.to_string_lossy().into_owned();
    let preflight = send_worker_task(
        "voice_actor_preflight",
        json!({"meeting_generation":meeting_generation, "meeting_start_prepare":true}),
    );
    let preflight_json = worker_response_value(&preflight);
    let Some(token) = preflight_json.get("actor_token").and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 512)
        .filter(|_| preflight.ok && preflight.task == "voice_actor_preflight"
            && preflight.generation_token == helper_generation
            && preflight_json.get("stage").and_then(Value::as_str)
                == Some("voice_actor_preflight"))
        .map(str::to_owned)
    else { return Err("codex_voice:actor_preflight_failed"); };
    if !runtime_generation_is_authoritative(meeting_generation) {
        return Err("codex_voice:meeting_authority_lost");
    }
    let voice = send_worker_task(
        "voice_actor_synthesize",
        json!({
            "text":"Please continue.",
            "output_path":requested,
            "expected_actor_token":token,
            "meeting_generation":meeting_generation,
            "meeting_start_prepare":true,
        }),
    );
    let valid = voice.generation_token == helper_generation
        && voice_only_receipt_matches(&voice, &token, &output_path)
        && voice_only_probe_has_wav_header(&output_path);
    // Delete only the fixed request path, never an arbitrary worker-supplied path.
    let _ = fs::remove_file(&output_path);
    if !valid { return Err("codex_voice:synthesis_unverified"); }
    let after = get_helper_bridge_status();
    if after.state != "ready" || after.generation_token != helper_generation
        || !runtime_generation_is_authoritative(meeting_generation)
    {
        return Err("codex_voice:voice_or_meeting_authority_changed");
    }
    Ok((helper_generation, token))
}

/// Compare file identity rather than Rust/Python path spelling. On Windows
/// ProjectPaths emits forward slashes while Python Path.resolve() reports
/// native separators. Reject missing, relative or symlinked files.
fn voice_only_probe_path_matches(reported: &str, requested: &Path) -> bool {
    let candidate = Path::new(reported);
    if !candidate.is_absolute() || !requested.is_absolute()
        || !fs::symlink_metadata(requested)
            .is_ok_and(|meta| meta.file_type().is_file())
        || !fs::symlink_metadata(candidate)
            .is_ok_and(|meta| meta.file_type().is_file())
    {
        return false;
    }
    match (fs::canonicalize(requested), fs::canonicalize(candidate)) {
        (Ok(expected), Ok(actual)) => expected == actual,
        _ => false,
    }
}

/// A receipt alone cannot establish sound quality or semantic correctness.
fn voice_only_receipt_matches(
    response: &super::HelperBridgeWorkerResponse, token: &str, expected_path: &Path
) -> bool {
    let body = worker_response_value(response);
    response.ok && response.task == "voice_actor_synthesize" && !token.is_empty()
        && body.get("stage").and_then(Value::as_str) == Some("voice_actor_synthesize")
        && body.get("actor_token").and_then(Value::as_str) == Some(token)
        && body.get("output_path").and_then(Value::as_str)
            .is_some_and(|path| voice_only_probe_path_matches(path, expected_path))
}

/// Minimum WAV signature only, never a claim of audible/verified output.
fn voice_only_probe_has_wav_header(path: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else { return false; };
    if !meta.file_type().is_file() || meta.len() <= 44 { return false; }
    let Ok(mut file) = fs::File::open(path) else { return false; };
    let mut header = [0_u8; 12];
    file.read_exact(&mut header).is_ok()
        && header.starts_with(b"RIFF") && &header[8..12] == b"WAVE"
}

#[cfg(test)]
mod tests {
    use super::remove_probe_output_paths;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("translateit-probe-{label}-{nonce}.wav"))
    }

    #[test]
    fn codex_voice_only_receipt_rejects_foreign_actor_path_or_stage() {
        let path = temp_path("codex-requested");
        let foreign_path = temp_path("codex-foreign");
        fs::write(&path, b"owned fixture").expect("write requested fixture");
        fs::write(&foreign_path, b"foreign fixture").expect("write foreign fixture");
        let response = |actor: &str, reported: &str, stage: &str| super::super::HelperBridgeWorkerResponse {
            ok: true, state: "ready".into(), task: "voice_actor_synthesize".into(),
            request_id: "test".into(), scheduler_priority: "meeting_outbound".into(),
            message: String::new(), generation_token: 7, runtime_claim: "synthetic".into(),
            worker_response_json: serde_json::json!({
                "actor_token":actor, "output_path":reported, "stage":stage
            }).to_string(),
        };
        let requested = path.to_string_lossy().into_owned();
        let foreign = foreign_path.to_string_lossy().into_owned();
        assert!(super::voice_only_receipt_matches(
            &response("actor", &requested, "voice_actor_synthesize"), "actor", &path
        ));
        assert!(!super::voice_only_receipt_matches(
            &response("other", &requested, "voice_actor_synthesize"), "actor", &path
        ));
        assert!(!super::voice_only_receipt_matches(
            &response("actor", &foreign, "voice_actor_synthesize"), "actor", &path
        ));
        assert!(!super::voice_only_receipt_matches(
            &response("actor", "relative_voice.wav", "voice_actor_synthesize"), "actor", &path
        ));
        assert!(!super::voice_only_receipt_matches(
            &response("actor", &requested, "transcribe"), "actor", &path
        ));
        fs::remove_file(&path).expect("remove requested fixture");
        fs::remove_file(&foreign_path).expect("remove foreign fixture");
    }

    #[cfg(windows)]
    #[test]
    fn codex_voice_only_windows_reported_separators_match_same_wav() {
        let path = temp_path("codex-windows-path");
        fs::write(&path, b"owned fixture").expect("write fixture");
        let native = path.to_string_lossy().into_owned();
        let slash = native.replace('\\', "/");
        let canonical = fs::canonicalize(&path).expect("canonical WAV");
        assert!(super::voice_only_probe_path_matches(&native, &path));
        assert!(super::voice_only_probe_path_matches(&slash, &path));
        assert!(super::voice_only_probe_path_matches(
            canonical.to_string_lossy().as_ref(), &path
        ));
        fs::remove_file(&path).expect("remove requested fixture");
        assert!(!super::voice_only_probe_path_matches(&native, &path));
    }

    #[test]
    fn probe_cleanup_removes_requested_file_without_worker_report() {
        let requested = temp_path("requested");
        fs::write(&requested, b"probe").expect("write probe");

        remove_probe_output_paths(requested.to_string_lossy().as_ref(), None);

        assert!(!requested.exists());
    }

    #[test]
    fn probe_cleanup_removes_distinct_requested_and_reported_files() {
        let requested = temp_path("requested");
        let reported = temp_path("reported");
        fs::write(&requested, b"requested").expect("write requested");
        fs::write(&reported, b"reported").expect("write reported");

        remove_probe_output_paths(
            requested.to_string_lossy().as_ref(),
            Some(reported.to_string_lossy().as_ref()),
        );

        assert!(!requested.exists());
        assert!(!reported.exists());
    }
}
