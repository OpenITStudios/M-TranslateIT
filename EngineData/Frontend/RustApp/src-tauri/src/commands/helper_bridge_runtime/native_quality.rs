//! Explicit Windows-only translation quality replay; never a product command.
//! One real native execution owner, existing scheduler permits, same model
//! pins/ABI as the application. No Python worker, fake Meeting or device IO.
use super::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, io::Read};
use translateit_native_cuda::{InferenceInput, InferenceOutput, Language};
use crate::commands::helper_bridge::{NATIVE_MT_MODEL, NATIVE_MT_REVISION};

const MAX_REQUEST_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Replay {
    schema: String,
    source_sha: String,
    corpus_fingerprint: String,
    timeout_ms: u64,
    requests: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    case_id: String,
    text: String,
    source_language: String,
    target_language: String,
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}
fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn direction(case: &Case) -> Option<(Language, Language, &'static str)> {
    match (case.source_language.as_str(), case.target_language.as_str()) {
        ("id", "en") => Some((Language::Indonesian, Language::English, "id->en")),
        ("en", "id") => Some((Language::English, Language::Indonesian, "en->id")),
        _ => None,
    }
}
fn valid(replay: &Replay) -> bool {
    let mut ids = HashSet::new();
    replay.schema == "translateit.native_translation.requests.v1"
        && is_hex(&replay.source_sha, 40) && is_hex(&replay.corpus_fingerprint, 64)
        && (5_000..=600_000).contains(&replay.timeout_ms)
        && (1..=1024).contains(&replay.requests.len())
        && replay.requests.iter().all(|case| {
            !case.case_id.is_empty() && case.case_id.len() <= 128
                && case.case_id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
                && ids.insert(case.case_id.as_str()) && direction(case).is_some()
                && !case.text.trim().is_empty() && !case.text.contains('\0')
                && case.text.chars().take(2001).count() <= 2000
        })
}

#[test]
#[ignore = "explicit maintainer-authorized Local Windows native quality phase only"]
fn explicit_native_translation_quality_replay() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    let input = PathBuf::from(std::env::var_os("TRANSLATEIT_NATIVE_QUALITY_REQUESTS").expect("explicit request bundle"));
    let output = PathBuf::from(std::env::var_os("TRANSLATEIT_NATIVE_QUALITY_RESULTS").expect("new receipt path"));
    let expected = std::env::var("TRANSLATEIT_NATIVE_QUALITY_REQUEST_SHA256").expect("request digest");
    assert!(input.is_absolute() && output.is_absolute() && !output.exists());
    let metadata = fs::symlink_metadata(&input).expect("request metadata");
    assert!(metadata.is_file() && metadata.len() > 0 && metadata.len() <= MAX_REQUEST_BYTES);
    let mut bytes = Vec::new();
    fs::File::open(input).unwrap().take(MAX_REQUEST_BYTES+1).read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes.len() as u64, metadata.len());
    assert_eq!(hex(&bytes), expected);
    let replay: Replay = serde_json::from_slice(&bytes).expect("typed request bundle");
    assert!(valid(&replay), "request contract");
    drop(bytes);

    // Explicit test-only ownership; never construct a Meeting session or
    // synthesize a fake startup/functional readiness receipt.
    let mut native = unsafe { NativeInferenceOwner::open(None) }.expect("reviewed native pair");
    let pci = native.owner.device_id().to_owned();
    let pins = option_env!("TRANSLATEIT_NATIVE_DLL_PINS").expect("reviewed build DLL pins");
    let pin_digest = hex(pins.as_bytes());
    let mut rows = Vec::with_capacity(replay.requests.len());
    let mut error = None;
    for case in replay.requests {
        let (from, to, pair) = direction(&case).expect("validated direction");
        let permit = match acquire_helper_task_permit(HelperTaskPriority::Text) {
            Ok(value) => value,
            Err(e) => { error = Some(e); break; }
        };
        let started = Instant::now();
        let deadline = started + Duration::from_millis(replay.timeout_ms);
        let result = native.owner.begin(InferenceInput::Translation {
            source: case.text, from, to,
        }, deadline).and_then(|pending| pending.wait_output());
        let response = match result {
            Ok(InferenceOutput::Text(text)) => json!({
                "ok":true,"stage":"translate","request_id":permit.request_id(),
                "device":"cuda","cuda_pci_id":pci,"precision":"q6_k",
                "model_id":NATIVE_MT_MODEL,"model_revision":NATIVE_MT_REVISION,
                "translation_contract":"canonical_bidirectional_id_en",
                "translation_degraded":false,"translation_fallback_reason":"",
                "source_language":case.source_language,"target_language":case.target_language,
                "direction_pair":pair,"complete":true,"finished_with_eos":true,
                "hit_token_ceiling":false,"blocker":"","translated_text":text,
                "elapsed_ms":started.elapsed().as_millis() as u64
            }),
            failure => {
                let blocker = match failure {
                    Err(e) => format!("native_quality:{e:?}"),
                    _ => "native_quality:unexpected_output".into(),
                };
                error = Some(blocker.clone());
                json!({"ok":false,"stage":"translate","request_id":permit.request_id(),
                    "direction_pair":pair,"blocker":blocker})
            }
        };
        rows.push(json!({"case_id":case.case_id,"response":response}));
        drop(permit);
        if error.is_some() { break; }
    }
    // Report cleanup only after the owning thread has been joined. The
    // outer fixture process also has a bounded whole-run timeout.
    let end = Instant::now() + Duration::from_secs(30);
    let closed = loop {
        match native.owner.try_close() {
            Ok(true) => break true,
            Ok(false) if Instant::now() < end => std::thread::sleep(Duration::from_millis(10)),
            _ => break false,
        }
    };
    assert!(closed, "native owner cleanup not verified");
    let receipt = json!({
        "schema":"translateit.native_translation.replay.v1",
        "proof":"NATIVE_OWNER_TRANSLATION_ONLY_NOT_MEETING_ACCEPTANCE",
        "source_sha":replay.source_sha,"corpus_fingerprint":replay.corpus_fingerprint,
        "request_sha256":expected,"dll_pins_sha256":pin_digest,"cuda_pci_id":pci,
        "worker_pid":std::process::id(),"worker_terminated":closed,
        "execution_error":error,"results":rows,
    });
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(output).expect("new receipt");
    serde_json::to_writer(&mut file, &receipt).expect("receipt write");
    file.sync_all().expect("receipt sync");
}
