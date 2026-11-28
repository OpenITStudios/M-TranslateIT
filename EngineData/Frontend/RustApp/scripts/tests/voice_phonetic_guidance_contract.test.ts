import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const rust = (path: string) => readFileSync(resolve(root, "src-tauri/src/commands/" + path), "utf8");

test("retired phonetic guidance does not run or appear in the native product", () => {
  for (const file of ["guided_phonetic_guidance.rs", "guided_phonetic_inventory.rs"]) {
    assert.equal(existsSync(resolve(root, "src-tauri/src/commands/voice_lab_recording", file)), false);
  }
  assert.equal(existsSync(resolve(root, "../../../tools/generate_my_voice_phonetics.py")), false);
  for (const source of [
    rust("voice_lab_recording/guided_lines.rs"),
    rust("voice_lab_recording.rs"),
    rust("voice_lab_build.rs"),
    readFileSync(resolve(root, "src/app/bridge/myVoiceBuildApi.ts"), "utf8"),
    readFileSync(resolve(root, "src/pages/MyVoice.svelte"), "utf8"),
    readFileSync(resolve(root, "src/components/my-voice/MyVoiceBuild.svelte"), "utf8"),
    readFileSync(resolve(root, "scripts/ui-preview/fixtures.ts"), "utf8"),
  ]) {
    assert.doesNotMatch(source, /recording_suggestions|RecordingSuggestion|guided_recording_recommendations|phonetic_inventory|phonetic_guidance|browseFocus/);
  }
});

test("native eligibility retains reference count, duration and actual voice evaluation", () => {
  const build = rust("voice_lab_build.rs");
  const evaluation = rust("voice_lab_build/native_evaluation.rs");
  assert.match(build, /native_reference_count >= 2/);
  assert.match(build, /minimum_duration_ms: 2 \* MIN_REFERENCE_MS/);
  assert.match(build, /takes\.len\(\) <= MAX_TRAINING_TAKES/);
  assert.match(evaluation, /select_references\(accepted, 3,/);
  assert.match(evaluation, /references\.len\(\) < 2/);
  assert.match(rust("voice_lab_build/evaluation.rs"), /1008,/);
  assert.match(rust("voice_lab_build/native_review.rs"), /manifest\.valid\(\)/);
});
