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

// Candidate ABI v7: one serialized owner for ASR, MT and optional candidate voice.
typedef struct ti_runtime ti_runtime;
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
    TI_INTERNAL = 12,
    TI_ARTIFACT = 13,
    TI_VRAM = 14,
    TI_BUSY = 15,
    TI_CANCELLED = 16,
    TI_DEADLINE = 17
};

// Synchronous, non-retained callback: return TI_OK, TI_CANCELLED or
// TI_DEADLINE. Caller owns the data until the request returns. A stop
// does not interrupt an already-running CUDA kernel; never close concurrently.
typedef int32_t (*ti_stop_check)(const void * data);

TI_EXPORT uint32_t ti_cuda_abi_version(void);
// All selected exact pinned GGUF files are verified before model load. Both
// voice paths must be null (existing ASR/MT only) or both present (candidate). Paths
// are UTF-8 in a controlled directory. A nonempty device_id selects an exact
// CUDA PCI ID; empty admits only one visible CUDA GPU with a physical PCI ID.
// actual_device (cap >=32) is empty on failure, NUL-terminated on success.
// On failure *out is null. On success the caller owns the one runtime.
TI_EXPORT int32_t ti_runtime_open(const char * asr_path, const char * projector_path,
                                   const char * mt_path, const char * voice_path,
                                   const char * voice_projector_path, const char * device_id,
                                   char * actual_device, size_t device_cap,
                                   ti_runtime ** out);
TI_EXPORT void ti_runtime_close(ti_runtime * runtime);
// 16 kHz mono normalized F32, 300ms..60s, language 1=id / 2=en.
// Output cap >=16000 bytes. Only success authorizes *out_len bytes;
// failure has zero length, and scratch contents are unspecified.
TI_EXPORT int32_t ti_runtime_transcribe(ti_runtime * runtime, const float * pcm, size_t count,
                                        uint32_t language, ti_stop_check stop, const void * stop_data,
                                        char * output,
                                        size_t output_cap, size_t * out_len);
// UTF-8 <=8000 bytes. Different directions only, same output contract.
TI_EXPORT int32_t ti_runtime_translate(ti_runtime * runtime, const char * source, size_t source_len,
                                       uint32_t source_language, uint32_t target_language,
                                       ti_stop_check stop, const void * stop_data,
                                       char * output, size_t output_cap, size_t * out_len);

// Replace the sole selected reference: 16 kHz mono F32 (3..30s).
// Failure/cancellation revokes the old binding. No reference is retained by caller.
TI_EXPORT int32_t ti_runtime_bind_voice(ti_runtime * runtime, const float * reference,
                                         size_t reference_len, ti_stop_check stop, const void * stop_data);
// Candidate TTS: normalized English UTF-8 <=4000 bytes, prior successful binding.
// Complete mono F32 44.1 kHz output <=60s.
// Caller provides >=2646000 samples; only success authorizes *out_len samples.
TI_EXPORT int32_t ti_runtime_synthesize(ti_runtime * runtime, const char * normalized,
                                         size_t text_len,
                                         ti_stop_check stop, const void * stop_data,
                                         float * output, size_t output_cap, size_t * out_len);

#ifdef __cplusplus
}
#endif
