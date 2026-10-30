import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const source = (path: string) => readFileSync(new URL(path, root), "utf8");
const page = source("src/pages/MyVoice.svelte");
const build = source("src/components/my-voice/MyVoiceBuild.svelte");
const voiceApi = source("src/app/bridge/myVoiceApi.ts");
const registry = source("src-tauri/src/commands/registry.rs");
const recording = source("src-tauri/src/commands/voice_lab_recording.rs");
const storage = source("src-tauri/src/commands/voice_lab_recording/storage_transaction.rs");
const archive = source("src-tauri/src/commands/voice_lab/storage/recording_archive.rs");

test("normal My Voice opens with a ready-to-use voice picker before optional creation", () => {
  assert.match(page, /<BuiltinVoicePicker/);
  assert.match(page, /let creatorOpen = \$state\(false\)/);
  assert.match(page, /hidden=\{!creatorOpen\}/);
  assert.ok(page.indexOf("<BuiltinVoicePicker") < page.indexOf('id="my-voice-create-workspace"'));
  assert.match(page, /Built-in voices work without recording or training/);
  assert.match(page, /aria-controls="my-voice-create-workspace" aria-expanded=\{creatorOpen\}/);
});

test("hidden voice creator stays mounted and must show active or uncertain work", () => {
  assert.match(page, /const creatorNeedsAttention = \$derived\(Boolean\(/);
  for (const state of [
    "recordingStatusUnknown", "isRecording", "pendingLineId !== null",
    "buildStatusUnavailable", "buildSnapshot?.active", "buildSnapshot?.preview_active",
    "buildSnapshot?.evaluation_ready",
  ]) assert.ok(page.includes(state), state);
  assert.match(page, /if \(creatorNeedsAttention\) creatorOpen = true/);
  assert.match(page, /disabled=\{creatorNeedsAttention\}/);
  assert.match(page, /<MyVoiceBuild/);
  assert.match(build, /Stop Creating/);
  assert.match(build, /Stop Preview/);
  assert.doesNotMatch(page, /\{#if creatorOpen\}[\s\S]*?<MyVoiceBuild/);
});

test("ongoing Improve My Voice request cannot be hidden while submission is pending", () => {
  assert.match(page, /\|\| improvementStarting \|\| buildStatusUnavailable/);
  assert.match(page, /if \(creatorNeedsAttention\) creatorOpen = true/);
  assert.match(page, /disabled=\{creatorNeedsAttention\}/);
});

test("saved recordings, historic versions and backup remain accessible but secondary", () => {
  assert.match(page, /<details[^>]*aria-label="Saved voice management"/);
  assert.match(page, /Saved voices &amp; backups/);
  assert.match(page, /Recording storage needs attention/);
  assert.match(page, /<VoiceVersionBrowser/);
  assert.match(page, /<VoiceBackupRestore/);
  assert.match(page, /<section class="mt-4" aria-label="Recording library">/);
  assert.match(page, /protectLegacyRecordings\(\)/);
});

test("training hardware internals and nonselectable quick preview are disclosed only on demand", () => {
  assert.match(build, /<summary[^>]*>Technical device details<\/summary>/);
  assert.match(build, /<summary[^>]*>Advanced: temporary voice sample<\/summary>/);
  assert.match(build, /This sample cannot be selected as a Meeting voice/);
  assert.match(build, /<QuickVoicePreview/);
  assert.match(build, /onGenerate=\{generateQuickPreview\}/);
  assert.match(build, /onPlay=\{playQuickPreview\}/);
  assert.match(build, /computeStatus\?\.admission_state === "admitted"/);
  assert.match(build, /Recheck hardware/);
});

test("orphan Draft recovery is explicitly user initiated and never auto-accepts a recovered WAV", () => {
  assert.match(voiceApi, /orphaned_drafts: OrphanedGuidedDraft\[\]/);
  assert.match(voiceApi, /draft_recovery_error: string \| null/);
  assert.match(voiceApi, /recover_voice_lab_guided_take/);
  assert.match(registry, /crate::commands::voice_lab_recording::recover_voice_lab_guided_take/);
  assert.match(recording, /pub fn recover_voice_lab_guided_take\(/);
  assert.match(recording, /recover_guided_draft_pair\(&review_path, &native_path, line_id\)/);
  assert.match(recording, /authorized_voice_confirmed/);
  assert.match(recording, /ensure_new_guided_draft_paths_are_free\(/);
  assert.match(storage, /fn ensure_new_guided_draft_paths_are_free\(/);
  assert.match(storage, /path\.with_extension\("wav\.tmp"\)/);
  assert.match(recording, /let current = latest_runtime_session_state\(\)/);
  assert.match(recording, /let latest = latest_runtime_session_state\(\)/);
  assert.match(page, /Recover for review/);
  assert.match(page, /onClick|onclick=\{\(\) => void recoverTake\(\)\}/);
  assert.match(page, /myVoiceApi\.recoverTake\(selectedLineId, authorized\)/);
  assert.match(page, /selectedOrphanDraft !== null/);
  assert.match(page, /This Draft was recovered from disk/);
  assert.match(storage, /fn checked_recovery_pcm16\(/);
  assert.match(storage, /fn recover_guided_draft_pair\(/);
  assert.match(storage, /resample_guided_native_for_review/);
  assert.match(storage, /voice_lab:recovery:draft_audio_mismatch/);
  const recover = recording.slice(
    recording.indexOf("pub fn recover_voice_lab_guided_take("),
    recording.indexOf("pub fn retry_voice_lab_guided_take("),
  );
  assert.doesNotMatch(recover, /accept_review_take|archive_guided_take|fs::remove_file/,
    "recovery must not publish training take, archive bytes or delete old audio");
  assert.match(recover, /\*guard = Some\(PendingDraft \{/);
});

test("recovered review keeps verified WAV ownership through replay, accept and discard", () => {
  assert.match(storage, /struct RecoveredDraftIdentity/);
  assert.match(storage, /sha2::\{Digest, Sha256\}/);
  assert.match(storage, /fn verify_recovered_draft_identity\(/);
  assert.match(storage, /fn recovered_review_matches_bytes\(/);
  assert.match(storage, /fn recovered_pair_identity_rejects_swapped_file_before_accept_or_replay\(/);
  assert.match(storage, /fn partial_retry_accepts_missing_half_but_rejects_replaced_survivor\(/);
  assert.match(recording, /recovered_identity: Some\(recovered_identity\)/);
  assert.match(recording, /recovered_identity: None/);
  const retry = recording.slice(
    recording.indexOf("pub fn retry_voice_lab_guided_take("),
    recording.indexOf("pub fn accept_voice_lab_guided_take("),
  );
  assert.ok(retry.indexOf("verify_recovered_draft_identity(") <
    retry.indexOf("discard_review_take(&native_rate_path)"));
  assert.match(retry, /identity, true/);
  const accept = recording.slice(
    recording.indexOf("pub fn accept_voice_lab_guided_take("),
    recording.indexOf("pub fn list_voice_lab_saved_recordings("),
  );
  assert.ok(accept.indexOf("verify_recovered_draft_identity(") <
    accept.indexOf("archive_guided_take("),
    "recovered Save must verify the pair before any archive or accepted-take mutation");
  assert.match(accept, /identity, false/);
  const replay = recording.slice(recording.indexOf("pub fn get_voice_lab_guided_take_audio("));
  assert.ok(replay.indexOf("let guard = draft_store()") <
    replay.indexOf("verify_recovered_draft_identity("));
  assert.ok(replay.indexOf("fs::read(&path)") <
    replay.indexOf("recovered_review_matches_bytes(") &&
    replay.indexOf("recovered_review_matches_bytes(") <
    replay.indexOf("drop(guard)"),
    "the actual replay bytes must be fingerprint checked while the Draft owner is locked");
  assert.match(page, /case "recovery_changed":/);
});

test("timestamped review reuses guided-take evidence and the current verified Replay", () => {
  const audioOwner = source("src-tauri/src/engine/audio/guided_take.rs");
  assert.match(audioOwner, /struct GuidedReviewInterval/);
  assert.match(audioOwner, /MAX_REVIEW_WARNING_INTERVALS: usize = 12/);
  assert.match(audioOwner, /fn clipping_review_intervals\(/);
  assert.match(audioOwner, /warning_intervals_truncated: timeline\.truncated/);
  assert.match(audioOwner, /clipping_intervals_are_exact_20ms_replay_hints_not_new_blockers/);
  assert.match(audioOwner, /edge_warning_intervals_use_measured_boundaries_not_noise_claims/);
  assert.match(audioOwner, /too_many_warning_intervals_are_capped_and_truncation_is_disclosed/);
  assert.match(storage, /let mut review = review_guided_samples\(line_id, &canonical\.samples\)/);
  assert.match(voiceApi, /warning_intervals: GuidedReviewInterval\[\]/);
  assert.match(voiceApi, /warning_intervals_truncated: boolean/);
  assert.match(page, /recordingState\.pending_review\.warning_intervals/);
  assert.match(page, /onclick=\{\(\) => void replayTake\(warning\.start_ms\)\}/);
  assert.match(page, /const requestRevision = replayRevision/);
  assert.match(page, /if \(startMs !== null\) track\.currentTime = startMs \/ 1000;/);
  assert.match(page, /Only the first 12 detected moments are shown/);
  assert.doesNotMatch(page, /getAudioQuality\(|new Worker\(|startCapture\(/);
});

test("P1 Replay stops before microphone capture and rejects a changed review owner", () => {
  const state = page.slice(page.indexOf("function applyState(next: GuidedRecordingState)"),
    page.indexOf("async function checkMicrophone()"));
  assert.match(state, /recordingState\.recording_line_id !== next\.recording_line_id/);
  assert.match(state, /recordingState\.pending_review_id !== next\.pending_review_id/);
  assert.doesNotMatch(state, /recordingState\.pending_review !== next\.pending_review/);
  const start = page.slice(page.indexOf("async function startRecording()"),
    page.indexOf("async function stopRecording()"));
  const stop = start.indexOf("stopAllMyVoiceAudio();");
  const startCapture = start.indexOf("myVoiceApi.startTake(");
  assert.ok(stop >= 0 && startCapture > stop, "both players must stop before microphone capture");
  const replay = page.slice(page.indexOf("async function replayTake("),
    page.indexOf("onMount(() =>"));
  assert.match(replay, /recordingStatusUnknown \|\| recordingState\.recording_line_id !== null/);
  assert.match(replay, /requestRevision !== replayRevision/);
  assert.match(replay, /const expectedReviewId = pendingForThisLine \? recordingState\.pending_review_id : null/);
  assert.match(replay, /myVoiceApi\.getTakeAudio\(lineId, expectedReviewId\)/);
  assert.match(replay, /recordingState\.pending_review_id !== expectedReviewId/);
  assert.match(voiceApi, /pending_review_id: string \| null/);
  assert.match(voiceApi, /\{ lineId, expectedReviewId \}/);
  assert.match(recording, /pending_review_id: Option<String>/);
  assert.match(recording, /fn next_pending_review_id\(\)/);
  assert.match(recording, /pending_review_id,\s*orphaned_drafts/);
  assert.match(recording, /expected_review_id: Option<String>/);
  assert.match(recording, /voice_lab:replay_review_changed/);
  assert.match(recording, /pending\.map\(\|draft\| draft\.review_id\.as_str\(\)\) != expected_review_id\.as_deref\(\)/);
  assert.match(recording, /read_bounded_replay_wav\(&path\)\?/);
  assert.match(recording, /file\.take\(MAX_REPLAY_WAV_BYTES\.saturating_add\(1\)\)/);
  assert.match(recording, /fn replay_read_is_bounded_before_and_during_file_open\(\)/);
  assert.match(recording, /fn pending_review_id_changes_for_each_published_draft\(\)/);
  assert.match(page, /const requestVersion = \+\+recordingReadVersion/);
  assert.match(page, /requestVersion !== recordingReadVersion/);
});

test("My Voice audio owners hand off playback before record and ignore stale previews", () => {
  assert.match(build, /export function stopPlayback\(\): void/);
  assert.match(build, /if \(!playbackEnabled \|\| voiceChangesLocked\) stopAudio\(\)/);
  assert.match(build, /onBeforePlayback\?\.\(\);/);
  assert.match(build, /!playbackEnabled \|\| voiceChangesLocked \|\| busy/);
  assert.match(page, /let buildPlaybackOwner = \$state/);
  assert.match(page, /<MyVoiceBuild bind:this=\{buildPlaybackOwner\}/);
  assert.match(page, /onBeforePlayback=\{\(\) => stopReplay\(\)\}/);
  assert.match(page, /playbackEnabled=\{creatorOpen && !showRecordPanel/);
  const cancel = page.slice(page.indexOf("function stopAllMyVoiceAudio()"),
    page.indexOf("function openCreationStage("));
  assert.match(cancel, /stopReplay\(\)/);
  assert.match(cancel, /buildPlaybackOwner\?\.stopPlayback\(\)/);
  const record = page.slice(page.indexOf("async function startRecording()"),
    page.indexOf("async function stopRecording()"));
  const stopBoth = record.indexOf("stopAllMyVoiceAudio();");
  const startCapture = record.indexOf("myVoiceApi.startTake(");
  assert.ok(stopBoth >= 0 && startCapture > stopBoth,
    "an absent Stop cannot pass the microphone ownership test");
  assert.match(page, /onclick=\{\(\) => openCreationStage\("record"\)\}>1\. Record/);
  assert.match(page, /onclick=\{\(\) => openCreationStage\("record"\)\}>\s*Record more/);
  for (const name of ["playSavedRecording", "previewBuiltin"]) {
    const first = page.indexOf(`async function ${name}(`);
    const next = page.indexOf("\n  async function ", first + 1);
    const code = page.slice(first, next < 0 ? undefined : next);
    assert.match(code, /stopAllMyVoiceAudio\(\)/);
    assert.match(code, /const requestRevision = replayRevision/);
    assert.match(code, /requestRevision !== replayRevision/);
    assert.match(code, /requestRevision === replayRevision/);
    assert.match(code, /audio === player/);
  }
  const reviewPlayback = build.slice(build.indexOf("async function playEvaluation("),
    build.indexOf("  $effect(() => {", build.indexOf("async function playEvaluation(")));
  assert.match(reviewPlayback, /!playbackEnabled \|\| voiceChangesLocked/);
  assert.match(reviewPlayback, /onBeforePlayback\?\.\(\)/);
});

test("accepted WAV inspection is read-only and backed by exact immutable archive receipts", () => {
  assert.match(voiceApi, /inspect_voice_lab_accepted_take_recovery/);
  assert.match(voiceApi, /type AcceptedTakeRecoveryInspection/);
  assert.match(registry, /crate::commands::voice_lab_recording::inspect_voice_lab_accepted_take_recovery/);
  assert.match(recording, /pub fn inspect_voice_lab_accepted_take_recovery\(/);
  assert.match(archive, /fn verified_archive_id_for_pair_locked\(/);
  assert.match(archive, /verified_archive\(&complete, &prepared\.receipt\)/);
  assert.match(archive, /fn classify_accepted_take_candidates\(/);
  assert.match(archive, /"accepted", "previous", "draft"/);
  assert.match(archive, /previous_backup_present \|\| !unverified_files\.is_empty\(\)/);
  assert.match(archive, /verified_archive_id_for_pair_locked\(\s*storage, line_id, exact_text/);
  assert.match(archive, /interrupted_save_finds_old_previous_pair_and_new_draft_active_pair/);
  assert.match(archive, /legacy_canonical_only_archive_never_claims_an_unmatched_native_sidecar/);
  assert.match(page, /Inspect Save files/);
  assert.match(page, /myVoiceApi\.inspectAcceptedTake\(inspectedLine\)/);
  const inspectUi = page.slice(
    page.indexOf("async function inspectAcceptedFiles()"),
    page.indexOf("async function startRecording()"),
  );
  assert.match(inspectUi, /busy = true;/);
  assert.match(inspectUi, /busy = false;/);
  assert.match(page, /acceptedFileInspection = null;/);
  assert.match(page, /selectedAcceptedEvidence\.verified_pairs/);
  assert.match(page, /selectedAcceptedEvidence\.unverified_files/);
  const inspect = recording.slice(
    recording.indexOf("pub fn inspect_voice_lab_accepted_take_recovery("),
    recording.indexOf("pub fn start_voice_lab_guided_take("),
  );
  assert.match(inspect, /let _draft_owner = draft_store\(\)\.lock\(\)/);
  assert.match(inspect, /inspect_accepted_take_recovery\(/);
  assert.doesNotMatch(inspect, /archive_guided_take\(|fs::remove_file\(|fs::rename\(|accept_review_take\(/,
    "inspection must never promote or delete ambiguous accepted files");
  const scanner = archive.slice(
    archive.indexOf("pub(crate) fn inspect_accepted_take_recovery("),
    archive.indexOf("pub(crate) fn archive_guided_take("),
  );
  assert.doesNotMatch(scanner, /fs::remove_file\(|fs::rename\(|fs::write\(/);
  assert.match(scanner, /ARCHIVE_LOCK/);
});
