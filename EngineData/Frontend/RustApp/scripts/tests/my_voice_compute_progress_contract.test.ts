import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const app = new URL("../../", import.meta.url);
const read = (name: string) => readFileSync(new URL(name, app), "utf8");
const owner = read("src-tauri/src/commands/voice_lab_build.rs");
const native = read("src-tauri/src/commands/voice_lab_build/native_build.rs");
const runtime = read("src-tauri/src/commands/my_voice_runtime.rs");
const lifecycle = read("src-tauri/src/commands/voice_lab.rs");
const receipt = read("src-tauri/src/commands/voice_lab_build/receipts.rs");
const review = read("src-tauri/src/commands/voice_lab_build/native_review.rs");
const evaluation = read("src-tauri/src/commands/voice_lab_build/native_evaluation.rs");
const ui = read("src/components/my-voice/MyVoiceBuild.svelte");

test("native GPU admission is authoritative and legacy training launch is absent", () => {
  assert.doesNotMatch(owner, /Command::new|resolve_worker_python_command|my_voice_compute_probe\.py|voice_lab_build\.py|OwnedBuildJob/);
  assert.match(native, /my_voice_resources::preflight/);
  assert.match(runtime, /NativeInferenceOwner::open_voice_candidate/);
  assert.match(ui, /native_pins_ready/);
  assert.match(owner, /native_pins_ready && native_reference_count >= 2/);
});

test("native completion and cancellation share generation authority", () => {
  assert.match(native, /create_identity\(paths, generation\)/);
  assert.match(native, /mark_native_voice_evaluating\(generation\)/);
  assert.match(native, /commit_voice_lab_review_if_active\(generation/);
  assert.match(native, /commit_completion\(paths, &identity, &review_id\)/);
  assert.match(review, /completion_matches\(paths, review_id\)/);
  assert.match(receipt, /create_new\(true\)/);
  assert.match(owner, /active: snapshot\.active \|\| process_cleanup_pending/);
  assert.match(owner, /request_voice_lab_build_cancel\(generation\)/);
  assert.match(lifecycle, /active\.phase != BuildPhase::Evaluating \|\| active\.cancel_requested/);
  assert.match(runtime, /wait_output_with_cancel\(\|\| !authority\(generation, deadline\)\)/);
  assert.match(runtime, /release_idle_native_inference\(\)/);
});

test("native review binds actual audio, source and model identity before approval", () => {
  assert.match(evaluation, /encode_pcm16_wav/);
  assert.match(evaluation, /decode_native_review_pcm/);
  assert.match(review, /wav_sha256: hash\(&s\.wav\)/);
  assert.match(review, /verify_source\(paths, &manifest\)\?/);
  assert.match(review, /verify_files\(root, &manifest\)/);
  assert.match(review, /manifest\.models != ModelIdentity::current\(\)\?/);
  assert.match(review, /!quality_confirmed \|\| reviewed\.len\(\) != HELD_OUT_LINES\.len\(\)/);
  assert.match(owner, /preserve_native_version/);
  assert.match(ui, /myVoiceBuildApi\.approve\(reviewedLineIds, qualityConfirmed, build\.evaluation_review_id\)/);
});

test("voice preparation rechecks cancellation after idle cleanup and before cold model open", () => {
  const setup = runtime.slice(runtime.indexOf("pub(super) fn with_runtime"), runtime.indexOf("host.native_inference = Some(native)"));
  const release = setup.indexOf("!host.release_idle_native_inference()");
  const authorityAfter = setup.indexOf("if !authority(generation, deadline)", release);
  const open = setup.indexOf("NativeInferenceOwner::open_voice_candidate(None)");
  assert.ok(release >= 0 && release < authorityAfter && authorityAfter < open);
  assert.match(setup.slice(authorityAfter, open), /return Err\("preview_cancelled_or_timed_out"\.into\(\)\)/);
});
