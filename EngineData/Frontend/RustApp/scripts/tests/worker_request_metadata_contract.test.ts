import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const read = (path: string) => readFileSync(new URL("../../src-tauri/src/commands/" + path, import.meta.url), "utf8");
const policy = read("helper_bridge/request_policy.rs");
const transport = read("helper_bridge/transport.rs");
const native = read("helper_bridge/transport/native.rs");
const startup = read("helper_bridge.rs");

test("native requests retain scheduler identity, exact Meeting scope and a bounded deadline", () => {
  assert.match(transport, /acquire_scoped_helper_task_permit\(priority, wait_generation, deferred_deadline\)/);
  assert.match(transport, /permit\.request_id\(\)/);
  assert.match(native, /checked_add\(Duration::from_millis\(ms\)\)/);
  assert.match(native, /scope\(payload\)/);
  assert.match(native, /body\["request_id"\] = json!\(request_id\)/);
  assert.match(native, /body\["scheduler_priority"\] = json!\(priority\.label\(\)\)/);
  for (const field of ["meeting_generation","meeting_session_id","meeting_lane","meeting_start_prepare"]) {
    assert.ok(policy.includes(field));
  }
  assert.doesNotMatch(startup + transport, /Command::new|resolve_worker_python_command|begin_worker_stdin_write|begin_worker_stdout_read/);
});

test("native synthesis borrows text and actor authority through admission and returns owned PCM", () => {
  assert.match(transport, /Synthesize \{ text: &\x27a str, actor_token: &\x27a str, samples: &\x27a mut Option<Vec<f32>> \}/);
  assert.match(native, /is_voice && \(payload\.get\("text"\)\.is_some\(\) \|\| payload\.get\("expected_actor_token"\)\.is_some\(\)\)/);
  assert.match(native, /actor_token\.expect\("checked token"\), preparing, \(\*text\)\.to_owned\(\), deadline/);
  assert.match(native, /token\.is_empty\(\) \|\| token\.len\(\) > 512/);
  assert.match(native, /\*\*slot = Some\(samples\)/);
  for (const caller of [read("meeting_session/outbound_pipeline.rs"), read("helper_bridge/functional_probe.rs")]) {
    const call = caller.slice(caller.indexOf("synthesize_native_voice(json!({"));
    assert.ok(call.startsWith("synthesize_native_voice(json!({"));
    const metadata = call.slice(0, call.indexOf("}),"));
    assert.doesNotMatch(metadata, /"text":|"expected_actor_token":/);
    assert.match(call, /\}\), &?translated_(?:text|fixture), &actor_token\)/);
  }
});

test("native scope fences observe borrowed session identity without building diagnostics", () => {
  const state = readFileSync(new URL("../../src-tauri/src/engine/runtime_state.rs", import.meta.url), "utf8");
  assert.match(state, /Some\(observe\(guard\.as_ref\(\)\)\)/);
  assert.doesNotMatch(native + policy, /latest_runtime_session_state\(\)/);
  assert.equal((native.match(/!scope_is_current\(&request_scope\)/g) ?? []).length, 6);
  assert.match(native, /snapshot\.generation == generation\s*&& snapshot\.session_id == session_id/);
  const recheck = native.slice(native.indexOf("fn scope_matches_snapshot("), native.indexOf("fn error_blocker("));
  assert.doesNotMatch(recheck, /payload|scope_from_snapshot\(/);
  assert.match(native, /fn resolved_scope_rechecks_match_exact_request_without_reparsing/);
  assert.match(native, /observe_runtime_session\(\|snapshot\| scope_matches_snapshot\(expected, snapshot\)\)\.unwrap_or\(false\)/);
  assert.match(transport, /let request_id = permit\.request_id\(\);/);
});

test("native receipts construct the final runtime claim directly", () => {
  assert.match(transport, /runtime_claim: claim,/);
  assert.doesNotMatch(native, /response\.runtime_claim = NATIVE_INFERENCE_CLAIM|response\.runtime_claim = super::super::NATIVE_STATUS_CLAIM/);
  assert.match(native, /&host, NATIVE_INFERENCE_CLAIM\.into\(\)\)/);
  assert.match(native, /if task == "status" \{ super::super::NATIVE_STATUS_CLAIM\.into\(\) \}/);
});

