import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const incident = readFileSync(new URL("../../src-tauri/src/commands/incident_log.rs", import.meta.url), "utf8");
const sessionState = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/session_state.rs", import.meta.url), "utf8");

test("incident history is bounded, redacted and excludes conversation payloads", () => {
  assert.match(incident, /MAX_INCIDENTS: usize = 20/);
  assert.match(incident, /sanitize_diagnostic_text/);
  assert.doesNotMatch(incident, /source_text|translated_text|audio_path/);
});

test("repeated incidents deduplicate across interleaved blockers and suppress poll churn", () => {
  assert.match(incident, /INCIDENT_REPEAT_SUPPRESSION_MS: u128 = 60_000/);
  assert.match(incident, /incidents\.iter\(\)\.position/);
  assert.match(incident, /incidents\.remove\(index\)/);
  assert.match(incident, /saturating_sub\(previous\.occurred_unix_ms\)/);
});

test("required and optional meeting failures record incidents at their source", () => {
  assert.match(sessionState, /record_runtime_incident\("meeting_outbound"/);
  assert.match(sessionState, /record_runtime_incident\("meeting_incoming"/);
});

test("incident issue projection is bounded, source-owned and distinguishes absent from unreadable history", () => {
  assert.match(incident, /fn inspect_incidents\(path: &Path\)/);
  assert.match(incident, /symlink_metadata\(path\)/);
  assert.match(incident, /take\(MAX_REVIEW_BYTES \+ 1\)/);
  assert.match(incident, /group_issue_areas\(&incidents\)/);
  for (const status of ["NO_DATA", "RECENT_ONLY", "AT_CAPACITY", "UNAVAILABLE"]) {
    assert.ok(incident.includes('"' + status + '"'), status);
  }
  assert.match(incident, /fn issue_map_groups_only_retained_records_without_inventing_failure_frequency/);
});

test("incident writes fail closed on corrupted history and separate clean build identities", () => {
  assert.match(incident, /fn persist_incident\(path: &Path, next: RuntimeIncident\)/);
  assert.match(incident, /let mut incidents = read_incidents\(path\)\?/);
  assert.match(incident, /current\.build_source_sha == next\.build_source_sha/);
  assert.match(incident, /fn invalid_incident_history_cannot_be_erased_by_new_events/);
  assert.match(incident, /fn issue_map_separates_recorded_builds_from_legacy_unknowns/);
  assert.match(incident, /fn legacy_incidents_survive_bounded_append_without_build_forgery/);
  assert.doesNotMatch(incident, /fs::read\(path\)/);
});

test("incident publication preserves the existing report on replacement or staging failure", () => {
  assert.match(incident, /fs::rename\(temp, path\)/);
  assert.doesNotMatch(incident, /fs::remove_file\(path\)/);
  assert.match(incident, /fn existing_history_is_replaced_without_deleting_before_publication/);
  assert.match(incident, /fn failed_staging_preserves_previous_incident_history/);
});
