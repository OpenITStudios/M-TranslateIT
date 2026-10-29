#pragma once
// Canonical kernel PCM queue and producer/session ownership.
// The WaveRT capture pin owns each independent reader cursor.
#include "capture_control_protocol.h"

void TranslateITCaptureInitialize();
void TranslateITCaptureSetOnline();
ULONGLONG TranslateITCaptureDeviceEpoch();
void TranslateITCaptureInvalidateAll();
void TranslateITCaptureCloseWriter(_In_ PFILE_OBJECT file);
NTSTATUS TranslateITCaptureBegin(_In_ PFILE_OBJECT file, _In_ const TI_CAPTURE_BEGIN* request);
NTSTATUS TranslateITCaptureWrite(_In_ PFILE_OBJECT file, _In_ const TI_CAPTURE_WRITE* request, _In_ ULONG inputBytes);
NTSTATUS TranslateITCaptureRevoke(_In_ PFILE_OBJECT file, _In_ const TI_CAPTURE_REVOKE* request, _In_ BOOLEAN releaseWriter);
void TranslateITCaptureStatus(_Out_ TI_CAPTURE_STATUS* report);

// Read performs no allocation or I/O and is safe at DISPATCH_LEVEL.
// Each capture stream must provide its own independent cursor and serial.
void TranslateITCaptureRead(
    _Out_writes_bytes_(bytes) UCHAR* destination,
    _In_ ULONG bytes,
    _Inout_ ULONGLONG* nextFrame,
    _Inout_ ULONGLONG* observedSerial);
