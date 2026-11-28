import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import test from "node:test";

const source = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");

test("standalone Text has no application route, setup shortcut or native entrypoint", () => {
  for (const path of ["src/App.svelte", "src/components/layout/Sidebar.svelte", "src/components/setup/SetupNavigation.svelte", "src/pages/FirstSetup.svelte", "src/pages/Settings.svelte"]) {
    assert.doesNotMatch(source(path), /pages\/Text|route: "text"|route === "text"|onOpenText|Translate text|Text translation/);
  }
  assert.doesNotMatch(source("src/app/shared/types.ts"), /AppRoute[^;]*"text"/);
  assert.doesNotMatch(source("src/app/bridge/runtimeApi.ts"), /translateText|translate_text|TextTranslation/);
  assert.doesNotMatch(source("src-tauri/src/commands/registry.rs"), /text_translation|quick_translate_text|translate_text_alternative/);
  for (const retired of ["src/pages/Text.svelte", "src/app/bridge/productTranslationFacade.ts", "src/app/runtime/textTranslationCache.ts", "src/app/runtime/textResultPolicy.ts", "src-tauri/src/commands/text_translation.rs"]) {
    assert.equal(existsSync(new URL(`../../${retired}`, import.meta.url)), false, retired);
  }
});

test("text diagnostics stay on the canonical native translation path", () => {
  const transport = source("src-tauri/src/commands/helper_bridge/transport.rs");
  const native = source("src-tauri/src/commands/helper_bridge/transport/native.rs");
  const abi = readFileSync(new URL("../../../../../tools/translation_quality/native_cuda/src/lib.rs", import.meta.url), "utf8");
  const runner = readFileSync(new URL("../../../../../tools/audio_validation/worker_runner.py", import.meta.url), "utf8");
  assert.match(transport, /return native::dispatch\(task,/);
  assert.match(native, /payload\.get\("text"\)\.and_then\(Value::as_str\)/);
  assert.match(native, /translation_source_admitted\(source\)/);
  assert.match(native, /InferenceInput::Translation \{ source, from, to \}/);
  assert.match(abi, /pub fn translate<'a>\(/);
  assert.match(runner, /def native_translation_requests\(/);
  assert.match(runner, /def native_translation_replay\(/);
});
