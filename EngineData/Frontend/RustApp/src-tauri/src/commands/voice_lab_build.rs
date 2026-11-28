use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_events::emit_voice_build_runtime_event;

use super::helper_bridge::invalidate_required_outbound_readiness_for_voice_change;
use super::builtin_voice::{
    install_builtin_voice, is_supported_builtin_voice, meeting_blocks_voice_change,
};
use super::voice_lab::{
    approved_voice_actor_ready, begin_voice_lab_build, current_voice_lab_build_snapshot, fail_voice_lab_build,
    commit_voice_lab_review_if_active,
    preserve_training_checkpoints, prepare_guided_dataset,
    prepare_guided_dataset_from_archives, choose_archived_takes,
    request_voice_lab_build_cancel,
    voice_lab_build_blocks_meeting, archive_selected_personal_voice,
    activate_stored_personal_voice, recover_selected_actor_if_missing,
    selected_voice_info, actor_version_page, activate_archived_personal_voice, selected_personal_actor_matches_version, voice_backup_active,
    ActorVersionPage, GuidedDatasetManifest, GuidedTakeContract,
    VoiceLabStoragePaths, MAX_REFERENCE_MS, MIN_REFERENCE_MS,
    VOICE_ACTOR_ENGINE_REVISION, VOICE_LAB_SCHEMA_VERSION,
};
use super::voice_lab_recording::{
    accepted_guided_recordings, guided_training_prompts,
    guided_recording_recommendations, RecordingSuggestion, voice_lab_recording_active,
};

mod evaluation;
#[cfg(windows)]
mod native_speaker;
#[cfg(windows)]
mod native_evaluation;
#[cfg(windows)]
pub(super) mod native_review;
#[cfg(windows)]
mod native_build;
mod receipts;
use receipts::{completion_file, commit_completion, create_identity, identity_file, manual_stop_file};
pub use evaluation::{VoiceLabCandidateComparison, VoiceLabEvaluationSample, VoiceLabReferenceSelection};
use evaluation::{evaluation_dir, held_out_contract, MAX_EVALUATION_WAV_BYTES};
const MAX_TRAINING_TAKES: usize = 128;

// Guard select/approve on the shared worker actor path. No parallel promotion.
static VOICE_SELECTION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
pub(crate) struct VoiceSelectionGuard {
    // Rust drops struct fields in declaration order. Release the existing
    // voice-selection mutex before releasing the updater operation permit.
    _selection: std::sync::MutexGuard<'static, ()>,
    _update_admission: crate::engine::runtime_state::RuntimeOperationPermit,
}

