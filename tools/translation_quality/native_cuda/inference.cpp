#include "abi.h"
#include "llama.h"
#include "ggml-backend.h"
#include "mtmd.h"
#include "mtmd-helper.h"

#include <cmath>
#include <cstring>
#include <cwchar>
#include <memory>
#include <mutex>
#include <string>
#include <string_view>
#include <algorithm>
#include <array>
#include <atomic>
#include <vector>
#define WIN32_LEAN_AND_MEAN
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <bcrypt.h>

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
constexpr int32_t k_max_token_piece_bytes = 256;
constexpr size_t k_min_samples = 4800;
constexpr size_t k_max_samples = 960000;
// Bounded, complete candidate utterances only; no partial playback.
constexpr size_t k_voice_min_ref_samples = 16000 * 3;
constexpr size_t k_voice_max_ref_samples = 16000 * 30;
constexpr size_t k_voice_max_samples = 44100 * 60;
constexpr uint32_t k_voice_max_frames = 900;

std::once_flag backend_once;
std::atomic<bool> runtime_open{false};
constexpr uint64_t k_reserve_bytes = 1536ull * 1024 * 1024;

// Lease is destroyed last, after both model contexts and file handles.
struct runtime_lease {
    bool acquired = false;
    bool acquire() {
        bool expected = false;
        acquired = runtime_open.compare_exchange_strong(expected, true);
        return acquired;
    }
    ~runtime_lease() { if (acquired) runtime_open.store(false); }
};

struct locked_asset {
    HANDLE file = INVALID_HANDLE_VALUE;
    std::string path;
    uint64_t bytes = 0;
    ~locked_asset() { if (file != INVALID_HANDLE_VALUE) CloseHandle(file); }
    locked_asset() = default;
    locked_asset(const locked_asset &) = delete;
    locked_asset & operator=(const locked_asset &) = delete;
};

struct hash_algorithm {
    BCRYPT_ALG_HANDLE handle = nullptr;
    ~hash_algorithm() { if (handle) BCryptCloseAlgorithmProvider(handle, 0); }
};
struct hash_state {
    BCRYPT_HASH_HANDLE handle = nullptr;
    ~hash_state() { if (handle) BCryptDestroyHash(handle); }
};

// Hash the exact locked read handle in bounded chunks. Keep it open for
// the model lifetime to deny normal writes/deletes, including while mapped.
bool verify_asset(locked_asset & asset, const char * path, const char * expected,
                  uint64_t expected_size, BCRYPT_ALG_HANDLE algorithm,
                  std::vector<unsigned char> & scratch) {
    const int n = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, nullptr, 0);
    if (n < 4) return false;
    std::wstring wide(static_cast<size_t>(n), L'\0');
    if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, wide.data(), n) != n)
        return false;
    // Reject UNC, device paths, relative names and non-fixed drives before
    // opening. Accept ordinary DOS paths and their extended-length form.
    const wchar_t * dos = wide.c_str();
    if (wide.rfind(L"\\\\?\\", 0) == 0) dos += 4;
    if (std::wcslen(dos) < 3 ||
        !((dos[0] >= L'A' && dos[0] <= L'Z') ||
          (dos[0] >= L'a' && dos[0] <= L'z')) ||
        dos[1] != L':' || dos[2] != L'\\') return false;
    wchar_t drive[] = {dos[0], L':', L'\\', L'\0'};
    if (GetDriveTypeW(drive) != DRIVE_FIXED) return false;
    asset.file = CreateFileW(wide.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr,
                            OPEN_EXISTING, FILE_FLAG_SEQUENTIAL_SCAN | FILE_FLAG_OPEN_REPARSE_POINT, nullptr);
    if (asset.file == INVALID_HANDLE_VALUE || GetFileType(asset.file) != FILE_TYPE_DISK) return false;
    BY_HANDLE_FILE_INFORMATION info{};
    if (!GetFileInformationByHandle(asset.file, &info) ||
        (info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT))) return false;
    LARGE_INTEGER size{};
    if (!GetFileSizeEx(asset.file, &size) || size.QuadPart <= 0 ||
        size.QuadPart > 3ll * 1024 * 1024 * 1024) return false;
    asset.bytes = static_cast<uint64_t>(size.QuadPart);
    if (expected_size && asset.bytes != expected_size) return false;

    // Pass the resolved DOS path to llama.cpp rather than the original
    // spelling. The controlled parent directory must remain trusted.
    const DWORD needed = GetFinalPathNameByHandleW(asset.file, nullptr, 0, FILE_NAME_NORMALIZED | VOLUME_NAME_DOS);
    if (!needed) return false;
    std::wstring resolved(needed, L'\0');
    const DWORD written = GetFinalPathNameByHandleW(asset.file, resolved.data(), needed,
                                                   FILE_NAME_NORMALIZED | VOLUME_NAME_DOS);
    if (!written || written >= needed) return false;
    // The resolved volume must also be a local fixed drive, not UNC.
    if (written < 7 || resolved.rfind(L"\\\\?\\", 0) != 0 ||
        !((resolved[4] >= L'A' && resolved[4] <= L'Z') ||
          (resolved[4] >= L'a' && resolved[4] <= L'z')) ||
        resolved[5] != L':' || resolved[6] != L'\\') return false;
    wchar_t resolved_drive[] = {resolved[4], L':', L'\\', L'\0'};
    if (GetDriveTypeW(resolved_drive) != DRIVE_FIXED) return false;
    const int utf8 = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, resolved.data(),
                                        static_cast<int>(written), nullptr, 0, nullptr, nullptr);
    if (utf8 <= 0) return false;
    asset.path.resize(static_cast<size_t>(utf8));
    if (WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, resolved.data(),
                           static_cast<int>(written), asset.path.data(), utf8, nullptr, nullptr) != utf8)
        return false;

    DWORD object_bytes = 0, returned = 0;
    if (BCryptGetProperty(algorithm, BCRYPT_OBJECT_LENGTH, reinterpret_cast<PUCHAR>(&object_bytes),
                          sizeof(object_bytes), &returned, 0) < 0 ||
        returned != sizeof(object_bytes) || !object_bytes) return false;
    std::vector<unsigned char> object(object_bytes);
    hash_state hash;
    if (BCryptCreateHash(algorithm, &hash.handle, object.data(), object_bytes, nullptr, 0, 0) < 0)
        return false;
    uint64_t total = 0;
    for (;;) {
        DWORD count = 0;
        if (!ReadFile(asset.file, scratch.data(), static_cast<DWORD>(scratch.size()), &count, nullptr))
            return false;
        if (!count) break;
        if (BCryptHashData(hash.handle, scratch.data(), count, 0) < 0) return false;
        total += count;
        if (total > asset.bytes) return false;
    }
    std::array<unsigned char, 32> digest{};
    if (total != asset.bytes || BCryptFinishHash(hash.handle, digest.data(),
                                                static_cast<ULONG>(digest.size()), 0) < 0) return false;
    constexpr char hex[] = "0123456789abcdef";
    for (size_t i = 0; i < digest.size(); ++i) {
        if (expected[2*i] != hex[digest[i] >> 4] || expected[2*i+1] != hex[digest[i] & 15])
            return false;
    }
    return true;
}

