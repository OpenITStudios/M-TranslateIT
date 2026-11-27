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
#include <array>

namespace {
constexpr uint32_t k_context = 2048;
constexpr uint32_t k_batch = 256;
constexpr uint32_t k_asr_max_tokens = 512;
constexpr uint32_t k_mt_max_tokens = 256;
constexpr size_t k_max_input_bytes = 8000;
constexpr char k_mt_bos[] = u8"<｜hy_begin▁of▁sentence｜>";
constexpr char k_mt_user[] = u8"<｜hy_User｜>";
constexpr char k_mt_assistant[] = u8"<｜hy_Assistant｜>";
constexpr char k_mt_eos[] = u8"<｜hy_place▁holder▁no▁2｜>";
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

// One decoder implementation shared by ASR and translation. Each model owns
// its context, batch and sampler; no duplicate inference engine or router.
namespace {
struct decoder {
    owned<llama_model, llama_model_free> model{nullptr, llama_model_free};
    owned<llama_context, llama_free> context{nullptr, llama_free};
    owned<llama_batch_ext, llama_batch_ext_free> batch{nullptr, llama_batch_ext_free};
    owned<llama_sampler, llama_sampler_free> sampler{nullptr, llama_sampler_free};
    bool poisoned = false;
};

int32_t open_decoder(decoder & d, const char * path, ggml_backend_dev_t device,
                     const char * expected_arch) {
    ggml_backend_dev_t devices[] = {device, nullptr};
    auto model_params = llama_model_default_params();
    model_params.devices = devices;
    model_params.n_gpu_layers = -1;
    model_params.split_mode = LLAMA_SPLIT_MODE_NONE;
    model_params.main_gpu = 0;
    model_params.no_host = true;
    model_params.use_extra_bufts = false;
    d.model.reset(llama_model_load_from_file(path, model_params));
    if (!d.model) return TI_MODEL_LOAD;
    char architecture[32]{};
    const int n = llama_model_meta_val_str(d.model.get(), "general.architecture",
                                           architecture, sizeof(architecture));
    if (n != static_cast<int>(std::strlen(expected_arch)) ||
        std::strcmp(architecture, expected_arch) != 0) return TI_MODEL_LOAD;
    auto params = llama_context_default_params();
    params.n_ctx = k_context;
    params.n_batch = k_batch;
    params.n_ubatch = k_batch;
    params.n_seq_max = 1;
    params.n_threads = 4;
    params.n_threads_batch = 4;
    params.offload_kqv = true;
    params.op_offload = true;
    d.context.reset(llama_init_from_model(d.model.get(), params));
    if (!d.context || !llama_get_memory(d.context.get()) ||
        llama_n_ctx_seq(d.context.get()) < k_context ||
        llama_n_batch(d.context.get()) < k_batch) return TI_CONTEXT_LOAD;
    d.batch.reset(llama_batch_ext_init(d.context.get()));
    d.sampler.reset(llama_sampler_init_greedy());
    if (!d.batch || !d.sampler) return TI_CONTEXT_LOAD;
    return TI_OK;
}

ggml_backend_dev_t initialize_cuda(const char * device_id) {
    std::call_once(backend_once, [] { llama_backend_init(); });
    return llama_supports_gpu_offload() ? exact_cuda_device(device_id) : nullptr;
}
}

struct ti_asr : decoder {
    // Derived member destroyed before the base decoder/model.
    owned<mtmd_context, mtmd_free> projector{nullptr, mtmd_free};
};
struct ti_mt : decoder {
    llama_token eos = LLAMA_TOKEN_NULL;
};

extern "C" uint32_t ti_cuda_abi_version(void) { return 2; }

extern "C" int32_t ti_asr_open(const char * model_path, const char * projector_path,
                                const char * device_id, ti_asr ** out) {
    if (!out) return TI_INVALID_INPUT;
    *out = nullptr;
    if (!model_path || !*model_path || !projector_path || !*projector_path ||
        !device_id || !*device_id) return TI_INVALID_INPUT;
    try {
        const auto device = initialize_cuda(device_id);
        if (!device) return TI_CUDA_DEVICE;
        auto asr = std::make_unique<ti_asr>();
        const auto status = open_decoder(*asr, model_path, device, "qwen3vl");
        if (status != TI_OK) return status;

        auto audio_params = mtmd_context_params_default();
        audio_params.use_gpu = true;
        audio_params.device = device; // SAME exact device as decoder
        audio_params.n_threads = 4;
        audio_params.print_timings = false;
        audio_params.warmup = false; // real functional inference is a separate gate
        asr->projector.reset(mtmd_init_from_file(projector_path, asr->model.get(), audio_params));
        if (!asr->projector || !mtmd_support_audio(asr->projector.get()) ||
            mtmd_get_audio_sample_rate(asr->projector.get()) != 16000) return TI_PROJECTOR_LOAD;
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

int32_t generate(decoder & d, llama_pos next_pos, uint32_t max_tokens,
                 llama_token required_eos, std::string_view echo,
                 char * output, size_t * out_len) {
    auto * context = d.context.get();
    const auto * vocab = llama_model_get_vocab(d.model.get());
    if (!vocab) return TI_INFERENCE;
    // Only publish once EOG is observed. No retry with a second model or
    // decoding mode, no partial transcript at the token limit.
    size_t used = 0;
    for (uint32_t i = 0; i < max_tokens; ++i) {
        const llama_token token = llama_sampler_sample(d.sampler.get(), context, -1);
        if (token == LLAMA_TOKEN_NULL) return TI_INFERENCE;
        if (llama_vocab_is_eog(vocab, token)) {
            if (required_eos != LLAMA_TOKEN_NULL && token != required_eos)
                return TI_INCOMPLETE;
            const std::string_view transcript(output, used);
            if (transcript.empty() ||
                transcript.find_first_not_of(" \r\n\t") == std::string_view::npos) return TI_EMPTY;
            if (transcript.find('\0') != std::string_view::npos ||
                transcript.find("<asr_text>") != std::string_view::npos ||
                transcript.find("<|im_start|>") != std::string_view::npos ||
                transcript.find("<|im_end|>") != std::string_view::npos)
                return TI_INCOMPLETE;
            if (!echo.empty() &&
                (transcript.find(u8"<｜hy_") != std::string_view::npos ||
                 transcript.find("<think>") != std::string_view::npos ||
                 transcript.find("</think>") != std::string_view::npos ||
                 transcript.find("<answer>") != std::string_view::npos ||
                 transcript.find("</answer>") != std::string_view::npos ||
                 transcript.find("<tool_") != std::string_view::npos ||
                 transcript.find("</tool_") != std::string_view::npos ||
                 transcript.substr(transcript.find_first_not_of(" \r\n\t")).rfind(echo, 0) == 0))
                return TI_INCOMPLETE;
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
        if (i + 1 == max_tokens) break;
        llama_batch_ext_clear(d.batch.get());
        const int idx = llama_batch_ext_add_token(d.batch.get(), 0, token);
        if (idx < 0 || !llama_batch_ext_set_pos(d.batch.get(), idx, &next_pos) ||
            !llama_batch_ext_set_output_logits(d.batch.get(), idx, true) ||
            llama_process(context, LLAMA_PROCESS_TYPE_DECODE, d.batch.get()) != 0) {
            return TI_INFERENCE;
        }
        ++next_pos;
    }
    return TI_INCOMPLETE;
}

int32_t infer(ti_asr & asr, const float * pcm, size_t count, const char * language,
              char * output, size_t * out_len) {
    auto * context = asr.context.get();
    auto * projector = asr.projector.get();
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
    if (n_pos <= 0 || n_tokens == 0 || limit <= k_asr_max_tokens ||
        static_cast<uint64_t>(n_pos) > limit - k_asr_max_tokens ||
        n_tokens > limit - k_asr_max_tokens) return TI_INPUT_LIMIT;
    llama_pos next_pos = 0;
    if (mtmd_helper_eval_chunks(projector, context, chunks.get(), 0, 0,
                               k_batch, true, &next_pos) != 0 ||
        next_pos <= 0 || static_cast<uint64_t>(next_pos) > limit - k_asr_max_tokens) {
        return TI_INFERENCE;
    }

    return generate(asr, next_pos, k_asr_max_tokens, LLAMA_TOKEN_NULL, {}, output, out_len);
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

extern "C" int32_t ti_mt_open(const char * model_path, const char * device_id, ti_mt ** out) {
    if (!out) return TI_INVALID_INPUT;
    *out = nullptr;
    if (!model_path || !*model_path || !device_id || !*device_id) return TI_INVALID_INPUT;
    try {
        const auto device = initialize_cuda(device_id);
        if (!device) return TI_CUDA_DEVICE;
        auto mt = std::make_unique<ti_mt>();
        const auto status = open_decoder(*mt, model_path, device, "hunyuan-dense");
        if (status != TI_OK) return status;
        const auto * vocab = llama_model_get_vocab(mt->model.get());
        // Verify publisher control markers resolve to single native tokens,
        // and the template stop marker is an actual EOG in this GGUF.
        if (!vocab) return TI_MODEL_LOAD;
        const char * markers[] = {k_mt_bos, k_mt_user, k_mt_assistant, k_mt_eos};
        // Pinned Tencent tokenizer and generation_config: BOS/User/Assistant/EOS.
        const llama_token expected[] = {120000, 120006, 120007, 120020};
        for (size_t i = 0; i < 4; ++i) {
            llama_token id = LLAMA_TOKEN_NULL;
            if (llama_tokenize(vocab, markers[i], static_cast<int32_t>(std::strlen(markers[i])),
                               &id, 1, false, true) != 1 || id != expected[i])
                return TI_MODEL_LOAD;
        }
        mt->eos = expected[3];
        if (llama_vocab_bos(vocab) != expected[0] ||
            llama_vocab_eos(vocab) != mt->eos || !llama_vocab_is_eog(vocab, mt->eos))
            return TI_MODEL_LOAD;
        *out = mt.release();
        return TI_OK;
    } catch (...) {
        return TI_INTERNAL;
    }
}

extern "C" void ti_mt_close(ti_mt * mt) { delete mt; }

namespace {
bool mt_source_admitted(std::string_view source) {
    return !source.empty() && source.size() <= k_max_input_bytes &&
        source.find_first_not_of(" \r\n\t") != std::string_view::npos &&
        source.find('\0') == std::string_view::npos &&
        source.find(u8"<｜hy_") == std::string_view::npos &&
        source.find("<think>") == std::string_view::npos &&
        source.find("</think>") == std::string_view::npos &&
        source.find("<answer>") == std::string_view::npos &&
        source.find("</answer>") == std::string_view::npos &&
        source.find("<tool_") == std::string_view::npos &&
        source.find("</tool_") == std::string_view::npos;
}

int32_t translate(ti_mt & mt, std::string_view source, const char * target,
                  char * output, size_t * out_len) {
    auto * context = mt.context.get();
    const auto * vocab = llama_model_get_vocab(mt.model.get());
    if (!vocab) return TI_INFERENCE;
    // Tencent's single-user-turn template and default translation instruction.
    // No system message, past turns, user instructions or second model.
    std::string prompt = k_mt_bos;
    prompt += k_mt_user;
    const size_t body_start = prompt.size();
    prompt += "Translate the following text into ";
    prompt += target;
    prompt += ". Note that you should only output the translated result without any additional explanation:\n\n";
    prompt.append(source.data(), source.size());
    const size_t body_size = prompt.size() - body_start;
    prompt += k_mt_assistant;
    std::array<llama_token, k_context> tokens;
    const int n = llama_tokenize(vocab, prompt.data(), static_cast<int32_t>(prompt.size()),
                                 tokens.data(), static_cast<int32_t>(tokens.size()), false, true);
    const auto limit = std::min<uint32_t>(llama_n_ctx_seq(context), k_context);
    // Preserve the existing 256-token ceiling, choose a bounded adaptive
    // budget, and reserve room before evaluating any token. No truncation.
    if (n <= 0 || limit <= k_mt_max_tokens ||
        static_cast<uint32_t>(n) > limit - k_mt_max_tokens) return TI_INPUT_LIMIT;
    const uint32_t budget = std::min(k_mt_max_tokens, std::max(64u, 2u * static_cast<uint32_t>(n) + 32u));

    llama_memory_clear(llama_get_memory(context), false);
    clear_utterance clear{context};
    llama_sampler_reset(mt.sampler.get());
    for (int i = 0; i < n;) {
        llama_batch_ext_clear(mt.batch.get());
        const int end = std::min(n, i + static_cast<int>(k_batch));
        for (; i < end; ++i) {
            const int idx = llama_batch_ext_add_token(mt.batch.get(), 0, tokens[i]);
            const llama_pos pos = i;
            if (idx < 0 || !llama_batch_ext_set_pos(mt.batch.get(), idx, &pos) ||
                !llama_batch_ext_set_output_logits(mt.batch.get(), idx, i + 1 == n))
                return TI_INFERENCE;
        }
        if (llama_process(context, LLAMA_PROCESS_TYPE_DECODE, mt.batch.get()) != 0)
            return TI_INFERENCE;
    }
    return generate(mt, n, budget, mt.eos,
                    std::string_view(prompt).substr(body_start, body_size), output, out_len);
}
}

extern "C" int32_t ti_mt_translate(ti_mt * mt, const char * source, size_t source_len,
                                   uint32_t source_language, uint32_t target_language,
                                   char * output, size_t output_cap, size_t * out_len) {
    if (!out_len) return TI_INVALID_INPUT;
    *out_len = 0;
    if (!mt || !source || source_len == 0 || source_len > k_max_input_bytes ||
        !output || output_cap < k_max_output_bytes ||
        !((source_language == 1 && target_language == 2) ||
          (source_language == 2 && target_language == 1))) return TI_INVALID_INPUT;
    if (mt->poisoned) return TI_POISONED;
    const std::string_view input(source, source_len);
    if (!mt_source_admitted(input)) return TI_INVALID_INPUT;
    try {
        const auto status = translate(*mt, input, target_language == 1 ? "Indonesian" : "English",
                                      output, out_len);
        if (status == TI_INFERENCE) mt->poisoned = true;
        return status;
    } catch (...) {
        mt->poisoned = true;
        *out_len = 0;
        return TI_INTERNAL;
    }
}
