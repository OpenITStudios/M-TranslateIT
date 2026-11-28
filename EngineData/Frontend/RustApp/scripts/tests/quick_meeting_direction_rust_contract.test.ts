import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const incoming = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_pipeline.rs", import.meta.url), "utf8");
const deferred = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_deferred.rs", import.meta.url), "utf8");
const outbound = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/outbound_pipeline.rs", import.meta.url), "utf8");

test("incoming direction is snapshotted per utterance and survives deferral", () => {
  assert.match(incoming, /meeting_listen_source_language/);
  assert.match(incoming, /meeting_listen_target_language/);
  assert.match(deferred, /source_language: String/);
  assert.match(deferred, /target_language: String/);
  assert.match(incoming, /match deferred_direction \{\s*Some\(direction\) => \{\s*if !super::local_translated_meeting_route_admitted\(\)/);
  assert.match(incoming, /None => \{\s*let \(settings, admission\) = load_settings_with_admission\(\);\s*if !super::local_translated_route_for_admission\(admission\)/);
  assert.equal((incoming.match(/load_settings_with_admission\(\)/g) ?? []).length, 1);
  assert.doesNotMatch(incoming, /\bload_settings\(\)/);
});

test("outbound voice remains explicitly Indonesian to English", () => {
  assert.match(outbound, /send_helper_meeting_translation\([\s\S]*?transcript,\s*"id",\s*"en",/);
  assert.doesNotMatch(outbound, /"text": transcript/);
  assert.match(incoming, /send_helper_meeting_translation\([\s\S]*?transcript\.as_ref\(\),\s*&source_language,\s*&target_language,/);
});
