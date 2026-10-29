// An authenticated process is required BEFORE the privileged kernel handle
// may be opened. No per-user private meeting data or credentials are stored.
#include "broker_identity.h"
#include <bcrypt.h>
#include <sddl.h>
#include <array>
#include <string>
#include <vector>

#pragma comment(lib, "bcrypt.lib")
#pragma comment(lib, "advapi32.lib")

namespace {
constexpr wchar_t kApprovedDesktopName[] = L"translateit.exe";
constexpr wchar_t kApprovedRegistryKey[] = L"SOFTWARE\\OpenITStudios\\TranslateIT";
constexpr wchar_t kApprovedHashValue[] = L"ApprovedDesktopSha256";
constexpr ULONG kHashBytes = 32;

struct ScopedHandle {
    HANDLE handle = nullptr;
    ~ScopedHandle() {
        if (handle && handle != INVALID_HANDLE_VALUE) CloseHandle(handle);
    }
    ScopedHandle(const ScopedHandle&) = delete;
    ScopedHandle& operator=(const ScopedHandle&) = delete;
    ScopedHandle() = default;
};

struct Impersonation {
    bool active = false;
    ~Impersonation() {
        if (active && !RevertToSelf()) {
            // Never continue in an unknown impersonation context as SYSTEM.
            TerminateProcess(GetCurrentProcess(), ERROR_ACCESS_DENIED);
        }
    }
    bool start(HANDLE pipe) {
        active = ImpersonateNamedPipeClient(pipe) != FALSE;
        return active;
    }
    bool finish() {
        if (!active) return false;
        if (!RevertToSelf()) {
            TerminateProcess(GetCurrentProcess(), ERROR_ACCESS_DENIED);
            return false;
        }
        active = false;
        return true;
    }
};

bool SidAndLogonMatch(HANDLE threadToken, HANDLE processToken) {
    // Identification-only tokens can expose a name but cannot prove that
    // the broker is genuinely operating in the local caller's context.
    SECURITY_IMPERSONATION_LEVEL level = SecurityAnonymous;
    DWORD levelBytes = 0;
    if (!GetTokenInformation(threadToken, TokenImpersonationLevel, &level,
                             sizeof(level), &levelBytes)
        || levelBytes != sizeof(level) || level < SecurityImpersonation)
        return false;

    auto tokenUser = [](HANDLE token, std::vector<UCHAR>& buffer) {
        DWORD bytes = 0;
        GetTokenInformation(token, TokenUser, nullptr, 0, &bytes);
        if (GetLastError() != ERROR_INSUFFICIENT_BUFFER || bytes < sizeof(TOKEN_USER)
            || bytes > 4096) return false;
        buffer.resize(bytes);
        return GetTokenInformation(token, TokenUser, buffer.data(), bytes, &bytes) != FALSE;
    };
    std::vector<UCHAR> a, b;
    if (!tokenUser(threadToken, a) || !tokenUser(processToken, b)) return false;
    const auto left = reinterpret_cast<const TOKEN_USER*>(a.data());
    const auto right = reinterpret_cast<const TOKEN_USER*>(b.data());
    if (!left->User.Sid || !right->User.Sid
        || !IsValidSid(left->User.Sid) || !IsValidSid(right->User.Sid)
        || !EqualSid(left->User.Sid, right->User.Sid)) return false;

    // Same user and logon is insufficient for a duplicated pipe handle:
    // a low-integrity process can have the same SID and AuthenticationId
    // as the medium-integrity desktop executable. Require the actual
    // sender's integrity SID to match that process as well.
    auto tokenIntegrity = [](HANDLE token, std::vector<UCHAR>& buffer) {
        DWORD bytes = 0;
        GetTokenInformation(token, TokenIntegrityLevel, nullptr, 0, &bytes);
        if (GetLastError() != ERROR_INSUFFICIENT_BUFFER
            || bytes < sizeof(TOKEN_MANDATORY_LABEL) || bytes > 4096) return false;
        buffer.resize(bytes);
        if (!GetTokenInformation(token, TokenIntegrityLevel,
                                 buffer.data(), bytes, &bytes)) return false;
        const auto label = reinterpret_cast<const TOKEN_MANDATORY_LABEL*>(buffer.data());
        return label->Label.Sid && IsValidSid(label->Label.Sid);
    };
    std::vector<UCHAR> ilSender, ilProcess;
    if (!tokenIntegrity(threadToken, ilSender)
        || !tokenIntegrity(processToken, ilProcess)) return false;
    const auto senderLabel = reinterpret_cast<const TOKEN_MANDATORY_LABEL*>(ilSender.data());
    const auto processLabel = reinterpret_cast<const TOKEN_MANDATORY_LABEL*>(ilProcess.data());
    if (!EqualSid(senderLabel->Label.Sid, processLabel->Label.Sid)) return false;

    TOKEN_STATISTICS ls = {}, ps = {};
    DWORD size = 0;
    if (!GetTokenInformation(threadToken, TokenStatistics, &ls, sizeof(ls), &size)
        || !GetTokenInformation(processToken, TokenStatistics, &ps, sizeof(ps), &size)) return false;
    return ls.AuthenticationId.LowPart == ps.AuthenticationId.LowPart
        && ls.AuthenticationId.HighPart == ps.AuthenticationId.HighPart;
}

bool ExactlySamePath(const std::wstring& a, const std::wstring& b) {
    return !a.empty() && !b.empty()
        && CompareStringOrdinal(a.c_str(), -1, b.c_str(), -1, TRUE) == CSTR_EQUAL;
}

bool ExpectedDesktopPath(std::wstring& expected) {
    std::array<wchar_t, 32768> own = {};
    std::array<wchar_t, 32768> programFiles = {};
    DWORD got = GetModuleFileNameW(nullptr, own.data(), DWORD(own.size()));
    if (!got || got >= own.size()) return false;
    DWORD pf = GetEnvironmentVariableW(L"ProgramFiles", programFiles.data(),
                                      DWORD(programFiles.size()));
    if (!pf || pf >= programFiles.size()) return false;
    std::wstring dir(own.data(), got);
    const auto slash = dir.find_last_of(L'\\');
    if (slash == std::wstring::npos) return false;
    dir.resize(slash + 1);
    const std::wstring prefix(programFiles.data(), pf);
    // Release broker and desktop must live together under a machine-owned
    // Program Files directory, never a per-user-writable deployment path.
    if (dir.size() <= prefix.size() + 1
        || CompareStringOrdinal(dir.c_str(), int(prefix.size()),
                                prefix.c_str(), int(prefix.size()), TRUE) != CSTR_EQUAL
        || dir[prefix.size()] != L'\\') return false;
    expected = dir + kApprovedDesktopName;
    return true;
}

bool Sha256File(const std::wstring& path, std::array<UCHAR, kHashBytes>& digest) {
    ScopedHandle file;
    file.handle = CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ,
                              nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file.handle == INVALID_HANDLE_VALUE) return false;
    BCRYPT_ALG_HANDLE algorithm = nullptr;
    BCRYPT_HASH_HANDLE hash = nullptr;
    bool ok = false;
    std::vector<UCHAR> hashObject;
    do {
        if (BCryptOpenAlgorithmProvider(&algorithm, BCRYPT_SHA256_ALGORITHM, nullptr, 0) < 0) break;
        ULONG objectLength = 0, written = 0;
        if (BCryptGetProperty(algorithm, BCRYPT_OBJECT_LENGTH,
            reinterpret_cast<PUCHAR>(&objectLength), sizeof(objectLength), &written, 0) < 0
            || !objectLength || objectLength > 1u << 20) break;
        hashObject.resize(objectLength);
        if (BCryptCreateHash(algorithm, &hash, hashObject.data(), objectLength,
                             nullptr, 0, 0) < 0) break;
        std::array<UCHAR, 32 * 1024> chunk = {};
        DWORD received = 0;
        bool readOk = true;
        for (;;) {
            if (!ReadFile(file.handle, chunk.data(), DWORD(chunk.size()), &received, nullptr)) {
                readOk = false;
                break;
            }
            if (!received) break;
            if (BCryptHashData(hash, chunk.data(), received, 0) < 0) {
                readOk = false;
                break;
            }
        }
        if (readOk && BCryptFinishHash(hash, digest.data(), ULONG(digest.size()), 0) >= 0) ok = true;
    } while (false);
    if (hash) BCryptDestroyHash(hash);
    if (algorithm) BCryptCloseAlgorithmProvider(algorithm, 0);
    return ok;
}

