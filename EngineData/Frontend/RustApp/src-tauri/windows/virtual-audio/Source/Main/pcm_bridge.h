#pragma once
// TranslateIT native WaveRT render -> capture PCM bridge. No user-mode IOCTL.
#include <ntddk.h>

// The only accepted stream format is 48 kHz stereo signed PCM16.
void TranslateITBridgeInitialize();
void TranslateITBridgeReset();
void TranslateITBridgeWrite(_In_reads_bytes_(byteCount) const UCHAR* source, _In_ ULONG byteCount);
void TranslateITBridgeRead(_Out_writes_bytes_(byteCount) UCHAR* destination, _In_ ULONG byteCount);
