#pragma once
#include <Windows.h>

// One outstanding overlapped operation per connected pipe, with its own
// manual-reset event. STOP always wins against blocked Connect/Read/Write.
class BrokerPipeIo final {
public:
    BrokerPipeIo(HANDLE pipe, HANDLE stop);
    ~BrokerPipeIo();
    BrokerPipeIo(const BrokerPipeIo&) = delete;
    BrokerPipeIo& operator=(const BrokerPipeIo&) = delete;

    bool Ready() const { return event_ != nullptr; }
    // Peer HANDLE is held by BrokerClientIdentity until this connection ends.
    // Never close it here; this is a borrowed liveness notification.
    void WatchPeerProcess(HANDLE peer) { peer_ = peer; }
    bool Connect();
    bool ReadExactly(void* destination, DWORD bytes, DWORD timeoutMs);
    bool WriteExactly(const void* source, DWORD bytes, DWORD timeoutMs);
    DWORD LastError() const { return error_; }
private:
    bool Transfer(bool reading, void* buffer, DWORD bytes, DWORD timeoutMs, DWORD& transferred);
    bool Wait(OVERLAPPED& overlapped, DWORD timeoutMs, DWORD& transferred);
    HANDLE pipe_;
    HANDLE stop_;
    HANDLE event_;
    HANDLE peer_ = nullptr;
    DWORD error_ = ERROR_SUCCESS;
};
