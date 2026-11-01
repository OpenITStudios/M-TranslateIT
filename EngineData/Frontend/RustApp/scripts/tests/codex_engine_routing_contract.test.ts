import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const rust = new URL("../../src-tauri/src/", import.meta.url);
const frontend = new URL("../../src/", import.meta.url);
const rustSource = (path: string): string => readFileSync(new URL(path, rust), "utf8");
const uiSource = (path: string): string => readFileSync(new URL(path, frontend), "utf8");

const settings = rustSource("engine/settings.rs");
const persistence = rustSource("commands/settings.rs");
const meeting = rustSource("commands/meeting_session.rs");
const preflight = rustSource("commands/meeting_session/preflight.rs");
const consumer = rustSource("commands/meeting_session/consumer_runtime.rs");
const outbound = rustSource("commands/meeting_session/outbound_pipeline.rs");
const committed = rustSource("commands/meeting_session/committed_turns.rs");
const playback = rustSource("commands/meeting_session/playback_runtime.rs");
const lifecycle = rustSource("commands/meeting_session/lifecycle.rs");
const switching = rustSource("commands/meeting_session/audio_source.rs");
const gateway = rustSource("commands/codex_gateway.rs");
const types = uiSource("app/shared/types.ts");
const state = uiSource("app/shared/state.ts");
const selector = uiSource("components/settings/MeetingTranslationEngine.svelte");
const page = uiSource("pages/Settings.svelte");
const boot = uiSource("App.svelte");
const facade = uiSource("app/bridge/runtimeProductFacade.ts");
const readiness = uiSource("app/bridge/productReadinessState.ts");

test("engine selection is one strict, persisted Rust and TypeScript preference", () => {
  assert.match(settings, /CURRENT_SCHEMA_VERSION: u32 = 14/);
  assert.match(settings, /pub translation_engine: String/);
  assert.match(settings, /translation_engine: default_translation_engine\(\)/);
  assert.match(settings, /sanitize_translation_engine\(&self\.translation_engine\)/);
  assert.match(settings, /"codex" => "codex"\.to_string\(\)/);
  assert.match(settings, /_ => "local"\.to_string\(\)/);
  assert.match(types, /translation_engine: "local" \| "codex"/);
  assert.match(state, /translation_engine: "local"/);
  assert.match(state, /translation_engine: value\.translation_engine/);
});

test("engine cannot change during an owned Meeting or pending Retry Stop", () => {
  assert.match(persistence, /fn engine_preference_changed/);
  assert.match(persistence, /if runtime_session_owns_audio_resources\(\)\s*&& engine_preference_changed/);
  assert.match(persistence, /audio_resource_lock_message\("changing the translation engine"\)/);
});

test("Codex preflight has no fake Local readiness or local helper probe", () => {
  const guard = preflight.indexOf('!super::local_translated_meeting_route_admitted()');
  const probe = preflight.indexOf("let helper = get_helper_bridge_status();");
  assert.ok(guard >= 0 && probe > guard, "Codex branch must precede the Local worker probe");
  assert.match(preflight, /meeting_session:codex_voice_not_admitted/);
  assert.match(preflight, /start_eligible: false/);
  assert.match(preflight, /ready_for_start: false/);
  assert.match(preflight, /provider_ready: false/);
  assert.match(preflight, /models_ready: false/);
});

test("all Start and Original-to-TranslateIT paths reject unverified Codex before local models", () => {
  const start = lifecycle.slice(lifecycle.indexOf("fn start_meeting_impl("));
  const guard = start.indexOf('!super::local_translated_meeting_route_admitted()');
  const helperRecovery = start.indexOf('recover_helper_after_meeting_stop_if_needed()');
  assert.ok(guard >= 0 && helperRecovery > guard, "must block before local helper recovery");
  assert.match(start, /codex_voice_not_admitted/);

  const source = switching.slice(switching.indexOf("pub(super) fn set_source("));
  const switchGuard = source.indexOf('!super::local_translated_meeting_route_admitted()');
  const helperStatus = source.indexOf("let mut helper = get_helper_bridge_status();");
  assert.ok(switchGuard >= 0 && helperStatus > switchGuard, "source switch must block before loading MiLMMT");
  assert.match(source, /Original Microphone stays active/);
  assert.match(gateway, /realtime_audio_verified: false/);
  assert.match(gateway, /meeting_route_enabled: false/);
});

