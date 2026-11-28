import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const rust = (p: string) => readFileSync(resolve(root, "src-tauri/src/commands/" + p), "utf8");
const ui = (p: string) => readFileSync(resolve(root, "src/" + p), "utf8");

test("archive improvement is a separate user-authorized intent on the native build owner", () => {
  const build = rust("voice_lab_build.rs");
  const source = rust("voice_lab/storage/archive_dataset.rs");
  const worker = rust("voice_lab/storage/dataset_history.rs");
  const recording = rust("voice_lab_recording.rs");
  const registry = rust("registry.rs");
  const wrapper = rust("application_runtime/mutations.rs");
  assert.match(registry, /start_product_voice_improvement/);
  assert.match(wrapper, /emit_after\(&app, "start_voice_improvement"/);
  assert.match(build, /pub fn start_voice_lab_improvement/);
  assert.match(build, /start_voice_lab_build_selected\(authorized_voice_confirmed, Some\(archive_ids\)\)/);
  const native = rust("voice_lab_build/native_build.rs");
  assert.match(build, /native_build::start\(current, selected_archives\)/);
  assert.match(native, /choose_archived_takes\(/);
  assert.match(native, /prepare_guided_dataset_from_archives\(/);
  assert.match(native, /native_evaluation::evaluate/);
  assert.doesNotMatch(build, /MIN_TRAINING_SPEECH_MS|missing_training_coverage_group/);
  assert.match(source, /const MAX_SELECTED_TAKES: usize = 128/);
  assert.match(source, /improvement_one_recording_per_line/);
  assert.match(source, /improvement_transcript_mismatch/);
  assert.match(recording, /fn guided_training_prompts/);
  assert.match(worker, /preserve_frozen_dataset_with_archives/);
});

test("existing held-out approval follows exact frozen immutable audio for archive rebuilds", () => {
  const evaluation = rust("voice_lab_build/native_review.rs");
  const snapshot = rust("voice_lab/storage/dataset_history.rs");
  const archive = rust("voice_lab/storage/recording_archive.rs");
  assert.match(evaluation, /archived_frozen_review_matches\(paths\)/);
  assert.match(evaluation, /frozen_dataset_matches_current/);
  assert.match(snapshot, /from_saved_archive: bool/);
  assert.match(snapshot, /snapshot_selected_take_changed/);
  assert.match(snapshot, /staged != verified/);
  assert.match(snapshot, /snapshot\.from_saved_archive\.then_some\(true\)/);
  assert.doesNotMatch(archive.split("#[cfg(test)]")[0], /pub native_audio: Option<Vec<u8>>/);
  assert.match(snapshot, /verify_saved_take_binding/);
  assert.match(archive, /archived_take_for_training/);
  assert.match(snapshot, /archived_frozen_review_matches/);
  assert.match(evaluation, /verify_source\(paths, &manifest\)\?/);
});

test("UI selects one saved version of each sentence, and uncertain builds never claim success", () => {
  const page = ui("pages/MyVoice.svelte");
  const api = ui("app/bridge/myVoiceBuildApi.ts");
  assert.match(page, /improvementTakes\[take.line_id\] === take.archive_id/);
  assert.match(page, /Create improved version/);
  assert.match(page, /I own these recordings or have permission/);
  assert.match(page, /improvementIds.length > 128/);
  assert.match(page, /buildSnapshot\?\.evaluation_ready/);
  assert.match(page, /buildStatusUnavailable = true/);
  assert.match(api, /start_product_voice_improvement/);
  assert.match(api, /Couldn't verify whether My Voice improvement started/);
});


test("review verifies each immutable archive once with exact prompt and canonical bytes", () => {
  const snapshot = rust("voice_lab/storage/dataset_history.rs");
  const review = snapshot.slice(snapshot.indexOf("pub(crate) fn archived_frozen_review_matches("), snapshot.indexOf("#[cfg(test)]"));
  assert.match(review, /read_frozen_snapshot_id\(storage\)/);
  assert.match(review, /load_snapshot\(storage, &id\)/);
  assert.doesNotMatch(review, /verify_snapshot\(|= frozen_dataset_snapshot_id\(/);
  assert.match(review, /archived_take_for_training\(storage, &link\.archive_id\)/);
  assert.match(review, /source\.line_id != take\.line_id \|\| source\.exact_text != take\.exact_text/);
  assert.match(review, /staged != verified/);
  const legacy = snapshot.slice(snapshot.indexOf("pub(super) fn frozen_dataset_snapshot_id("), snapshot.indexOf("struct TrainingLineage"));
  assert.match(legacy, /verify_snapshot\(storage, &id\)/);
});
