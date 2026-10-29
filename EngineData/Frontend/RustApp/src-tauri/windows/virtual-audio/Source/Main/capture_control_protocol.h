#pragma once
// TranslateIT capture-only, privileged, buffered IOCTL ABI v1.
// All offsets and sizes are fixed. Native Rust client encodes little-endian bytes.
// The security descriptor of the control device restricts writes to elevated
// Administrators/SYSTEM while the broker/normal-user authorization is pending.
#include <ntddk.h>

#define TI_CAPTURE_PROTOCOL_VERSION 1u
#define TI_CAPTURE_RATE_HZ 48000u
#define TI_CAPTURE_CHANNELS 1u
#define TI_CAPTURE_SAMPLE_BITS 16u
#define TI_CAPTURE_MAX_FRAMES_PER_WRITE 960u // 20 ms at 48 kHz
#define TI_CAPTURE_FIFO_FRAMES 9600u          // 200 ms, per-reader cursors

#define TI_CAPTURE_IOCTL_BEGIN  CTL_CODE(FILE_DEVICE_UNKNOWN, 0x900, METHOD_BUFFERED, FILE_WRITE_DATA)
#define TI_CAPTURE_IOCTL_WRITE  CTL_CODE(FILE_DEVICE_UNKNOWN, 0x901, METHOD_BUFFERED, FILE_WRITE_DATA)
#define TI_CAPTURE_IOCTL_FLUSH  CTL_CODE(FILE_DEVICE_UNKNOWN, 0x902, METHOD_BUFFERED, FILE_WRITE_DATA)
#define TI_CAPTURE_IOCTL_END    CTL_CODE(FILE_DEVICE_UNKNOWN, 0x903, METHOD_BUFFERED, FILE_WRITE_DATA)
#define TI_CAPTURE_IOCTL_STATUS CTL_CODE(FILE_DEVICE_UNKNOWN, 0x904, METHOD_BUFFERED, FILE_READ_DATA)

typedef struct _TI_CAPTURE_BEGIN {
    ULONG Version;
    ULONG SampleRateHz;
    ULONG Channels;
    ULONG BitsPerSample;
    ULONGLONG Generation;
    ULONGLONG SourceEpoch;
} TI_CAPTURE_BEGIN;

typedef struct _TI_CAPTURE_WRITE {
    ULONG Version;
    ULONG DataBytes;
    ULONGLONG Generation;
    ULONGLONG SourceEpoch;
    ULONGLONG Sequence;
    UCHAR Data[ANYSIZE_ARRAY];
} TI_CAPTURE_WRITE;

typedef struct _TI_CAPTURE_REVOKE {
    ULONG Version;
    ULONG Reserved;
    ULONGLONG Generation;
    ULONGLONG SourceEpoch;
} TI_CAPTURE_REVOKE;

typedef struct _TI_CAPTURE_STATUS {
    ULONG Version;
    ULONG ProducerActive;
    ULONG CapacityFrames;
    ULONG BufferedFrames;
    ULONGLONG AcceptedFrames;
    ULONGLONG OverwrittenFrames;
    ULONGLONG UnderrunFrames;
} TI_CAPTURE_STATUS;

C_ASSERT(sizeof(TI_CAPTURE_BEGIN) == 32);
C_ASSERT(FIELD_OFFSET(TI_CAPTURE_WRITE, Data) == 32);
C_ASSERT(sizeof(TI_CAPTURE_REVOKE) == 24);
C_ASSERT(sizeof(TI_CAPTURE_STATUS) == 40);
