import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const root = new URL("../../src-tauri/src/", import.meta.url);
const read = (file: string) => readFileSync(new URL(file, root), "utf8");
const gateway = read("commands/codex_gateway.rs");
const transport = read("commands/codex_gateway/transport.rs");
const fakeServer = read("commands/codex_gateway/transport/fake_app_server_tests.rs");
const compatibility = read("commands/codex_gateway/compatibility.rs");
const protocol = read("commands/codex_gateway/protocol.rs");
const realtime = read("commands/codex_gateway/realtime.rs");
const realtimeRpc = read("commands/codex_gateway/realtime_rpc.rs");
const offlineText = read("commands/codex_gateway/text.rs");
const correlation = read("commands/codex_gateway/correlation.rs");
const helperBridge = read("commands/helper_bridge.rs");
const helperProbe = read("commands/helper_bridge/functional_probe.rs");
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
  assert.match(transport, /fn cleanup_proven\(/);
  assert.match(transport, /child_reaped && writer_released && reader_released && cleanup_failure\.is_none\(\)/);
  assert.match(transport, /Ok\(cleanup_proven\(/);
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
  assert.match(gateway, /refresh_account\(generation, &mut server, &cancellation, &diagnostic\)/);
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
  assert.match(correlation, /fn has_acknowledged_source_audio\(/);
  assert.match(realtimeRpc, /fn audio_rpc_ack_covers_source\(/);
  assert.match(realtimeRpc, /fn audio_ack_evidence_requires_contiguous_full_window_same_lease_and_live/);
  assert.match(correlation, /fn observe_event\(/);
  assert.match(correlation, /fn completed_realtime_items_form_only_one_unverified_pair/);
  assert.match(correlation, /fn invalid_realtime_item_revokes_the_pairing_boundary/);
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
  assert.match(realtimeRpc, /"clientManagedHandoffs": true/);
  assert.doesNotMatch(realtimeRpc, /"outputModality": "text"/);
  assert.match(realtime, /CloudAudioIgnored/);
  assert.match(realtime, /codex_realtime:session_failed/);
  assert.match(realtime, /codex_realtime:session_outcome_invalid/);
  assert.match(realtime, /codex_realtime:agent_item_promoted/);
  assert.match(realtime, /codex_realtime:transport_closed_unverified/);
  assert.match(realtime, /codex_realtime:close_reason_invalid/);
  assert.match(realtime, /fn coding_agent_item_promoted_revokes_realtime_observer/);
  assert.match(realtime, /fn unrequested_transport_closure_never_claims_clean_stop/);
  assert.match(realtimeRpc, /codex_wire:remote_close_not_requested/);
  assert.match(realtimeRpc, /fn remote_error_or_missing_reason_cannot_complete_clean_stop/);
  assert.match(correlation, /fn coding_agent_promotion_retires_provisional_pairing/);
  assert.match(realtime, /fn failed_or_missing_realtime_outcome_cannot_become_a_clean_close/);
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
  assert.match(realtime, /fn pending_state\(/);
  assert.match(realtime, /struct Pcm24kResampler/);
  assert.match(realtime, /fn push_mono_f32\(/);
  assert.match(realtime, /codex_realtime:input_clock_gap/);
  assert.match(realtime, /fn streaming_pcm24k_conversion_is_partition_independent/);
  assert.match(realtime, /fn native_48k_mic_clock_and_invalid_input_fail_closed/);
  assert.match(realtimeRpc, /fn send_queued_audio\(/);
  assert.match(realtimeRpc, /fn prepare_stop_with_queue\(/);
  assert.match(realtimeRpc, /fn queued_audio_never_pops_without_authority_and_stop_retires_queue/);
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
  assert.match(transport, /fn queue_realtime_frame\(/);
  assert.match(transport, /_admission: &super::realtime_rpc::VerifiedMediaAuthority/);
  assert.doesNotMatch(gateway, /queue_realtime_frame\(/);
  assert.match(gateway, /"experimentalApi": false/);
  assert.match(gateway, /meeting_route_enabled: false/);
});


test("P6 keeps the account-only RPC allowlist and single read/write ownership", () => {
  assert.match(protocol, /"initialize" \| "account\/read" \| "account\/rateLimits\/read"/);
  assert.match(transport, /fn queue_realtime_frame\(/);
  assert.match(transport, /_admission: &super::realtime_rpc::VerifiedMediaAuthority/);
  assert.doesNotMatch(gateway, /queue_realtime_frame\(/);
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


test("P8 fatal cleanup faults are terminal and cannot silently reconnect", () => {
  assert.match(gateway, /const CLEANUP_SETTLEMENT_LIMIT: Duration = Duration::from_secs\(3\)/);
  assert.match(gateway, /enum CleanupDisposition/);
  assert.match(gateway, /fn cleanup_disposition/);
  assert.match(gateway, /Ok\(false\) if !deadline_reached => CleanupDisposition::Retry/);
  assert.match(gateway, /Err\(reason\) => CleanupDisposition::Incomplete\(reason\)/);
  assert.match(gateway, /let cleanup_deadline = Instant::now\(\) \+ CLEANUP_SETTLEMENT_LIMIT/);
  assert.match(gateway, /CleanupDisposition::Incomplete\(reason\) =>/);
  assert.match(gateway, /CodexGatewayStatus::cleanup_incomplete\(/);
  assert.match(gateway, /state.status.state == "cleanup_incomplete" \|\| state.thread.is_some\(\)/);
  assert.match(gateway, /if state.status.state == "cleanup_incomplete" \{/);
  assert.match(gateway, /"codex_gateway:owner_thread_failed"/);
  assert.match(gateway, /"codex_gateway:cleanup_not_verified"/);
  assert.doesNotMatch(gateway, /Ok\(false\) \| Err\(_\) =>/);
  assert.match(diagnostics, /const cleanupIncomplete = \$derived/);
  assert.match(diagnostics, /role="alert"/);
  assert.match(diagnostics, /\{#if cleanupIncomplete\}/);
  assert.match(fakeServer, /fn latched_join_failure_is_terminal_and_does_not_retry_cleanup_forever/);
  assert.match(fakeServer, /fn unverified_owner_exit_does_not_rearm_codex_voice_or_child_start/);
  assert.match(transport, /self.cleanup_failure = Some\("codex_gateway:writer_join_failed"\)/);
  assert.match(transport, /self.cleanup_failure = Some\("codex_gateway:reader_join_failed"\)/);
  assert.match(protocol, /"initialize" \| "account\/read" \| "account\/rateLimits\/read"/);
  assert.match(gateway, /realtime_audio_verified: false/);
  assert.match(gateway, /meeting_route_enabled: false/);
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


test("P9 parses actual InitializeResponse user-agent version and platform without a second process", () => {
  assert.match(gateway, /mod compatibility;/);
  assert.match(gateway, /compatibility::verify_initialize\(&initialized\)/);
  assert.match(compatibility, /fn parse_build_version\(user_agent: &str\)/);
  assert.match(compatibility, /"userAgent"/);
  assert.match(compatibility, /"codexHome"/);
  assert.match(compatibility, /"platformFamily"/);
  assert.match(compatibility, /"platformOs"/);
  assert.match(compatibility, /codex_compatibility:cli_version_placeholder/);
  assert.match(compatibility, /codex_compatibility:server_platform_mismatch/);
  assert.doesNotMatch(compatibility, /Command::new|OPENAI_API_KEY|thread\/realtime\/start/);
  assert.match(gateway, /app_server_version: Option<String>/);
  assert.match(gateway, /diagnostic_protocol_compatible: bool/);
  assert.match(gateway, /app_server_version: Some\(diagnostic\.cli_version\.clone\(\)\)/);
  assert.match(gateway, /diagnostic_protocol_compatible: true/);
  assert.match(frontend, /app_server_version: string \| null/);
  assert.match(frontend, /diagnostic_protocol_compatible: boolean/);
  assert.match(diagnostics, /Compatible with diagnostic reads only/);
  assert.match(diagnostics, /Voice entitlement, realtime protocol support/);
});

test("P9 proves read-only explicit gateway policy before account or token quota probe", () => {
  assert.match(protocol, /method == "account\/gatewayOAuth\/read"/);
  assert.match(protocol, /json!\(\{"id": id, "method": method\}\)/);
  assert.match(protocol, /account\/gatewayOAuth\/read/);
  assert.doesNotMatch(protocol, /"account\/gatewayOAuth\/login" \|/);
  assert.doesNotMatch(protocol, /"account\/gatewayOAuth\/cancel" \|/);
  assert.match(gateway, /server\.request\("account\/gatewayOAuth\/read", json!\(\{\}\), cancellation\)\?/);
  assert.match(gateway, /compatibility::verify_gateway_oauth_read\(&gateway\)\?/);
  assert.match(compatibility, /Some\("openai"\)/);
  assert.match(compatibility, /Some\(false\)/);
  assert.match(compatibility, /gateway_provider_not_admitted/);
  assert.match(fakeServer, /p9_scripted_gateway_capability_precedes_account_and_usage_reads/);
  assert.match(fakeServer, /p9_unsupported_gateway_oauth_read_blocks_without_account_request/);
  assert.match(fakeServer, /send_rpc_method_not_found/);
  assert.match(gateway, /"experimentalApi": false/);
  assert.match(gateway, /realtime_audio_verified: false/);
  assert.match(gateway, /meeting_route_enabled: false/);
});


test("P10 rejects stale account and provider claims before notifications are dequeued", () => {
  assert.match(transport, /use std::sync::atomic::\{AtomicBool, AtomicU64, Ordering\}/);
  assert.match(transport, /fn affects_account_authority\(method: &str\)/);
  assert.match(transport, /method.starts_with\("modelProvider\/"\)/);
  assert.match(transport, /method.starts_with\("account\/"\)/);
  assert.match(transport, /fn read_jsonl\(/);
  assert.match(transport, /account_revision.fetch_update\(/);
  assert.match(transport, /fn account_revision\(&self\) -> u64/);
  assert.match(transport, /fn reader_revision_invalidates_account_and_provider_before_dispatch/);

  assert.match(gateway, /fn invalidate_stale_account\(owner: &mut GatewayOwner\)/);
  assert.match(gateway, /fn can_publish_account\(/);
  assert.match(gateway, /fn publish_verified_account\(/);
  assert.match(gateway, /state\.verified_account_revision = Some\(revision\)/);
  assert.match(gateway, /state: "account_revalidating"/);
  assert.match(gateway, /"codex_gateway:account_or_provider_changed"/);
  assert.match(gateway, /state\.status = status/);
  assert.match(gateway, /server\.account_revision\(\) != revision/);
  assert.match(gateway, /publish_verified_account\(generation, revision, status\)/);
  assert.match(gateway, /invalidate_stale_account\(&mut state\)/);
  assert.match(gateway, /fn auth_event_revision_immediately_masks_old_account_and_cli_claims/);
  assert.match(gateway, /fn cancelled_or_stale_revision_cannot_republish_account_authority/);

  assert.match(fakeServer, /fn p10_account_revision_advances_before_fake_controller_dequeues_event/);
  assert.match(fakeServer, /fn p10_provider_policy_event_forces_new_account_diagnostic_probe/);
  assert.match(fakeServer, /account_revision: Arc<AtomicU64>/);
  assert.match(fakeServer, /reader_revision/);
  assert.match(gateway, /"experimentalApi": false/);
  assert.match(gateway, /meeting_route_enabled: false/);
  assert.match(transport, /fn queue_realtime_frame\(/);
  assert.match(transport, /_admission: &super::realtime_rpc::VerifiedMediaAuthority/);
  assert.doesNotMatch(gateway, /queue_realtime_frame\(/);
  assert.doesNotMatch(protocol, /"thread\/realtime\/start" \|/);
});

test("Codex TTS-only hook does not route through Local ASR/MT", () => {
  assert.match(helperBridge, /fn prepare_codex_voice_only_runtime\(/);
  assert.match(helperBridge, /MeetingEngineAdmission::Codex/);
  const only = helperProbe.split("pub(super) fn run_codex_voice_only_probe(")[1]?.split("#[cfg(test)]")[0] ?? "";
  assert.ok(only.length > 0);
  assert.match(only, /"voice_actor_preflight"/);
  assert.match(only, /"voice_actor_synthesize"/);
  assert.match(only, /voice_only_receipt_matches/);
  assert.match(only, /voice_only_probe_has_wav_header/);
  assert.doesNotMatch(only, /"asr_preload"|"transcribe"|"translate"|"translation_preload"/);
  assert.doesNotMatch(only, /\.provider_ready\s*(?:==|&&|\|\|)/);
  assert.match(helperProbe, /fn codex_voice_only_receipt_rejects_foreign_actor_path_or_stage/);
  assert.match(helperProbe, /fn voice_only_probe_path_matches\(/);
  assert.match(helperProbe, /fs::canonicalize\(requested\)/);
  assert.match(helperProbe, /fs::canonicalize\(candidate\)/);
  assert.match(helperProbe, /fn codex_voice_only_windows_reported_separators_match_same_wav/);
  assert.doesNotMatch(helperProbe, /Path::new\(path\) == expected_path/);
  assert.match(realtime, /assert_eq!\(combined\.len\(\), 5\)/);
  assert.match(gateway, /meeting_route_enabled: false/);
});

test("P25 joins WebView SDP exchange to the sealed Rust AppServer source seam", () => {
  const peer = readFileSync(
    new URL("../../src/app/bridge/codexRealtimePeer.ts", import.meta.url), "utf8",
  );
  assert.match(peer, /async negotiate\(/);
  assert.match(peer, /this\.bounded\(/);
  assert.match(peer, /this\.cancelled\.signal/);
  assert.match(transport, /fn exchange_realtime_offer\(/);
  assert.match(transport, /fn observe_realtime_start_signal\(/);
  assert.match(transport, /self\.next_realtime_request_id\(/);
  assert.match(transport, /self\.queue_realtime_frame\(/);
  assert.match(transport, /self\.next_realtime_event\(/);
  assert.match(transport, /codex_gateway:voice_signalling_timeout/);
  assert.match(transport, /fn realtime_start_signalling_accepts_ack_started_sdp_in_any_order/);
  assert.match(transport, /fn realtime_start_signalling_rejects_wrong_id_and_unrelated_events/);
  assert.match(transport, /admission: &super::realtime_rpc::VerifiedMediaAuthority/);
  assert.match(gateway, /"experimentalApi": false/);
  assert.match(gateway, /meeting_route_enabled: false/);
  assert.doesNotMatch(gateway, /exchange_realtime_offer\(/);
  assert.doesNotMatch(peer, /runCommand\(|getUserMedia\s*\(|\.send\(/);
});

test("Codex realtime control uses the SAME owned JSONL writer/reader, with typed frame budgets", () => {
  assert.match(transport, /enum WriteScope/);
  assert.match(transport, /Self::Diagnostic => protocol::MAX_REQUEST_BYTES \+ 1/);
  assert.match(transport, /fn submit_with_receipt_scoped\(/);
  assert.match(transport, /fn queue_realtime_frame\(/);
  assert.match(transport, /fn next_realtime_event\(/);
  assert.match(transport, /fn owned_realtime_inbound_allowed\(/);
  assert.match(transport, /codex_gateway:voice_account_revision_changed/);
  assert.match(transport, /fn separate_writer_limits_preserve_diagnostics_and_realtime_pcm/);
  assert.match(transport, /fn owned_realtime_dispatch_rejects_unowned_response_and_unrelated_events/);
  assert.match(realtimeRpc, /fn validate_for_transport\(/);
  assert.match(realtimeRpc, /fn pcm16_base64_is_canonical\(/);
  assert.match(realtimeRpc, /fn pcm16_base64_rejects_noncanonical_bits_wrong_count_and_bad_alphabet/);
  assert.match(realtimeRpc, /fn typed_audio_frame_rechecks_pcm16_base64_after_encoding/);
  assert.match(realtimeRpc, /fn wire_transport_budget_accepts_pcm_frame_but_rejects_forged_rpc/);
  assert.match(realtimeRpc, /codex_wire:transport_frame_mismatch/);
  assert.doesNotMatch(protocol, /"thread\/realtime\/start" \|/);
  assert.match(gateway, /"experimentalApi": false/);
  assert.match(gateway, /meeting_route_enabled: false/);
});

test("Codex text candidate cannot send inference, execute tools or commit to Meeting", () => {
  assert.match(gateway, /mod text;/);
  assert.match(offlineText, /fn offline_thread_start\(/);
  assert.match(offlineText, /fn offline_turn_start\(/);
  assert.match(offlineText, /"ephemeral": true/);
  assert.match(offlineText, /"sandbox": "read-only"/);
  assert.match(offlineText, /"approvalPolicy": "never"/);
  assert.match(offlineText, /"outputSchema"/);
  assert.match(offlineText, /"additionalProperties": false/);
  assert.match(offlineText, /MAX_SOURCE_BYTES: usize = 4 \* 1024/);
  assert.match(offlineText, /super::protocol::MAX_REQUEST_BYTES - RPC_ID_AND_NEWLINE_RESERVE_BYTES/);
  assert.match(offlineText, /codex_text:request_too_large/);
  assert.match(offlineText, /method != "turn\/completed"/);
  assert.match(offlineText, /codex_text:not_completed_event/);
  assert.match(offlineText, /fn escaped_utterances_cannot_exceed_existing_rpc_frame_budget/);
  assert.match(offlineText, /codex_text:foreign_thread/);
  assert.match(offlineText, /expected_input: String/);
  assert.match(offlineText, /codex_text:source_mismatch/);
  assert.match(offlineText, /codex_text:duplicate_source/);
  assert.match(offlineText, /codex_text:assistant_before_source/);
  assert.match(offlineText, /fn wrong_utterance_or_direction_and_duplicate_source_are_rejected/);
  assert.match(offlineText, /codex_text:items_not_fully_loaded/);
  assert.match(offlineText, /codex_text:completion_summary_unverified/);
  assert.match(offlineText, /fn upstream_terminal_summary_cannot_prove_source_or_absence_of_tools/);
  assert.match(offlineText, /codex_text:tool_or_unknown_item/);
  assert.match(offlineText, /codex_text:ambiguous_final/);
  assert.match(offlineText, /fn rejects_wrong_direction_empty_oversized_or_unsafe_source/);
  assert.match(offlineText, /fn exact_one_shot_final_or_explicit_failure_never_admits_tools/);
  assert.doesNotMatch(offlineText, /Command::new|send_helper_worker_task|voice_actor_synthesize|commit_meeting_turn|deliver_meeting_output/);
  // Check outbound request method literals, not thread/started notifications.
  assert.doesNotMatch(transport, /text::|"thread\/start"|"turn\/start"/);
  assert.doesNotMatch(protocol, /"turn\/start" \|/);
  assert.match(gateway, /meeting_route_enabled: false/);
});

test("current Codex handoff telemetry is denied before candidate delivery, never promoted to TTS", () => {
  assert.match(realtime, /fn reject_unsafe_realtime_item\(/);
  assert.match(realtime, /Some\("handoff_request" \| "function_call"\)/);
  assert.match(realtime, /codex_realtime:agent_handoff_observed/);
  assert.match(realtime, /fn upstream_handoff_item_added_retires_unverified_translation_before_next_item/);
  assert.match(realtime, /fn raw_item_added_never_grants_final_translation_or_tts_authority/);
  assert.match(transport, /realtime::reject_unsafe_realtime_item\(method, params\)/);
  assert.match(transport, /fn owned_voice_reader_refuses_handoff_before_delivering_a_candidate/);
  assert.match(transport, /codex_gateway:voice_agent_turn_observed/);
  assert.match(transport, /codex_gateway:agent_activity_in_diagnostics/);
  assert.match(gateway, /realtime_audio_verified: false/);
  assert.match(gateway, /meeting_route_enabled: false/);
  assert.doesNotMatch(realtime, /voice_actor_synthesize|deliver_meeting_output|commit_meeting_turn/);
});

test("Codex handoff after a source cannot survive correlation or account-only transport", () => {
  assert.match(correlation, /fn codex_handoff_after_source_revokes_pair_and_late_reply_cannot_be_committed/);
  assert.match(correlation, /codex_realtime:agent_handoff_observed/);
  assert.match(correlation, /codex_correlation:retired/);
  assert.match(fakeServer, /fn agent_turn_during_account_probe_refuses_a_later_successful_account_reply/);
  assert.match(fakeServer, /fn agent_turn_during_idle_diagnostic_poll_is_not_ignored/);
  assert.match(fakeServer, /account\/read/);
  assert.match(fakeServer, /codex_gateway:agent_activity_in_diagnostics/);
  assert.match(gateway, /realtime_audio_verified: false/);
  assert.match(gateway, /meeting_route_enabled: false/);
});