bool memory_admitted(ggml_backend_dev_t device, uint64_t required) {
    size_t available = 0, total = 0;
    ggml_backend_dev_memory(device, &available, &total);
    return total > 0 && available <= total && available >= required;
}

bool physical_pci_id(std::string_view id) {
    // CUDA may use a four- or eight-hex-digit PCI domain. Reject virtual
    // device suffixes, unknown identities and ambiguous visible devices.
    const size_t domain = id.size() == 12 ? 4 : id.size() == 16 ? 8 : 0;
    if (!domain || id[domain] != ':' || id[domain+3] != ':' || id[domain+6] != '.' ||
        id.back() < '0' || id.back() > '7') return false;
    for (size_t i = 0; i + 1 < id.size(); ++i) {
        if (i == domain || i == domain+3 || i == domain+6) continue;
        const char c = id[i];
        if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f'))) return false;
    }
    return true;
}

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
        if (*wanted && (!props.device_id || std::strcmp(props.device_id, wanted) != 0)) continue;
        if (!props.device_id || !physical_pci_id(props.device_id) || selected) return nullptr;
        selected = device;
    }
    return selected;
}

template<class T, void (*Free)(T *)>
using owned = std::unique_ptr<T, decltype(Free)>;
}

// One decoder implementation shared by ASR and translation. Each model owns
// its context and batch; no duplicate inference engine or router.
namespace {
struct decoder {
    owned<llama_model, llama_model_free> model{nullptr, llama_model_free};
    owned<llama_context, llama_free> context{nullptr, llama_free};
    owned<llama_batch_ext, llama_batch_ext_free> batch{nullptr, llama_batch_ext_free};
    bool poisoned = false;
};

int32_t open_decoder(decoder & d, const char * path, ggml_backend_dev_t device,
                     const char * expected_arch, bool audio_generation = false) {
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
    params.embeddings = audio_generation;
    // ASR/MT use raw logits and stateless host greedy selection only.
    // Keep backend sampling disabled explicitly, as in the pinned defaults.
    if (!audio_generation) {
        params.samplers = nullptr;
        params.n_samplers = 0;
    }
    d.context.reset(llama_init_from_model(d.model.get(), params));
    if (!d.context || !llama_get_memory(d.context.get()) ||
        llama_n_ctx_seq(d.context.get()) < k_context ||
        llama_n_batch(d.context.get()) < k_batch) return TI_CONTEXT_LOAD;
    if (!audio_generation) {
        d.batch.reset(llama_batch_ext_init(d.context.get()));
        if (!d.batch) return TI_CONTEXT_LOAD;
    }
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

struct ti_voice : decoder {
    owned<mtmd_context, mtmd_free> projector{nullptr, mtmd_free};
    owned<mtmd_helper_gen_audio, mtmd_helper_gen_audio_free> gen{nullptr, mtmd_helper_gen_audio_free};
    bool bound = false;
};

extern "C" uint32_t ti_cuda_abi_version(void) { return 7; }

static int32_t open_voice(const char * model_path, const char * projector_path,
                          ggml_backend_dev_t device, std::unique_ptr<ti_voice> & out) {
    auto voice = std::make_unique<ti_voice>();
    const auto status = open_decoder(*voice, model_path, device, "zonos3", true);
    if (status != TI_OK) return status;
    auto params = mtmd_context_params_default();
    params.use_gpu = true;
    params.device = device;
    params.n_threads = 4;
    params.print_timings = false;
    params.warmup = false;
    voice->projector.reset(mtmd_init_from_file(projector_path, voice->model.get(), params));
    if (!voice->projector || !mtmd_support_audio(voice->projector.get()) ||
        mtmd_get_audio_sample_rate(voice->projector.get()) != 16000) return TI_PROJECTOR_LOAD;
    const auto info = mtmd_gen_audio_get_info(voice->projector.get());
    if (info.type != MTMD_GEN_AUDIO_TYPE_ZONOS3 || info.sample_rate != 44100 ||
        info.n_channels != 1) return TI_PROJECTOR_LOAD;
    voice->gen.reset(mtmd_helper_gen_audio_init(voice->context.get(), voice->projector.get()));
    if (!voice->gen) return TI_CONTEXT_LOAD;
    out = std::move(voice);
    return TI_OK;
}

static int32_t open_asr(const char * model_path, const char * projector_path,
                        ggml_backend_dev_t device, ti_asr ** out) {
    if (!out) return TI_INVALID_INPUT;
    *out = nullptr;
    if (!model_path || !*model_path || !projector_path || !*projector_path ||
        !device) return TI_INVALID_INPUT;
    try {
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



namespace {
// The request owns its callback only for this synchronous invocation.
// Upstream's abort_callback is CPU-only; observe Stop at safe call
// boundaries. The per-utterance guard drains the decoder on every exit,
// including inference failure, exceptions and token-limit rejection.
struct request_control {
    ti_stop_check stop;
    const void * data;
    int32_t check() const {
        const auto status = stop(data);
        if (status == TI_OK) return TI_OK;
        return status == TI_DEADLINE ? TI_DEADLINE : TI_CANCELLED;
    }
};

struct clear_utterance {
    llama_context * context;
    ~clear_utterance() {
        llama_synchronize(context);
        llama_memory_clear(llama_get_memory(context), false);
    }
};

// Reuse the decoder's batch for text prefill, including text chunks around
// Qwen3-ASR audio. Multimodal embedding positions remain upstream-owned.
int32_t decode_text_prefill(decoder & d, const llama_token * tokens, size_t count,
                            llama_pos & next_pos, bool logits_last, const request_control & control) {
    if (!tokens && count != 0) return TI_INFERENCE;
    for (size_t i = 0; i < count;) {
        if (const auto stop = control.check(); stop != TI_OK) return stop;
        llama_batch_ext_clear(d.batch.get());
        const size_t end = i + std::min<size_t>(k_batch, count - i);
        for (; i < end; ++i) {
            const int idx = llama_batch_ext_add_token(d.batch.get(), 0, tokens[i]);
            const llama_pos pos = next_pos++;
            if (idx < 0 || !llama_batch_ext_set_pos(d.batch.get(), idx, &pos) ||
                !llama_batch_ext_set_output_logits(d.batch.get(), idx, logits_last && i + 1 == count))
                return TI_INFERENCE;
        }
        if (llama_process(d.context.get(), LLAMA_PROCESS_TYPE_DECODE, d.batch.get()) != 0)
            return TI_INFERENCE;
    }
    return TI_OK;
}

// Match pinned llama_sampler_greedy_apply's strict comparison in token-ID
// order: first maximum wins, including ties. No candidate vector or state.
constexpr llama_token greedy_token(const float * logits, int32_t count) {
    if (!logits || count <= 0) return LLAMA_TOKEN_NULL;
    llama_token selected = 0;
    for (llama_token i = 1; i < count; ++i) {
        if (logits[i] > logits[selected]) selected = i;
    }
    return selected;
}

constexpr float k_greedy_tie_fixture[] = {-3.0f, 2.0f, 2.0f, -1.0f};
constexpr float k_greedy_negative_fixture[] = {-9.0f, -4.0f, -2.0f};
static_assert(greedy_token(k_greedy_tie_fixture, 4) == 1);
static_assert(greedy_token(k_greedy_negative_fixture, 3) == 2);
static_assert(greedy_token(k_greedy_negative_fixture, 1) == 0);
static_assert(greedy_token(nullptr, 3) == LLAMA_TOKEN_NULL);
static_assert(greedy_token(k_greedy_negative_fixture, 0) == LLAMA_TOKEN_NULL);

// All forbidden MT markers begin with '<'. Ordinary speech needs one
// search, not a separate full-text scan for each marker.
constexpr bool contains_mt_marker(std::string_view text) {
    constexpr std::string_view markers[] = {
        u8"<｜hy_", "<think>", "</think>", "<answer>", "</answer>", "<tool_", "</tool_"
    };
    for (size_t pos = text.find('<'); pos != std::string_view::npos; pos = text.find('<', pos + 1)) {
        for (const auto marker : markers) {
            if (text.substr(pos, marker.size()) == marker) return true;
        }
    }
    return false;
}

static_assert(!contains_mt_marker("A < B; <thinking> is not a reserved marker."));
static_assert(contains_mt_marker(u8"prefix <｜hy_User｜>"));
static_assert(contains_mt_marker("prefix <<think>"));
static_assert(contains_mt_marker("</think>"));
static_assert(contains_mt_marker("<answer>"));
static_assert(contains_mt_marker("</answer>"));
static_assert(contains_mt_marker("<tool_call"));
static_assert(contains_mt_marker("</tool_call"));

// Pinned llama_token_to_piece returns -required_size without copying when
// capacity is insufficient. Preserve the original per-piece rejection
// before the complete-output limit, including the final byte boundary.
constexpr int32_t piece_status(int32_t len, size_t remaining) {
    if (len == 0 || len > k_max_token_piece_bytes || len < -k_max_token_piece_bytes)
        return TI_INCOMPLETE;
    if (len < 0) return static_cast<size_t>(-len) > remaining ? TI_OUTPUT_LIMIT : TI_INCOMPLETE;
    return static_cast<size_t>(len) > remaining ? TI_OUTPUT_LIMIT : TI_OK;
}
static_assert(piece_status(256, 256) == TI_OK);
static_assert(piece_status(-256, 255) == TI_OUTPUT_LIMIT);
static_assert(piece_status(-257, 255) == TI_INCOMPLETE);
static_assert(piece_status(0, 256) == TI_INCOMPLETE);
static_assert(piece_status(-1, 0) == TI_OUTPUT_LIMIT);

int32_t generate(decoder & d, llama_pos next_pos, uint32_t max_tokens,
                 llama_token required_eos, std::string_view echo,
                 const request_control & control, char * output, size_t * out_len) {
    auto * context = d.context.get();
    const auto * vocab = llama_model_get_vocab(d.model.get());
    if (!vocab) return TI_INFERENCE;
    const int32_t n_vocab = llama_vocab_n_tokens(vocab);
    if (n_vocab <= 0) return TI_INFERENCE;
    // Only publish once EOG is observed. No retry with a second model or
    // decoding mode, no partial transcript at the token limit.
    size_t used = 0;
    for (uint32_t i = 0; i < max_tokens; ++i) {
        if (const auto stop = control.check(); stop != TI_OK) return stop;
        // One synchronized raw-logits read; no backend sampler is installed.
        const llama_token token = greedy_token(llama_get_logits_ith(context, -1), n_vocab);
        if (token == LLAMA_TOKEN_NULL) return TI_INFERENCE;
        if (const auto stop = control.check(); stop != TI_OK) return stop;
        if (llama_vocab_is_eog(vocab, token)) {
            if (required_eos != LLAMA_TOKEN_NULL && token != required_eos)
                return TI_INCOMPLETE;
            const std::string_view transcript(output, used);
            const size_t first = transcript.find_first_not_of(" \r\n\t");
            if (first == std::string_view::npos) return TI_EMPTY;
            if (transcript.find('\0') != std::string_view::npos ||
                transcript.find("<asr_text>") != std::string_view::npos ||
                transcript.find("<|im_start|>") != std::string_view::npos ||
                transcript.find("<|im_end|>") != std::string_view::npos)
                return TI_INCOMPLETE;
            if (!echo.empty() &&
                (contains_mt_marker(transcript) || transcript.substr(first).rfind(echo, 0) == 0))
                return TI_INCOMPLETE;
            *out_len = used;
            return TI_OK;
        }
        if (llama_vocab_is_control(vocab, token)) return TI_INCOMPLETE;
        const size_t remaining = k_max_output_bytes - used;
        const auto capacity = static_cast<int32_t>(std::min<size_t>(remaining, k_max_token_piece_bytes));
        const int32_t len = llama_token_to_piece(vocab, token, output + used, capacity, 0, true);
        if (const auto status = piece_status(len, remaining); status != TI_OK) return status;
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
              const request_control & control, char * output, size_t * out_len) {
    auto * context = asr.context.get();
    auto * projector = asr.projector.get();
    llama_memory_clear(llama_get_memory(context), false);
    clear_utterance clear{context};

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
    // Pinned mtmd tokenization owns the preprocessed audio (including any
    // deferred samples); the copied input bitmap is not needed by decoding.
    bitmap.reset();

    if (const auto stop = control.check(); stop != TI_OK) return stop;
    const auto n_pos = mtmd_helper_get_n_pos(chunks.get());
    const auto n_tokens = mtmd_helper_get_n_tokens(chunks.get());
    const auto limit = llama_n_ctx_seq(context);
    if (n_pos <= 0 || n_tokens == 0 || limit <= k_asr_max_tokens ||
        static_cast<uint64_t>(n_pos) > limit - k_asr_max_tokens ||
        n_tokens > limit - k_asr_max_tokens) return TI_INPUT_LIMIT;
    llama_pos next_pos = 0;
    // Reuse text prefill; keep the pinned helper for multimodal embeddings
    // and M-RoPE positions. Each call consumes its input before returning.
    const size_t chunk_count = mtmd_input_chunks_size(chunks.get());
    for (size_t i = 0; i < chunk_count; ++i) {
        if (const auto stop = control.check(); stop != TI_OK) return stop;
        const auto * chunk = mtmd_input_chunks_get(chunks.get(), i);
        if (!chunk) return TI_INFERENCE;
        const bool logits_last = i + 1 == chunk_count;
        if (mtmd_input_chunk_get_type(chunk) == MTMD_INPUT_CHUNK_TYPE_TEXT) {
            size_t count = 0;
            const auto * tokens = mtmd_input_chunk_get_tokens_text(chunk, &count);
            const auto status = decode_text_prefill(asr, tokens, count, next_pos, logits_last, control);
            if (status != TI_OK) return status;
        } else if (mtmd_helper_eval_chunk_single(projector, context, chunk, next_pos, 0,
                                                  k_batch, logits_last, &next_pos) != 0) {
            return TI_INFERENCE;
        }
    }
    if (const auto stop = control.check(); stop != TI_OK) return stop;
    if (next_pos <= 0 || static_cast<uint64_t>(next_pos) > limit - k_asr_max_tokens)
        return TI_INFERENCE;

    // All chunk data has been consumed by prefill. The context owns the
    // decoded state; generation needs only its logits, not the mel windows.
    chunks.reset();
    return generate(asr, next_pos, k_asr_max_tokens, LLAMA_TOKEN_NULL, {}, control, output, out_len);
}
}

static int32_t transcribe_asr(ti_asr * asr, const float * pcm, size_t count,
                                      uint32_t language, const request_control & control,
                                      char * output, size_t output_cap, size_t * out_len) {
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
                                  control, output, out_len);
        if (status == TI_INFERENCE) asr->poisoned = true;
        return status;
    } catch (...) {
        asr->poisoned = true;
        *out_len = 0;
        return TI_INTERNAL;
    }
}

static int32_t open_mt(const char * model_path, ggml_backend_dev_t device, ti_mt ** out) {
    if (!out) return TI_INVALID_INPUT;
    *out = nullptr;
    if (!model_path || !*model_path || !device) return TI_INVALID_INPUT;
    try {
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



namespace {
bool mt_source_admitted(std::string_view source) {
    return !source.empty() && source.size() <= k_max_input_bytes &&
        source.find_first_not_of(" \r\n\t") != std::string_view::npos &&
        source.find('\0') == std::string_view::npos && !contains_mt_marker(source);
}

int32_t translate(ti_mt & mt, std::string_view source, const char * target,
                  const request_control & control, char * output, size_t * out_len) {
    auto * context = mt.context.get();
    const auto * vocab = llama_model_get_vocab(mt.model.get());
    if (!vocab) return TI_INFERENCE;
    // Tencent's single-user-turn template and default translation instruction.
    // No system message, past turns, user instructions or second model.
    constexpr std::string_view instruction_prefix = "Translate the following text into ";
    constexpr std::string_view instruction_suffix = ". Note that you should only output the translated result without any additional explanation:\n\n";
    std::string prompt;
    prompt.reserve(sizeof(k_mt_bos) + sizeof(k_mt_user) + sizeof(k_mt_assistant)
        + instruction_prefix.size() + std::strlen(target) + instruction_suffix.size() + source.size());
    prompt += k_mt_bos;
    prompt += k_mt_user;
    const size_t body_start = prompt.size();
    prompt += instruction_prefix;
    prompt += target;
    prompt += instruction_suffix;
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
    llama_pos next_pos = 0;
    const auto status = decode_text_prefill(mt, tokens.data(), static_cast<size_t>(n), next_pos, true, control);
    if (status != TI_OK) return status;
    return generate(mt, next_pos, budget, mt.eos,
                    std::string_view(prompt).substr(body_start, body_size), control, output, out_len);
}
}

static int32_t translate_mt(ti_mt * mt, const char * source, size_t source_len,
                                   uint32_t source_language, uint32_t target_language,
                                   const request_control & control, char * output, size_t output_cap, size_t * out_len) {
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
                                      control, output, out_len);
        if (status == TI_INFERENCE) mt->poisoned = true;
        return status;
    } catch (...) {
        mt->poisoned = true;
        *out_len = 0;
        return TI_INTERNAL;
    }
}


namespace {
// Upstream helper state is reset even on cancellation or failed inference.
// The model and its allocation remain resident on the same owning thread.
struct clear_voice_utterance {
    ti_voice & voice;
    ~clear_voice_utterance() {
        llama_synchronize(voice.context.get());
        mtmd_helper_gen_audio_reset(voice.gen.get());
        llama_memory_clear(llama_get_memory(voice.context.get()), false);
    }
};

int32_t bind_voice(ti_voice & voice, const float * reference, size_t reference_len,
                   const request_control & control) {
    // A failed or cancelled replacement cannot leave the old voice selectable.
    voice.bound = false;
    if (mtmd_helper_gen_audio_bind_speaker(voice.gen.get(), nullptr) != 0) return TI_INFERENCE;
    if (!reference || reference_len < k_voice_min_ref_samples ||
        reference_len > k_voice_max_ref_samples) return TI_INVALID_INPUT;
    if (const auto status = control.check(); status != TI_OK) return status;
    bool signal = false;
    for (size_t i = 0; i < reference_len; ++i) {
        const float sample = reference[i];
        if (!std::isfinite(sample) || std::abs(sample) > 1.0f) return TI_INVALID_INPUT;
        signal |= std::abs(sample) >= 0.0001f;
    }
    if (!signal) return TI_INVALID_INPUT;
    owned<mtmd_bitmap, mtmd_bitmap_free> bitmap(
        mtmd_bitmap_init_from_audio(reference_len, reference), mtmd_bitmap_free);
    if (!bitmap) return TI_INTERNAL;
    if (mtmd_helper_gen_audio_bind_speaker(voice.gen.get(), bitmap.get()) != 0) return TI_INFERENCE;
    if (const auto status = control.check(); status != TI_OK) {
        mtmd_helper_gen_audio_bind_speaker(voice.gen.get(), nullptr);
        return status;
    }
    voice.bound = true;
    return TI_OK;
}

int32_t synthesize_voice(ti_voice & voice, const char * normalized, size_t text_len,
                         const request_control & control, float * output,
                         size_t output_cap, size_t * out_len) {
    if (!voice.bound || !normalized || !text_len || text_len > 4000 ||
        !output || output_cap < k_voice_max_samples) return TI_INVALID_INPUT;
    const std::string_view text(normalized, text_len);
    if (text.find('\0') != std::string_view::npos ||
        text.find("<|") != std::string_view::npos) return TI_INVALID_INPUT;
    if (const auto status = control.check(); status != TI_OK) return status;
    clear_voice_utterance clear{voice};
    auto inp = mtmd_helper_gen_audio_inp_default();
    inp.prompt = normalized;
    inp.prompt_len = text_len;
    inp.lang = "en";
    inp.out_type = MTMD_HELPER_GEN_AUDIO_OUTTYPE_PCM;
    inp.stream = false;
    const auto prepared = mtmd_helper_gen_audio_set_input(voice.gen.get(), &inp);
    if (prepared != 0) return prepared == 2 ? TI_INVALID_INPUT : TI_INFERENCE;
    for (;;) {
        if (const auto status = control.check(); status != TI_OK) return status;
        const auto remaining = mtmd_helper_gen_audio_step_prompt(
            voice.gen.get(), static_cast<int32_t>(llama_n_batch(voice.context.get())));
        if (remaining < 0) return TI_INFERENCE;
        if (remaining == 0) break;
    }
    bool stopped = false;
    for (uint32_t i = 0; i < k_voice_max_frames; ++i) {
        if (const auto status = control.check(); status != TI_OK) return status;
        const float * next = nullptr;
        // ZONOS3's pinned pipeline samples its own codebook heads and ignores
        // the caller's token and hidden state; no extra sampler or copy.
        if (mtmd_helper_gen_audio_step_gen(voice.gen.get(), LLAMA_TOKEN_NULL,
                                          nullptr, &next, &stopped) != 0) return TI_INFERENCE;
        if (stopped) break;
        if (!next) return TI_INFERENCE;
    }
    if (!stopped) return TI_INCOMPLETE;
    if (const auto status = control.check(); status != TI_OK) return status;
    int32_t rate = 0;
    int64_t samples = 0;
    const char * data = nullptr;
    size_t bytes = 0;
    if (mtmd_helper_gen_audio_get_output(voice.gen.get(), &rate, &data, &bytes, &samples) != 0)
        return TI_INFERENCE;
    if (rate != 44100 || !data || samples <= 0 ||
        static_cast<uint64_t>(samples) > k_voice_max_samples ||
        static_cast<uint64_t>(samples) > output_cap ||
        bytes != static_cast<size_t>(samples) * sizeof(float)) return TI_OUTPUT_LIMIT;
    if (const auto status = control.check(); status != TI_OK) return status;
    std::memcpy(output, data, bytes);
    *out_len = static_cast<size_t>(samples);
    return TI_OK;
}
}

// The only public owner; optional candidate voice assets join the same
// runtime atomically. A failed open releases all acquired resources.
struct ti_runtime {
    runtime_lease lease;
    std::array<locked_asset, 5> assets;
    std::unique_ptr<ti_asr> asr;
    std::unique_ptr<ti_mt> mt;
    std::unique_ptr<ti_voice> voice;
    bool poisoned = false;
    void invalidate() {
        poisoned = true;
        voice.reset();
        mt.reset();
        asr.reset();
    }
};

extern "C" int32_t ti_runtime_open(const char * asr_path, const char * projector_path,
                                   const char * mt_path, const char * voice_path,
                                   const char * voice_projector_path, const char * device_id,
                                   char * actual_device, size_t device_cap, ti_runtime ** out) {
    if (out) *out = nullptr;
    if (actual_device && device_cap) *actual_device = '\0';
    if (!out || !actual_device || device_cap < 32 ||
        !asr_path || !*asr_path || !projector_path || !*projector_path ||
        !mt_path || !*mt_path || !device_id ||
        (voice_path == nullptr) != (voice_projector_path == nullptr) ||
        (voice_path && (!*voice_path || !*voice_projector_path))) return TI_INVALID_INPUT;
    try {
        auto runtime = std::make_unique<ti_runtime>();
        if (!runtime->lease.acquire()) return TI_BUSY;
        const auto device = initialize_cuda(device_id);
        if (!device) return TI_CUDA_DEVICE;
        ggml_backend_dev_props device_props{};
        ggml_backend_dev_get_props(device, &device_props);
        if (!device_props.device_id || !physical_pci_id(device_props.device_id)) return TI_CUDA_DEVICE;
        const std::string selected_id(device_props.device_id);
        // Hash every selected asset before any model allocation, one shared 1 MiB buffer.
        {
            hash_algorithm algorithm;
            if (BCryptOpenAlgorithmProvider(&algorithm.handle, BCRYPT_SHA256_ALGORITHM, nullptr, 0) < 0)
                return TI_ARTIFACT;
            std::vector<unsigned char> scratch(1024 * 1024);
            if (!verify_asset(runtime->assets[0], asr_path,
                              "cd5f5ce9d7c9c03ff47d54d7f2726e63a39e61e3001640c5d08b6e4d3aed1e6d",
                              2165035232ull, algorithm.handle, scratch) ||
                !verify_asset(runtime->assets[1], projector_path,
                              "24eaf9a96fa94fe36ed0c1de68d44cccdb3f2229ee614e5b1b5bab951bc37883",
                              355709376ull, algorithm.handle, scratch) ||
                !verify_asset(runtime->assets[2], mt_path,
                              "d98fe604dec1f28f58f80d7d560f7177e584d3b8e5835862687660e5ff97cb40",
                              0, algorithm.handle, scratch)) return TI_ARTIFACT;
            if (voice_path &&
                (!verify_asset(runtime->assets[3], voice_path,
                               "60568efc88e2f9231f4f5c061e246421f575986642d72ab9ae68f77d6cf76be4",
                               854726208ull, algorithm.handle, scratch) ||
                 !verify_asset(runtime->assets[4], voice_projector_path,
                               "aeeb92325b4b6927bef656390b78e1246148065f947d8b59c2663557a809414d",
                               1142502880ull, algorithm.handle, scratch))) return TI_ARTIFACT;
        }
        // Device-global free bytes are a coarse admission signal, not app
        // peak VRAM or residency proof. Check only at lifecycle boundaries.
        uint64_t required = k_reserve_bytes;
        for (const auto & asset : runtime->assets) required += asset.bytes;
        if (!memory_admitted(device, required)) return TI_VRAM;
        ti_asr * speech = nullptr;
        const auto speech_status = open_asr(runtime->assets[0].path.c_str(),
                                            runtime->assets[1].path.c_str(), device, &speech);
        if (speech_status != TI_OK) return speech_status;
        runtime->asr.reset(speech);
        const auto voice_bytes = runtime->assets[3].bytes + runtime->assets[4].bytes;
        if (!memory_admitted(device, runtime->assets[2].bytes + voice_bytes + k_reserve_bytes)) return TI_VRAM;
        ti_mt * mt = nullptr;
        const auto mt_status = open_mt(runtime->assets[2].path.c_str(), device, &mt);
        if (mt_status != TI_OK) return mt_status;
        runtime->mt.reset(mt);
        if (!memory_admitted(device, voice_bytes + k_reserve_bytes)) return TI_VRAM;
        if (voice_path) {
            const auto voice_status = open_voice(runtime->assets[3].path.c_str(),
                runtime->assets[4].path.c_str(), device, runtime->voice);
            if (voice_status != TI_OK) return voice_status;
            if (!memory_admitted(device, k_reserve_bytes)) return TI_VRAM;
        }
        std::memcpy(actual_device, selected_id.c_str(), selected_id.size() + 1);
        *out = runtime.release();
        return TI_OK;
    } catch (...) { return TI_INTERNAL; }
}

extern "C" void ti_runtime_close(ti_runtime * runtime) { delete runtime; }

namespace {
int32_t finish_stage(ti_runtime & runtime, int32_t status,
                     const request_control & control, size_t * out_len) {
    if (status == TI_INFERENCE || status == TI_INTERNAL || status == TI_POISONED)
        runtime.invalidate();
    // Recheck after inference cleanup, even after a complete EOG. Do not
    // turn a real backend failure into an ordinary cancellation.
    if (status == TI_OK) status = control.check();
    if (status != TI_OK) *out_len = 0;
    return status;
}
}

extern "C" int32_t ti_runtime_transcribe(ti_runtime * runtime, const float * pcm, size_t count,
                                         uint32_t language, ti_stop_check stop, const void * stop_data,
                                         char * output, size_t output_cap, size_t * out_len) {
    if (!out_len) return TI_INVALID_INPUT;
    *out_len = 0;
    if (!runtime || !stop || !stop_data) return TI_INVALID_INPUT;
    if (runtime->poisoned) return TI_POISONED;
    const request_control control{stop, stop_data};
    if (const auto status = control.check(); status != TI_OK) return status;
    return finish_stage(*runtime, transcribe_asr(runtime->asr.get(), pcm, count, language,
                                                 control, output, output_cap, out_len), control, out_len);
}

extern "C" int32_t ti_runtime_translate(ti_runtime * runtime, const char * source, size_t source_len,
                                        uint32_t source_language, uint32_t target_language,
                                        ti_stop_check stop, const void * stop_data,
                                        char * output, size_t output_cap, size_t * out_len) {
    if (!out_len) return TI_INVALID_INPUT;
    *out_len = 0;
    if (!runtime || !stop || !stop_data) return TI_INVALID_INPUT;
    if (runtime->poisoned) return TI_POISONED;
    const request_control control{stop, stop_data};
    if (const auto status = control.check(); status != TI_OK) return status;
    return finish_stage(*runtime, translate_mt(runtime->mt.get(), source, source_len,
                                              source_language, target_language,
                                              control, output, output_cap, out_len), control, out_len);
}

extern "C" int32_t ti_runtime_bind_voice(ti_runtime * runtime, const float * reference,
                                         size_t reference_len, ti_stop_check stop, const void * stop_data) {
    if (!runtime || !stop || !stop_data) return TI_INVALID_INPUT;
    if (runtime->poisoned) return TI_POISONED;
    if (!runtime->voice) return TI_INVALID_INPUT;
    const request_control control{stop, stop_data};
    int32_t status;
    try {
        status = bind_voice(*runtime->voice, reference, reference_len, control);
    } catch (...) { status = TI_INTERNAL; }
    size_t unused = 0;
    status = finish_stage(*runtime, status, control, &unused);
    if (status != TI_OK && runtime->voice && runtime->voice->bound) {
        runtime->voice->bound = false;
        mtmd_helper_gen_audio_bind_speaker(runtime->voice->gen.get(), nullptr);
    }
    return status;
}

extern "C" int32_t ti_runtime_synthesize(ti_runtime * runtime, const char * normalized,
                                         size_t text_len,
                                         ti_stop_check stop, const void * stop_data,
                                         float * output, size_t output_cap, size_t * out_len) {
    if (!out_len) return TI_INVALID_INPUT;
    *out_len = 0;
    if (!runtime || !stop || !stop_data) return TI_INVALID_INPUT;
    if (runtime->poisoned) return TI_POISONED;
    if (!runtime->voice) return TI_INVALID_INPUT;
    const request_control control{stop, stop_data};
    if (const auto status = control.check(); status != TI_OK) return status;
    int32_t status;
    try {
        status = synthesize_voice(*runtime->voice, normalized, text_len,
                                  control, output, output_cap, out_len);
    } catch (...) {
        status = TI_INTERNAL;
    }
    return finish_stage(*runtime, status, control, out_len);
}
