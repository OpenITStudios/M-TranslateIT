import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const root = new URL("../../src-tauri/src/", import.meta.url);
const read = (file: string) => readFileSync(new URL(file, root), "utf8");
const gateway = read("commands/codex_gateway.rs");
const transport = read("commands/codex_gateway/transport.rs");
const protocol = read("commands/codex_gateway/protocol.rs");
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
});

test("No experimental Codex readiness is substituted for Local Meeting readiness", () => {
  const lifecycle = read("commands/meeting_session/lifecycle.rs");
  const probe = read("commands/helper_bridge/functional_probe.rs");
  assert.doesNotMatch(lifecycle, /codex_gateway/);
  assert.doesNotMatch(probe, /codex_gateway/);
  assert.match(gateway, /realtime_audio_verified: false/);
  assert.match(gateway, /meeting_route_enabled: false/);
});