pub(crate) fn voice_selection_guard() -> Result<VoiceSelectionGuard, ()> {
    // Keep one existing selection lock for voice changes and backup/restore.
    // Its updater permit covers the entire file operation, not just admission.
    let admission = crate::engine::runtime_state::admit_app_runtime_operation()
        .map_err(|_| ())?;
    if voice_backup_active() {
        return Err(());
    }
    let selection = VOICE_SELECTION_LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| ())?;
    Ok(VoiceSelectionGuard {
        _selection: selection,
        _update_admission: admission,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceLabCoverageGuidance {
    pub start_line_id: u32,
    pub end_line_id: u32,
    pub label: &'static str,
    pub suggested_line_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyVoiceComputeStatus {
    pub state: String,
    pub cpu_name: String,
    pub gpu_name: Option<String>,
    pub gpu_uuid: Option<String>,
    pub compute_capability: Option<String>,
    pub precision_profile: Option<String>,
    pub cuda_index: Option<u32>,
    pub visible_cuda_devices: u32,
    pub vram_mib: Option<u64>,
    pub free_vram_mib: Option<u64>,
    pub reserve_mib: Option<u64>,
    pub safe_budget_mib: Option<u64>,
    pub admission_state: String,
    pub profile: String,
    pub detail: String,
}

impl MyVoiceComputeStatus {
    fn unavailable(detail: &str) -> Self {
        Self {
            state: "unavailable".into(),
            cpu_name: "CPU name unavailable".into(),
            gpu_name: None,
            gpu_uuid: None,
            compute_capability: None,
            precision_profile: None,
            cuda_index: None,
            visible_cuda_devices: 0,
            vram_mib: None,
            free_vram_mib: None,
            reserve_mib: None,
            safe_budget_mib: None,
            admission_state: "unknown".into(),
            profile: "SAFE_CUDA_V1".into(),
            detail: detail.into(),
        }
    }
}

// Compatibility command; GPU admission belongs to the native runtime.
#[tauri::command]
pub async fn get_my_voice_compute_status() -> MyVoiceComputeStatus {
    MyVoiceComputeStatus::unavailable("Native My Voice checks GPU admission when creating the voice.")
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceLabBuildStatus {
    pub active: bool,
    pub preview_active: bool,
    pub native_candidate: bool,
    pub native_pins_ready: bool,
    pub generation: Option<u64>,
    pub phase: String,
    pub message: String,
    pub accepted_take_count: usize,
    pub accepted_duration_ms: u64,
    pub minimum_duration_ms: u64,
    pub missing_coverage: Option<VoiceLabCoverageGuidance>,
    pub recording_suggestions: Vec<RecordingSuggestion>,
    pub can_build: bool,
    pub evaluation_ready: bool,
    pub evaluation_review_id: Option<String>,
    pub evaluation_samples: Vec<VoiceLabEvaluationSample>,
    // Advisory comparison of checkpoints from this SAME completed build only.
    pub evaluation_comparison: Vec<VoiceLabCandidateComparison>,
    // Advisory-only; absent when legacy or when source origin is unverified.
    pub reference_selection: Option<VoiceLabReferenceSelection>,
    pub approved_voice_ready: bool,
    pub selected_voice_kind: String,
    pub selected_builtin_voice_id: Option<String>,
    pub saved_personal_voice_available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceLabBuildActionResult {
    pub ok: bool,
    pub state: String,
    pub message: String,
    pub build: VoiceLabBuildStatus,
}

#[derive(Debug, Default)]
struct BuildProcessState {
    generation: Option<u64>,
    terminal_message: String,
}

static BUILD_PROCESS: OnceLock<Mutex<BuildProcessState>> = OnceLock::new();

fn process_store() -> &'static Mutex<BuildProcessState> {
    BUILD_PROCESS.get_or_init(|| Mutex::new(BuildProcessState::default()))
}

pub(crate) fn build_process_cleanup_pending() -> bool {
    // Keep the finalizing fence until the native operation thread exits.
    match process_store().lock() {
        Ok(process) => process.generation.is_some(),
        Err(_) => true,
    }
}

fn storage() -> VoiceLabStoragePaths {
    VoiceLabStoragePaths::from_project_paths(&ProjectPaths::discover())
}

fn work_dir(paths: &VoiceLabStoragePaths) -> PathBuf {
    paths.cache_root.join("Build").join("Runtime")
}

fn status_path(paths: &VoiceLabStoragePaths) -> PathBuf {
    paths.cache_root.join("Build").join("status.json")
}

fn log_path(paths: &VoiceLabStoragePaths) -> PathBuf {
    paths.cache_root.join("Build").join("voice_lab_build.log")
}

fn remove_file_if_present(path: &Path, label: &str) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("voice_lab:{label}_cleanup_failed:{error}")),
    }
}

fn remove_dir_if_present(path: &Path, label: &str) -> Result<(), String> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("voice_lab:{label}_cleanup_failed:{error}")),
    }
}

fn clear_previous_build_workspace(paths: &VoiceLabStoragePaths) -> Result<(), String> {
    remove_file_if_present(&manual_stop_file(paths), "manual_stop")?;
    remove_file_if_present(&completion_file(paths), "completion_receipt")?;
    remove_file_if_present(&identity_file(paths), "run_identity")?;
    remove_file_if_present(&status_path(paths), "status")?;
    remove_dir_if_present(&evaluation_dir(paths), "evaluation")?;
    remove_dir_if_present(&work_dir(paths), "work")?;
    remove_dir_if_present(&paths.candidate_actor_dir, "candidate_actor")?;
    Ok(())
}

