import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const rust = new URL("../../src-tauri/src/", import.meta.url);
const frontend = new URL("../../src/", import.meta.url);
const rustSource = (path: string): string => readFileSync(new URL(path, rust), "utf8");
const uiSource = (path: string): string => readFileSync(new URL(path, frontend), "utf8");

const settings = rustSource("engine/settings.rs");
const settingsRuntime = rustSource("engine/runtime_settings.rs");
const engineModule = rustSource("engine/mod.rs");
const persistence = rustSource("commands/settings.rs");
const meeting = rustSource("commands/meeting_session.rs");
const nativeMeetingStart = rustSource("commands/runtime.rs");
const nativeIntents = rustSource("commands/application_runtime/intents.rs");
const incoming = rustSource("commands/meeting_session/incoming_pipeline.rs");
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
const meetingPage = uiSource("pages/Meeting.svelte");
const quickAccess = uiSource("pages/QuickAccess.svelte");
const boot = uiSource("App.svelte");
const facade = uiSource("app/bridge/runtimeProductFacade.ts");
const readiness = uiSource("app/bridge/productReadinessState.ts");
const setupFacade = uiSource("app/bridge/productSetupFacade.ts");
const firstSetup = uiSource("pages/FirstSetup.svelte");

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
  assert.match(preflight, /meeting_session:settings_unverified/);
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
  assert.match(meeting, /fn local_translated_route_for_admission\(/);
  assert.match(meeting, /admission == crate::engine::settings::MeetingEngineAdmission::Local/);
  assert.match(meeting, /fn local_translated_meeting_route_admitted\(\) -> bool/);
  assert.match(meeting, /crate::engine::meeting_engine_admission\(\)/);
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
  assert.match(readiness, /meeting_session:settings_unverified/);
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

test("P12 checks the persisted engine before direct native Meeting route preparation", () => {
  assert.match(meeting, /pub\(super\) fn local_translated_meeting_route_admitted\(\) -> bool/);
  const translated = nativeMeetingStart.slice(
    nativeMeetingStart.indexOf("pub fn start_meeting_translation()"),
    nativeMeetingStart.indexOf("pub fn start_meeting_original()"),
  );
  const guard = translated.indexOf("!meeting_session::local_translated_meeting_route_admitted()");
  const routePrepare = translated.indexOf("clear_prepared_virtual_mic_route_selection();");
  assert.ok(guard >= 0 && routePrepare > guard, "Codex Start must be rejected before VB-CABLE preparation");
  assert.match(translated.slice(guard, routePrepare), /return meeting_session::start_meeting_translation\(\)/);
  const original = nativeMeetingStart.slice(nativeMeetingStart.indexOf("pub fn start_meeting_original()"));
  assert.doesNotMatch(original, /local_translated_meeting_route_admitted/);
});

test("P12 guards alternate native setup intents from hidden Local worker bootstrap", () => {
  assert.match(nativeIntents, /fn require_local_outbound_setup\(\) -> Result<\(\), IntentOutcome>/);
  assert.match(nativeIntents, /meeting_session::local_translated_meeting_route_admitted\(\)/);
  assert.match(nativeIntents, /"engine_not_admitted"/);
  const check = nativeIntents.slice(
    nativeIntents.indexOf('"check_readiness" => {'),
    nativeIntents.indexOf('"ensure_runtime_ready" =>'),
  );
  assert.ok(check.indexOf("require_local_outbound_setup()") >= 0);
  assert.ok(check.indexOf("require_local_outbound_setup()") < check.indexOf("runtime::verify_required_outbound_ai_readiness()"));
  const ensure = nativeIntents.slice(
    nativeIntents.indexOf("fn ensure_runtime_ready()"),
    nativeIntents.indexOf("fn retry_blocked_worker()"),
  );
  assert.ok(ensure.indexOf("require_local_outbound_setup()") >= 0);
  assert.ok(ensure.indexOf("require_local_outbound_setup()") < ensure.indexOf("helper_bridge::start_helper_bridge()"));
  const repair = nativeIntents.slice(nativeIntents.indexOf("fn fix_setup()"));
  assert.ok(repair.indexOf("require_local_outbound_setup()") >= 0);
  assert.ok(repair.indexOf("require_local_outbound_setup()") < repair.indexOf("helper_bridge::start_helper_bridge()"));
  // Explicit worker recovery remains a distinct user action; it is not a
  // Codex Meeting fallback or an automatic bootstrap.
  assert.match(nativeIntents, /"retry_blocked_worker" => retry_blocked_worker\(\)/);
});

test("P12 preserves Original Microphone and the independent incoming caption lane", () => {
  assert.match(lifecycle, /if !original_requested && !super::local_translated_meeting_route_admitted\(\)/);
  assert.match(lifecycle, /let original_eligible = preflight.microphone_ready && preflight.meeting_route_ready/);
  assert.match(meetingPage, /currentSource === "original" && !meeting.hasSession/);
  assert.match(quickAccess, /choice === "original" \|\| meeting.preflight.start_eligible/);
  assert.doesNotMatch(incoming, /local_translated_meeting_route_admitted/);
  assert.match(committed, /if lane == "you" && !super::local_translated_meeting_route_admitted\(\)/);
  assert.match(incoming, /session_id, event_sequence, None, utterance_id, "incoming"/);
});

test("P13 preserves the native Codex blocker through First Setup readiness notices", () => {
  const source = setupFacade.slice(
    setupFacade.indexOf('if (action === "check-readiness")'),
    setupFacade.indexOf('const result = await runtimeApi.verifyModels()'),
  );
  assert.match(source, /dispatchIntent\("check_readiness"\)/);
  assert.match(source, /result\?\.ok/);
  assert.match(source, /compact\(\s*result\?\.message,/);
  const refresh = firstSetup.slice(
    firstSetup.indexOf("async function refreshReadiness()"),
    firstSetup.indexOf("async function selectBuiltin("),
  );
  assert.match(refresh, /const readinessMessage = step === 4/);
  assert.match(refresh, /readinessMessage && readinessMessage !== "TranslateIT is ready\."/);
  assert.match(refresh, /meetingVoiceReady && snapshot\?\.readiness\.meetingReady/);
});

test("P14 admits Local only from verified persisted settings or a clean first install", () => {
  assert.match(settings, /pub enum MeetingEngineAdmission/);
  assert.match(settings, /pub fn meeting_engine_admission\(path: &Path\)/);
  assert.match(settings, /MAX_SETTINGS_FILE_BYTES/);
  assert.match(settings, /file\.take\(MAX_SETTINGS_FILE_BYTES as u64 \+ 1\)/);
  assert.match(settings, /read_to_end\(&mut bytes\)/);
  assert.match(settings, /let legacy_without_engine/);
  assert.match(settings, /None if legacy_without_engine =>/);
  assert.match(settings, /backup\.as_path\(\), temporary\.as_path\(\)/);
  assert.match(settings, /\.all\(\|path\| is_verified_missing\(path\)\)/);
  assert.match(settings, /fn meeting_admission_distinguishes_fresh_install_and_damaged_saved_engine/);
  assert.match(settingsRuntime, /RuntimeSettings::meeting_engine_admission\(&settings_path\(\)\)/);
  assert.match(engineModule, /meeting_engine_admission/);
  assert.doesNotMatch(meeting, /load_settings\(\)\.translation_engine/);
});

test("P14 serialized settings saves cannot race canonical Meeting Start or silently reset Codex", () => {
  assert.match(settings, /static SETTINGS_IO_GATE/);
  assert.match(settings, /pub fn save_pretty\(&self, path: &Path\)[\s\S]*settings_io_gate\(\)\.lock\(\)/);
  assert.match(lifecycle, /pub\(super\) fn try_settings_mutation_guard\(/);
  assert.match(meeting, /lifecycle::try_settings_mutation_guard\(\)/);
  const save = persistence.slice(persistence.indexOf("pub fn save_runtime_settings("));
  const guardIndex = save.indexOf("super::meeting_session::try_settings_mutation_guard()");
  const settingsRead = save.indexOf("let current = engine::load_settings();");
  assert.ok(guardIndex >= 0 && guardIndex < settingsRead);
  assert.match(save, /let unverified_engine = engine::meeting_engine_admission\(\)/);
  assert.match(save, /if runtime_session_owns_audio_resources\(\) && unverified_engine/);
  const device = persistence.slice(persistence.indexOf("pub fn select_audio_device("));
  assert.match(device, /settings_engine_unverified/);
  assert.match(device, /let _meeting_transition = match super::meeting_session::try_settings_mutation_guard\(\)/);
  assert.match(device, /let current = engine::load_settings\(\);\s*if engine::meeting_engine_admission\(\)/);
  assert.match(device, /if runtime_session_owns_audio_resources\(\)/);
  assert.match(meeting, /pub\(super\) fn translated_route_settings_unverified\(\)/);
  assert.match(preflight, /meeting_session:settings_unverified/);
  assert.match(lifecycle, /"settings_unverified"/);
  assert.match(nativeIntents, /"settings_unverified"/);
  assert.match(readiness, /settingsUnverified/);
  assert.match(selector, /needsReconfirmation = false/);
  assert.match(selector, /settings\.translation_engine === engine && !needsReconfirmation/);
  assert.match(selector, /Reconfirm Local engine/);
  assert.match(selector, /chooseEngine\("local"\)/);
  assert.match(page, /needsReconfirmation=\{snapshot\.meetingSession\?\.preflight\?\.blockers\?\.includes\("meeting_session:settings_unverified"\) === true\}/);
});
