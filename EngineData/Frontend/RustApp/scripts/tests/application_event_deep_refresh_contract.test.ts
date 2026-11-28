import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const facade = readFileSync(new URL("../../src/app/bridge/runtimeProductFacade.ts", import.meta.url), "utf8");
const api = readFileSync(new URL("../../src/app/bridge/applicationRuntimeApi.ts", import.meta.url), "utf8");
const snapshot = readFileSync(new URL("../../src-tauri/src/commands/application_runtime/snapshot.rs", import.meta.url), "utf8");
const summaries = readFileSync(new URL("../../src-tauri/src/commands/application_runtime/summaries.rs", import.meta.url), "utf8");
const contract = readFileSync(new URL("../../src-tauri/src/commands/application_runtime/contract.rs", import.meta.url), "utf8");
const build = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build.rs", import.meta.url), "utf8");

test("ordinary runtime events retain generation-bound worker readiness reuse", () => {
  for (const reason of ["stop_meeting", "start_mic_test", "stop_mic_test", "select_audio_device",
    "save_settings", "start_voice_recording", "stop_voice_recording"]) {
    assert.match(facade, new RegExp(`"${reason}"`));
  }
  assert.match(facade, /WORKER_REUSE_SAFE_REASONS/);
  assert.match(facade, /!WORKER_REUSE_SAFE_REASONS\.has\(reason \?\? ""\)/);
  assert.match(facade, /observedWorkerStatus\.generation_token === helper\.generation_token/);
  assert.match(facade, /: previous\.workerStatus;/);
  assert.doesNotMatch(facade, /"apply_meeting_preset"/);
});

test("application voice readiness observes the existing selected actor predicate without requesting build details", () => {
  assert.match(snapshot, /Some\(voice_lab::approved_voice_actor_ready\(&storage\)\)/);
  assert.equal((snapshot.match(/approved_voice_actor_ready\(/g) ?? []).length, 1);
  assert.match(build, /approved_voice_actor_ready\(&paths\)/);
  assert.doesNotMatch(snapshot, /get_voice_lab_build_status|accepted_contract\(|native_review::status/);
  assert.match(summaries, /approved_voice_ready: Option<bool>/);
  assert.match(contract, /pub approved_voice_ready: Option<bool>/);
  assert.match(api, /approved_voice_ready: boolean \| null/);
  assert.match(api, /approved_voice_ready: null/);
  assert.match(facade, /application\.summaries\.voice\.approved_voice_ready/);
  assert.match(facade, /typeof observedVoiceReady === "boolean" \? observedVoiceReady : null/);
  assert.doesNotMatch(facade, /myVoiceBuildApi|loadApprovedVoiceReady|VOICE_REUSE_SAFE_REASONS|previous\.readiness\.approvedVoiceReady/);
});

test("unverified root and unavailable build state cannot claim voice eligibility", () => {
  assert.match(snapshot, /voice_build\.phase == "state_unavailable" \{\s*None/);
  assert.match(snapshot, /paths\.packaged_context_initialized \|\| paths\.development_root_verified/);
  assert.match(snapshot, /Some\(voice_lab::approved_voice_actor_ready\(&storage\)\)\s*\} else \{\s*None/);
});
