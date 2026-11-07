import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const health = readFileSync(new URL("../../src-tauri/src/commands/long_session_health.rs", import.meta.url), "utf8");
const deferred = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_deferred.rs", import.meta.url), "utf8");

test("long-session health observes bounded pressure without automatic cleanup", () => {
  assert.match(health, /outbound_overflow_dropped/);
  assert.match(health, /deferred_incoming_depth/);
  assert.match(health, /transcript_dropped_turns/);
  assert.match(health, /meeting_temp_file_count/);
  assert.doesNotMatch(health, /remove_file|clear_deferred_incoming_queue|stop_meeting_translation/);
});

test("deferred incoming exposes counts without exposing queued transcript content", () => {
  assert.match(deferred, /deferred_incoming_health_counts/);
  assert.match(deferred, /DEFERRED_DROPPED_OVERFLOW/);
  assert.match(deferred, /DEFERRED_DROPPED_STALE/);
});


test("long-session health uses only the bounded incident snapshot and does not expose speech", () => {
  assert.match(health, /committed_turn_health_counts/);
  assert.match(health, /let incident_history = get_recent_runtime_incidents\(\)/);
  assert.match(health, /observed_incident_count\(incident_history\.coverage, incident_history\.count\)/);
  assert.match(health, /pub incident_count: Option<usize>/);
  assert.match(health, /pub incident_history_coverage: String/);
  assert.doesNotMatch(health, /get_meeting_committed_turns|source_text|translated_text/);
});

test("unavailable incident history stays unknown, not an inferred clean runtime", () => {
  const incident = readFileSync(new URL("../../src-tauri/src/commands/incident_log.rs", import.meta.url), "utf8");
  assert.match(health, /observed_incident_count\("UNAVAILABLE", 0\), None/);
  assert.match(health, /observed_incident_count\("NO_DATA", 0\), Some\(0\)/);
  assert.match(incident, /Err\(_\) => \(Vec::new\(\), "UNAVAILABLE"\)/);
  assert.match(incident, /fn inspect_incidents\(path: &Path\)/);
});

test("meeting start resets pressure counters for the new session", () => {
  const lifecycle = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/lifecycle.rs", import.meta.url), "utf8");
  assert.match(lifecycle, /reset_finalized_drop_counters\(\)/);
  assert.match(lifecycle, /reset_deferred_incoming_drop_counters\(\)/);
});
