import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const stage = readFileSync(
  new URL("../stage_release_inputs_impl.ps1", import.meta.url), "utf8",
);
const validator = readFileSync(
  new URL("../validate_release_payload.mjs", import.meta.url), "utf8",
);
const sources = JSON.parse(readFileSync(
  new URL("../../../../Backend/RuntimeAssets/Voice/BuiltInVoices/SOURCES.json", import.meta.url),
  "utf8",
));

const builder = readFileSync(new URL("../build_r3_external_payload.py", import.meta.url), "utf8");
const stagingOwner = readFileSync(new URL("../ReleaseStaging.psm1", import.meta.url), "utf8");
test("staging verifies candidate built-in bytes against tracked sources and never promotes the tracked directory", () => {
  assert.match(stage, /Built-in voice source or attribution mismatch/);
  assert.match(stage, /Get-FileHash -LiteralPath \$source -Algorithm SHA256/);
  assert.match(stage, /--verify-inputs --asset-root \$PayloadRoot/);
  assert.match(stagingOwner, /Where-Object \{ \$_ -cne \$tracked \}/);
  assert.doesNotMatch(stage, /uv sync|Copy-Tree|Invoke-WebRequest/);
});

test("native payload rejects unapproved voice identities and verifies reference hashes at its single owner", () => {
  assert.match(builder, /built-in voice source inventory mismatch/);
  assert.match(builder, /"MaleVoice", "FemaleVoice"/);
  assert.match(builder, /record\(builtins \+ voice\["voice_id"\] \+ "\/reference.wav", voice\["wav_sha256"\]\)/);
  assert.match(validator, /--verify-inputs/);
  assert.doesNotMatch(validator, /createHash\(/);
});

test("canonical built-in voice source inventory has two approved IDs and digest shapes", () => {
  assert.equal(sources.schema, "translateit.builtin_voice_sources.v1");
  assert.deepEqual(sources.voices.map((voice: { voice_id: string }) => voice.voice_id).sort(), ["FemaleVoice", "MaleVoice"]);
  for (const voice of sources.voices) {
    assert.match(voice.wav_sha256, /^[a-f0-9]{64}$/);
    assert.equal(voice.license, "CC-BY-4.0");
  }
});


test("release payload no longer requires the retired Quick Preview Python child", () => {
  assert.doesNotMatch(validator, /"voice_lab_quick_preview\.py"/);
});


test("native built-in provisioning bypasses legacy weights and release stays fail-closed", () => {
  const provision = readFileSync(new URL("../../src-tauri/src/commands/builtin_voice.rs", import.meta.url), "utf8");
  const store = readFileSync(new URL("../../src-tauri/src/commands/voice_lab/storage.rs", import.meta.url), "utf8");
  const release = readFileSync(new URL("../build_release.ps1", import.meta.url), "utf8");
  const wrapper = readFileSync(new URL("../stage_release_inputs.ps1", import.meta.url), "utf8");
  assert.match(provision, /native_voice_candidate_enabled\(\)[\s\S]*?return super::voice_lab::install_native_builtin/);
  const install = store.slice(store.indexOf("pub(crate) fn install_native_builtin"), store.indexOf("pub(crate) fn selected_native_identity"));
  assert.match(install, /native_builtin_preview/);
  assert.match(install, /write_new_synced/);
  assert.doesNotMatch(install, /GPT_WEIGHT_FILE|SOVITS_WEIGHT_FILE|fs::copy|Command::new/);
  assert.match(release, /throw 'Native My Voice release is held/);
  assert.match(wrapper, /throw 'Native My Voice packaging is held/);
});
