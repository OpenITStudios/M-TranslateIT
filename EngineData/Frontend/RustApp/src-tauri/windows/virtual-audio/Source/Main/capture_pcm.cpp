// Capture-only driver PCM transport. No speaker endpoint or sine generator.
// Nonpaged bounded memory; one privileged producer, multiple independent readers.
#include "definitions.h"
#include "capture_pcm.h"

namespace {
static_assert(TI_CAPTURE_FIFO_FRAMES > TI_CAPTURE_MAX_FRAMES_PER_WRITE, "bounded producer");
static_assert(TI_CAPTURE_SAMPLE_BITS == 16 && TI_CAPTURE_CHANNELS == 1, "mono PCM contract");

SHORT g_samples[TI_CAPTURE_FIFO_FRAMES] = {};
KSPIN_LOCK g_lock;
BOOLEAN g_initialized = FALSE;
BOOLEAN g_online = FALSE;
ULONGLONG g_deviceEpoch = 1;
PFILE_OBJECT g_writer = nullptr; // Compared only; CLEANUP/END clears before close.
BOOLEAN g_started = FALSE;
ULONGLONG g_generation = 0;
ULONGLONG g_sourceEpoch = 0;
ULONGLONG g_lastGeneration = 0;
ULONGLONG g_lastSourceEpoch = 0;
ULONGLONG g_expectedSequence = 0;
ULONGLONG g_written = 0;       // Logical frames since last admission/flush.
ULONGLONG g_serial = 1;        // Reader invalidation epoch, not a producer token.
ULONGLONG g_acceptedFrames = 0;
ULONGLONG g_overwrittenFrames = 0;
ULONGLONG g_underrunFrames = 0;

void invalidateLocked(BOOLEAN releaseWriter) {
    g_started = FALSE;
    g_written = 0;
    g_expectedSequence = 0;
    g_generation = 0;
    g_sourceEpoch = 0;
    if (releaseWriter) {
        g_writer = nullptr;
        g_lastGeneration = 0;
        g_lastSourceEpoch = 0;
    }
    RtlZeroMemory(g_samples, sizeof(g_samples));
    ++g_serial;
    if (!g_serial) ++g_serial;
}
}

void TranslateITCaptureInitialize() {
    KeInitializeSpinLock(&g_lock);
    g_writer = nullptr;
    g_started = FALSE;
    g_generation = g_sourceEpoch = g_expectedSequence = g_written = 0;
    g_lastGeneration = g_lastSourceEpoch = 0;
    g_online = FALSE;
    g_deviceEpoch = 1;
    g_serial = 1;
    g_acceptedFrames = g_overwrittenFrames = g_underrunFrames = 0;
    RtlZeroMemory(g_samples, sizeof(g_samples));
    g_initialized = TRUE;
}

void TranslateITCaptureSetOnline() {
    if (!g_initialized) return;
    KIRQL irql;
    KeAcquireSpinLock(&g_lock, &irql);
    invalidateLocked(TRUE);
    if (++g_deviceEpoch == 0) ++g_deviceEpoch;
    g_online = TRUE;
    KeReleaseSpinLock(&g_lock, irql);
}

void TranslateITCaptureInvalidateAll() {
    if (!g_initialized) return;
    KIRQL irql;
    KeAcquireSpinLock(&g_lock, &irql);
    g_online = FALSE;
    invalidateLocked(TRUE);
    if (++g_deviceEpoch == 0) ++g_deviceEpoch;
    KeReleaseSpinLock(&g_lock, irql);
}

ULONGLONG TranslateITCaptureDeviceEpoch() {
    if (!g_initialized) return 0;
    KIRQL irql;
    KeAcquireSpinLock(&g_lock, &irql);
    const ULONGLONG value = g_online ? g_deviceEpoch : 0;
    KeReleaseSpinLock(&g_lock, irql);
    return value;
}

void TranslateITCaptureCloseWriter(PFILE_OBJECT file) {
    if (!file || !g_initialized) return;
    KIRQL irql;
    KeAcquireSpinLock(&g_lock, &irql);
    if (g_writer == file) invalidateLocked(TRUE);
    KeReleaseSpinLock(&g_lock, irql);
}

