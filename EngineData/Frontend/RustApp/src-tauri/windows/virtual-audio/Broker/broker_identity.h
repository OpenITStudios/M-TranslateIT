#pragma once
#include <Windows.h>

// Hold the actual authenticated process HANDLE so a dead client cannot leave
// a still-open pipe writing PCM through a duplicated/inherited handle.
// The pipe has no authority to mint Meeting generations or source epochs.
class BrokerClientIdentity final {
public:
    BrokerClientIdentity() = default;
    ~BrokerClientIdentity();
    BrokerClientIdentity(const BrokerClientIdentity&) = delete;
    BrokerClientIdentity& operator=(const BrokerClientIdentity&) = delete;

    HANDLE ProcessHandle() const { return process_; }
    bool IsLive(HANDLE pipe) const;
    // Revalidate the security token of the LAST pipe read before forwarding
    // any privileged request; the original creator PID is not sender proof.
    bool SenderTokenMatches(HANDLE pipe) const;
private:
    friend DWORD AuthenticatePipeClient(HANDLE pipe, BrokerClientIdentity& result);
    // Retain the same approved on-disk image for the lifetime of this pipe.
    // Read-only sharing is not attestation of a process's loaded image.
    HANDLE approvedImage_ = INVALID_HANDLE_VALUE;
    HANDLE process_ = nullptr;
    ULONG pid_ = 0;
    ULONG session_ = 0;
};

DWORD AuthenticatePipeClient(HANDLE pipe, BrokerClientIdentity& result);
