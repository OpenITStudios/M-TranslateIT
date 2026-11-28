//! Quick Voice Preview is strictly an isolated VoiceLab creation task.
//! It cannot install, approve, or select any voice for Meeting.
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::engine::paths::ProjectPaths;
use super::voice_lab::{
    begin_voice_lab_build, current_voice_lab_build_snapshot, fail_voice_lab_build,
    request_voice_lab_build_cancel, VoiceLabStoragePaths,
};
use super::voice_lab_recording::{accepted_guided_recordings, voice_lab_recording_active};

#[cfg(windows)]
mod native;

const TIMEOUT: Duration = Duration::from_secs(180);
static ACTIVE_PREVIEW_GENERATION: AtomicU64 = AtomicU64::new(0);
const PREVIEW_WAV: &str = "quick_voice_preview.wav";
const MAX_PREVIEW_WAV_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct QuickVoicePreviewResult {
    pub ok: bool,
    pub state: String,
    pub message: String,
}

fn outcome(ok: bool, state: &str, message: &str) -> QuickVoicePreviewResult {
    QuickVoicePreviewResult { ok, state: state.to_string(), message: message.to_string() }
}

fn preview_dir() -> PathBuf {
    VoiceLabStoragePaths::from_project_paths(&ProjectPaths::discover())
        .cache_root.join("QuickPreview")
}

fn generate_once() -> QuickVoicePreviewResult {
    if voice_lab_recording_active() {
        return outcome(false, "recording_active", "Finish recording before creating a preview.");
    }
    let accepted = accepted_guided_recordings();
    if accepted.is_empty() {
        return outcome(false, "reference_required", "Accept one clear 3–10 second recording first.");
    }
    #[cfg(windows)]
    { native::generate(accepted) }
    #[cfg(not(windows))]
    { outcome(false, "platform_unsupported", "Native voice creation requires Windows.") }
}

pub fn quick_voice_preview_active() -> bool {
    let generation = ACTIVE_PREVIEW_GENERATION.load(Ordering::Acquire);
    generation != 0 && current_voice_lab_build_snapshot().generation == Some(generation)
}

#[tauri::command]
pub async fn generate_voice_lab_quick_preview() -> QuickVoicePreviewResult {
    match tauri::async_runtime::spawn_blocking(generate_once).await {
        Ok(result) => result,
        Err(_) => outcome(false, "preview_task_failed", "Preview could not finish safely."),
    }
}

#[tauri::command]
pub fn cancel_voice_lab_quick_preview() -> QuickVoicePreviewResult {
    let current = current_voice_lab_build_snapshot();
    if let Some(generation) = current.generation {
        // Never cancel a real training run through the preview endpoint.
        if ACTIVE_PREVIEW_GENERATION.load(Ordering::Acquire) == generation
            && current.phase == "preparing"
            && request_voice_lab_build_cancel(generation).is_ok() {
            return outcome(true, "cancelling", "Stopping preview generation.");
        }
    }
    outcome(false, "no_active_preview", "There is no pending Quick Preview to cancel.")
}

#[tauri::command]
pub fn get_voice_lab_quick_preview_audio() -> Result<tauri::ipc::Response, String> {
    if current_voice_lab_build_snapshot().active {
        return Err("voice_lab:preview_busy".into());
    }
    let path = preview_dir().join(PREVIEW_WAV);
    let size = fs::metadata(&path).map_err(|_| "voice_lab:preview_unavailable")?.len();
    if !(45..=MAX_PREVIEW_WAV_BYTES).contains(&size) || path.is_symlink() {
        return Err("voice_lab:preview_invalid".into());
    }
    let bytes = fs::read(path).map_err(|_| "voice_lab:preview_unavailable")?;
    Ok(tauri::ipc::Response::new(bytes))
}
