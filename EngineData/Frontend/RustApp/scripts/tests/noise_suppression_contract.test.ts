import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const settingsPage = readFileSync(new URL("../../src/pages/Settings.svelte", import.meta.url), "utf8");
const settings = readFileSync(new URL("../../src/components/settings/MeetingNoiseSuppression.svelte", import.meta.url), "utf8");
const consumer = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/consumer_runtime.rs", import.meta.url), "utf8");
const evidence = readFileSync(new URL("../../src-tauri/src/engine/audio/evidence.rs", import.meta.url), "utf8");
const native = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge/transport/native.rs", import.meta.url), "utf8");
const incoming = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_pipeline.rs", import.meta.url), "utf8");
const outbound = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/outbound_pipeline.rs", import.meta.url), "utf8");

test("noise suppression exposes only auto and off through its dedicated settings owner", () => {
  assert.match(settingsPage, /MeetingNoiseSuppression/);
  assert.match(settings, /Auto · recommended/);
  assert.match(settings, />Off</);
  assert.doesNotMatch(settings, /Low|Medium|High|Aggressive/);
});

test("meeting consumers snapshot suppression once instead of rereading per utterance", () => {
  assert.match(consumer, /noise_suppression_enabled/);
  assert.match(consumer, /engine::load_settings\(\)\.audio\.noise_suppression != "off"/);
  for (const pipeline of [incoming, outbound]) {
    assert.match(pipeline, /send_helper_worker_pcm_task\([\s\S]*?noise_suppression_enabled/);
    assert.doesNotMatch(pipeline, /write_finalized_.*_wav/);
  }
});

test("noise suppression conditions finalized native PCM in place before inference", () => {
  assert.match(evidence, /fn condition_asr_pcm\(samples: &mut \[f32\]/);
  assert.match(evidence, /\*sample = condition_asr_sample\(\*sample, threshold\)/);
  assert.doesNotMatch(evidence, /onnxruntime|tch::|std::process::Command/);
  const take = native.indexOf("pcm.utterance.borrow_mut().take()");
  const condition = native.indexOf("condition_asr_pcm(&mut samples", take);
  const begin = native.indexOf("owner.begin(input, deadline)", condition);
  assert.ok(take >= 0 && condition > take && begin > condition);
});