fn clear_build_terminal_message() {
    if let Ok(mut process) = process_store().lock() {
        process.terminal_message.clear();
    }
}

fn cleanup_approved_build_workspace(paths: &VoiceLabStoragePaths) -> Result<(), String> {
    let mut errors = Vec::new();
    for result in [
        remove_dir_if_present(&evaluation_dir(paths), "evaluation"),
        remove_dir_if_present(&work_dir(paths), "work"),
        remove_dir_if_present(&paths.candidate_actor_dir, "candidate_actor"),
        remove_dir_if_present(&paths.build_dataset_dir, "build_dataset"),
        remove_file_if_present(&status_path(paths), "status"),
        remove_file_if_present(&completion_file(paths), "completion_receipt"),
        remove_file_if_present(&manual_stop_file(paths), "manual_stop"),
        remove_file_if_present(&identity_file(paths), "run_identity"),
        remove_file_if_present(&log_path(paths), "build_log"),
    ] {
        if let Err(error) = result {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(";"))
    }
}

fn a3_wav_duration_ms(path: &Path) -> Option<u64> {
    let mut file = File::open(path).ok()?;
    let mut header = [0u8; 44];
    file.read_exact(&mut header).ok()?;
    if &header[0..4] != b"RIFF"
        || &header[8..12] != b"WAVE"
        || &header[12..16] != b"fmt "
        || u16::from_le_bytes([header[20], header[21]]) != 1
        || u16::from_le_bytes([header[22], header[23]]) != 1
        || u32::from_le_bytes([header[24], header[25], header[26], header[27]]) != 32_000
        || u16::from_le_bytes([header[34], header[35]]) != 16
        || &header[36..40] != b"data"
    {
        return None;
    }
    let data_size = u32::from_le_bytes([header[40], header[41], header[42], header[43]]) as u64;
    (data_size > 0).then_some(data_size.saturating_mul(1_000) / 64_000)
}

fn accepted_contract() -> (Vec<GuidedTakeContract>, u64, usize) {
    let mut takes = Vec::new();
    let mut duration_ms = 0u64;
    let mut reference_count = 0;
    for recording in accepted_guided_recordings() {
        let Some(duration) = a3_wav_duration_ms(&recording.path) else {
            continue;
        };
        let Some(wav_file) = recording
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        duration_ms = duration_ms.saturating_add(duration);
        if (MIN_REFERENCE_MS..=MAX_REFERENCE_MS).contains(&duration) {
            reference_count += 1;
        }
        takes.push(GuidedTakeContract {
            line_id: recording.line_id,
            exact_text: recording.text.to_string(),
            wav_file,
        });
    }
    (takes, duration_ms, reference_count)
}