test("scheduler fast path avoids wait registration and native scope keeps the queued epoch", () => {
  const scheduler = read("helper_bridge_runtime/scheduler.rs");
  const enter = scheduler.indexOf("if scheduler_can_enter(&state, priority)");
  const register = scheduler.indexOf("if !registered {", enter);
  const wait = scheduler.indexOf("wake.wait_timeout(state, remaining)", register);
  assert.ok(enter >= 0 && register > enter && wait > register);
  assert.match(scheduler, /if registered \{\s*scheduler_waiting_decrement\(&mut state, priority\);\s*\}\s*state.active = true/);
  assert.match(transport, /Ok\(current\) if current != wait_generation/);
  assert.match(native, /request_scope\.generation != wait_generation/);
  assert.match(native, /scope\.generation == wait_generation/);
});

test("invalid text and language direction are rejected before model preparation", () => {
  const dispatch = native.slice(native.indexOf("pub(super) fn dispatch("), native.indexOf("let mut bound_id = None;"));
  const direction = dispatch.indexOf("let translation_direction =");
  const invalid = dispatch.indexOf("translation_source_admitted(source)");
  const lock = dispatch.indexOf("runtime().lock()");
  const open = dispatch.indexOf("NativeInferenceOwner::open(");
  const scope = dispatch.indexOf("request_scope.generation != wait_generation");
  const lifecycle = dispatch.indexOf("!host.native_lifecycle_ready()");
  assert.ok(lock >= 0 && lock < scope && scope < lifecycle && lifecycle < direction
    && direction < invalid && invalid < open);
  assert.match(dispatch.slice(direction, open), /if from == to/);
  assert.match(native, /translation_direction\.expect\("checked translation direction"\)/);
  assert.match(native, /translation_direction\.expect\("checked translation preload direction"\)/);
  const abi = readFileSync(new URL("../../../../../tools/translation_quality/native_cuda/src/lib.rs", import.meta.url), "utf8");
  assert.match(abi, /pub fn translation_source_admitted\(source: &str\)/);
  assert.match(abi, /if from == to \|\| !translation_source_admitted\(source\)/);
});

