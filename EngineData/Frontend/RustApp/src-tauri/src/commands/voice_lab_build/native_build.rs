//! Native Create/Improve runs under the existing build generation and
//! process-state cleanup gate. No Python child, new scheduler or model owner.
use super::*;
use super::super::voice_lab::{mark_native_voice_evaluating, ArchivedSelection};
use super::super::voice_lab_recording::{AcceptedGuidedRecording, voice_lab_recording_blocks_app_exit};
use super::super::my_voice_runtime::authority;

const NATIVE_DEADLINE: Duration = Duration::from_secs(600);

pub(super) fn start(current: VoiceLabBuildStatus, archive_ids: Option<Vec<String>>) -> VoiceLabBuildActionResult {
    if !native_review::pins_ready() {
        return result(false, "assets_unavailable", "Reviewed native voice runtime and speaker verifier pins are required.");
    }
    if meeting_blocks_voice_change() || voice_backup_active() || voice_lab_recording_blocks_app_exit() {
        return result(false, "voice_busy", "Finish Meeting, recording review, or backup before creating My Voice.");
    }
    let paths = storage();
    let selection = if let Some(ids) = archive_ids {
        match choose_archived_takes(&paths, &ids, guided_training_prompts()) {
            Ok(selection) => Some(selection),
            Err(error) => return result(false, "saved_recordings_invalid", error),
        }
    } else {
        if !current.can_build { return result(false, "more_recording_needed", current.message); }
        None
    };
    // Serialize admission with approval and backup's existing selection lock.
    let _selection_guard = match voice_selection_guard() {
        Ok(guard) => guard,
        Err(()) => return result(false, "voice_busy", "Another voice action is active."),
    };
    let started = match begin_voice_lab_build() {
        Ok(s) => s,
        Err(error) => return result(false, "build_blocked", error),
    };
    let Some(generation) = started.generation else {
        return result(false, "build_state_invalid", "My Voice build authority is unavailable.");
    };
    if voice_backup_active() || voice_lab_recording_blocks_app_exit() {
        let _ = fail_voice_lab_build(generation);
        return result(false, "voice_busy", "Finish recording or backup first.");
    }
    // Reuse the existing finalizing gate. The operation thread owns the
    // generation until completion; no PID or Windows training Job is created.
    {
        let mut process = match process_store().lock() {
            Ok(process) => process,
            Err(_) => { let _ = fail_voice_lab_build(generation); return result(false, "build_blocked", "My Voice cleanup state is unavailable."); }
        };
        if process.generation.is_some() {
            drop(process);
            let _ = fail_voice_lab_build(generation);
            return result(false, "build_blocked", "Previous My Voice cleanup is incomplete.");
        }
        process.generation = Some(generation);
        process.terminal_message.clear();
    }
    let spawned = std::thread::Builder::new().name("my-voice-build".into()).spawn(move || {
        let outcome = run(&paths, generation, selection);
        let cancelled = current_voice_lab_build_snapshot().cancel_requested;
        if outcome.is_err() { let _ = fail_voice_lab_build(generation); }
        let (reason, message) = match outcome {
            Ok(()) => ("voice_build_completed", "Native voice samples are ready for review. Your previous Meeting voice is unchanged.".into()),
            Err(_) if cancelled => ("voice_build_cancelled", "My Voice creation stopped. Saved recordings and the previous Meeting voice are unchanged.".into()),
            Err(error) => ("voice_build_failed", format!("Native My Voice could not finish: {error}. The previous Meeting voice is unchanged.")),
        };
        if let Ok(mut process) = process_store().lock() {
            if process.generation == Some(generation) {
                process.generation = None;
                process.terminal_message = message;
            }
        }
        emit_voice_build_runtime_event(reason, generation);
    });
    if spawned.is_err() {
        let _ = fail_voice_lab_build(generation);
        if let Ok(mut process) = process_store().lock() {
            if process.generation == Some(generation) { process.generation = None; }
        }
        return result(false, "build_blocked", "Could not start the native My Voice operation.");
    }
    result(true, "building", "Creating My Voice locally without personal model training.")
}

fn run(paths: &VoiceLabStoragePaths, generation: u64, selection: Option<ArchivedSelection>) -> Result<(), String> {
    let deadline = Instant::now() + NATIVE_DEADLINE;
    if !authority(generation, deadline) { return Err("native_build:cancelled".into()); }
    // Preserve historical interrupted training before touching its dataset.
    // Retain the existing conservative RAM/disk preflight until the frozen
    // dataset and legacy recovery budgets are separately measured.
    super::super::my_voice_resources::preflight(&paths.cache_root, &paths.saved_root)?;
    preserve_training_checkpoints(paths)?;
    let takes = selection.as_ref().map(|s| s.takes.clone()).unwrap_or_else(|| accepted_contract().0);
    if !(2..=MAX_TRAINING_TAKES).contains(&takes.len()) {
        return Err("native_build:distinct_recordings_required".into());
    }
    let manifest = GuidedDatasetManifest { schema_version: VOICE_LAB_SCHEMA_VERSION,
        authorized_voice_confirmed: true, takes, held_out_lines: held_out_contract() };
    let project = ProjectPaths::discover();
    match selection {
        Some(selected) => prepare_guided_dataset_from_archives(&project, generation, &manifest, &selected.archive_ids)?,
        None => { prepare_guided_dataset(&project, generation, &manifest)?; }
    }
    if !authority(generation, deadline) { return Err("native_build:cancelled".into()); }
    clear_previous_build_workspace(paths)?;
    fs::create_dir_all(paths.cache_root.join("Build")).map_err(|_| "native_build:workspace_unavailable")?;
    let identity = create_identity(paths, generation)?;
    let mut accepted = Vec::with_capacity(manifest.takes.len());
    for take in manifest.takes {
        let text = guided_training_prompts().iter().find(|(id, _)| *id == take.line_id)
            .map(|(_, text)| *text).filter(|text| *text == take.exact_text.as_str())
            .ok_or("native_build:prompt_mismatch")?;
        accepted.push(AcceptedGuidedRecording { line_id: take.line_id, text,
            path: paths.build_dataset_dir.join(take.wav_file) });
    }
    mark_native_voice_evaluating(generation)?;
    let evaluation = native_evaluation::evaluate(generation, deadline, accepted)?;
    let staged = native_review::stage(paths, generation, deadline, evaluation)?;
    let review_id = staged.review_id.clone();
    // Both receipts must exist. Stop and publication share the existing
    // lifecycle lock. A partial publication is not a completed review.
    if !authority(generation, deadline) { return Err("native_build:cancelled".into()); }
    commit_voice_lab_review_if_active(generation, || {
        if Instant::now() >= deadline { return Err("native_build:deadline".into()); }
        staged.publish()?;
        commit_completion(paths, &identity, &review_id)
    })?;
    Ok(())
}
