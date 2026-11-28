import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import test from "node:test";

const preview = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_preview.rs", import.meta.url), "utf8");
const registry = readFileSync(new URL("../../src-tauri/src/commands/registry.rs", import.meta.url), "utf8");
const bridge = readFileSync(new URL("../../src/app/bridge/myVoiceBuildApi.ts", import.meta.url), "utf8");
const ui = readFileSync(new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url), "utf8");
const quickPreviewUi = readFileSync(new URL("../../src/components/my-voice/QuickVoicePreview.svelte", import.meta.url), "utf8");
const status = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build.rs", import.meta.url), "utf8");
const nativeReview = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build/native_review.rs", import.meta.url), "utf8");
const releaseConfig = JSON.parse(readFileSync(new URL("../../src-tauri/tauri.release.conf.json", import.meta.url), "utf8"));
const releaseContract = readFileSync(new URL("../validate_release_package_contract.mjs", import.meta.url), "utf8");
const releasePayload = readFileSync(new URL("../validate_release_payload.mjs", import.meta.url), "utf8");
const nativePreview = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_preview/native.rs", import.meta.url), "utf8");
const nativeRuntime = readFileSync(new URL("../../src-tauri/src/commands/my_voice_runtime.rs", import.meta.url), "utf8");
const nativeReference = readFileSync(new URL("../../src-tauri/src/commands/my_voice_reference.rs", import.meta.url), "utf8");
const retiredPreview = new URL("../../../../Backend/LocalWorker/WorkerRuntime/voice_lab_quick_preview.py", import.meta.url);
const buildChild = readFileSync(new URL("../../../../Backend/LocalWorker/WorkerRuntime/voice_lab_build.py", import.meta.url), "utf8");
const actor = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build.rs", import.meta.url), "utf8");
const mutations = readFileSync(new URL("../../src-tauri/src/commands/application_runtime/mutations.rs", import.meta.url), "utf8");
const recordingUi = readFileSync(new URL("../../src/pages/MyVoice.svelte", import.meta.url), "utf8");
const guidedAudio = readFileSync(new URL("../../src-tauri/src/engine/audio/guided_take.rs", import.meta.url), "utf8");
const evaluation = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build/evaluation.rs", import.meta.url), "utf8");

