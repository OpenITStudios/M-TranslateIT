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
  assert.match(incoming, /deferred_direction\s*\.unwrap_or_else\(\|\| \{\s*let settings = load_settings\(\);/);
});

test("outbound voice remains explicitly Indonesian to English", () => {
  assert.match(outbound, /send_helper_meeting_translation\([\s\S]*?transcript,\s*"id",\s*"en",/);
  assert.doesNotMatch(outbound, /"text": transcript/);
  assert.match(incoming, /send_helper_meeting_translation\([\s\S]*?transcript\.as_ref\(\),\s*&source_language,\s*&target_language,/);
});