fn current_status() -> VoiceLabBuildStatus {
    let paths = storage();
    let snapshot = current_voice_lab_build_snapshot();
    let process_cleanup_pending = build_process_cleanup_pending();
    let recording_active = voice_lab_recording_active();
    let (takes, duration_ms, native_reference_count) = accepted_contract();
    let native_candidate = super::helper_bridge::native_voice_candidate_enabled();
    #[cfg(windows)]
    let native_pins_ready = native_review::pins_ready();
    #[cfg(not(windows))]
    let native_pins_ready = false;
    let accepted_ids: Vec<u32> = takes.iter().map(|take| take.line_id).collect();
    let recording_suggestions = guided_recording_recommendations(&accepted_ids, None);
    #[cfg(windows)]
    let native_evaluation = if !snapshot.active && !process_cleanup_pending {
        native_review::status(&paths)
    } else { None };
    #[cfg(not(windows))]
    let native_evaluation: Option<(String, Vec<VoiceLabEvaluationSample>)> = None;
    let preview_active = super::voice_lab_preview::quick_voice_preview_active();
    let approved_ready = approved_voice_actor_ready(&paths);
    let selected_voice = selected_voice_info(&paths);
    let terminal_message = process_store().lock().ok()
        .map(|state| state.terminal_message.clone()).unwrap_or_default();
    let message = if preview_active && snapshot.phase == "cancelling" {
        "Stopping Quick Preview safely.".to_string()
    } else if preview_active {
        "Creating Quick Preview locally.".to_string()
    } else if snapshot.active {
        match snapshot.phase.as_str() {
            "cancelling" => "Stopping native voice creation safely. The previous Meeting voice is unchanged.",
            "evaluating" => "Creating and evaluating your personal voice locally.",
            _ => "Preparing your accepted recordings.",
        }.to_string()
    } else if process_cleanup_pending {
        "Finalizing My Voice and checking saved files.".to_string()
    } else if native_evaluation.is_some() {
        "Voice samples are ready to listen to and save. Choose the saved version for Meeting.".to_string()
    } else if !native_candidate {
        "Native My Voice requires Windows.".to_string()
    } else if !terminal_message.is_empty() {
        terminal_message
    } else if !native_pins_ready {
        "Native voice evaluation requires reviewed runtime and speaker verifier assets.".to_string()
    } else if takes.len() > MAX_TRAINING_TAKES {
        "Choose up to 128 recordings from Saved Recordings.".to_string()
    } else if native_reference_count < 2 {
        "Accept at least two clear, distinct recordings between 3 and 10 seconds each.".to_string()
    } else {
        "Your recordings are ready for voice creation. GPU admission is checked at Start.".to_string()
    };
    VoiceLabBuildStatus {
        active: snapshot.active || process_cleanup_pending,
        preview_active,
        native_candidate,
        native_pins_ready,
        generation: snapshot.generation,
        phase: if !snapshot.active && process_cleanup_pending { "finalizing".to_string() } else { snapshot.phase },
        message,
        accepted_take_count: takes.len(),
        accepted_duration_ms: duration_ms,
        minimum_duration_ms: 2 * MIN_REFERENCE_MS,
        missing_coverage: None,
        recording_suggestions,
        can_build: !snapshot.active
            && !process_cleanup_pending && !recording_active && takes.len() <= MAX_TRAINING_TAKES
            && native_pins_ready && native_reference_count >= 2,
        evaluation_ready: native_evaluation.is_some(),
        evaluation_review_id: native_evaluation.as_ref().map(|(id, _)| id.clone()),
        evaluation_samples: native_evaluation.map(|(_, samples)| samples).unwrap_or_default(),
        evaluation_comparison: Vec::new(),
        reference_selection: None,
        approved_voice_ready: approved_ready,
        selected_voice_kind: selected_voice.kind,
        selected_builtin_voice_id: selected_voice.builtin_id,
        saved_personal_voice_available: selected_voice.personal_available,
    }
}

fn result(ok: bool, state: &str, message: impl Into<String>) -> VoiceLabBuildActionResult {
    VoiceLabBuildActionResult {
        ok,
        state: state.to_string(),
        message: message.into(),
        build: current_status(),
    }
}

#[tauri::command]
pub async fn get_voice_lab_build_status() -> Result<VoiceLabBuildStatus, String> {
    tauri::async_runtime::spawn_blocking(current_status)
        .await
        .map_err(|_| "My Voice status collection was interrupted.".to_string())
}

pub fn start_voice_lab_build(authorized_voice_confirmed: bool) -> VoiceLabBuildActionResult {
    start_voice_lab_build_selected(authorized_voice_confirmed, None)
}

/// User-selected archived recordings use the same native evaluation pipeline.
pub fn start_voice_lab_improvement(
    authorized_voice_confirmed: bool,
    archive_ids: Vec<String>,
) -> VoiceLabBuildActionResult {
    start_voice_lab_build_selected(authorized_voice_confirmed, Some(archive_ids))
}

