#pragma once
// Only an installed, identity-checked local service may forward these requests
// to the existing privileged capture-only kernel control device.
#include "../Source/Main/capture_control_protocol.h"

#define TI_BROKER_PIPE_NAME L"\\\\.\\pipe\\TranslateITMicBrokerV1"
#define TI_BROKER_VERSION 1u
#define TI_BROKER_MAGIC 0x52424954u  // little-endian "TIBR"
#define TI_BROKER_MAX_PAYLOAD (32u + (2u * TI_CAPTURE_MAX_FRAMES_PER_WRITE))

enum TiBrokerOperation : USHORT {
    TiBrokerBegin = 1,
    TiBrokerWrite = 2,
    TiBrokerFlush = 3,
    TiBrokerEnd = 4,
    TiBrokerStatus = 5,
};

typedef struct _TI_BROKER_REQUEST {
    ULONG Magic;
    USHORT Version;
    USHORT Operation;
    ULONG PayloadBytes;
    ULONG Reserved;
} TI_BROKER_REQUEST;

typedef struct _TI_BROKER_REPLY {
    ULONG Magic;
    USHORT Version;
    USHORT Operation;
    ULONG Status;       // explicit Win32 error code; zero means success
    ULONG PayloadBytes; // nonzero only for STATUS
} TI_BROKER_REPLY;

static_assert(sizeof(TI_BROKER_REQUEST) == 16, "fixed broker request layout");
static_assert(sizeof(TI_BROKER_REPLY) == 16, "fixed broker reply layout");
static_assert(TI_BROKER_MAX_PAYLOAD == 1952, "kernel 20ms mono PCM16 max");
static_assert(offsetof(TI_CAPTURE_WRITE, Data) == 32, "shared user/kernel ABI");
