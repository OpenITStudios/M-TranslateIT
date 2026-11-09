import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const source = (path: string) => readFileSync(new URL(path, root), "utf8");
const page = source("src/pages/MyVoice.svelte");
const build = source("src/components/my-voice/MyVoiceBuild.svelte");
const picker = source("src/components/my-voice/BuiltinVoicePicker.svelte");
const versions = source("src/components/my-voice/VoiceVersionBrowser.svelte");
const voiceApi = source("src/app/bridge/myVoiceApi.ts");
const registry = source("src-tauri/src/commands/registry.rs");
const recording = source("src-tauri/src/commands/voice_lab_recording.rs");
const storage = source("src-tauri/src/commands/voice_lab_recording/storage_transaction.rs");
const archive = source("src-tauri/src/commands/voice_lab/storage/recording_archive.rs");

test("My Voice has persistent Current Voice, Record–Train–Review and Voice Library hierarchy", () => {
  assert.match(page, /<BuiltinVoicePicker/);
  assert.match(picker, />Current Voice<\/h3>/);
  assert.match(picker, /Change Voice/);
  assert.match(page, /id="my-voice-create-title"/);
  assert.match(page, /aria-label="My Voice creation stages"/);
  assert.match(page, />1\. Record<\/button>/);
  assert.match(page, />2\. Train<\/button>/);
  assert.match(page, />3\. Review &amp; Use<\/button>/);
  assert.match(page, /<section id="my-voice-create-workspace"/);
  assert.doesNotMatch(page, /creatorOpen|Hide Steps/);
  assert.match(page, /id="my-voice-library-heading"/);
  assert.ok(page.indexOf("<BuiltinVoicePicker") < page.indexOf('id="my-voice-create-title"'));
  assert.ok(page.indexOf('aria-label="My Voice creation stages"') <
    page.indexOf('id="my-voice-create-workspace"'));
  assert.ok(page.indexOf('id="my-voice-create-workspace"') <
    page.indexOf('id="my-voice-library-heading"'));
});

test("My Voice keeps a single native recording/build owner visible for recovery", () => {
  assert.match(page, /const activeStage = \$derived\(/);
  assert.match(page, /recordingStatusUnknown \|\| isRecording \|\| pendingLineId !== null/);
  assert.match(page, /const showRecordPanel = \$derived\(/);
  assert.match(page, /const showBuildPanel = \$derived\(/);
  assert.match(page, /hidden={!showRecordPanel}/);
  assert.match(page, /hidden={!showBuildPanel}/);
  assert.equal((page.match(/<MyVoiceBuild\b/g) ?? []).length, 1);
  assert.match(build, /Stop Creating/);
  assert.match(build, /Stop Preview/);
  assert.doesNotMatch(page, /\{#if creatorOpen\}[\s\S]*?<MyVoiceBuild/);
  assert.match(page, /function openCreationStage\(pane: "record" \| "create" \| "review"\): void \{[\s\S]*?stopAllMyVoiceAudio\(\);[\s\S]*?selectedPane = pane;/);
});

test("Recording puts truthful progress above one focus-sized sentence and context actions", () => {
  const record = page.slice(page.indexOf('id="my-voice-create-workspace"'),
    page.indexOf('<div class="mt-4" hidden={!showBuildPanel}>'));
  assert.match(record, /class="grid grid-cols-1 gap-4"/);
  assert.doesNotMatch(record, /xl:grid-cols-\[minmax\(0,1fr\)_280px\]/);
  assert.doesNotMatch(record, /Recording progress and lines/);
  assert.match(record, /aria-label="Recording progress"/);
  assert.match(record, /buildSnapshot\.accepted_duration_ms/);
  assert.match(record, /buildSnapshot\.accepted_take_count/);
  assert.match(record, /aria-valuenow=\{minimumProgress\}/);
  assert.ok(record.indexOf('aria-label="Recording progress"') <
    record.indexOf('>Microphone</strong>'));
  assert.ok(record.indexOf('>Microphone</strong>') <
    record.indexOf('{currentLine.text}'));
  assert.match(record, /bind:checked=\{authorized\}/);
  assert.match(record, /aria-label="Recording actions"/);
  assert.match(record, /onclick=\{\(\) => void startRecording\(\)\}/);
  assert.match(record, /onclick=\{\(\) => void stopRecording\(\)\}/);
  assert.match(record, /onclick=\{\(\) => void retryTake\(\)\}/);
  assert.match(record, /onclick=\{\(\) => void acceptTake\(\)\}/);
  assert.match(record, /onclick=\{\(\) => void replayTake\(\)\}/);
  assert.match(record, /aria-label="Additional recording lines and suggestions"/);
  assert.match(record, /Browse lines/);
  assert.match(record, /Suggested next lines/);
  assert.match(record, /Recording diagnostics/);
});

test("Voice Library shortcuts open existing native version/Improve tools without activating a voice", () => {
  assert.match(page, /id="my-voice-library-heading"/);
  assert.match(page, /onclick=\{\(\) => openVoiceLibrary\("versions"\)\}>Saved Voices<\/button>/);
  assert.match(page, /onclick=\{\(\) => openVoiceLibrary\("improve"\)\}>Improve My Voice<\/button>/);
  assert.match(page, /bind:open=\{voiceLibraryOpen\}/);
  assert.match(page, /if \(destination === "versions"\) \{[\s\S]*?savedVersionsOpenRevision \+= 1;/);
  assert.match(page, /else if \(!savedLibraryOpen\) \{[\s\S]*?toggleSavedLibrary\(\)/);
  assert.match(page, /if \(improvementStarting\) voiceLibraryOpen = true/);
  assert.match(page, /if \(improvementStarting\) event.preventDefault\(\)/);
  assert.match(page, /<VoiceVersionBrowser/);
  assert.match(page, /openRevision=\{savedVersionsOpenRevision\}/);
  assert.match(versions, /const revision = openRevision/);
  assert.match(versions, /if \(!opened\) \{[\s\S]*?void load\(\)/);
  assert.match(page, /<VoiceBackupRestore/);
  assert.match(page, /protectLegacyRecordings\(\)/);
  assert.match(page, /myVoiceBuildApi\.improve\(\[\.\.\.improvementIds\], improvementAuthorized\)/);
  assert.doesNotMatch(page.slice(page.indexOf("function openVoiceLibrary("),
    page.indexOf("function selectSavedTake(")), /myVoiceBuildApi\.improve|myVoiceBuildApi\.approve|selectBuiltin/);
});

test("Train readiness is grouped once and the build/review owners are unchanged", () => {
  assert.match(build, /aria-label="Training readiness"/);
  assert.match(build, /aria-label="Recording readiness"/);
  assert.match(build, /aria-label="Training hardware"/);
  assert.match(build, /build\.can_build \? "Recording requirements met"/);
  assert.match(build, /onclick=\{\(\) => void checkCompute\(\)\}/);
  assert.match(build, /await myVoiceBuildApi\.start\(authorized\)/);
  assert.match(build, /myVoiceBuildApi\.approve\(reviewedLineIds, qualityConfirmed, build\.evaluation_review_id\)/);
  assert.match(build, /!evaluationReviewComplete \|\| !qualityConfirmed/);
  assert.match(build, /Advanced: temporary voice sample/);
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
  assert.match(page, /playbackEnabled=\{!showRecordPanel && !voiceChangesLocked/);
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
