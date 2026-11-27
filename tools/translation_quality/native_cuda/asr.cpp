#include "abi.h"
#include "llama.h"
#include "ggml-backend.h"
#include "mtmd.h"
#include "mtmd-helper.h"

#include <cmath>
#include <cstring>
#include <memory>
#include <mutex>
#include <string>
#include <string_view>
#include <algorithm>

namespace {
constexpr uint32_t k_context = 2048;
constexpr uint32_t k_batch = 256;
constexpr uint32_t k_max_tokens = 512;
constexpr size_t k_max_output_bytes = 16000;
constexpr size_t k_min_samples = 4800;
constexpr size_t k_max_samples = 960000;

std::once_flag backend_once;

ggml_backend_dev_t exact_cuda_device(const char * wanted) {
    ggml_backend_dev_t selected = nullptr;
    for (size_t i = 0; i < ggml_backend_dev_count(); ++i) {
        const auto device = ggml_backend_dev_get(i);
        if (!device) continue;
        const auto reg = ggml_backend_dev_backend_reg(device);
        const char * name = reg ? ggml_backend_reg_name(reg) : nullptr;
        if (!name || std::strcmp(name, "CUDA") != 0 ||
            ggml_backend_dev_type(device) != GGML_BACKEND_DEVICE_TYPE_GPU) continue;
        ggml_backend_dev_props props{};
        ggml_backend_dev_get_props(device, &props);
        if (!props.device_id || std::strcmp(props.device_id, wanted) != 0) continue;
        if (selected) return nullptr; // ambiguous physical identity
        selected = device;
    }
    return selected;
}

template<class T, void (*Free)(T *)>
using owned = std::unique_ptr<T, decltype(Free)>;
}

struct ti_asr {
    // Reverse destruction order: batch/projector/context before model.
    owned<llama_model, llama_model_free> model{nullptr, llama_model_free};
    owned<llama_context, llama_free> context{nullptr, llama_free};
    owned<mtmd_context, mtmd_free> projector{nullptr, mtmd_free};
    owned<llama_batch_ext, llama_batch_ext_free> batch{nullptr, llama_batch_ext_free};
    owned<llama_sampler, llama_sampler_free> sampler{nullptr, llama_sampler_free};
    bool poisoned = false;
};

extern "C" uint32_t ti_cuda_abi_version(void) { return 1; }

extern "C" int32_t ti_asr_open(const char * model_path, const char * projector_path,
                                const char * device_id, ti_asr ** out) {
    if (!out) return TI_INVALID_INPUT;
    *out = nullptr;
    if (!model_path || !*model_path || !projector_path || !*projector_path ||
        !device_id || !*device_id) return TI_INVALID_INPUT;
    try {
        std::call_once(backend_once, [] { llama_backend_init(); });
        const auto device = exact_cuda_device(device_id);
        if (!device || !llama_supports_gpu_offload()) return TI_CUDA_DEVICE;

        auto asr = std::make_unique<ti_asr>();
        ggml_backend_dev_t devices[] = {device, nullptr};
        auto model_params = llama_model_default_params();
        model_params.devices = devices;
        model_params.n_gpu_layers = -1;
        model_params.split_mode = LLAMA_SPLIT_MODE_NONE;
        model_params.main_gpu = 0; // index into the explicit singleton list
        model_params.no_host = true;
        model_params.use_extra_bufts = false;
        asr->model.reset(llama_model_load_from_file(model_path, model_params));
        if (!asr->model) return TI_MODEL_LOAD;

        // Qwen3-ASR's converted text decoder is qwen3vl. This is only an
        // architecture sanity check; the pinned GGUF hashes own identity.
        char architecture[32]{};
        const int n = llama_model_meta_val_str(asr->model.get(), "general.architecture",
                                               architecture, sizeof(architecture));
        if (n != 7 || std::strcmp(architecture, "qwen3vl") != 0) return TI_MODEL_LOAD;

        auto context_params = llama_context_default_params();
        context_params.n_ctx = k_context;
        context_params.n_batch = k_batch;
        context_params.n_ubatch = k_batch;
        context_params.n_seq_max = 1;
        context_params.n_threads = 4;
        context_params.n_threads_batch = 4;
        context_params.offload_kqv = true;
        context_params.op_offload = true;
        asr->context.reset(llama_init_from_model(asr->model.get(), context_params));
        if (!asr->context || !llama_get_memory(asr->context.get()) ||
            llama_n_ctx_seq(asr->context.get()) < k_context ||
            llama_n_batch(asr->context.get()) < k_batch) return TI_CONTEXT_LOAD;

        auto audio_params = mtmd_context_params_default();
        audio_params.use_gpu = true;
        audio_params.device = device; // SAME exact device as decoder
        audio_params.n_threads = 4;
        audio_params.print_timings = false;
        audio_params.warmup = false; // real functional inference is a separate gate
        asr->projector.reset(mtmd_init_from_file(projector_path, asr->model.get(), audio_params));
        if (!asr->projector || !mtmd_support_audio(asr->projector.get()) ||
            mtmd_get_audio_sample_rate(asr->projector.get()) != 16000) return TI_PROJECTOR_LOAD;
        asr->batch.reset(llama_batch_ext_init(asr->context.get()));
        asr->sampler.reset(llama_sampler_init_greedy());
        if (!asr->batch || !asr->sampler) return TI_CONTEXT_LOAD;
        *out = asr.release();
        return TI_OK;
    } catch (...) {
        return TI_INTERNAL;
    }
}

extern "C" void ti_asr_close(ti_asr * asr) { delete asr; }

