#pragma once
#include <stddef.h>
#include <stdint.h>

#ifdef _WIN32
#define TI_EXPORT __declspec(dllexport)
#else
#define TI_EXPORT
#endif

#ifdef __cplusplus
extern "C" {
#endif

// Candidate ABI v2. All memory remains owned by its allocator; the
// caller supplies input/output buffers. Each mutable handle is serialized.
typedef struct ti_asr ti_asr;
typedef struct ti_mt ti_mt;
enum ti_status {
    TI_OK = 0,
    TI_INVALID_INPUT = 1,
    TI_CUDA_DEVICE = 2,
    TI_MODEL_LOAD = 3,
    TI_CONTEXT_LOAD = 4,
    TI_PROJECTOR_LOAD = 5,
    TI_INPUT_LIMIT = 6,
    TI_INFERENCE = 7,
    TI_INCOMPLETE = 8,
    TI_EMPTY = 9,
    TI_OUTPUT_LIMIT = 10,
    TI_POISONED = 11,
    TI_INTERNAL = 12
};

TI_EXPORT uint32_t ti_cuda_abi_version(void);
// device_id is the exact CUDA PCI identifier from ggml_backend_dev_props.
// Model paths are UTF-8, verified by the caller before opening. On failure
// *out remains null. Loading alone is not proof of inference or residency.
TI_EXPORT int32_t ti_asr_open(const char * model_path, const char * projector_path,
                             const char * device_id, ti_asr ** out);
TI_EXPORT void ti_asr_close(ti_asr * asr);
// 16 kHz mono normalized f32, 300ms..60s, language 1=id / 2=en.
// On success *out_len is the byte length of a complete UTF-8 transcript.
// On failure *out_len is zero and scratch bytes are unspecified; never
// consume them. Caller supplies at least 16000 bytes.
TI_EXPORT int32_t ti_asr_transcribe(ti_asr * asr, const float * pcm, size_t count,
                                   uint32_t language, char * output,
                                   size_t output_cap, size_t * out_len);

// Same exact CUDA device as ASR. No model or direction router.
TI_EXPORT int32_t ti_mt_open(const char * model_path, const char * device_id, ti_mt ** out);
TI_EXPORT void ti_mt_close(ti_mt * mt);
// Input is UTF-8, at most 8000 bytes; Rust additionally enforces 2000 scalars.
// Language 1=id / 2=en, different directions only. Same output contract as ASR.
TI_EXPORT int32_t ti_mt_translate(ti_mt * mt, const char * source, size_t source_len,
                                   uint32_t source_language, uint32_t target_language,
                                   char * output, size_t output_cap, size_t * out_len);

#ifdef __cplusplus
}
#endif
