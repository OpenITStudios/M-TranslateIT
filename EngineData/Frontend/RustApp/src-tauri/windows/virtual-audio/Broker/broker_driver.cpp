#include "broker_driver.h"

namespace {
const wchar_t kKernelControl[] = L"\\\\.\\TranslateITMicControl";
}

BrokerDriver::~BrokerDriver() { Close(); }

DWORD BrokerDriver::Open() {
    if (handle_ != INVALID_HANDLE_VALUE) return ERROR_ALREADY_EXISTS;
    handle_ = CreateFileW(kKernelControl, GENERIC_READ | GENERIC_WRITE,
                          0, nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (handle_ == INVALID_HANDLE_VALUE) return GetLastError();
    producerActive_ = false;
    return ERROR_SUCCESS;
}

DWORD BrokerDriver::Send(USHORT operation, const UCHAR* data, ULONG bytes,
                         UCHAR* reply, ULONG* replyBytes) {
    if (handle_ == INVALID_HANDLE_VALUE) return ERROR_INVALID_HANDLE;
    if (bytes > TI_BROKER_MAX_PAYLOAD || (!data && bytes != 0)) return ERROR_INVALID_PARAMETER;
    DWORD code = 0;
    DWORD resultLen = 0;
    switch (operation) {
    case TiBrokerBegin:
        if (producerActive_ || bytes != sizeof(TI_CAPTURE_BEGIN))
            return ERROR_INVALID_PARAMETER;
        code = TI_CAPTURE_IOCTL_BEGIN;
        break;
    case TiBrokerWrite:
        if (!producerActive_ || bytes < FIELD_OFFSET(TI_CAPTURE_WRITE, Data))
            return ERROR_INVALID_PARAMETER;
        code = TI_CAPTURE_IOCTL_WRITE;
        break;
    case TiBrokerFlush:
        if (!producerActive_ || bytes != sizeof(TI_CAPTURE_REVOKE))
            return ERROR_INVALID_PARAMETER;
        code = TI_CAPTURE_IOCTL_FLUSH;
        break;
    case TiBrokerEnd:
        if (!producerActive_ || bytes != sizeof(TI_CAPTURE_REVOKE))
            return ERROR_INVALID_PARAMETER;
        code = TI_CAPTURE_IOCTL_END;
        break;
    case TiBrokerStatus:
        if (bytes != 0 || !reply || !replyBytes || *replyBytes < sizeof(TI_CAPTURE_STATUS))
            return ERROR_INVALID_PARAMETER;
        code = TI_CAPTURE_IOCTL_STATUS;
        break;
    default:
        return ERROR_INVALID_FUNCTION;
    }

    const DWORD outputSize = operation == TiBrokerStatus ? sizeof(TI_CAPTURE_STATUS) : 0;
    if (!DeviceIoControl(handle_, code, const_cast<UCHAR*>(data), bytes,
                         outputSize ? reply : nullptr, outputSize,
                         &resultLen, nullptr))
        return GetLastError();

    if (resultLen != outputSize) return ERROR_INVALID_DATA;
    if (operation == TiBrokerBegin) producerActive_ = true;
    if (operation == TiBrokerFlush || operation == TiBrokerEnd) producerActive_ = false;
    if (replyBytes) *replyBytes = resultLen;
    return ERROR_SUCCESS;
}

void BrokerDriver::Close() {
    if (handle_ == INVALID_HANDLE_VALUE) return;
    // Kernel IRP_MJ_CLEANUP revokes the exact producer FILE_OBJECT and zeros
    // the queue. Never claim an explicit Stop ACK just from process cleanup.
    CloseHandle(handle_);
    handle_ = INVALID_HANDLE_VALUE;
    producerActive_ = false;
}
