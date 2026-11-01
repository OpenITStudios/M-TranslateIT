import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const root = new URL("../../src-tauri/src/", import.meta.url);
const read = (file: string) => readFileSync(new URL(file, root), "utf8");
const gateway = read("commands/codex_gateway.rs");
const transport = read("commands/codex_gateway/transport.rs");
const protocol = read("commands/codex_gateway/protocol.rs");
const realtime = read("commands/codex_gateway/realtime.rs");
const correlation = read("commands/codex_gateway/correlation.rs");
const registry = read("commands/registry.rs");
const shutdown = read("commands/application_runtime/shutdown.rs");
const frontend = readFileSync(
  new URL("../../src/app/bridge/codexGatewayApi.ts", import.meta.url), "utf8"
);
const diagnostics = readFileSync(
  new URL("../../src/components/settings/CodexGatewayDiagnostics.svelte", import.meta.url), "utf8"
);

test("Codex App Server has one owned stdio process, bounded reader and no paid key inheritance", () => {
  assert.match(transport, /Command::new\(binary\)/);
  assert.match(transport, /"app-server", "--listen", "stdio:\/\/"/);
  assert.match(transport, /MAX_FRAME_BYTES/);
  assert.match(transport, /sync_channel\(EVENT_QUEUE_CAPACITY\)/);
  assert.match(transport, /event_queue_overflow/);
  assert.match(transport, /child\.kill\(\)/);
  assert.match(transport, /reader\.join\(\)/);
  assert.match(transport, /command\.env_remove\(key\)/);
  assert.match(transport, /"OPENAI_API_KEY"/);
  assert.doesNotMatch(transport, /Command::new\("cmd"/);
  assert.match(transport, /native_candidates_in_bin/);
  assert.match(transport, /"CODEX_CLI_PATH"/);
  assert.match(transport, /"codex-win32-x64"/);
  assert.match(transport, /account_dirty/);
  assert.match(transport, /take_account_dirty/);
});

test("RPC admission is introspection-only, no inference and no automatic paid fallback", () => {
  assert.match(protocol, /"initialize" \| "account\/read" \| "account\/rateLimits\/read"/);
  assert.match(protocol, /Some\("chatgpt"\) => Ok\(\(\)\)/);
  assert.match(protocol, /Some\(_\) => Err\("codex_gateway:non_chatgpt_provider"\)/);
  assert.match(protocol, /ordinaryUsageAllowed/);
  assert.match(gateway, /"supportsLunaReserve": false/);
  assert.match(gateway, /"experimentalApi": false/);
  assert.match(gateway, /realtime_audio_verified: false/);
  assert.match(gateway, /meeting_route_enabled: false/);
  assert.doesNotMatch(gateway, /"thread\/realtime\/start"/);
  assert.doesNotMatch(gateway, /"turn\/start"/);
  assert.match(protocol, /Notification \{ method: String, params: Value \}/);
});

test("Native commands are actually wired to the diagnostics client", () => {
  for (const method of [
    "get_codex_gateway_status", "connect_codex_gateway", "disconnect_codex_gateway"
  ]) {
    assert.match(registry, new RegExp("codex_gateway::" + method));
    assert.match(frontend, new RegExp('"' + method + '"'));
  }
  assert.match(diagnostics, /codexGatewayApi\[action\]/);
  assert.match(shutdown, /codex_gateway::shutdown_for_app_exit/);
  assert.match(gateway, /meeting_stop_cleanup_pending/);
  assert.match(gateway, /refresh_account\(generation, &mut server, &cancellation\)/);
  assert.match(gateway, /account_changed_during_probe/);
  assert.match(transport, /refuse_unadmitted_event/);
});

test("No experimental Codex readiness is substituted for Local Meeting readiness", () => {
  const lifecycle = read("commands/meeting_session/lifecycle.rs");
  const probe = read("commands/helper_bridge/functional_probe.rs");
  assert.doesNotMatch(lifecycle, /codex_gateway/);
  assert.doesNotMatch(probe, /codex_gateway/);
  assert.match(gateway, /realtime_audio_verified: false/);
  assert.match(gateway, /meeting_route_enabled: false/);
});


test("P2 maps only canonical role-bearing completed items to unverified candidates", () => {
  assert.match(realtime, /thread\/realtime\/item\/completed/);
  assert.match(realtime, /realtimeSessionId/);
  assert.match(realtime, /Some\("assistant"\) => Ok\(Observation::UnverifiedAssistantText/);
  assert.match(realtime, /Some\("user"\) => Ok\(Observation::SourceTranscript/);
  assert.match(realtime, /thread\/realtime\/transcript\/done/);
  assert.match(realtime, /duplicate_item/);
  assert.match(realtime, /foreign_thread/);
  assert.match(realtime, /CloudAudioIgnored/);
  assert.doesNotMatch(realtime, /voice_actor_synthesize|deliver_meeting_output|commit_meeting_turn/);
});

test("P2 has bounded PCM framing with gap and Stop rejection, but no live Voice RPC", () => {
  assert.match(realtime, /MAX_AUDIO_FRAMES: usize = 4/);
  assert.match(realtime, /SAMPLES_PER_FRAME: usize = 4_800/);
  assert.match(realtime, /audio_backpressure/);
  assert.match(realtime, /audio_clock_gap/);
  assert.match(realtime, /audio_clock_overflow/);
  assert.match(realtime, /fn base64_pcm/);
  assert.match(gateway, /mod realtime;/);
  assert.doesNotMatch(protocol, /"thread\/realtime\/appendAudio" \|/);
  assert.doesNotMatch(gateway, /"thread\/realtime\/start"/);
  assert.match(gateway, /realtime_audio_verified: false/);
  assert.match(gateway, /meeting_route_enabled: false/);
});


test("P3 source/assistant correlation is one-flight and always unverified", () => {
  assert.match(gateway, /mod correlation;/);
  assert.match(correlation, /struct SourceWindow/);
  assert.match(correlation, /struct UnverifiedPair/);
  assert.match(correlation, /struct CorrelationGate/);
  assert.match(correlation, /codex_correlation:assistant_without_source/);
  assert.match(correlation, /codex_correlation:source_ambiguous/);
  assert.match(correlation, /codex_correlation:overlapping_source/);
  assert.match(correlation, /codex_correlation:retired/);
  assert.match(correlation, /MAX_UTTERANCE_SAMPLES/);
  assert.match(gateway, /source_derived_realtime_items_produce_no_authorized_tts_result/);
  assert.doesNotMatch(correlation, /voice_actor_synthesize|enqueue_meeting_playback|commit_meeting_turn/);
  assert.match(gateway, /meeting_route_enabled: false/);
});