namespace {
struct clear_utterance {
    llama_context * context;
    ~clear_utterance() { llama_memory_clear(llama_get_memory(context), false); }
};

int32_t infer(ti_asr & asr, const float * pcm, size_t count, const char * language,
              char * output, size_t * out_len) {
    auto * context = asr.context.get();
    auto * projector = asr.projector.get();
    const auto * vocab = llama_model_get_vocab(asr.model.get());
    if (!vocab) return TI_INFERENCE;
    llama_memory_clear(llama_get_memory(context), false);
    clear_utterance clear{context};
    llama_sampler_reset(asr.sampler.get());

    // Fixed current-utterance-only Qwen3-ASR prompt. mtmd inserts its own
    // audio_start/audio_end tokens around the marker. Force the language
    // with the official assistant prefill, never inject history or hotwords.
    const std::string prompt = std::string("<|im_start|>system\n<|im_end|>\n"
        "<|im_start|>user\n") + mtmd_default_marker() +
        "<|im_end|>\n<|im_start|>assistant\nlanguage " + language + "<asr_text>";
    owned<mtmd_bitmap, mtmd_bitmap_free> bitmap(mtmd_bitmap_init_from_audio(count, pcm), mtmd_bitmap_free);
    owned<mtmd_input_chunks, mtmd_input_chunks_free> chunks(mtmd_input_chunks_init(), mtmd_input_chunks_free);
    if (!bitmap || !chunks) return TI_INFERENCE;
    const mtmd_bitmap * bitmaps[] = {bitmap.get()};
    const mtmd_input_text text{prompt.data(), prompt.size(), false, true};
    if (mtmd_tokenize(projector, chunks.get(), &text, bitmaps, 1) != 0 ||
        mtmd_input_chunks_size(chunks.get()) == 0) return TI_INFERENCE;

    const auto n_pos = mtmd_helper_get_n_pos(chunks.get());
    const auto n_tokens = mtmd_helper_get_n_tokens(chunks.get());
    const auto limit = llama_n_ctx_seq(context);
    if (n_pos <= 0 || n_tokens == 0 || limit <= k_max_tokens ||
        static_cast<uint64_t>(n_pos) > limit - k_max_tokens ||
        n_tokens > limit - k_max_tokens) return TI_INPUT_LIMIT;
    llama_pos next_pos = 0;
    if (mtmd_helper_eval_chunks(projector, context, chunks.get(), 0, 0,
                               k_batch, true, &next_pos) != 0 ||
        next_pos <= 0 || static_cast<uint64_t>(next_pos) > limit - k_max_tokens) {
        return TI_INFERENCE;
    }

    // Only publish once EOG is observed. No retry with a second model or
    // decoding mode, no partial transcript at the token limit.
    size_t used = 0;
    for (uint32_t i = 0; i < k_max_tokens; ++i) {
        const llama_token token = llama_sampler_sample(asr.sampler.get(), context, -1);
        if (token == LLAMA_TOKEN_NULL) return TI_INFERENCE;
        if (llama_vocab_is_eog(vocab, token)) {
            const std::string_view transcript(output, used);
            if (transcript.empty() ||
                transcript.find_first_not_of(" \r\n\t") == std::string_view::npos) return TI_EMPTY;
            if (transcript.find('\0') != std::string_view::npos ||
                transcript.find("<asr_text>") != std::string_view::npos ||
                transcript.find("<|im_start|>") != std::string_view::npos ||
                transcript.find("<|im_end|>") != std::string_view::npos) return TI_INCOMPLETE;
            *out_len = used;
            return TI_OK;
        }
        if (llama_vocab_is_control(vocab, token)) return TI_INCOMPLETE;
        char piece[256];
        const int len = llama_token_to_piece(vocab, token, piece, sizeof(piece), 0, true);
        if (len <= 0 || len > static_cast<int>(sizeof(piece))) return TI_INCOMPLETE;
        if (used + static_cast<size_t>(len) > k_max_output_bytes) return TI_OUTPUT_LIMIT;
        std::memcpy(output + used, piece, static_cast<size_t>(len));
        used += static_cast<size_t>(len);
        // Last token without an EOG is incomplete; avoid decoding it just to
        // discard the result.
        if (i + 1 == k_max_tokens) break;
        llama_batch_ext_clear(asr.batch.get());
        const int idx = llama_batch_ext_add_token(asr.batch.get(), 0, token);
        if (idx < 0 || !llama_batch_ext_set_pos(asr.batch.get(), idx, &next_pos) ||
            !llama_batch_ext_set_output_logits(asr.batch.get(), idx, true) ||
            llama_process(context, LLAMA_PROCESS_TYPE_DECODE, asr.batch.get()) != 0) {
            return TI_INFERENCE;
        }
        ++next_pos;
    }
    return TI_INCOMPLETE;
}
}

extern "C" int32_t ti_asr_transcribe(ti_asr * asr, const float * pcm, size_t count,
                                      uint32_t language, char * output, size_t output_cap,
                                      size_t * out_len) {
    if (!out_len) return TI_INVALID_INPUT;
    *out_len = 0;
    if (!asr || !pcm || !output || output_cap < k_max_output_bytes ||
        count < k_min_samples || count > k_max_samples ||
        (language != 1 && language != 2)) return TI_INVALID_INPUT;
    if (asr->poisoned) return TI_POISONED;
    if (!std::all_of(pcm, pcm + count, [](float v) {
        return std::isfinite(v) && v >= -1.0f && v <= 1.0f;
    })) return TI_INVALID_INPUT;
    try {
        const auto status = infer(*asr, pcm, count, language == 1 ? "Indonesian" : "English",
                                  output, out_len);
        if (status == TI_INFERENCE) asr->poisoned = true;
        return status;
    } catch (...) {
        asr->poisoned = true;
        *out_len = 0;
        return TI_INTERNAL;
    }
}
