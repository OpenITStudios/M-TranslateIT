#pragma once
// Admin/SYSTEM-only local control object, not a Windows audio endpoint.
#include <ntddk.h>

NTSTATUS TranslateITControlInitialize(_In_ PDRIVER_OBJECT driver);
void TranslateITControlShutdown();
