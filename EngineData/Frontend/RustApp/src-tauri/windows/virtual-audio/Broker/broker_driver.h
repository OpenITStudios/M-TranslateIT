#pragma once
#include "broker_protocol.h"

// Service process owns this handle ONLY after an authenticated pipe client.
// A second service/backend audio scheduler is forbidden.
class BrokerDriver final {
public:
    BrokerDriver() = default;
    ~BrokerDriver();
    BrokerDriver(const BrokerDriver&) = delete;
    BrokerDriver& operator=(const BrokerDriver&) = delete;

    DWORD Open();
    DWORD Send(USHORT operation, const UCHAR* data, ULONG dataBytes,
               UCHAR* reply, ULONG* replyBytes);
    void Close();
private:
    HANDLE handle_ = INVALID_HANDLE_VALUE;
    bool producerActive_ = false;
};