test("P11 uses one Meeting-local engine admission gate, never a Codex diagnostic flag", () => {
  assert.match(meeting, /fn local_translated_route_for_engine\(engine: &str\) -> bool/);
  assert.match(meeting, /engine == "local"/);
  assert.match(meeting, /fn local_translated_meeting_route_admitted\(\) -> bool/);
  assert.match(meeting, /crate::engine::load_settings\(\)\.translation_engine/);
  assert.match(meeting, /fn only_the_installed_local_translation_route_can_run_existing_ai_pipeline/);
  for (const owner of [preflight, lifecycle, switching, consumer, outbound, committed, playback]) {
    assert.match(owner, /super::local_translated_meeting_route_admitted\(\)/);
    assert.doesNotMatch(owner, /codex_gateway::|connect_codex_gateway\(/);
  }
  assert.match(lifecycle, /if !original_requested && !super::local_translated_meeting_route_admitted\(\)/);
  assert.match(preflight, /meeting_session:codex_voice_not_admitted/);
  assert.match(switching, /Original Microphone stays active/);
});

test("P11 protects worker, final transcript and native playback without a second TTS owner", () => {
  const consumerGuard = consumer.indexOf("pub(super) fn start_meeting_outbound_consumer(");
  const consumerSpawn = consumer.indexOf('.name("translateit-meeting-outbound"', consumerGuard);
  assert.ok(consumerGuard >= 0 && consumer.indexOf("!super::local_translated_meeting_route_admitted()", consumerGuard) < consumerSpawn);
  assert.match(consumer, /meeting_outbound:codex_voice_not_admitted/);

  const pipeline = outbound.slice(outbound.indexOf("pub(super) fn process_outbound_wav("));
  assert.ok(pipeline.indexOf("!super::local_translated_meeting_route_admitted()") <
      pipeline.indexOf('send_helper_worker_task(\n        "transcribe"'));
  assert.match(outbound, /state: "engine_not_admitted"/);
  assert.match(outbound, /"meeting_outbound:codex_voice_not_admitted"/);
  assert.match(pipeline, /"translate"/);
  assert.match(pipeline, /"voice_actor_synthesize"/);

  for (const mutation of [
    "pub(super) fn commit_meeting_turn(",
    "pub(super) fn finish_meeting_turn_translation(",
    "pub(super) fn begin_committed_playback(",
    "pub(super) fn update_committed_turn_delivery_state(",
  ]) {
    const start = committed.indexOf(mutation);
    assert.ok(start >= 0, mutation);
    assert.match(committed.slice(start, start + 950), /!super::local_translated_meeting_route_admitted\(\)/);
  }
  assert.match(committed, /delivery_state != "interrupted" && !super::local_translated_meeting_route_admitted\(\)/);
  assert.match(playback, /"meeting_playback:codex_voice_not_admitted"/);
  assert.match(playback, /deliver_meeting_output_wav_for_source/);
  assert.match(playback, /begin_committed_playback\(/);
  assert.doesNotMatch(consumer, /codex_gateway::/);
});

test("startup does not warm the local AI worker for a selected Codex engine", () => {
  assert.match(boot, /initialSnapshot\.settings\.translation_engine === "local"/);
  assert.match(facade, /settings\.translation_engine === "local"/);
  assert.match(readiness, /meeting_session:codex_voice_not_admitted/);
  assert.match(readiness, /Select Local in Settings to start offline translation/);
});

test("Settings makes Codex explicitly opt-in, unavailable and non-networked", () => {
  assert.match(page, /<MeetingTranslationEngine/);
  assert.match(selector, /<option value="local">/);
  assert.match(selector, /<option value="codex">/);
  assert.match(selector, /Translation is not yet verified|Meeting translation remains blocked|not yet available/);
  assert.match(selector, /runtimeApi\.saveSettings\(candidate\)/);
  assert.match(selector, /runtimeApi\.loadSettings\(\)/);
  assert.match(selector, /does not send microphone audio/);
  assert.match(selector, /disabled=\{locked \|\| saving\}/);
});