fn start_voice_lab_build_selected(
    authorized_voice_confirmed: bool,
    selected_archives: Option<Vec<String>>,
) -> VoiceLabBuildActionResult {
    // The desktop's trained My Voice engine is Windows-only. This is the
    // authoritative native gate shared by Create and Improve, before any
    // recording selection, dataset mutation or Python process launch.
    // A CUDA probe on another OS must not silently grant training support.
    if !cfg!(windows) {
        return result(false, "platform_unsupported",
            "Native My Voice requires Windows.");
    }
    if voice_backup_active() {
        return result(false, "backup_active", "Finish the My Voice backup or restore before creating My Voice.");
    }
    if !authorized_voice_confirmed {
        return result(false, "authorization_required", "Confirm that this is your voice, or that you have permission to create it.");
    }
    if voice_lab_recording_active() {
        return result(false, "recording_active", "Stop the current VoiceLab recording before creating My Voice.");
    }
    let current = current_status();
    if current.active {
        return result(false, "build_active", "VoiceLab creation is already running.");
    }
    if selected_archives.is_some() && (meeting_blocks_voice_change()
        || super::voice_lab_preview::quick_voice_preview_active())
    {
        return result(false, "voice_busy",
            "Stop Meeting or Quick Preview before improving My Voice.");
    }
    #[cfg(windows)]
    { native_build::start(current, selected_archives) }
    #[cfg(not(windows))]
    { result(false, "platform_unsupported", "Native My Voice requires Windows.") }
}

pub fn cancel_voice_lab_build() -> VoiceLabBuildActionResult {
    let snapshot = current_voice_lab_build_snapshot();
    let Some(generation) = snapshot.generation else {
        return result(true, "idle", "There is no active My Voice training to stop.");
    };
    if !snapshot.active {
        return result(true, "idle", "There is no active My Voice training to stop.");
    }
    if snapshot.phase != "cancelling" {
        if let Err(error) = request_voice_lab_build_cancel(generation) {
            return result(false, "cancel_failed", error);
        }
    }

    result(false, "cancel_pending",
        "Stop requested. Waiting for the owned native voice operation to finish safely.")
}

#[tauri::command]
pub fn get_builtin_voice_preview_audio(voice_id: String) -> Result<tauri::ipc::Response, String> {
    if !is_supported_builtin_voice(&voice_id) {
        return Err("builtin_voice:unknown_voice".to_string());
    }
    let project_paths = ProjectPaths::discover();
    #[cfg(windows)]
    if super::helper_bridge::native_voice_candidate_enabled() {
        return super::voice_lab::native_builtin_preview(&project_paths, &voice_id)
            .map(tauri::ipc::Response::new);
    }
    let path = std::path::PathBuf::from(project_paths.voice_runtime_dir)
        .join("BuiltInVoices")
        .join(&voice_id)
        .join("reference.wav");
    let bytes = fs::read(path).map_err(|_| "builtin_voice:preview_unavailable".to_string())?;
    if bytes.len() < 44 || bytes.len() as u64 > MAX_EVALUATION_WAV_BYTES {
        return Err("builtin_voice:preview_invalid".to_string());
    }
    Ok(tauri::ipc::Response::new(bytes))
}

// Built-in selection replaces the canonical selected actor. Preserve any
// approved personal actor in a verified, separate store before replacing it.
fn confirmed_builtin_actor(actor_dir: &Path) -> bool {
    let Ok(bytes) = fs::read(actor_dir.join("actor.json")) else {
        return false;
    };
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|value| value.get("builtin_voice").and_then(serde_json::Value::as_bool))
        == Some(true)
}

fn voice_selection_recovery_pending(saved_root: &Path) -> bool {
    [
        saved_root.join(".MyVoice.previous"),
        saved_root.join(".MyVoice.builtin-previous"),
    ]
    .iter()
    .any(|path| path.exists())
}

