//! Offline raw/finalized PCM16 replay through the existing native owner.
use super::*;
use crate::commands::helper_bridge::NATIVE_ASR_MODEL;
use crate::engine::audio::evidence::condition_asr_pcm;
use crate::engine::audio::meeting_output::decode_quality_asr_pcm;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use translateit_native_cuda::Error;

const MAX_WAV_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AsrReplay {
    schema: String,
    source_sha: String,
    segment_fingerprint: Option<String>,
    audio_fingerprint: Option<String>,
    timeout_ms: u64,
    requests: Vec<AsrCase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AsrCase {
    case_id: String,
    audio_path: PathBuf,
    audio_sha256: String,
    frames: usize,
    language: String,
    #[serde(default)]
    translate: bool,
}

fn language(code: &str) -> Option<Language> {
    match code {
        "id" => Some(Language::Indonesian),
        "en" => Some(Language::English),
        _ => None,
    }
}

fn valid(replay: &AsrReplay) -> bool {
    let mut ids = HashSet::new();
    let finalized = replay.schema == "translateit.native_asr.requests.v1"
        && replay.segment_fingerprint.as_deref().is_some_and(|hash| is_hex(hash, 64))
        && replay.audio_fingerprint.is_none();
    let raw = replay.schema == "translateit.native_audio.requests.v1"
        && replay.audio_fingerprint.as_deref().is_some_and(|hash| is_hex(hash, 64))
        && replay.segment_fingerprint.is_none();
    (finalized || raw) && is_hex(&replay.source_sha, 40)
        && (5_000..=600_000).contains(&replay.timeout_ms)
        && (1..=1024).contains(&replay.requests.len())
        && replay.requests.iter().all(|case| {
            !case.case_id.is_empty() && case.case_id.len() <= 128
                && case.case_id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
                && ids.insert(case.case_id.as_str())
                && case.audio_path.is_absolute() && is_hex(&case.audio_sha256, 64)
                && (4_800..=960_000).contains(&case.frames)
                && language(&case.language).is_some() && (raw || !case.translate)
        })
}

fn verified_pcm(case: &AsrCase) -> Result<Vec<f32>, String> {
    // Read and hash the same bounded handle, with no write/delete sharing
    // or leaf reparse following. Only explicit offline public fixtures.
    let file = fs::OpenOptions::new().read(true).share_mode(1)
        .custom_flags(0x00200000).open(&case.audio_path).map_err(|_| "asr:fixture_open_failed")?;
    let metadata = file.metadata().map_err(|_| "asr:fixture_metadata_failed")?;
    if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0
        || !(44..=MAX_WAV_BYTES).contains(&metadata.len()) {
        return Err("asr:fixture_invalid".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_WAV_BYTES + 1).read_to_end(&mut bytes).map_err(|_| "asr:fixture_read_failed")?;
    if bytes.len() as u64 != metadata.len() || hex(&bytes) != case.audio_sha256 {
        return Err("asr:fixture_identity_mismatch".into());
    }
    let mut pcm = decode_quality_asr_pcm(&bytes)?;
    if pcm.len() != case.frames { return Err("asr:fixture_frame_count_mismatch".into()); }
    // Fixed diagnostic policy: same production conditioner, suppression off.
    condition_asr_pcm(&mut pcm, false);
    Ok(pcm)
}

#[test]
#[ignore = "explicit maintainer-authorized Local Windows native quality phase only"]
fn explicit_native_asr_quality_replay() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    let (bytes, expected, output) = read_request_bundle();
    let replay: AsrReplay = serde_json::from_slice(&bytes).expect("typed ASR requests");
    assert!(valid(&replay), "ASR request contract");
    drop(bytes);
    let raw = replay.audio_fingerprint.is_some();
    let mut native = unsafe { NativeInferenceOwner::open(None) }.expect("reviewed native pair");
    let pci = native.owner.device_id().to_owned();
    let pins = option_env!("TRANSLATEIT_NATIVE_DLL_PINS").expect("reviewed build DLL pins");
    let mut rows = Vec::with_capacity(replay.requests.len());
    let mut execution_error = None;
    for case in replay.requests {
        let pcm = match verified_pcm(&case) {
            Ok(pcm) => pcm,
            Err(error) => { execution_error = Some(error); break; }
        };
        let permit = match acquire_helper_task_permit(HelperTaskPriority::Text) {
            Ok(permit) => permit,
            Err(error) => { execution_error = Some(error); break; }
        };
        let started = Instant::now();
        let result = native.owner.begin(InferenceInput::Speech {
            pcm, language: language(&case.language).expect("validated language"),
        }, started + Duration::from_millis(replay.timeout_ms))
            .and_then(|pending| pending.wait_output());
        let (ok, text, blocker) = match result {
            Ok(InferenceOutput::Text(text)) if !text.trim().is_empty() => (true, text, String::new()),
            Err(Error::Native(9)) => (false, String::new(), "asr:empty_transcript".into()),
            failure => {
                let blocker = match failure {
                    Err(error) => format!("native_asr_quality:{error:?}"),
                    _ => "native_asr_quality:unexpected_output".into(),
                };
                execution_error = Some(blocker.clone());
                (false, String::new(), blocker)
            }
        };
        let mut row = json!({
            "case_id":case.case_id,"audio_sha256":case.audio_sha256,"frames":case.frames,
            "response":{"ok":ok,"stage":"transcribe","request_id":permit.request_id(),
                "device":"cuda","cuda_pci_id":pci,"model_id":NATIVE_ASR_MODEL,
                "compute_type":"q8_0","language":case.language,"transcript_text":text,
                "blocker":blocker,"elapsed_ms":started.elapsed().as_millis() as u64}
        });
        drop(permit);
        if ok && case.translate {
            let permit = match acquire_helper_task_permit(HelperTaskPriority::Text) {
                Ok(permit) => permit,
                Err(error) => { execution_error = Some(error); rows.push(row); break; }
            };
            let direction = if case.language == "id" {
                (Language::Indonesian, Language::English, "id->en")
            } else {
                (Language::English, Language::Indonesian, "en->id")
            };
            // The exact current ASR output is the sole translation input.
            let (response, failure) = replay_translation(&mut native, &permit, &pci, text,
                direction, replay.timeout_ms);
            row["translation"] = response;
            execution_error = failure;
        }
        rows.push(row);
        if execution_error.is_some() { break; }
    }
    let closed = close_replay_owner(&mut native);
    let mut receipt = json!({
        "schema":if raw {"translateit.native_audio.replay.v1"} else {"translateit.native_asr.replay.v1"},
        "proof":if raw {"NATIVE_OWNER_RAW_ASR_MT_ONLY_NOT_MEETING_ACCEPTANCE"} else {"NATIVE_OWNER_FINALIZED_ASR_ONLY_NOT_MEETING_ACCEPTANCE"},
        "source_sha":replay.source_sha,
        "request_sha256":expected,"dll_pins_sha256":hex(pins.as_bytes()),"cuda_pci_id":pci,
        "worker_pid":std::process::id(),"worker_terminated":closed,
        "noise_suppression_enabled":false,"execution_error":execution_error,"results":rows
    });
    if raw { receipt["audio_fingerprint"] = json!(replay.audio_fingerprint); }
    else { receipt["segment_fingerprint"] = json!(replay.segment_fingerprint); }
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(output).expect("new ASR receipt");
    serde_json::to_writer(&mut file, &receipt).expect("ASR receipt write");
    file.sync_all().expect("ASR receipt sync");
}

#[test]
fn asr_quality_request_rejects_duplicate_ids_and_out_of_bounds_frames() {
    let case = || AsrCase { case_id:"A01_1.wav".into(), audio_path:PathBuf::from(r"C:\fixtures\A01_1.wav"),
        audio_sha256:"a".repeat(64), frames:4_800, language:"id".into(), translate:false };
    let mut replay = AsrReplay { schema:"translateit.native_asr.requests.v1".into(),
        source_sha:"a".repeat(40), segment_fingerprint:Some("b".repeat(64)), audio_fingerprint:None,
        timeout_ms:5_000, requests:vec![case()] };
    assert!(valid(&replay));
    replay.requests[0].translate = true;
    assert!(!valid(&replay));
    replay.schema = "translateit.native_audio.requests.v1".into();
    replay.audio_fingerprint = replay.segment_fingerprint.take();
    assert!(valid(&replay));
    replay.requests.push(case());
    assert!(!valid(&replay));
    replay.requests.pop();
    replay.requests[0].frames = 4_799;
    assert!(!valid(&replay));
}
