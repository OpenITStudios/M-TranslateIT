import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const root = new URL("../../src-tauri/src/", import.meta.url);
const read = (file: string) => readFileSync(new URL(file, root), "utf8");
const gateway = read("commands/codex_gateway.rs");
const transport = read("commands/codex_gateway/transport.rs");
const fakeServer = read("commands/codex_gateway/transport/fake_app_server_tests.rs");
const protocol = read("commands/codex_gateway/protocol.rs");
const realtime = read("commands/codex_gateway/realtime.rs");
const realtimeRpc = read("commands/codex_gateway/realtime_rpc.rs");
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
  assert.match(transport, /WRITE_QUEUE_CAPACITY: usize = 1/);
  assert.match(transport, /sync_channel\(WRITE_QUEUE_CAPACITY\)/);
  assert.match(transport, /fn write_jsonl\(/);
  assert.match(transport, /fn write_frame\(/);
  assert.match(transport, /writer\.try_send\(WriteCommand/);
  assert.match(transport, /receipt\.recv_timeout\(remaining\.min\(WAIT_SLICE\)\)/);
  assert.match(transport, /codex_gateway:write_timeout/);
  assert.match(transport, /codex_gateway:writer_queue_full/);
  assert.match(transport, /self\.writer_tx\.take\(\)/);
  assert.match(transport, /self\.writer\.is_none\(\) && self\.reader\.is_none\(\)/);
  assert.match(transport, /fn notification_dispatch_retains_only_account_changes_and_denies_voice/);
  assert.match(transport, /fn writer_ack_preserves_exact_jsonl_without_using_the_control_thread_for_io/);
  assert.match(transport, /fn writer_failure_stays_redacted_and_poisoned/);
  assert.doesNotMatch(transport, /self\.stdin\.write_all\(/);
  assert.match(transport, /writer_failed\.load\(Ordering::Acquire\)/);
  assert.match(transport, /cleanup_failure/);
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


test("P5 stages typed V3 WebRTC start/audio/stop envelopes without live RPC admission", () => {
  assert.match(gateway, /mod realtime_rpc;/);
  assert.match(gateway, /offline_realtime_rpc_is_not_admitted_by_account_probe_transport/);
  assert.match(realtimeRpc, /"thread\/realtime\/start"/);
  assert.match(realtimeRpc, /"thread\/realtime\/appendAudio"/);
  assert.match(realtimeRpc, /"thread\/realtime\/stop"/);
  assert.match(realtimeRpc, /"outputModality": "audio"/);
  assert.match(realtimeRpc, /"version": "v3"/);
  assert.match(realtimeRpc, /"type": "webrtc"/);
  assert.match(realtimeRpc, /"includeStartupContext": false/);
  assert.match(realtimeRpc, /START_MAX_BYTES: usize = 32 \* 1024/);
  assert.match(realtimeRpc, /AUDIO_MAX_BYTES: usize = 24 \* 1024/);
  assert.match(realtimeRpc, /STOP_MAX_BYTES: usize = 1024/);
  assert.match(realtimeRpc, /MAX_INFLIGHT_AUDIO: usize = 4/);
  assert.match(realtimeRpc, /AUDIO_ACK_TIMEOUT: Duration = Duration::from_secs\(5\)/);
  assert.match(realtimeRpc, /fn enforce_ack_deadline/);
  assert.match(realtimeRpc, /codex_wire:audio_ack_timeout/);
  assert.match(realtimeRpc, /codex_wire:audio_not_live/);
  assert.match(realtimeRpc, /fn outbound_audio_drops_unexpected_json_keys_from_provider_payload/);
  assert.match(realtimeRpc, /fn duplicate_start_and_stop_acks_are_rejected/);
  assert.match(realtimeRpc, /struct VerifiedMediaAuthority/);
  assert.match(realtimeRpc, /fn confirm_media_ready/);
  assert.match(realtimeRpc, /_admission: &VerifiedMediaAuthority/);
  assert.match(realtimeRpc, /fn acked_through_sample/);
  assert.match(realtimeRpc, /codex_wire:stale_audio_generation/);
  assert.match(realtimeRpc, /codex_wire:audio_backpressure/);
  assert.match(realtimeRpc, /codex_wire:audio_clock_gap/);
  assert.match(realtimeRpc, /codex_wire:audio_ack_unknown/);
  assert.match(realtimeRpc, /codex_wire:stop_ack_invalid/);
  assert.match(realtimeRpc, /fn close_verified/);
  assert.match(realtimeRpc, /impl std::fmt::Debug for RpcFrame/);
  assert.doesNotMatch(realtimeRpc, /voice_actor_synthesize|enqueue_meeting_playback|commit_meeting_turn/);
  assert.doesNotMatch(protocol, /"thread\/realtime\/start" \|/);
  assert.doesNotMatch(protocol, /"thread\/realtime\/appendAudio" \|/);
  assert.doesNotMatch(transport, /realtime_rpc::/);
  assert.match(gateway, /"experimentalApi": false/);
  assert.match(gateway, /meeting_route_enabled: false/);
});


test("P6 keeps the account-only RPC allowlist and single read/write ownership", () => {
  assert.match(protocol, /"initialize" \| "account\/read" \| "account\/rateLimits\/read"/);
  assert.doesNotMatch(transport, /realtime_rpc::/);
  assert.match(transport, /fn account_notification\(method: &str\)/);
  assert.match(transport, /account_notification\(&method\)\?/);
  assert.match(transport, /codex_gateway:unexpected_response_id/);
  assert.match(transport, /codex_gateway:unsolicited_response/);
  assert.match(transport, /fn stop_and_reap/);
  assert.match(transport, /writer\.join\(\)/);
  assert.match(transport, /reader\.join\(\)/);
  assert.match(gateway, /realtime_audio_verified: false/);
  assert.match(gateway, /meeting_route_enabled: false/);
});


test("P7 tests use the real owned JSONL reader/writer and response dispatcher", () => {
  assert.match(transport, /mod fake_app_server_tests;/);
  assert.match(transport, /fn submit_with_receipt/);
  assert.match(transport, /fn recv_expected_response/);
  assert.match(transport, /fn recv_idle_notification/);
  assert.match(transport, /fn cleanup_proven/);
  assert.match(transport, /submit_with_receipt\(&writer, body, deadline/);
  assert.match(transport, /recv_expected_response\(/);
  assert.match(fakeServer, /struct FakeCodexAppServer/);
  assert.match(fakeServer, /impl Write for ScriptedStdin/);
  assert.match(fakeServer, /impl Read for ScriptedStdout/);
  assert.match(fakeServer, /write_jsonl\(/);
  assert.match(fakeServer, /read_jsonl\(/);
  assert.match(fakeServer, /submit_with_receipt\(/);
  assert.match(fakeServer, /recv_expected_response\(/);
  assert.match(fakeServer, /recv_idle_notification\(/);
  assert.match(fakeServer, /cleanup_proven\(/);
});

test("P7 scenarios cover account changes, stalls, cancellation and exact IDs", () => {
  for (const scenario of [
    "scripted_account_notification_before_response_invalidates_same_rpc_probe",
    "scripted_jsonl_fragments_keep_exact_request_identity",
    "scripted_wrong_reply_id_is_not_reused_by_another_request",
    "scripted_cancellation_rejects_a_late_success_and_does_not_replay",
    "scripted_reply_after_deadline_cannot_complete_the_next_rpc",
    "scripted_idle_account_change_and_unsolicited_reply_are_distinct",
    "unsolicited_realtime_notification_is_rejected_by_the_real_idle_dispatch",
    "simulated_stalled_writer_times_out_and_stop_remains_unverified_until_join",
    "cancellation_ends_a_blocked_writer_wait_before_pipe_release",
    "cleanup_uncertainty_never_promotes_a_clean_stop_marker",
    "fake_transport_never_admits_voice_or_paid_inference",
  ]) {
    assert.ok(fakeServer.includes("fn " + scenario + "()"), scenario + " regression missing");
  }
  assert.match(fakeServer, /thread\/realtime\/start/);
  assert.match(fakeServer, /codex_gateway:request_not_allowed/);
  assert.doesNotMatch(fakeServer, /Command::new\(|codex app-server|OPENAI_API_KEY/);
  assert.match(protocol, /"initialize" \| "account\/read" \| "account\/rateLimits\/read"/);
  assert.match(gateway, /"experimentalApi": false/);
  assert.match(gateway, /meeting_route_enabled: false/);
});