NTSTATUS TranslateITCaptureBegin(PFILE_OBJECT file, const TI_CAPTURE_BEGIN* request) {
    if (!g_initialized) return STATUS_DEVICE_NOT_READY;
    if (!file || !request || request->Version != TI_CAPTURE_PROTOCOL_VERSION
        || request->SampleRateHz != TI_CAPTURE_RATE_HZ
        || request->Channels != TI_CAPTURE_CHANNELS
        || request->BitsPerSample != TI_CAPTURE_SAMPLE_BITS
        || !request->Generation || !request->SourceEpoch) return STATUS_INVALID_PARAMETER;

    KIRQL irql;
    KeAcquireSpinLock(&g_lock, &irql);
    NTSTATUS status = STATUS_SUCCESS;
    if (!g_online) {
        status = STATUS_DEVICE_NOT_READY;
    } else if ((g_writer && g_writer != file) || g_started) {
        status = STATUS_DEVICE_BUSY;
    } else if (g_writer == file &&
        (request->Generation < g_lastGeneration ||
         (request->Generation == g_lastGeneration && request->SourceEpoch <= g_lastSourceEpoch))) {
        // FLUSH/Mute must advance the source epoch; old in-flight writes
        // may never be admitted by replaying the same BEGIN identity.
        status = STATUS_INVALID_DEVICE_STATE;
    } else {
        // Every new BEGIN is a fresh producer epoch. A FLUSH cannot preserve
        // old buffered speech; two independent capture streams never share cursors.
        invalidateLocked(FALSE);
        g_writer = file;
        g_generation = request->Generation;
        g_sourceEpoch = request->SourceEpoch;
        g_lastGeneration = request->Generation;
        g_lastSourceEpoch = request->SourceEpoch;
        g_expectedSequence = 1;
        g_started = TRUE;
    }
    KeReleaseSpinLock(&g_lock, irql);
    return status;
}

NTSTATUS TranslateITCaptureWrite(PFILE_OBJECT file, const TI_CAPTURE_WRITE* request, ULONG inputBytes) {
    constexpr ULONG headerBytes = FIELD_OFFSET(TI_CAPTURE_WRITE, Data);
    if (!g_initialized) return STATUS_DEVICE_NOT_READY;
    if (!file || !request || inputBytes < headerBytes
        || request->Version != TI_CAPTURE_PROTOCOL_VERSION
        || !request->DataBytes
        || request->DataBytes > 2u * TI_CAPTURE_MAX_FRAMES_PER_WRITE
        || (request->DataBytes % sizeof(SHORT)) != 0
        || request->DataBytes != inputBytes - headerBytes
        || !request->Sequence) return STATUS_INVALID_PARAMETER;

    KIRQL irql;
    KeAcquireSpinLock(&g_lock, &irql);
    NTSTATUS status = STATUS_SUCCESS;
    if (!g_online) {
        status = STATUS_DEVICE_NOT_READY;
    } else if (!g_started || g_writer != file) {
        status = STATUS_ACCESS_DENIED;
    } else if (request->Generation != g_generation
        || request->SourceEpoch != g_sourceEpoch
        || request->Sequence != g_expectedSequence) {
        status = STATUS_INVALID_DEVICE_STATE;
    } else if (g_expectedSequence == MAXULONGLONG) {
        status = STATUS_INTEGER_OVERFLOW;
    } else {
        const ULONG frames = request->DataBytes / sizeof(SHORT);
        // Count overwritten history (not assumed lost receiver frames).
        const ULONGLONG before = g_written > TI_CAPTURE_FIFO_FRAMES
            ? g_written - TI_CAPTURE_FIFO_FRAMES : 0;
        const ULONGLONG afterCount = g_written + frames;
        const ULONGLONG after = afterCount > TI_CAPTURE_FIFO_FRAMES
            ? afterCount - TI_CAPTURE_FIFO_FRAMES : 0;
        g_overwrittenFrames += after - before;
        for (ULONG i = 0; i < frames; ++i) {
            SHORT sample = 0;
            RtlCopyMemory(&sample, request->Data + i * sizeof(SHORT), sizeof(SHORT));
            g_samples[(g_written + i) % TI_CAPTURE_FIFO_FRAMES] = sample;
        }
        g_written = afterCount;
        g_acceptedFrames += frames;
        ++g_expectedSequence;
    }
    KeReleaseSpinLock(&g_lock, irql);
    return status;
}

