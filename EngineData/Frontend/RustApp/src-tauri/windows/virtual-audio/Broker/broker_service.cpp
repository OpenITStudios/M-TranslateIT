// Native capture broker (SYSTEM service, no audio generation, no AI runtime).
// Only transports validated, paced frames between an authenticated local
// TranslateIT.exe and the already-secured kernel control device.
#include "broker_identity.h"
#include "broker_driver.h"
#include "broker_io.h"
#include <sddl.h>
#include <array>
#include <chrono>
#include <cstring>
#include <mutex>
#include <cstddef>

#pragma comment(lib, "advapi32.lib")

namespace {
constexpr wchar_t kServiceName[] = L"TranslateITMicBroker";
constexpr wchar_t kPipeAcl[] =
    L"D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;0x0012008B;;;AU)";
constexpr DWORD kMaxPayload = TI_BROKER_MAX_PAYLOAD;
constexpr DWORD kFrameBytes = 2u;
constexpr DWORD kSampleRate = TI_CAPTURE_RATE_HZ;
constexpr ULONGLONG kBurstFrames = TI_CAPTURE_MAX_FRAMES_PER_WRITE * 2ull;

SERVICE_STATUS_HANDLE g_serviceHandle = nullptr;
SERVICE_STATUS g_status = {};
HANDLE g_stop = nullptr;
HANDLE g_worker = nullptr;
std::mutex g_statusLock;

void reportLocked(DWORD state, DWORD error, DWORD waitHint = 0) {
    if (!g_serviceHandle) return;
    g_status.dwServiceType = SERVICE_WIN32_OWN_PROCESS;
    g_status.dwCurrentState = state;
    g_status.dwWin32ExitCode = error;
    g_status.dwControlsAccepted = state == SERVICE_RUNNING ? SERVICE_ACCEPT_STOP : 0;
    g_status.dwCheckPoint = state == SERVICE_START_PENDING
        || state == SERVICE_STOP_PENDING ? 1 : 0;
    g_status.dwWaitHint = waitHint;
    SetServiceStatus(g_serviceHandle, &g_status);
}
void report(DWORD state, DWORD error, DWORD waitHint = 0) {
    const std::lock_guard<std::mutex> guard(g_statusLock);
    reportLocked(state, error, waitHint);
}

struct ClosePipe {
    HANDLE h = INVALID_HANDLE_VALUE;
    ~ClosePipe() {
        if (h != INVALID_HANDLE_VALUE) {
            DisconnectNamedPipe(h);
            CloseHandle(h);
        }
    }
};

bool quitting() { return WaitForSingleObject(g_stop, 0) == WAIT_OBJECT_0; }

class ProducerClock final {
public:
    void Reset() {
        updated_ = std::chrono::steady_clock::now();
        tokens_ = kBurstFrames;
        active_ = true;
    }
    void Revoke() { active_ = false; tokens_ = 0; }
    bool CanWrite(ULONG count) {
        if (!active_ || !count || count > TI_CAPTURE_MAX_FRAMES_PER_WRITE) return false;
        const auto now = std::chrono::steady_clock::now();
        const auto elapsedUs = std::chrono::duration_cast<std::chrono::microseconds>(now - updated_).count();
        if (elapsedUs < 0) return false;
        // Token bucket: permits short scheduling jitter, never an unbounded
        // backlog. A legitimate new utterance may follow many seconds of
        // silence without being mislabeled as stale.
        const ULONGLONG credit = (ULONGLONG(elapsedUs) * kSampleRate) / 1000000ull;
        tokens_ = credit >= kBurstFrames - tokens_ ? kBurstFrames : tokens_ + credit;
        updated_ = now;
        if (count > tokens_) return false;
        return true;
    }
    void Accepted(ULONG count) { tokens_ -= count; }
private:
    std::chrono::steady_clock::time_point updated_{};
    ULONGLONG tokens_ = 0;
    bool active_ = false;
};

DWORD handleClient(BrokerPipeIo& io, HANDLE pipe) {
    TI_BROKER_REQUEST request = {};
    if (!io.ReadExactly(&request, sizeof(request), 3000)) return ERROR_BROKEN_PIPE;

    // Windows named-pipe impersonation only identifies the LAST READ request.
    // Do not open the privileged kernel control handle before this succeeds.
    BrokerClientIdentity client;
    const DWORD authentication = AuthenticatePipeClient(pipe, client);
    if (authentication != ERROR_SUCCESS) return authentication;
    io.WatchPeerProcess(client.ProcessHandle());

    BrokerDriver driver;
    DWORD status = driver.Open();
    if (status != ERROR_SUCCESS) return status;
    ProducerClock clock;
    bool firstRequest = true;
    while (!quitting()) {
        if (!client.IsLive(pipe)) return ERROR_ACCESS_DENIED;
        if (!firstRequest && !io.ReadExactly(&request, sizeof(request), INFINITE)) return ERROR_BROKEN_PIPE;
        firstRequest = false;
        if (request.Magic != TI_BROKER_MAGIC || request.Version != TI_BROKER_VERSION
            || request.Reserved != 0 || request.PayloadBytes > kMaxPayload
            || request.Operation < TiBrokerBegin || request.Operation > TiBrokerStatus)
            return ERROR_INVALID_DATA;

        if (!client.IsLive(pipe)) return ERROR_ACCESS_DENIED;
        std::array<UCHAR, TI_BROKER_MAX_PAYLOAD> payload = {};
        if (!io.ReadExactly(payload.data(), request.PayloadBytes, 2500)) return ERROR_BROKEN_PIPE;

        // The broker is a bounded forwarder, never the session owner.
        // Kernel owns the generation/epoch/file/sequence authority; this
        // process enforces frame rate and closes the handle on any disconnect.
        if (request.Operation == TiBrokerWrite) {
            if (request.PayloadBytes < FIELD_OFFSET(TI_CAPTURE_WRITE, Data))
                return ERROR_INVALID_DATA;
            ULONG version = 0, declaredBytes = 0;
            static_assert(offsetof(TI_CAPTURE_WRITE, Data) == 32, "wire header");
            // The receive array has byte alignment; never reinterpret it as
            // a potentially misaligned 64-bit ABI structure.
            memcpy(&version, payload.data(), sizeof(version));
            memcpy(&declaredBytes, payload.data() + sizeof(version), sizeof(declaredBytes));
            const ULONG bytes = request.PayloadBytes - FIELD_OFFSET(TI_CAPTURE_WRITE, Data);
            if (version != TI_CAPTURE_PROTOCOL_VERSION
                || declaredBytes != bytes || !bytes || bytes % kFrameBytes != 0
                || !clock.CanWrite(bytes / kFrameBytes))
                return ERROR_INVALID_DATA;
        }
        // Revoke the privileged forwarding path immediately if the process
        // that passed the original image/SID/session check has terminated.
        if (!client.IsLive(pipe)) return ERROR_ACCESS_DENIED;
        std::array<UCHAR, sizeof(TI_CAPTURE_STATUS)> response = {};
        ULONG responseBytes = ULONG(response.size());
        status = driver.Send(request.Operation, payload.data(), request.PayloadBytes,
                             response.data(), &responseBytes);
        if (status == ERROR_SUCCESS) {
            if (request.Operation == TiBrokerBegin) clock.Reset();
            if (request.Operation == TiBrokerWrite)
                clock.Accepted((request.PayloadBytes - FIELD_OFFSET(TI_CAPTURE_WRITE, Data)) / kFrameBytes);
            if (request.Operation == TiBrokerFlush || request.Operation == TiBrokerEnd)
                clock.Revoke();
        }
        TI_BROKER_REPLY reply = { TI_BROKER_MAGIC, TI_BROKER_VERSION,
                                  request.Operation, status,
                                  status == ERROR_SUCCESS && request.Operation == TiBrokerStatus
                                      ? responseBytes : 0u };
        if (!io.WriteExactly(&reply, sizeof(reply), 2500)) return ERROR_BROKEN_PIPE;
        if (reply.PayloadBytes != 0
            && !io.WriteExactly(response.data(), reply.PayloadBytes, 2500)) return ERROR_BROKEN_PIPE;

        // Invalid sequence/epoch, wrong user input or driver disconnect is
        // fatal to this pipe session; no retry and no silent source switch.
        if (status != ERROR_SUCCESS) return status;
        if (request.Operation == TiBrokerEnd) return ERROR_SUCCESS;
    }
    return ERROR_OPERATION_ABORTED;
}

DWORD WINAPI worker(LPVOID) {
    PSECURITY_DESCRIPTOR descriptor = nullptr;
    if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(
            kPipeAcl, SDDL_REVISION_1, &descriptor, nullptr))
        return GetLastError();
    SECURITY_ATTRIBUTES attributes = { sizeof(attributes), descriptor, FALSE };
    DWORD result = ERROR_SUCCESS;
    while (!quitting()) {
        ClosePipe pipe;
        pipe.h = CreateNamedPipeW(TI_BROKER_PIPE_NAME,
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1, 4096, 4096, 0, &attributes);
        if (pipe.h == INVALID_HANDLE_VALUE) {
            result = GetLastError();
            break; // fail closed on a name collision or ACL failure
        }
        BrokerPipeIo io(pipe.h, g_stop);
        if (!io.Ready()) { result = io.LastError(); break; }
        if (!io.Connect()) {
            const DWORD error = io.LastError();
            if (quitting()) break;
            if (error == ERROR_NO_DATA || error == ERROR_BROKEN_PIPE) continue;
            result = error;
            break;
        }
        // A denied or disconnected client drops its kernel FILE_OBJECT.
        // The next connection is reauthenticated and has a fresh pipe.
        (void)handleClient(io, pipe.h);
    }
    LocalFree(descriptor);
    return result;
}

