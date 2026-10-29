#pragma once
// Canonical kernel PCM queue and producer/session ownership.
// The WaveRT capture pin owns each independent reader cursor.
#include "capture_control_protocol.h"

void TranslateITCaptureInitialize();
void TranslateITCaptureSetOnline();
ULONGLONG TranslateITCaptureDeviceEpoch();
void TranslateITCaptureInvalidateAll();
void TranslateITCaptureCloseWriter(_In_ PFILE_OBJECT file);
NTSTATUS TranslateITCaptureBegin(_In_ PFILE_OBJECT file, _In_ ULONGLONG openedDeviceEpoch, _In_ const TI_CAPTURE_BEGIN* request);
NTSTATUS TranslateITCaptureWrite(_In_ PFILE_OBJECT file, _In_ ULONGLONG openedDeviceEpoch, _In_ const TI_CAPTURE_WRITE* request, _In_ ULONG inputBytes);
NTSTATUS TranslateITCaptureRevoke(_In_ PFILE_OBJECT file, _In_ ULONGLONG openedDeviceEpoch, _In_ const TI_CAPTURE_REVOKE* request, _In_ BOOLEAN releaseWriter);
NTSTATUS TranslateITCaptureStatus(_In_ ULONGLONG openedDeviceEpoch, _Out_ TI_CAPTURE_STATUS* report);

// IOCTL calls compare openedDeviceEpoch with the current PnP epoch WHILE
// holding the same PCM spinlock that gates writer/session changes. A separate
// pre-check in a dispatch routine is insufficient during Stop/Start.
//
// Read performs no allocation or I/O and is safe at DISPATCH_LEVEL.
// Each capture stream must provide its own independent cursor and serial.
// A WaveRT stream that missed more than one full DMA cycle must not replay
// old Meeting PCM as if it were a new capture period.
void TranslateITCaptureDiscardStaleFrames(
    _Inout_ ULONGLONG* nextFrame,
    _Inout_ ULONGLONG* observedSerial);

void TranslateITCaptureRead(
    _Out_writes_bytes_(bytes) UCHAR* destination,
    _In_ ULONG bytes,
    _Inout_ ULONGLONG* nextFrame,
    _Inout_ ULONGLONG* observedSerial);