test("quick preview is an isolated native one-WAV task with existing VoiceLab authority", () => {
  assert.match(preview, /native::generate\(accepted\)/);
  assert.match(nativePreview, /begin_voice_lab_build\(\)/);
  assert.match(nativePreview, /fail_voice_lab_build\(generation\)/);
  assert.match(nativePreview, /ACTIVE_PREVIEW_GENERATION\.store\(generation, Ordering::Release\)/);
  assert.match(preview, /join\("QuickPreview"\)/);
  assert.match(preview, /const PREVIEW_WAV: &str = "quick_voice_preview\.wav"/);
  assert.match(nativePreview, /with_runtime\(generation, deadline/);
  assert.doesNotMatch(preview + nativePreview, /Command::new|voice_lab_quick_preview\.py|promote_voice_actor_candidate|install_builtin_voice|select_builtin_voice|dispatch_product_intent/);
});

test("preview cancel is restricted to its own generation, never a full build", () => {
  assert.match(preview, /ACTIVE_PREVIEW_GENERATION\.load\(Ordering::Acquire\) == generation/);
  assert.match(preview, /current\.phase == "preparing"/);
  assert.match(preview, /request_voice_lab_build_cancel\(generation\)/);
  assert.match(nativeRuntime, /wait_output_with_cancel\(\|\| !authority\(generation, deadline\)\)/);
  assert.match(actor, /approve_voice_lab_candidate\(\s*reviewed_line_ids: Vec<u32>,\s*quality_confirmed: bool,/);
  assert.doesNotMatch(preview + nativePreview, /approve_voice_lab_candidate/);
});

test("typed Quick Preview bridge exposes only generation, cancellation and reading", () => {
  for (const command of [
    "generate_voice_lab_quick_preview",
    "cancel_voice_lab_quick_preview",
    "get_voice_lab_quick_preview_audio",
  ]) {
    assert.ok(registry.includes(command), command);
    assert.ok(bridge.includes('"' + command + '"'), command);
  }
  assert.match(bridge, /QuickVoicePreviewResult/);
  assert.doesNotMatch(preview, /get_meeting_session_status|start_meeting_translation/);
});

test("My Voice UI has one collapsible voice selector and a stage-aware recording workspace", () => {
  const selector = readFileSync(new URL("../../src/components/my-voice/BuiltinVoicePicker.svelte", import.meta.url), "utf8");
  assert.match(selector, /Current Voice/);
  assert.match(selector, /aria-expanded={choicesOpen \|\| builtinPendingId !== null}/);
  assert.match(selector, /hidden={!choicesOpen && !builtinPendingId}/);
  assert.match(recordingUi, /Create My Voice/);
  assert.match(recordingUi, /const activeStage = \$derived/);
  assert.match(recordingUi, /hidden={!showRecordPanel}/);
  assert.match(recordingUi, /hidden={!showBuildPanel}/);
  assert.match(recordingUi, /<MyVoiceBuild bind:this=\{buildPlaybackOwner\} \{onNotice\}/);
  assert.match(ui, /onBuildStatus\?\.\(next\)/);
  assert.match(recordingUi, /Browse lines/);
  // The recording catalog contains 600 prompts, but a single build still
  // accepts at most 128 takes. Never hard-code the old prompt count in UI.
  assert.match(recordingUi, /You don't have to record all \{recordingState\.lines\.length\} sentences\./);
  assert.doesNotMatch(recordingUi, /You don't have to record all 128/);
  assert.match(status, /takes\.len\(\) <= MAX_TRAINING_TAKES/);
  assert.match(recordingUi, /!buildSnapshot\.can_build/);
  assert.match(recordingUi, /buildSnapshot\.minimum_duration_ms/);
});

test("keyboard focus follows explicit recording and Browse lines actions, not background polling", () => {
  const page = readFileSync(new URL("../../src/pages/MyVoice.svelte", import.meta.url), "utf8");
  assert.match(page, /import \{ onMount, tick \} from "svelte"/);
  assert.match(page, /bind:this=\{lineHeading\}/);
  assert.match(page, /bind:this=\{reviewReplayButton\}/);
  assert.match(page, /bind:this=\{recordActionButton\}/);
  assert.match(page, /async function chooseBrowseLine\(lineId: number\)/);
  assert.match(page, /browseLinesOpen = false;[\s\S]*?await tick\(\);[\s\S]*?lineHeading\?\.focus\(\)/);
  assert.match(page, /aria-label=\{`Line \$\{line\.line_id\}, \$\{line\.accepted/);
  assert.match(page, /result\.state === "needs_review"[\s\S]*?reviewReplayButton\?\.focus\(\)/);
  assert.match(page, /selectedPane = "record";[\s\S]*?recordActionButton\?\.focus\(\)/);
  const status = page.slice(page.indexOf("function applyState("), page.indexOf("async function checkMicrophone()"));
  assert.doesNotMatch(status, /\.focus\(/);
  assert.match(page, /xl:grid-cols-\[minmax\(0,1fr\)_280px\]/);
});

test("native Stop and Retry remain visible when recording and creation status disagree", () => {
  const page = readFileSync(new URL("../../src/pages/MyVoice.svelte", import.meta.url), "utf8");
  assert.match(page, /showRecordPanel = \$derived\(activeStage === "record" \|\| isRecording \|\| recordingStatusUnknown\)/);
  assert.match(page, /showBuildPanel = \$derived\(/);
  assert.match(page, /buildStatusUnavailable \|\| buildSnapshot\?\.active === true \|\| buildSnapshot\?\.preview_active === true/);
  assert.match(page, /hidden={!showRecordPanel}/);
  assert.match(page, /hidden={!showBuildPanel}/);
  assert.match(page, /A status check needs attention/);
  assert.match(page, /if \(busy \|\| voiceChangesLocked \|\| buildStatusUnavailable \|\| buildSnapshot\?\.active/);
  assert.match(page, /async function acceptTake\(\): Promise<void> \{\s*if \(busy \|\| buildStatusUnavailable \|\| buildSnapshot\?\.active/);
  assert.match(page, /disabled=\{busy \|\| voiceChangesLocked \|\| buildStatusUnavailable \|\| buildSnapshot\?\.active === true/);
  assert.match(page, /<MyVoiceBuild bind:this=\{buildPlaybackOwner\} \{onNotice\}/);
});

test("completed evaluations take priority over Create and unknown status locks mutations", () => {
  const page = readFileSync(new URL("../../src/pages/MyVoice.svelte", import.meta.url), "utf8");
  const build = readFileSync(new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url), "utf8");
  const stage = page.slice(page.indexOf("const activeStage = $derived"), page.indexOf("const acceptedCount = $derived"));
  assert.ok(stage.indexOf('selectedPane === "record"') < stage.indexOf("buildSnapshot?.evaluation_ready"));
  assert.ok(stage.indexOf("buildSnapshot?.evaluation_ready") < stage.indexOf('selectedPane === "create"'));
  assert.match(page, /voiceChangesLocked=\{voiceChangesLocked \|\| buildStatusUnavailable/);
  assert.match(page, /if \(busy \|\| voiceChangesLocked \|\| buildStatusUnavailable \|\| buildSnapshot\?\.active/);
  assert.match(page, /disabled=\{recordingStatusUnknown \|\| buildStatusUnavailable/);
  assert.match(build, /if \(busy \|\| statusCheckUnavailable \|\| voiceChangesLocked \|\| quickPreviewBusy/);
  assert.match(build, /if \(busy \|\| statusCheckUnavailable \|\| voiceChangesLocked \|\| build\.active/);
  assert.match(build, /disabled=\{busy \|\| statusCheckUnavailable \|\| voiceChangesLocked \|\| !evaluationReviewComplete/);
  assert.match(build, /onBuildStatusUnavailable\?\.\(\)/);
});

test("Quick Preview remains training-only in the visible My Voice UI", () => {
  assert.match(ui, /<QuickVoicePreview/);
  assert.match(quickPreviewUi, /not for meetings|never usable in Meeting/i);
  assert.match(quickPreviewUi, /Generate Quick Preview/);
  assert.match(quickPreviewUi, /Listen to Preview/);
  assert.match(ui, /Stop Preview/);
  assert.match(ui, /myVoiceBuildApi\.generateQuickPreview\(\)/);
  assert.match(ui, /myVoiceBuildApi\.cancelQuickPreview\(\)/);
  assert.match(ui, /myVoiceBuildApi\.getQuickPreviewAudio\(\)/);
  assert.match(quickPreviewUi, /quickPreviewBusy \|\| build\.preview_active/);
  assert.match(ui, /bind:previewAuthorized/);
  assert.match(status, /let preview_active = super::voice_lab_preview::quick_voice_preview_active\(\);/);
  assert.match(status, /VoiceLabBuildStatus \{[\s\S]*?\n\s*preview_active,/);
  assert.match(preview, /pub fn quick_voice_preview_active\(\) -> bool/);
  const start = ui.indexOf("async function generateQuickPreview()");
  const end = ui.indexOf("async function startBuild()", start);
  const block = ui.slice(start, end);
  assert.doesNotMatch(block, /onMeetingVoiceChanged|approve\(|selectBuiltin|dispatchProductIntent/);
});


test("preview publication rechecks generation and cancellation after WAV write", () => {
  const write = nativePreview.indexOf("write_pcm16_wav(&file");
  const finalCheck = nativePreview.indexOf("if !authority(generation, deadline)", write);
  const publish = nativePreview.indexOf("finish_native_voice_preview(generation)");
  assert.ok(write >= 0 && finalCheck > write && publish > finalCheck);
});

test("reference ranking belongs to shared native selector", () => {
  assert.match(preview, /accepted_guided_recordings\(\)/);
  assert.match(nativePreview, /select_references\(accepted, 1,/);
  assert.match(nativeReference, /accepted\.len\(\) > 128/);
  assert.match(nativeReference, /if !active\(\)/);
  assert.doesNotMatch(preview + nativePreview, /Stdio::piped|reference-candidates-stdin|Command::new/);
});

test("preview readiness is invalidated when accepted recording revision or training changes", () => {
  assert.match(ui, /let seenRefreshRevision = untrack\(\(\) => refreshRevision\)/);
  assert.match(ui, /if \(revision !== seenRefreshRevision\)/);
  assert.match(ui, /seenRefreshRevision = revision;\s*quickPreviewReady = false;\s*reviewedLineIds = \[\];\s*qualityConfirmed = false;\s*stopAudio\(\)/);
  assert.match(ui, /if \(next.active && !next.preview_active\)/);
  assert.match(ui, /quickPreviewReady = false;\s*stopAudio\(\)/);
});

test("preview phase is owned by native lifecycle, not stale child status", () => {
  assert.doesNotMatch(status, /child_status|reconcile_phase|mod phase;/);
  assert.match(status, /"Creating Quick Preview locally\."/);
  assert.match(status, /"Stopping Quick Preview safely\."/);
});

test("preview cancellation retains native owner through request cleanup", () => {
  assert.match(nativePreview, /Instant::now\(\) \+ TIMEOUT/);
  assert.match(nativeRuntime, /wait_output_with_cancel\(\|\| !authority\(generation, deadline\)\)/);
  assert.match(nativeRuntime, /release_idle_native_inference\(\)/);
  assert.match(nativePreview, /fail_voice_lab_build\(generation\)[\s\S]*ACTIVE_PREVIEW_GENERATION\.store\(0, Ordering::Release\)/);
});

test("retired Python Quick Preview has no packaged consumer", () => {
  assert.equal(existsSync(retiredPreview), false);
  const name = "voice_lab_quick_preview.py";
  const from = `../../../Backend/LocalWorker/WorkerRuntime/${name}`;
  assert.equal(releaseConfig.bundle.resources[from], undefined);
  assert.ok(!releaseContract.includes(name));
  assert.ok(!releasePayload.includes(name));
  // Remaining legacy training assets are not yet physically retired.
  assert.match(buildChild, /from voice_lab_gpt_sovits_build import build_candidate/);
});

test("native approval requires audible review and affirmative human quality confirmation", () => {
  assert.match(ui, /qualityConfirmed = \$state\(false\)/);
  assert.match(ui, /!evaluationReviewComplete \|\| !qualityConfirmed/);
  assert.match(ui, /myVoiceBuildApi\.approve\(reviewedLineIds, qualityConfirmed, build\.evaluation_review_id\)/);
  assert.match(bridge, /reviewedLineIds,\s*qualityConfirmed,\s*reviewId,/);
  assert.match(mutations, /approve_voice_lab_candidate\(reviewed_line_ids, quality_confirmed, review_id\)/);
  assert.match(nativeReview, /!quality_confirmed \|\| reviewed\.len\(\) != HELD_OUT_LINES\.len\(\)/);
  assert.match(nativeReview, /verify_review\(paths, review_id\)\?/);
  assert.match(actor, /native_review::approved_payload/);
  assert.match(ui, /Create Another Candidate/);
});

test("guided-take signal reasons provide actionable recording guidance without a new scorer", () => {
  for (const reason of ["rejected_silence", "severe_clipping", "signal_too_low", "dc_offset_too_high"]) {
    assert.ok(guidedAudio.includes("voice_lab:take_signal_unusable:" + reason), reason);
    assert.ok(recordingUi.includes("voice_lab:take_signal_unusable:" + reason), reason);
  }
  assert.match(recordingUi, /recordingQualityGuidance\(recordingState\.pending_review\.quality_blocker\)/);
});

test("My Voice microphone check reuses the selected physical input functional probe", () => {
  assert.match(recordingUi, /runtimeApi\.loadSettings\(\)/);
  assert.match(recordingUi, /probeProductAudioDevice\("microphone", settings\.audio\.input_device_id\)/);
  assert.match(recordingUi, /Record a sample to check its sound quality/);
  assert.match(recordingUi, /microphoneCheck === "checking"/);
  assert.doesNotMatch(recordingUi, /getUserMedia\(/);
});

test("guided take recording environment hints are advisory and do not rewrite the voice", () => {
  assert.match(guidedAudio, /fn recording_environment_hints\(/);
  assert.match(guidedAudio, /voice_lab:review:long_edge_pause/);
  assert.match(guidedAudio, /voice_lab:review:possible_room_sound/);
  assert.match(recordingUi, /voice_lab:review:long_edge_pause/);
  assert.match(recordingUi, /voice_lab:review:possible_room_sound/);
  assert.match(guidedAudio, /review_guided_samples\(take\.line_id, &samples_mono\)/);
  assert.match(guidedAudio, /if quality_blocker\.is_empty\(\) \{\s*recording_environment_hints\(samples, &mut review_advice, duration_ms, &mut timeline\)/);
  assert.match(guidedAudio, /return; \/\/ No comparable quiet context/);
  assert.match(guidedAudio, /samples_mono: Vec<f32>/);
  assert.match(guidedAudio, /quality_blocker = guided_take_quality_blocker/);
});

test("guided take offers conservative replay advice without changing the approved engine or hard quality gates", () => {
  const recording = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_recording.rs", import.meta.url), "utf8");
  const bridge = readFileSync(new URL("../../src/app/bridge/myVoiceApi.ts", import.meta.url), "utf8");
  assert.match(guidedAudio, /review_advice: Vec<String>/);
  assert.match(guidedAudio, /voice_lab:review:possible_clipping/);
  assert.match(recording, /fn unusually_short_for_line\(/);
  assert.match(recording, /voice_lab:review:verify_all_words/);
  assert.match(recording, /review\.quality_blocker\.is_empty\(\) && unusually_short_for_line/);
  assert.match(bridge, /review_advice: string\[\]/);
  assert.match(recordingUi, /recordingReviewAdvice\(advice\)/);
  assert.match(recordingUi, /read every word exactly as shown/);
  assert.match(recording, /fn accept_voice_lab_guided_take/);
  assert.doesNotMatch(recording, /run.*asr|whisper.*transcribe/i);
});

test("My Voice listening and approval require the same synthesized review identity", () => {
  assert.match(status, /evaluation_review_id: native_evaluation\.as_ref\(\)\.map\(/);
  assert.match(ui, /build\.evaluation_review_id !== next\.evaluation_review_id/);
  assert.match(ui, /myVoiceBuildApi\.getEvaluationAudio\(lineId, reviewId, expectedSha256\)/);
  assert.match(bridge, /get_voice_lab_evaluation_audio", \{ lineId, reviewId \}/);
  assert.match(nativeReview, /hex\(&hash\(&bytes\)\) != review_id/);
  assert.match(nativeReview, /completion_matches\(paths, review_id\)/);
});

test("review playback hashes the actual returned WAV and never accepts a stale clip", () => {
  assert.match(nativeReview, /wav_sha256: hash\(&s\.wav\)/);
  assert.match(evaluation, /pub sha256: String/);
  assert.match(nativeReview, /verified_bytes\(&sample_path\(root, line_id\), MAX_SAMPLE, &sample\.wav_sha256\)/);
  assert.match(bridge, /subtle\.digest\("SHA-256", bytes\)/);
  assert.match(bridge, /actual === expectedSha256 \? bytes : null/);
  assert.match(ui, /sample\.line_id === lineId\)\?\.sha256/);
  // A stale onended callback must not satisfy the held-out review. The
  // active player, request generation and current review ID must all match.
  const playback = ui.slice(ui.indexOf("async function playEvaluation("));
  const ended = playback.slice(playback.indexOf("player.onended ="), playback.indexOf("player.onerror ="));
  assert.match(ended, /audio !== player \|\| playbackVersion !== audioRequestVersion \|\| reviewId !== build\.evaluation_review_id/);
  assert.ok(ended.indexOf("audio !== player") < ended.indexOf("reviewedLineIds ="));
});

test("My Voice puts the compact current Meeting voice above an accessible three-stage workspace and advanced libraries below it", () => {
  const page = readFileSync(new URL("../../src/pages/MyVoice.svelte", import.meta.url), "utf8");
  const picker = readFileSync(new URL("../../src/components/my-voice/BuiltinVoicePicker.svelte", import.meta.url), "utf8");
  const versions = readFileSync(new URL("../../src/components/my-voice/VoiceVersionBrowser.svelte", import.meta.url), "utf8");

  const current = page.indexOf("<BuiltinVoicePicker");
  const create = page.indexOf('aria-labelledby="my-voice-create-title"');
  const library = page.indexOf('aria-label="Recording library"');
  const history = page.indexOf("<VoiceVersionBrowser");
  const backup = page.indexOf("<VoiceBackupRestore");
  assert.ok(current >= 0 && current < create, "current voice stays above creation");
  assert.ok(create < library && library < history && history < backup,
    "record/create/review must precede secondary saved-version and backup controls");
  assert.match(picker, /Current Voice/);
  assert.match(picker, /border-b border-\[var\(--ti-border\)\] px-0 pb-5/);
  assert.match(picker, /onPreview=|onSelect|onConfirm/);
  assert.match(page, /onPreview=\{previewBuiltin\}/);
  assert.match(page, /onSelect=\{selectBuiltin\}/);
  assert.match(page, /onConfirm=\{confirmPendingBuiltin\}/);
  assert.match(page, /onSelectPersonal=\{selectStoredPersonalVoice\}/);
  assert.match(page, /onVoiceActivated=\{async \(versionId, message\)/);
  assert.match(versions, /<details class="mt-2 text-xs/);
  assert.match(versions, /\{voice\.version_id\}<\/code>/);
  assert.doesNotMatch(versions, /created_at|createdAt|last_used_at/,
    "do not invent dates or provenance which the version API cannot supply");

  const nav = page.match(/<nav\b[^>]*aria-label="My Voice creation stages"[^>]*>[\s\S]*?<\/nav>/)?.[0];
  assert.ok(nav, "the creation stage navigation must be labelled");
  assert.match(nav, /aria-current=\{activeStage === "review" \? "step"/);
  assert.match(nav, /disabled=\{busy \|\| recordingStatusUnknown \|\| buildStatusUnavailable/);
  for (const stage of ["record", "create", "review"]) {
    assert.ok(nav.includes(`onclick={() => openCreationStage("${stage}")}`), stage);
  }
  assert.match(page, /\{#if buildSnapshot\.evaluation_ready && !isRecording && pendingLineId === null\}/);
  assert.match(page, /Return to Review/);
  assert.match(page, /let selectedPane = \$state<"auto" \| "record" \| "create" \| "review">/);
});

test("My Voice keeps audio load feedback separate from verified playback and invalidates pending requests", () => {
  const ui = readFileSync(new URL("../../src/components/my-voice/MyVoiceBuild.svelte", import.meta.url), "utf8");
  const preview = readFileSync(new URL("../../src/components/my-voice/QuickVoicePreview.svelte", import.meta.url), "utf8");
  const stop = ui.slice(ui.indexOf("function stopAudio()"), ui.indexOf("function schedulePoll()"));
  const quick = ui.slice(ui.indexOf("async function playQuickPreview("), ui.indexOf("async function startBuild("));
  const evaluation = ui.slice(ui.indexOf("async function playEvaluation("), ui.indexOf("  $effect(", ui.indexOf("async function playEvaluation(")));
  assert.match(ui, /let evaluationLoadingLineId = \$state<number \| null>\(null\)/);
  assert.match(ui, /let quickPreviewLoading = \$state\(false\)/);
  assert.match(stop, /audioRequestVersion \+= 1/);
  assert.match(stop, /evaluationLoadingLineId = null/);
  assert.match(stop, /quickPreviewLoading = false/);
  assert.match(quick, /quickPreviewLoading = true/);
  assert.match(quick, /finally \{\s*if \(!disposed && requestVersion === audioRequestVersion\) quickPreviewLoading = false/);
  assert.match(evaluation, /evaluationLoadingLineId = lineId/);
  assert.match(evaluation, /finally \{\s*if \(!disposed && requestVersion === audioRequestVersion\) evaluationLoadingLineId = null/);
  assert.match(ui, /aria-busy=\{evaluationLoadingLineId === sample\.line_id\}/);
  assert.match(ui, /evaluationLoadingLineId === sample\.line_id \? "Loading audio\.\.\."/);
  assert.match(ui, /\{quickPreviewLoading\}/);
  assert.match(preview, /aria-busy=\{quickPreviewLoading\}/);
  assert.match(preview, /quickPreviewLoading \? "Loading audio\.\.\."/);
  assert.match(evaluation, /reviewId !== build\.evaluation_review_id/);
  assert.match(evaluation, /audio !== player \|\| playbackVersion !== audioRequestVersion/);
});


test("native Create and Improve use the same build lifecycle and cannot approve an incomplete profile", () => {
  const nativeBuild = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build/native_build.rs", import.meta.url), "utf8");
  const nativeReview = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build/native_review.rs", import.meta.url), "utf8");
  assert.match(status, /native_build::start\(current, selected_archives\)/);
  assert.match(nativeBuild, /prepare_guided_dataset_from_archives/);
  assert.match(nativeBuild, /prepare_guided_dataset\(/);
  assert.match(nativeBuild, /mark_native_voice_evaluating\(generation\)/);
  assert.match(nativeBuild, /commit_voice_lab_review_if_active\(generation,/);
  assert.match(nativeBuild, /commit_completion\(paths, &identity, &review_id\)/);
  assert.doesNotMatch(nativeBuild, /resolve_worker_python_command|Command::new/);
  assert.match(nativeReview, /completion_matches\(paths, review_id\)/);
  assert.match(nativeReview, /verify_source\(paths, &manifest\)\?/);
  assert.match(status, /native_review::approved_payload/);
  assert.match(status, /preserve_native_version/);
  assert.match(ui, /Approve & Save/);
  assert.match(status, /version_runtime_incompatible/);
});


test("native personal Meeting selection uses the approved immutable version and exact scope", () => {
  const store = readFileSync(new URL("../../src-tauri/src/commands/voice_lab/storage.rs", import.meta.url), "utf8");
  const nativeTransport = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge/transport/native.rs", import.meta.url), "utf8");
  const runtime = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge_runtime.rs", import.meta.url), "utf8");
  assert.match(store, /native-version\.json/);
  assert.match(store, /native_version_payload\(&storage, id\)/);
  assert.match(store, /prepare_saved_profile\(&profile, &reference\)/);
  assert.match(store, /version_runtime_incompatible/);
  assert.match(nativeTransport, /selected_native_reference\(&paths\)/);
  assert.match(runtime, /selected_native_identity/);
  assert.match(runtime, /voice\.actor_token == actor_token/);
});