DWORD WINAPI onControl(DWORD control, DWORD, LPVOID, LPVOID) {
    if (control != SERVICE_CONTROL_STOP) return ERROR_CALL_NOT_IMPLEMENTED;
    const std::lock_guard<std::mutex> guard(g_statusLock);
    reportLocked(SERVICE_STOP_PENDING, ERROR_SUCCESS, 5000);
    if (g_stop) SetEvent(g_stop);
    return ERROR_SUCCESS;
}

void WINAPI serviceMain(DWORD, LPWSTR*) {
    g_serviceHandle = RegisterServiceCtrlHandlerExW(kServiceName, onControl, nullptr);
    if (!g_serviceHandle) return;
    report(SERVICE_START_PENDING, ERROR_SUCCESS, 3000);
    g_stop = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    if (!g_stop) {
        report(SERVICE_STOPPED, GetLastError());
        return;
    }
    g_worker = CreateThread(nullptr, 0, worker, nullptr, 0, nullptr);
    if (!g_worker) {
        DWORD error = GetLastError();
        CloseHandle(g_stop); g_stop = nullptr;
        report(SERVICE_STOPPED, error);
        return;
    }
    report(SERVICE_RUNNING, ERROR_SUCCESS);
    WaitForSingleObject(g_worker, INFINITE);
    DWORD result = ERROR_SUCCESS;
    GetExitCodeThread(g_worker, &result);
    {
        const std::lock_guard<std::mutex> guard(g_statusLock);
        CloseHandle(g_worker); g_worker = nullptr;
        CloseHandle(g_stop); g_stop = nullptr;
        reportLocked(SERVICE_STOPPED, result);
    }
}
} // namespace

int wmain() {
    SERVICE_TABLE_ENTRYW services[] = {
        { const_cast<LPWSTR>(kServiceName), serviceMain },
        { nullptr, nullptr },
    };
    return StartServiceCtrlDispatcherW(services) ? 0 : 1;
}