test("routine helper observations do not write trace logs; poisoned locks still do", () => {
  const observation = startup.slice(startup.indexOf("pub fn get_helper_bridge_status()"), startup.indexOf("fn public_worker_start_has_idle_audio_owner()"));
  assert.match(observation, /Ok\(runtime\) => decorate_functional_readiness_status\(status_from_runtime\(&runtime\)\)/);
  assert.match(observation, /Err\(TryLockError::WouldBlock\) => status_lock_busy\(\)/);
  assert.doesNotMatch(observation, /trace_command_start|trace_command_end/);
  assert.match(observation, /Err\(TryLockError::Poisoned\(poisoned\)\)[\s\S]*?drop\(poisoned\);[\s\S]*?trace_command_error\(/);
});

test("native ASR and MT avoid full-vocabulary sampler materialization without changing greedy order", () => {
  const cpp = readFileSync(new URL("../../../../../tools/translation_quality/native_cuda/inference.cpp", import.meta.url), "utf8");
  const decoder = cpp.slice(cpp.indexOf("int32_t open_decoder("), cpp.indexOf("ggml_backend_dev_t initialize_cuda("));
  assert.match(decoder, /if \(!audio_generation\) \{\s*params\.samplers = nullptr;\s*params\.n_samplers = 0;/);
  const greedy = cpp.slice(cpp.indexOf("constexpr llama_token greedy_token("), cpp.indexOf("int32_t generate("));
  assert.match(greedy, /if \(logits\[i\] > logits\[selected\]\) selected = i;/);
  assert.match(greedy, /static_assert\(greedy_token\(k_greedy_tie_fixture, 4\) == 1\)/);
  const generate = cpp.slice(cpp.indexOf("int32_t generate("), cpp.indexOf("int32_t infer("));
  assert.match(generate, /greedy_token\(llama_get_logits_ith\(context, -1\), n_vocab\)/);
  assert.doesNotMatch(generate, /llama_sampler_sample\(|llama_token_data|std::vector/);
  assert.match(generate, /llama_vocab_is_eog\(vocab, token\)/);
  assert.match(generate, /if \(llama_vocab_is_control\(vocab, token\)\) return TI_INCOMPLETE/);
  assert.match(generate, /return TI_INCOMPLETE;/);
  const voice = cpp.slice(cpp.indexOf("int32_t synthesize_voice("), cpp.indexOf("struct ti_runtime {"));
  assert.match(voice, /mtmd_helper_gen_audio_step_gen/);
  assert.doesNotMatch(voice, /greedy_token\(/);
});

test("Zonos3 reuses bounded sampling scratch and MT reserves the intact prompt", () => {
  const patch = readFileSync(new URL("../../../../../tools/translation_quality/llama-strict-cuda.patch", import.meta.url), "utf8");
  for (const [field, initialization] of [
    ["sample_logits", "l.assign(in, in + n_vocab);"],
    ["sample_seen", "seen.assign(audio_eoa, false);"],
    ["sample_top_k", "tmp.assign(l.begin(), l.end());"],
    ["sample_indices", "idx.clear();"],
    ["sample_probabilities", "p.resize(idx.size());"],
    ["sample_order", "ord.resize(idx.size());"],
  ]) {
    assert.ok(patch.includes(field) && patch.includes("+" + " ".repeat(field === "sample_seen" || field === "sample_top_k" ? 12 : 8) + initialization));
  }
  assert.match(patch, /std::nth_element\(tmp\.begin\(\), tmp\.begin\(\) \+ \(top_k - 1\)/);
  assert.match(patch, /std::uniform_real_distribution<double>\(0\.0, kept\)\(rng\)/);
  const cpp = readFileSync(new URL("../../../../../tools/translation_quality/native_cuda/inference.cpp", import.meta.url), "utf8");
  const mt = cpp.slice(cpp.indexOf("int32_t translate(ti_mt"), cpp.indexOf("static int32_t translate_mt("));
  assert.ok(mt.indexOf("prompt.reserve(") >= 0 && mt.indexOf("prompt.reserve(") < mt.indexOf("prompt += k_mt_bos"));
  assert.equal((mt.match(/llama_tokenize\(/g) ?? []).length, 1);
  assert.match(mt, /prompt\.append\(source\.data\(\), source\.size\(\)\)/);
});

test("ASR releases completed input storage and reuses decoder text batches", () => {
  const cpp = readFileSync(new URL("../../../../../tools/translation_quality/native_cuda/inference.cpp", import.meta.url), "utf8");
  const prefill = cpp.slice(cpp.indexOf("int32_t decode_text_prefill("), cpp.indexOf("constexpr llama_token greedy_token("));
  assert.match(prefill, /llama_batch_ext_clear\(d\.batch\.get\(\)\)/);
  assert.match(prefill, /llama_batch_ext_add_token\(d\.batch\.get\(\), 0, tokens\[i\]\)/);
  assert.match(prefill, /const llama_pos pos = next_pos\+\+/);
  assert.match(prefill, /logits_last && i \+ 1 == count/);
  assert.match(prefill, /control\.check\(\)/);
  assert.doesNotMatch(prefill, /llama_batch_ext_init|llama_synchronize/);
  const asr = cpp.slice(cpp.indexOf("int32_t infer("), cpp.indexOf("static int32_t transcribe_asr("));
  assert.match(asr, /MTMD_INPUT_CHUNK_TYPE_TEXT[\s\S]*?decode_text_prefill\(asr, tokens, count, next_pos, logits_last, control\)[\s\S]*?else if \(mtmd_helper_eval_chunk_single/);
  assert.ok(asr.indexOf("chunks.reset();") > asr.indexOf("if (next_pos <= 0") && asr.indexOf("chunks.reset();") < asr.indexOf("return generate(asr,"));
  const mt = cpp.slice(cpp.indexOf("int32_t translate(ti_mt"), cpp.indexOf("static int32_t translate_mt("));
  assert.match(mt, /decode_text_prefill\(mt, tokens\.data\(\), static_cast<size_t>\(n\), next_pos, true, control\)/);
  const patch = readFileSync(new URL("../../../../../tools/translation_quality/llama-strict-cuda.patch", import.meta.url), "utf8");
  assert.match(patch, /mtmd_audio_fill_entries\(mels, false, out\.entries\);[\s\S]*?\+    std::vector<float>\(\)\.swap\(out\.samples\);/);
});