pub fn select_builtin_voice(
    voice_id: String,
    authorized_voice_confirmed: bool,
) -> VoiceLabBuildActionResult {
    if !is_supported_builtin_voice(&voice_id) {
        return result(false, "unknown_builtin_voice", "Choose one of the two built-in voices.");
    }
    if meeting_blocks_voice_change() {
        return result(
            false,
            "meeting_active",
            "Stop Meeting translation before changing the Meeting voice.",
        );
    }
    if voice_lab_build_blocks_meeting() || voice_lab_recording_active() {
        return result(false, "voice_busy", "Finish the current My Voice action before switching voices.");
    }
    let _selection_guard = match voice_selection_guard() {
        Ok(guard) => guard,
        Err(()) => return result(false, "voice_selection_unavailable", "Voice selection is busy. Try again."),
    };
    if meeting_blocks_voice_change() || voice_lab_build_blocks_meeting() || voice_lab_recording_active() {
        return result(false, "voice_busy", "Finish Meeting or My Voice before switching voices.");
    }

    let project_paths = ProjectPaths::discover();
    let storage = VoiceLabStoragePaths::from_project_paths(&project_paths);
    let target = storage.approved_actor_dir.clone();
    if recover_selected_actor_if_missing(&storage).is_err() {
        return result(false, "voice_recovery_required",
            "The prior Meeting voice could not be safely restored. Your saved files were kept.");
    }
    if voice_selection_recovery_pending(&storage.saved_root) {
        return result(
            false,
            "voice_recovery_required",
            "Voice switching is unavailable while a previous voice may need recovery. Your saved files were kept.",
        );
    }
    if target.exists() && !super::voice_lab::selected_actor_can_replace(&storage) {
        return result(false, "voice_unverified",
            "The current Meeting voice could not be verified. No voice was replaced.");
    }
    if target.exists() && !authorized_voice_confirmed {
        return result(
            false,
            "approval_required",
            "Replacing the current Meeting voice needs your explicit confirmation.",
        );
    }

    if target.exists() && !confirmed_builtin_actor(&target) {
        if archive_selected_personal_voice(&storage).is_err() {
            return result(false, "personal_voice_protection_failed",
                "Your personal voice could not be safely saved. The current voice was kept.");
        }
    }
    match install_builtin_voice(&project_paths, &voice_id, &target, VOICE_ACTOR_ENGINE_REVISION) {
        Ok(()) => {
            invalidate_required_outbound_readiness_for_voice_change();
            let label = voice_id.trim_end_matches("Voice").to_lowercase();
            result(
                true,
                "selected",
                format!("Built-in {label} voice selected for Meeting."),
            )
        }
        Err(error) => result(false, "builtin_selection_failed", error),
    }
}


#[tauri::command]
pub fn get_voice_lab_actor_versions(after: Option<String>) -> Result<ActorVersionPage, String> {
    actor_version_page(&storage(), after.as_deref())
}

/// Explicit read-only check of the currently selected Meeting actor's exact
/// immutable version identity. Full model SHA-256 hashing is blocking I/O:
/// never run this on Tauri's UI thread or in routine status polling.
#[tauri::command]
pub async fn get_voice_lab_selected_version_match(version_id: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if meeting_blocks_voice_change() || voice_lab_build_blocks_meeting()
            || voice_lab_recording_active() || super::voice_lab_preview::quick_voice_preview_active()
        {
            return Err("voice_lab:version_identity_busy".into());
        }
        let _guard = voice_selection_guard()
            .map_err(|_| "voice_lab:version_identity_lock_unavailable".to_string())?;
        // Re-check after lock acquisition: a voice resource may have changed
        // while this request waited for an earlier version selection.
        if meeting_blocks_voice_change() || voice_lab_build_blocks_meeting()
            || voice_lab_recording_active() || super::voice_lab_preview::quick_voice_preview_active()
        {
            return Err("voice_lab:version_identity_busy".into());
        }
        selected_personal_actor_matches_version(&storage(), &version_id)
    })
    .await
    .map_err(|_| "voice_lab:version_identity_worker_failed".to_string())?
}