NTSTATUS TranslateITCaptureRevoke(PFILE_OBJECT file, const TI_CAPTURE_REVOKE* request, BOOLEAN releaseWriter) {
    if (!g_initialized) return STATUS_DEVICE_NOT_READY;
    if (!file || !request || request->Version != TI_CAPTURE_PROTOCOL_VERSION
        || request->Reserved != 0 || !request->Generation || !request->SourceEpoch) {
        return STATUS_INVALID_PARAMETER;
    }
    KIRQL irql;
    KeAcquireSpinLock(&g_lock, &irql);
    NTSTATUS status = STATUS_SUCCESS;
    if (!g_online) {
        status = STATUS_DEVICE_NOT_READY;
    } else if (g_writer != file || !g_started) {
        status = STATUS_ACCESS_DENIED;
    } else if (request->Generation != g_generation || request->SourceEpoch != g_sourceEpoch) {
        status = STATUS_INVALID_DEVICE_STATE;
    } else {
        // The kernel buffer cannot recall frames an application already read
        // into its own WASAPI/meeting buffers.
        invalidateLocked(releaseWriter);
    }
    KeReleaseSpinLock(&g_lock, irql);
    return status;
}

void TranslateITCaptureStatus(TI_CAPTURE_STATUS* report) {
    if (!report) return;
    RtlZeroMemory(report, sizeof(*report));
    report->Version = TI_CAPTURE_PROTOCOL_VERSION;
    report->CapacityFrames = TI_CAPTURE_FIFO_FRAMES;
    if (!g_initialized) return;
    KIRQL irql;
    KeAcquireSpinLock(&g_lock, &irql);
    report->ProducerActive = g_started ? 1u : 0u;
    report->BufferedFrames = (ULONG)(g_written < TI_CAPTURE_FIFO_FRAMES
        ? g_written : TI_CAPTURE_FIFO_FRAMES);
    report->AcceptedFrames = g_acceptedFrames;
    report->OverwrittenFrames = g_overwrittenFrames;
    report->UnderrunFrames = g_underrunFrames;
    KeReleaseSpinLock(&g_lock, irql);
}

void TranslateITCaptureRead(UCHAR* destination, ULONG bytes,
    ULONGLONG* nextFrame, ULONGLONG* observedSerial) {
    if (!destination || !bytes) return;
    RtlZeroMemory(destination, bytes); // No owner -> explicit silence.
    if (!g_initialized || !nextFrame || !observedSerial
        || (bytes % sizeof(SHORT)) != 0) return;
    KIRQL irql;
    KeAcquireSpinLock(&g_lock, &irql);
    if (*observedSerial == 0) {
        // First read by a newly opened Windows microphone stream: do not
        // replay audio that was already buffered before this listener joined.
        *nextFrame = g_written;
        *observedSerial = g_serial;
    } else if (*observedSerial != g_serial) {
        // This listener was already open before BEGIN/FLUSH. A new producer
        // epoch starts at logical frame zero; do not discard its first packet.
        *nextFrame = 0;
        *observedSerial = g_serial;
    }
    if (g_started && g_online) {
        const ULONGLONG oldest = g_written > TI_CAPTURE_FIFO_FRAMES
            ? g_written - TI_CAPTURE_FIFO_FRAMES : 0;
        if (*nextFrame < oldest) *nextFrame = oldest;
        const ULONG requested = bytes / sizeof(SHORT);
        const ULONGLONG available = *nextFrame <= g_written
            ? g_written - *nextFrame : 0;
        const ULONG copied = (ULONG)(available < requested ? available : requested);
        for (ULONG i = 0; i < copied; ++i) {
            const SHORT sample = g_samples[(*nextFrame + i) % TI_CAPTURE_FIFO_FRAMES];
            RtlCopyMemory(destination + i * sizeof(SHORT), &sample, sizeof(SHORT));
        }
        *nextFrame += copied;
        g_underrunFrames += requested - copied;
    }
    KeReleaseSpinLock(&g_lock, irql);
}
