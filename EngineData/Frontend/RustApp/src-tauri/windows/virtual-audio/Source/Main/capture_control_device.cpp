// Capture PCM transport admission, distinct from the Windows WaveRT capture pin.
// This is an explicitly privileged *development* interface. Production cannot
// expose it to unprivileged processes until a reviewed authorization broker exists.
#include "definitions.h"
#include <wdmsec.h>
#include "capture_control_device.h"
#include "capture_pcm.h"

namespace {
PDEVICE_OBJECT g_controlDevice = nullptr;
const GUID kControlSecurityClass = {
    0x8a3f17d2, 0x19bc, 0x48ec, {0xa2, 0x10, 0xc7, 0x4d, 0x83, 0xb6, 0x6f, 0x25}
};
// No World, Authenticated Users or Interactive Users grants.
// Non-elevated desktop TranslateIT must fail closed until an approved
// least-privilege broker exists; do not weaken this DACL as a shortcut.
constexpr wchar_t kSecurity[] = L"D:P(A;;GA;;;SY)(A;;GA;;;BA)";
constexpr wchar_t kKernelPath[] = L"\\Device\\TranslateITMicControl";
constexpr wchar_t kDosPath[] = L"\\DosDevices\\TranslateITMicControl";

NTSTATUS finish(_Inout_ PIRP irp, NTSTATUS status, ULONG_PTR info = 0) {
    irp->IoStatus.Status = status;
    irp->IoStatus.Information = NT_SUCCESS(status) ? info : 0;
    IoCompleteRequest(irp, IO_NO_INCREMENT);
    return status;
}

NTSTATUS onCreateClose(PDEVICE_OBJECT device, PIRP irp) {
    if (device != g_controlDevice) return PcDispatchIrp(device, irp);
    PIO_STACK_LOCATION stack = IoGetCurrentIrpStackLocation(irp);
    if (!stack->FileObject) return finish(irp, STATUS_INVALID_HANDLE);
    if (stack->MajorFunction == IRP_MJ_CREATE) {
        const ULONGLONG pnpEpoch = TranslateITCaptureDeviceEpoch();
        if (!pnpEpoch) return finish(irp, STATUS_DEVICE_NOT_READY);
        // This file object belongs only to this control device, not PortCls.
        // No heap allocation: FsContext stores a non-dereferenced 64-bit token.
        stack->FileObject->FsContext = (PVOID)(ULONG_PTR)pnpEpoch;
    } else if (stack->MajorFunction == IRP_MJ_CLEANUP || stack->MajorFunction == IRP_MJ_CLOSE) {
        TranslateITCaptureCloseWriter(stack->FileObject);
        stack->FileObject->FsContext = nullptr;
    }
    return finish(irp, STATUS_SUCCESS);
}

NTSTATUS onDeviceControl(PDEVICE_OBJECT device, PIRP irp) {
    if (device != g_controlDevice) return PcDispatchIrp(device, irp);
    PIO_STACK_LOCATION stack = IoGetCurrentIrpStackLocation(irp);
    const ULONG code = stack->Parameters.DeviceIoControl.IoControlCode;
    const ULONG input = stack->Parameters.DeviceIoControl.InputBufferLength;
    const ULONG output = stack->Parameters.DeviceIoControl.OutputBufferLength;
    PVOID buffer = irp->AssociatedIrp.SystemBuffer;
    PFILE_OBJECT file = stack->FileObject;

    if (!file) return finish(irp, STATUS_INVALID_HANDLE);
    const ULONGLONG openedEpoch = (ULONGLONG)(ULONG_PTR)file->FsContext;
    const ULONGLONG currentEpoch = TranslateITCaptureDeviceEpoch();
    if (!currentEpoch || openedEpoch != currentEpoch)
        return finish(irp, STATUS_DEVICE_NOT_READY);
    // IOCTL access bits additionally enforce FILE_READ_DATA or FILE_WRITE_DATA.
    // All frame parsing must use the actual SYSTEM_BUFFER length, never a
    // payload length or pointer provided by an untrusted caller.
    switch (code) {
    case TI_CAPTURE_IOCTL_BEGIN:
        if (!buffer || input != sizeof(TI_CAPTURE_BEGIN)) return finish(irp, STATUS_INVALID_PARAMETER);
        return finish(irp, TranslateITCaptureBegin(file, (const TI_CAPTURE_BEGIN*)buffer));
    case TI_CAPTURE_IOCTL_WRITE:
        if (!buffer || input < FIELD_OFFSET(TI_CAPTURE_WRITE, Data))
            return finish(irp, STATUS_INVALID_PARAMETER);
        return finish(irp, TranslateITCaptureWrite(file, (const TI_CAPTURE_WRITE*)buffer, input));
    case TI_CAPTURE_IOCTL_FLUSH:
    case TI_CAPTURE_IOCTL_END:
        if (!buffer || input != sizeof(TI_CAPTURE_REVOKE)) return finish(irp, STATUS_INVALID_PARAMETER);
        return finish(irp, TranslateITCaptureRevoke(file, (const TI_CAPTURE_REVOKE*)buffer,
            code == TI_CAPTURE_IOCTL_END ? TRUE : FALSE));
    case TI_CAPTURE_IOCTL_STATUS:
        if (!buffer || output < sizeof(TI_CAPTURE_STATUS)) return finish(irp, STATUS_BUFFER_TOO_SMALL);
        TranslateITCaptureStatus((TI_CAPTURE_STATUS*)buffer);
        return finish(irp, STATUS_SUCCESS, sizeof(TI_CAPTURE_STATUS));
    default:
        return finish(irp, STATUS_INVALID_DEVICE_REQUEST);
    }
}
} // namespace

NTSTATUS TranslateITControlInitialize(PDRIVER_OBJECT driver) {
    if (!driver || g_controlDevice) return STATUS_INVALID_DEVICE_STATE;
    UNICODE_STRING name;
    UNICODE_STRING link;
    UNICODE_STRING sddl;
    RtlInitUnicodeString(&name, kKernelPath);
    RtlInitUnicodeString(&link, kDosPath);
    RtlInitUnicodeString(&sddl, kSecurity);

    PDEVICE_OBJECT created = nullptr;
    NTSTATUS status = IoCreateDeviceSecure(
        driver, 0, &name, FILE_DEVICE_UNKNOWN, FILE_DEVICE_SECURE_OPEN,
        FALSE, &sddl, &kControlSecurityClass, &created);
    if (!NT_SUCCESS(status)) return status;

    status = IoCreateSymbolicLink(&link, &name);
    if (!NT_SUCCESS(status)) {
        IoDeleteDevice(created);
        return status;
    }
    created->Flags |= DO_BUFFERED_IO;
    created->Flags &= ~DO_DEVICE_INITIALIZING;
    g_controlDevice = created;

    // A driver object has one dispatch table; unrelated PortCls WaveRT
    // requests must be forwarded to its documented default handler.
    driver->MajorFunction[IRP_MJ_CREATE] = onCreateClose;
    driver->MajorFunction[IRP_MJ_CLEANUP] = onCreateClose;
    driver->MajorFunction[IRP_MJ_CLOSE] = onCreateClose;
    driver->MajorFunction[IRP_MJ_DEVICE_CONTROL] = onDeviceControl;
    return STATUS_SUCCESS;
}

void TranslateITControlShutdown() {
    TranslateITCaptureInvalidateAll();
    if (!g_controlDevice) return;
    UNICODE_STRING link;
    RtlInitUnicodeString(&link, kDosPath);
    IoDeleteSymbolicLink(&link);
    IoDeleteDevice(g_controlDevice);
    g_controlDevice = nullptr;
}