bool ImageHashApproved(const std::wstring& path) {
    std::array<UCHAR, kHashBytes> expected = {};
    DWORD length = DWORD(expected.size());
    DWORD kind = 0;
    if (RegGetValueW(HKEY_LOCAL_MACHINE, kApprovedRegistryKey, kApprovedHashValue,
                     RRF_RT_REG_BINARY, &kind, expected.data(), &length) != ERROR_SUCCESS
        || length != expected.size()) return false;
    std::array<UCHAR, kHashBytes> actual = {};
    if (!Sha256File(path, actual)) return false;
    UCHAR difference = 0;
    for (size_t i = 0; i < expected.size(); ++i) {
        difference |= actual[i] ^ expected[i];
    }
    return difference == 0;
}
} // namespace

BrokerClientIdentity::~BrokerClientIdentity() {
    if (process_) CloseHandle(process_);
}

bool BrokerClientIdentity::IsLive(HANDLE pipe) const {
    if (!process_ || !pid_ || !session_ || !pipe
        || WaitForSingleObject(process_, 0) != WAIT_TIMEOUT) return false;
    ULONG currentPid = 0;
    ULONG currentSession = 0;
    return GetNamedPipeClientProcessId(pipe, &currentPid) && currentPid == pid_
        && GetNamedPipeClientSessionId(pipe, &currentSession)
        && currentSession == session_;
}

