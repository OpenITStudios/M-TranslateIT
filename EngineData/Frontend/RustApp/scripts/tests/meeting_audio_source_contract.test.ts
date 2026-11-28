import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const read = (path: string) => readFileSync(resolve(projectRoot, path), "utf8");

test("Meeting source uses one native selector and existing audio owners", () => {
  const meeting = read("src/pages/Meeting.svelte");
  const header = read("src/components/layout/AppHeader.svelte");
  const bridge = read("src/app/bridge/runtimeApi.ts");
  const registry = read("src-tauri/src/commands/registry.rs");
  const labels = read("src/components/ui/meetingSourceLabels.ts");
  assert.match(labels, /original: "Original Microphone"/);
  assert.match(labels, /translateit: "TranslateIT Audio"/);
  assert.match(header, /<option value="original">\{MEETING_SOURCE_LABELS\.original\}/);
  assert.match(header, /<option value="translateit">\{MEETING_SOURCE_LABELS\.translateit\}/);
  assert.match(header, /runtimeApi\.selectMeetingAudioSource\(source\)/);
  assert.match(bridge, /set_meeting_audio_source/);
  assert.match(registry, /meeting_session::set_meeting_audio_source/);
});

test("stale translation and playback are gated by source epoch", () => {
  const producer = read("src-tauri/src/commands/meeting_session/outbound_pipeline.rs");
  const player = read("src-tauri/src/commands/meeting_session/playback_runtime.rs");
  const native = read("src-tauri/src/engine/audio/meeting_output_runtime.rs");
  const capture = read("src-tauri/src/engine/audio/live_capture.rs");
  assert.match(producer, /translated_epoch_matches\(generation, source_epoch\)/);
  assert.match(player, /job\.source_epoch/);
  assert.match(native, /source_epoch\.is_some_and/);
  assert.match(native, /original_voice_pcm::translated_epoch_matches/);
  assert.match(capture, /original_voice_pcm::try_push_mono/);
});

test("unknown bridge state preserves a safe literal translated-mode fallback", () => {
  const fallback = read("src/app/runtime/meetingBridgeFallback.ts");
  assert.match(fallback, /audio_source: "translateit" as const/);
});


