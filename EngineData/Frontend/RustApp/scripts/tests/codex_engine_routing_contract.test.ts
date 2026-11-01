import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const rust = new URL("../../src-tauri/src/", import.meta.url);
const frontend = new URL("../../src/", import.meta.url);
const rustSource = (path: string): string => readFileSync(new URL(path, rust), "utf8");
const uiSource = (path: string): string => readFileSync(new URL(path, frontend), "utf8");

const settings = rustSource("engine/settings.rs");
const persistence = rustSource("commands/settings.rs");
const preflight = rustSource("commands/meeting_session/preflight.rs");
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
  const guard = preflight.indexOf('translation_engine == "codex"');
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
  const guard = start.indexOf('translation_engine == "codex"');
  const helperRecovery = start.indexOf('recover_helper_after_meeting_stop_if_needed()');
  assert.ok(guard >= 0 && helperRecovery > guard, "must block before local helper recovery");
  assert.match(start, /codex_voice_not_admitted/);

  const source = switching.slice(switching.indexOf("pub(super) fn set_source("));
  const switchGuard = source.indexOf('translation_engine == "codex"');
  const helperStatus = source.indexOf("let mut helper = get_helper_bridge_status();");
  assert.ok(switchGuard >= 0 && helperStatus > switchGuard, "source switch must block before loading MiLMMT");
  assert.match(source, /Original Microphone stays active/);
  assert.match(gateway, /realtime_audio_verified: false/);
  assert.match(gateway, /meeting_route_enabled: false/);
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