bool BrokerClientIdentity::SenderTokenMatches(HANDLE pipe) const {
    if (!IsLive(pipe)) return false;
    ScopedHandle processToken;
    if (!OpenProcessToken(process_, TOKEN_QUERY, &processToken.handle))
        return false;

    // Named-pipe impersonation represents the last bytes READ on this
    // server connection, not necessarily the process that created it.
    // This comparison MUST follow every complete request (header/payload).
    Impersonation impersonation;
    if (!impersonation.start(pipe)) return false;
    ScopedHandle senderToken;
    const bool match =
        OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, TRUE, &senderToken.handle)
        && SidAndLogonMatch(senderToken.handle, processToken.handle);
    if (!impersonation.finish()) return false;
    return match && IsLive(pipe);
}

DWORD AuthenticatePipeClient(HANDLE pipe, BrokerClientIdentity& result) {
    if (result.process_) return ERROR_INVALID_STATE;
    ULONG pid = 0;
    if (!GetNamedPipeClientProcessId(pipe, &pid) || !pid) return ERROR_ACCESS_DENIED;
    ScopedHandle process;
    process.handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, FALSE, pid);
    if (!process.handle) return ERROR_ACCESS_DENIED;

    DWORD processSession = 0, pipeSession = 0;
    if (!ProcessIdToSessionId(pid, &processSession) || !processSession
        || !GetNamedPipeClientSessionId(pipe, &pipeSession)
        || processSession != pipeSession
        || WaitForSingleObject(process.handle, 0) != WAIT_TIMEOUT)
        return ERROR_ACCESS_DENIED;

    Impersonation impersonation;
    if (!impersonation.start(pipe)) return ERROR_ACCESS_DENIED;
    ScopedHandle threadToken, processToken;
    if (!OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, TRUE, &threadToken.handle)
        || !OpenProcessToken(process.handle, TOKEN_QUERY, &processToken.handle)
        || !SidAndLogonMatch(threadToken.handle, processToken.handle)) {
        return ERROR_ACCESS_DENIED; // RAII always reverts before returning.
    }
    if (!impersonation.finish()) return ERROR_ACCESS_DENIED;

    std::array<wchar_t, 32768> processName = {};
    DWORD length = DWORD(processName.size());
    if (!QueryFullProcessImageNameW(process.handle, 0, processName.data(), &length)
        || !length || length >= processName.size()) return ERROR_ACCESS_DENIED;
    std::wstring expectedPath;
    if (!ExpectedDesktopPath(expectedPath)
        || !ExactlySamePath(std::wstring(processName.data(), length), expectedPath)
        || !ImageHashApproved(expectedPath)) return ERROR_ACCESS_DENIED;
    // Keep SYNCHRONIZE authority for the exact process that passed all
    // identity checks. This prevents PID reuse and detects process exit.
    result.process_ = process.handle;
    result.pid_ = pid;
    result.session_ = processSession;
    process.handle = nullptr;
    return ERROR_SUCCESS;
}