/// Explicit opt-in restoration from a full immutable and validated actor
/// package. Shares the existing selection lock and Meeting resource guards.
pub fn select_archived_personal_voice_version(
    version_id: String,
    replace_confirmed: bool,
) -> VoiceLabBuildActionResult {
    if meeting_blocks_voice_change() {
        return result(false, "meeting_active",
            "Stop Meeting translation before switching voice versions.");
    }
    if voice_lab_build_blocks_meeting() || voice_lab_recording_active()
        || super::voice_lab_preview::quick_voice_preview_active()
    {
        return result(false, "voice_busy",
            "Finish recording, voice creation, or Quick Preview before switching versions.");
    }
    let _guard = match voice_selection_guard() {
        Ok(guard) => guard,
        Err(()) => return result(false, "voice_selection_unavailable",
            "Another voice change is in progress. Try again."),
    };
    if meeting_blocks_voice_change()
        || voice_lab_build_blocks_meeting()
        || voice_lab_recording_active()
        || super::voice_lab_preview::quick_voice_preview_active()
    {
        return result(false, "voice_busy",
            "Finish the current Meeting or My Voice action before switching versions.");
    }
    let paths = storage();
    match super::voice_lab::version_is_native(&paths, &version_id) {
        Ok(native) if native == super::helper_bridge::native_voice_candidate_enabled() => {},
        Ok(_) => return result(false, "version_runtime_incompatible", "This saved voice requires its matching runtime. Your current voice is unchanged."),
        Err(_) => return result(false, "version_restore_failed", "Saved voice version could not be verified."),
    }
    if recover_selected_actor_if_missing(&paths).is_err()
        || voice_selection_recovery_pending(&paths.saved_root)
    {
        return result(false, "voice_recovery_required",
            "An earlier voice change needs recovery. Your existing voice was not replaced.");
    }
    let selected = &paths.approved_actor_dir;
    if selected.exists() && !super::voice_lab::selected_actor_can_replace(&paths) {
        return result(false, "voice_unverified",
            "The current Meeting voice needs recovery before a version can be restored.");
    }
    if !replace_confirmed {
        return result(false, "approval_required",
            "Confirm that you want to use this saved version for future meetings.");
    }
    match activate_archived_personal_voice(&paths, &version_id) {
        Ok(()) => {
            invalidate_required_outbound_readiness_for_voice_change();
            match archive_selected_personal_voice(&paths) {
                Ok(()) => result(true, "selected",
                    "Saved voice version is now selected for meetings. Other versions were kept."),
                Err(_) => result(true, "selected_archive_attention",
                    "Voice version is selected, but its extra saved personal copy needs attention. The immutable version is still retained."),
            }
        }
        Err(_) => result(false, "version_restore_failed",
            "Couldn't verify or select this voice version. Check My Voice recovery status before trying again."),
    }
}

pub fn select_stored_personal_voice() -> VoiceLabBuildActionResult {
    if meeting_blocks_voice_change() {
        return result(false, "meeting_active", "Stop Meeting translation before switching voices.");
    }
    if voice_lab_build_blocks_meeting() || voice_lab_recording_active() {
        return result(false, "voice_busy", "Finish the current My Voice action before switching voices.");
    }
    let _guard = match voice_selection_guard() {
        Ok(guard) => guard,
        Err(()) => return result(false, "voice_selection_unavailable", "Voice selection is busy. Try again."),
    };
    if meeting_blocks_voice_change() {
        return result(false, "meeting_active", "Stop Meeting translation before switching voices.");
    }
    let paths = storage();
    if recover_selected_actor_if_missing(&paths).is_err() {
        return result(false, "voice_recovery_required",
            "The prior Meeting voice could not be safely restored. Your saved files were kept.");
    }
    if voice_selection_recovery_pending(&paths.saved_root) {
        return result(false, "voice_recovery_required",
            "An earlier voice change needs recovery. The saved voice was not replaced.");
    }
    match activate_stored_personal_voice(&paths) {
        Ok(()) => {
            invalidate_required_outbound_readiness_for_voice_change();
            result(true, "selected", "Your saved My Voice is selected for Meeting.")
        }
        Err(_) => result(false, "saved_voice_unavailable",
            "Your saved My Voice could not be selected. The current voice was kept."),
    }
}