test("Original mode starts under existing Meeting ownership without AI", () => {
  const lifecycle = read("src-tauri/src/commands/meeting_session/lifecycle.rs");
  const intents = read("src-tauri/src/commands/application_runtime/intents.rs");
  const facade = read("src/app/bridge/runtimeProductFacade.ts");
  const meeting = read("src/pages/Meeting.svelte");
  const header = read("src/components/layout/AppHeader.svelte");
  const sources = read("src-tauri/src/commands/meeting_session/audio_source.rs");
  assert.match(lifecycle, /start_meeting_original_impl/);
  assert.match(lifecycle, /if original_requested \{/);
  assert.match(lifecycle, /start_original_voice_output\(output_device, generation, 2\)/);
  assert.match(intents, /"start_original_meeting"/);
  assert.match(facade, /source === "original" \? "start_original_meeting"/);
  assert.match(header, /selectMeetingPreferredAudioSource\(source\)/);
  assert.match(meeting, /currentSource === "original" && originalAvailable \? "Start Meeting"/);
  assert.match(sources, /prepare_required_outbound_ai_runtime\(generation\)/);
});

test("One session uses bounded cleanup, no silent raw speech fallback", () => {
  const source = read("src-tauri/src/commands/meeting_session/audio_source.rs");
  const output = read("src-tauri/src/engine/audio/original_voice_pcm.rs");
  const lifecycle = read("src-tauri/src/commands/meeting_session/lifecycle.rs");
  assert.match(source, /reset_finalized_outbound_speech_boundary/);
  assert.match(source, /original_voice_pcm::begin\(generation, epoch\)/);
  assert.match(output, /translated_epoch_matches/);
  assert.match(lifecycle, /original_voice_pcm::stop\(generation\)/);
});

test("unsafe output mode transitions require canonical Stop rather than fallback", () => {
  const source = read("src-tauri/src/commands/meeting_session/audio_source.rs");
  assert.match(source, /fn require_stop\(generation: u64, reason: &str\)/);
  assert.match(source, /mark_runtime_session_cleanup_incomplete\(generation, true, reason\)/);
  assert.match(source, /Translated output cancellation was not confirmed/);
  assert.match(source, /Original output cleanup failed during source switching/);
  assert.match(source, /fn still_live\(generation: u64, session_id: &str\)/);
});

test("late consumer or playback thread cannot bypass Meeting Stop ownership", () => {
  const playback = read("src-tauri/src/commands/meeting_session/playback_runtime.rs");
  const outbound = read("src-tauri/src/commands/meeting_session/consumer_runtime.rs");
  assert.match(playback, /runtime_generation_is_authoritative\(generation\)/);
  assert.match(outbound, /runtime_generation_is_authoritative\(generation\)/);
  assert.match(playback, /canonical Stop cleanup can release and join it/);
  assert.match(outbound, /retain ownership for canonical Stop teardown/);
});

test("Original Live is not mislabeled as active translation in product readiness", () => {
  const ready = read("src/app/bridge/productReadinessState.ts");
  const lifecycle = read("src-tauri/src/commands/meeting_session/lifecycle.rs");
  assert.match(ready, /const originalLive = productMeeting\.live/);
  assert.match(ready, /Original Microphone is live\./);
  assert.match(ready, /input\.meetingSession\?\.audio_source === "original"/);
  assert.match(lifecycle, /Original Microphone needs a working physical microphone/);
  assert.match(lifecycle, /Original Microphone needs the configured virtual audio route/);
});


test("Native mute gates both raw callback and translated TTS without mixing the source", () => {
  const pcm = read("src-tauri/src/engine/audio/original_voice_pcm.rs");
  const output = read("src-tauri/src/engine/audio/meeting_output_runtime.rs");
  const source = read("src-tauri/src/commands/meeting_session/audio_source.rs");
  assert.match(pcm, /static MUTED: AtomicBool/);
  assert.match(pcm, /fn set_output_muted\(generation: u64, muted: bool\)/);
  assert.match(pcm, /MUTED\.load\(Ordering::Acquire\)/);
  assert.match(pcm, /state\.clear\(\)/);
  assert.match(output, /original_voice_pcm::is_output_muted\(self\.generation\)/);
  assert.match(source, /set_meeting_output_muted|pub\(super\) fn set_muted/);
  assert.match(source, /cancel_meeting_output_for_generation\(generation\)/);
});


test("Mute blocks new VAD speech in all supported capture formats", () => {
  const capture = read("src-tauri/src/engine/audio/live_capture.rs");
  const source = read("src-tauri/src/commands/meeting_session/audio_source.rs");
  assert.equal(
    (capture.match(/&& !original_voice_pcm::is_output_muted\(generation\)/g) ?? []).length, 4,
  );
  assert.match(source, /reset_finalized_outbound_speech_boundary\(&session\.session_id, generation\)/);
});


test("capture, consumers and playback use the same bounded thread-exit observation", () => {
  const audio = read("src-tauri/src/engine/audio/mod.rs");
  assert.match(audio, /AUDIO_THREAD_STOP_WAIT_BUDGET: Duration = Duration::from_millis\(500\)/);
  assert.match(audio, /AUDIO_THREAD_STOP_POLL: Duration = Duration::from_millis\(10\)/);
  assert.match(audio, /while !thread\.is_finished\(\)/);
  for (const path of [
    "engine/audio/live_capture.rs", "engine/audio/meeting_sound_capture.rs",
    "commands/meeting_session/consumer_runtime.rs", "commands/meeting_session/playback_runtime.rs",
  ]) {
    const source = read("src-tauri/src/" + path);
    assert.match(source, /audio_thread_finished_within\([^,]+, AUDIO_THREAD_STOP_WAIT_BUDGET\)/);
    assert.doesNotMatch(source, /fn (?:capture|consumer_thread|playback)_finished_within/);
  }
});

test("source switching reuses only a matching running consumer or playback owner", () => {
  for (const [path, owner] of [
    ["consumer_runtime.rs", "outbound_consumer"],
    ["playback_runtime.rs", "playback_runtime"],
  ]) {
    const source = read("src-tauri/src/commands/meeting_session/" + path);
    assert.ok(source.includes("start_or_reuse_meeting_" + owner + "(generation, session_id, true)"));
    assert.ok(source.includes("start_or_reuse_meeting_" + owner + "(generation, session_id, false)"));
    const start = source.indexOf("fn start_or_reuse_meeting_" + owner + "(");
    const spawn = source.indexOf(".spawn(move ||", start);
    assert.ok(start >= 0 && spawn > start);
    const admission = source.slice(start, spawn);
    assert.equal((admission.match(/local_translated_meeting_route_admitted\(\)/g) ?? []).length, 1);
    assert.equal((admission.match(/\.lock\(\)/g) ?? []).length, 1);
    assert.match(admission, /active\.generation != generation \|\| active\.session_id != session_id/);
    assert.match(admission, /active\.join_failed[\s\S]*?active\.thread\.as_ref\(\)\.is_none_or\(\|thread\| thread\.is_finished\(\)\)/);
    assert.match(admission, /runtime_generation_is_authoritative\(generation\)/);
    if (owner === "playback_runtime") assert.match(admission, /active\.sender\.is_none\(\)/);
  }
});

test("microphone Stop retains failed join and verifies producer before owner release", () => {
  const capture = read("src-tauri/src/engine/audio/live_capture.rs");
  const stop = capture.slice(capture.indexOf("pub fn stop_live_capture_runtime("), capture.indexOf("fn ensure_scratch_capacity"));
  const join = stop.indexOf("active.capture_thread.take()");
  const producer = stop.indexOf("if !clear_finalized_outbound_utterance_producer()", join);
  const release = stop.indexOf("guard.take()", producer);
  assert.ok(join >= 0 && producer > join && release > producer);
  assert.match(stop.slice(0, join), /if active\.join_failed/);
  assert.match(stop.slice(join, producer), /active\.join_failed = true/);
  assert.doesNotMatch(stop.slice(release), /clear_finalized_outbound_utterance_producer/);
  const rollback = capture.slice(capture.indexOf("fn finish_failed_live_capture_start("), capture.indexOf("fn blocked_start("));
  assert.match(rollback, /runtime\.join_failed \|= !joined/);
  assert.match(rollback, /if !joined \|\| !clear_finalized_outbound_utterance_producer\(\)/);
  assert.match(rollback, /runtime\.cleanup_pending = true;\s*\*store = Some\(runtime\)/);
  assert.match(capture, /fn failed_capture_join_retains_owner_across_start_rollback_and_retry_stop/);
});
