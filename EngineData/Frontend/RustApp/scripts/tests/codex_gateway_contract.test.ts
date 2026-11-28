import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import test from "node:test";

const rust = new URL("../../src-tauri/src/commands/", import.meta.url);
const frontend = new URL("../../src/", import.meta.url);
const fixtures = new URL("./", import.meta.url);
const read = (path: string, base: URL) => readFileSync(new URL(path, base), "utf8");

test("retired Codex Voice Gateway has no source, application command or process owner", () => {
  for (const path of ["codex_gateway.rs", "codex_gateway/"]) {
    assert.equal(existsSync(new URL(path, rust)), false, path);
  }
  for (const path of ["app/bridge/codexGatewayApi.ts", "components/settings/CodexGatewayDiagnostics.svelte"]) {
    assert.equal(existsSync(new URL(path, frontend)), false, path);
  }
  assert.equal(existsSync(new URL("fixtures/codexRealtimePeer.ts", fixtures)), false);
  for (const path of ["mod.rs", "registry.rs", "application_runtime/shutdown.rs", "helper_bridge.rs"]) {
    assert.doesNotMatch(read(path, rust), /codex_gateway|prepare_codex_voice_only_runtime/);
  }
  assert.doesNotMatch(read("components/settings/SettingsDiagnostics.svelte", frontend), /CodexGatewayDiagnostics|Codex Gateway details/);
  assert.match(read("application_runtime/shutdown.rs", rust), /helper_bridge::shutdown_helper_bridge_for_app_exit/);
});

test("retired engine is not a Local AI fallback and Dictate Text remains separate", () => {
  assert.match(read("meeting_session.rs", rust), /admission == crate::engine::settings::MeetingEngineAdmission::Local/);
  assert.match(read("application_runtime/intents.rs", rust), /require_local_outbound_setup/);
  const requirements = readFileSync(new URL("../../../../../docs/foundation/02-product-requirements.md", import.meta.url), "utf8");
  assert.match(requirements, /PR-072B — Dictate Text into Meeting/);
  assert.match(requirements, /A local MCP tool may then hand off that transcript as text/);
});
