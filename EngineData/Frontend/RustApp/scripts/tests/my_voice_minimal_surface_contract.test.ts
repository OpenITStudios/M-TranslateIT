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
