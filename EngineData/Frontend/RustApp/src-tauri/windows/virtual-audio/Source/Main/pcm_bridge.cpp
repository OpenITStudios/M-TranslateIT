// TranslateIT additions to Microsoft's MS-PL Windows SimpleAudioSample.
// Full original MS-PL license is at ../../LICENSE.MS-PL.
#include "definitions.h"
#include "pcm_bridge.h"

// 200ms @ 48kHz * stereo * 2 bytes. Bounded nonpaged memory, silent on
// underflow, oldest-frame drop under overload; never log or persist PCM.
namespace {
constexpr ULONG kFrameBytes = 4;
constexpr ULONG kBufferBytes = 38400;
static_assert(kBufferBytes % kFrameBytes == 0, "PCM16 stereo alignment");
UCHAR g_pcmBytes[kBufferBytes] = {};
KSPIN_LOCK g_pcmLock;
ULONG g_read = 0;
ULONG g_write = 0;
ULONG g_available = 0;
BOOLEAN g_initialized = FALSE;

ULONG minimum(ULONG a, ULONG b) {
    return a < b ? a : b;
}
}

void TranslateITBridgeInitialize() {
    KeInitializeSpinLock(&g_pcmLock);
    g_read = 0;
    g_write = 0;
    g_available = 0;
    RtlZeroMemory(g_pcmBytes, sizeof(g_pcmBytes));
    g_initialized = TRUE;
}

void TranslateITBridgeReset() {
    if (!g_initialized) return;
    KIRQL irql;
    KeAcquireSpinLock(&g_pcmLock, &irql);
    g_read = 0;
    g_write = 0;
    g_available = 0;
    KeReleaseSpinLock(&g_pcmLock, irql);
}

void TranslateITBridgeWrite(const UCHAR* source, ULONG byteCount) {
    if (!g_initialized || !source || byteCount == 0 || byteCount % kFrameBytes != 0) return;
    // A single callback may never monopolize more than the fixed 200ms FIFO.
    if (byteCount > kBufferBytes) {
        source += byteCount - kBufferBytes;
        byteCount = kBufferBytes;
    }
    KIRQL irql;
    KeAcquireSpinLock(&g_pcmLock, &irql);
    ULONG overflow = byteCount > kBufferBytes - g_available
        ? byteCount - (kBufferBytes - g_available) : 0;
    if (overflow) {
        g_read = (g_read + overflow) % kBufferBytes;
        g_available -= overflow;
    }
    ULONG first = minimum(byteCount, kBufferBytes - g_write);
    RtlCopyMemory(g_pcmBytes + g_write, source, first);
    if (byteCount > first) RtlCopyMemory(g_pcmBytes, source + first, byteCount - first);
    g_write = (g_write + byteCount) % kBufferBytes;
    g_available += byteCount;
    KeReleaseSpinLock(&g_pcmLock, irql);
}

void TranslateITBridgeRead(UCHAR* destination, ULONG byteCount) {
    if (!destination || byteCount == 0) return;
    // Empty capture buffers are always silence, never previous session PCM.
    RtlZeroMemory(destination, byteCount);
    if (!g_initialized || byteCount % kFrameBytes != 0) return;
    KIRQL irql;
    KeAcquireSpinLock(&g_pcmLock, &irql);
    ULONG copied = minimum(byteCount, g_available);
    ULONG first = minimum(copied, kBufferBytes - g_read);
    if (first) RtlCopyMemory(destination, g_pcmBytes + g_read, first);
    if (copied > first) RtlCopyMemory(destination + first, g_pcmBytes, copied - first);
    g_read = (g_read + copied) % kBufferBytes;
    g_available -= copied;
    KeReleaseSpinLock(&g_pcmLock, irql);
}