pub fn approve_voice_lab_candidate(
    reviewed_line_ids: Vec<u32>,
    quality_confirmed: bool,
    review_id: String,
) -> VoiceLabBuildActionResult {
    if meeting_blocks_voice_change() {
        return result(false, "meeting_active", "Stop Meeting translation before selecting My Voice.");
    }
    let _guard = match voice_selection_guard() {
        Ok(guard) => guard,
        Err(()) => return result(false, "voice_selection_unavailable", "Voice selection is busy. Try again."),
    };
    // Recheck after acquiring the selection lock: Meeting may have started
    // while another voice change held this lock.
    if meeting_blocks_voice_change() {
        return result(false, "meeting_active", "Stop Meeting translation before selecting My Voice.");
    }
    if current_voice_lab_build_snapshot().active || build_process_cleanup_pending() {
        return result(false, "build_active",
            "Wait for the previous My Voice process to finish cleanup before approving.");
    }
    #[cfg(windows)]
    {
        if voice_backup_active() || super::voice_lab_recording::voice_lab_recording_blocks_app_exit() {
            return result(false, "voice_busy", "Finish recording or backup before saving My Voice.");
        }
        if !quality_confirmed {
            return result(false, "evaluation_quality_confirmation_required", "Confirm the voice is clear, natural and resembles the authorized speaker.");
        }
        let paths = storage();
        let (profile, reference) = match native_review::approved_payload(&paths, &review_id, &reviewed_line_ids, quality_confirmed) {
            Ok(payload) => payload,
            Err(error) => return result(false, "evaluation_candidate_mismatch", error),
        };
        if meeting_blocks_voice_change() || voice_lab_build_blocks_meeting() || voice_backup_active() {
            return result(false, "voice_busy", "My Voice resources changed; try again.");
        }
        return match super::voice_lab::preserve_native_version(&paths, &profile, &reference) {
            Ok(id) => {
                let cleanup = cleanup_approved_build_workspace(&paths);
                clear_build_terminal_message();
                let message = if cleanup.is_ok() {
                    format!("My Voice saved as version {}. Choose the saved version to use it for Meeting; the current voice is unchanged.", &id[..10])
                } else {
                    format!("My Voice saved as version {}. Temporary review cleanup needs attention; Meeting voice is unchanged.", &id[..10])
                };
                result(true, "native_saved", message)
            }
            Err(error) => result(false, "approval_failed", error),
        };
    }
    #[cfg(not(windows))]
    { result(false, "platform_unsupported", "Native My Voice requires Windows.") }
}

#[tauri::command]
pub fn get_voice_lab_evaluation_audio(
    line_id: u32,
    review_id: String,
) -> Result<tauri::ipc::Response, String> {
    let paths = storage();
    #[cfg(windows)]
    {
        if current_voice_lab_build_snapshot().active || build_process_cleanup_pending() {
            return Err("native_review:build_active".into());
        }
        return native_review::review_audio(&paths, &review_id, line_id).map(tauri::ipc::Response::new);
    }
    #[cfg(not(windows))]
    { Err("voice_lab:platform_unsupported".into()) }
}

#[cfg(test)]
mod builtin_selection_protection_tests {
    use super::{confirmed_builtin_actor, voice_selection_recovery_pending};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn personal_and_invalid_manifests_are_not_treated_as_builtin() {
        let id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("translateit_builtin_guard_{id}"));
        fs::create_dir_all(&root).unwrap();
        let manifest = root.join("actor.json");
        fs::write(&manifest, r#"{"schema_version":1,"engine":"gpt-sovits-v2proplus"}"#).unwrap();
        assert!(!confirmed_builtin_actor(&root));
        fs::write(&manifest, b"invalid json").unwrap();
        assert!(!confirmed_builtin_actor(&root));
        fs::write(&manifest, r#"{"builtin_voice":true}"#).unwrap();
        assert!(confirmed_builtin_actor(&root));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unfinished_voice_replacement_blocks_new_builtin_selection() {
        let id = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("translateit_voice_recovery_{id}"));
        fs::create_dir_all(&root).unwrap();
        assert!(!voice_selection_recovery_pending(&root));
        fs::create_dir_all(root.join(".MyVoice.builtin-previous")).unwrap();
        assert!(voice_selection_recovery_pending(&root));
        let _ = fs::remove_dir_all(root);
    }
}
