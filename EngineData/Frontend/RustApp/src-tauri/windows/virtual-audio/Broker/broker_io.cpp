#include "broker_io.h"
#include <chrono>

BrokerPipeIo::BrokerPipeIo(HANDLE pipe, HANDLE stop)
    : pipe_(pipe), stop_(stop), event_(CreateEventW(nullptr, TRUE, FALSE, nullptr)) {
    if (!event_) error_ = GetLastError();
}
BrokerPipeIo::~BrokerPipeIo() {
    if (event_) CloseHandle(event_);
}

bool BrokerPipeIo::Wait(OVERLAPPED& overlapped, DWORD timeoutMs, DWORD& transferred) {
    const HANDLE handles[] = { stop_, event_ };
    DWORD result = WaitForMultipleObjects(2, handles, FALSE, timeoutMs);
    if (result == WAIT_OBJECT_0 + 1
        && GetOverlappedResult(pipe_, &overlapped, &transferred, FALSE))
        return true;

    error_ = result == WAIT_OBJECT_0 ? ERROR_OPERATION_ABORTED :
        result == WAIT_TIMEOUT ? ERROR_TIMEOUT : GetLastError();
    CancelIoEx(pipe_, &overlapped);
    DWORD ignored = 0;
    // Wait for cancellation/completion before the stack OVERLAPPED and
    // buffers are destroyed. No pending kernel write can outlive this call.
    GetOverlappedResult(pipe_, &overlapped, &ignored, TRUE);
    return false;
}

bool BrokerPipeIo::Connect() {
    if (!event_) return false;
    OVERLAPPED op = {};
    op.hEvent = event_;
    ResetEvent(event_);
    if (ConnectNamedPipe(pipe_, &op)) return true;
    const DWORD error = GetLastError();
    if (error == ERROR_PIPE_CONNECTED) return true;
    if (error != ERROR_IO_PENDING) {
        error_ = error;
        return false;
    }
    DWORD transferred = 0;
    return Wait(op, INFINITE, transferred);
}

bool BrokerPipeIo::Transfer(bool reading, void* buffer, DWORD bytes,
                            DWORD timeoutMs, DWORD& transferred) {
    if (!event_ || !buffer || !bytes) return false;
    OVERLAPPED op = {};
    op.hEvent = event_;
    ResetEvent(event_);
    transferred = 0;
    BOOL ok = reading
        ? ReadFile(pipe_, buffer, bytes, &transferred, &op)
        : WriteFile(pipe_, buffer, bytes, &transferred, &op);
    if (ok) return transferred != 0;
    const DWORD error = GetLastError();
    if (error != ERROR_IO_PENDING) {
        error_ = error;
        return false;
    }
    return Wait(op, timeoutMs, transferred) && transferred != 0;
}

bool BrokerPipeIo::ReadExactly(void* destination, DWORD bytes, DWORD timeoutMs) {
    auto* out = static_cast<UCHAR*>(destination);
    const auto deadline = std::chrono::steady_clock::now()
        + std::chrono::milliseconds(timeoutMs == INFINITE ? 0 : timeoutMs);
    while (bytes) {
        if (WaitForSingleObject(stop_, 0) != WAIT_TIMEOUT) {
            error_ = ERROR_OPERATION_ABORTED;
            return false;
        }
        DWORD remaining = timeoutMs;
        if (timeoutMs != INFINITE) {
            const auto now = std::chrono::steady_clock::now();
            if (now >= deadline) { error_ = ERROR_TIMEOUT; return false; }
            remaining = DWORD(std::chrono::duration_cast<std::chrono::milliseconds>(
                deadline - now).count());
            if (!remaining) remaining = 1;
        }
        DWORD got = 0;
        if (!Transfer(true, out, bytes, remaining, got)) return false;
        bytes -= got;
        out += got;
    }
    return true;
}

bool BrokerPipeIo::WriteExactly(const void* source, DWORD bytes, DWORD timeoutMs) {
    auto* in = const_cast<UCHAR*>(static_cast<const UCHAR*>(source));
    const auto deadline = std::chrono::steady_clock::now()
        + std::chrono::milliseconds(timeoutMs == INFINITE ? 0 : timeoutMs);
    while (bytes) {
        if (WaitForSingleObject(stop_, 0) != WAIT_TIMEOUT) {
            error_ = ERROR_OPERATION_ABORTED;
            return false;
        }
        DWORD remaining = timeoutMs;
        if (timeoutMs != INFINITE) {
            const auto now = std::chrono::steady_clock::now();
            if (now >= deadline) { error_ = ERROR_TIMEOUT; return false; }
            remaining = DWORD(std::chrono::duration_cast<std::chrono::milliseconds>(
                deadline - now).count());
            if (!remaining) remaining = 1;
        }
        DWORD got = 0;
        if (!Transfer(false, in, bytes, remaining, got)) return false;
        bytes -= got;
        in += got;
    }
    return true;
}
