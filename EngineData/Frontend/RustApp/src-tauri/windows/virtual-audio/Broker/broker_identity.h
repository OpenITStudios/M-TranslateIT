#pragma once
#include <Windows.h>

// No authorization is inferred from a pipe name, a caller-supplied PID,
// session generation, or a user-configurable file path.
DWORD AuthenticatePipeClient(HANDLE pipe);
