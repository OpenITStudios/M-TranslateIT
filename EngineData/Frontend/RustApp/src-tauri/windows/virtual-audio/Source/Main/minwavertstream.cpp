#include "definitions.h"
#include <limits.h>
#include <ks.h>
#include "endpoints.h"
#include "minwavert.h"
#include "minwavertstream.h"
#include "capture_pcm.h"
#define MINWAVERTSTREAM_POOLTAG 'SRWM'

// A virtual capture miniport must never map arbitrarily large client DMA
// allocations and then zero/fill them under a DISPATCH_LEVEL spinlock.
// 48 kHz PCM16 mono => at most one second / 96,000 bytes per buffer.
static constexpr ULONG kMaxCaptureDmaBytes =
    TI_CAPTURE_RATE_HZ * TI_CAPTURE_CHANNELS * (TI_CAPTURE_SAMPLE_BITS / 8u);
static_assert(kMaxCaptureDmaBytes == 96000u, "bounded mono DMA allocation");

// Keep 64-bit intermediate math for long sleep/resume gaps. The position
// itself remains a byte count; cyclic wrapping happens only at the end.
static constexpr ULONGLONG CaptureDisplacementNumerator(
    ULONG bytesPerSecond, ULONGLONG elapsedMs, ULONG byteCarry)
{
    return (ULONGLONG)bytesPerSecond * elapsedMs + byteCarry;
}
static_assert(CaptureDisplacementNumerator(96000u, 60000u, 0u) / 1000u
    == 5760000ull, "one-minute gap cannot wrap 32-bit arithmetic");


#pragma warning (disable : 4127)

//=============================================================================
// CMiniportWaveRTStream
//=============================================================================

//=============================================================================
#pragma code_seg() // Nonpaged: protects shared stream state with m_PositionSpinLock
CMiniportWaveRTStream::~CMiniportWaveRTStream
( 
    void 
)
/*++

Routine Description:

  Destructor for wavertstream 

Arguments:

Return Value:

  NT status code.

--*/
{

    // ExDeleteTimer(Cancel=TRUE, Wait=TRUE) must complete BEFORE releasing
    // m_pMiniport, notification events or any state TimerNotifyRT reads.
    // The callback can run at DISPATCH_LEVEL while the destructor is at PASSIVE.
    if (m_pNotificationTimer)
    {
        ExDeleteTimer(m_pNotificationTimer, TRUE, TRUE, NULL);
        m_pNotificationTimer = NULL;
    }
    KeFlushQueuedDpcs();

    // PortCls owns the Free*Buffer callback ordering (including surprise
    // removal); the destructor must not free an MDL behind its back.
    ASSERT(m_pDmaMdl == NULL);

    // No timer callback can access the notification list now. Drain any
    // registrations PortCls did not explicitly unregister during teardown,
    // and release the same nonpaged allocations as UnregisterNotificationEvent.
    LIST_ENTRY retired;
    InitializeListHead(&retired);
    KIRQL oldIrql;
    KeAcquireSpinLock(&m_PositionSpinLock, &oldIrql);
    while (!IsListEmpty(&m_NotificationList))
    {
        InsertTailList(&retired, RemoveHeadList(&m_NotificationList));
    }
    KeReleaseSpinLock(&m_PositionSpinLock, oldIrql);
    while (!IsListEmpty(&retired))
    {
        NotificationListEntry* item = CONTAINING_RECORD(
            RemoveHeadList(&retired), NotificationListEntry, ListEntry);
        ExFreePoolWithTag(item, MINWAVERTSTREAM_POOLTAG);
    }

    if (NULL != m_pMiniport)
    {
    
        if (m_bUnregisterStream)
        {
            m_pMiniport->StreamClosed(m_ulPin, this);
            m_bUnregisterStream = FALSE;
        }
        
        m_pMiniport->Release();
        m_pMiniport = NULL;
    }

    if (m_pDpc)
    {
        ExFreePoolWithTag( m_pDpc, MINWAVERTSTREAM_POOLTAG );
        m_pDpc = NULL;
    }

    if (m_pTimer)
    {
        ExFreePoolWithTag( m_pTimer, MINWAVERTSTREAM_POOLTAG );
        m_pTimer = NULL;
    }

    if (m_pbMuted)
    {
        ExFreePoolWithTag( m_pbMuted, MINWAVERTSTREAM_POOLTAG );
        m_pbMuted = NULL;
    }

    if (m_plVolumeLevel)
    {
        ExFreePoolWithTag( m_plVolumeLevel, MINWAVERTSTREAM_POOLTAG );
        m_plVolumeLevel = NULL;
    }

    if (m_plPeakMeter)
    {
        ExFreePoolWithTag( m_plPeakMeter, MINWAVERTSTREAM_POOLTAG );
        m_plPeakMeter = NULL;
    }

    if (m_pWfExt)
    {
        ExFreePoolWithTag( m_pWfExt, MINWAVERTSTREAM_POOLTAG );
        m_pWfExt = NULL;
    }
    DPF_ENTER(("[CMiniportWaveRTStream::~CMiniportWaveRTStream]"));
} // ~CMiniportWaveRTStream

//=============================================================================
#pragma code_seg("PAGE")

NTSTATUS CMiniportWaveRTStream::ReadRegistrySettings()
{
    PAGED_CODE();

    NTSTATUS                    ntStatus;
    PDRIVER_OBJECT              DriverObject;
    HANDLE                      DriverKey;
    RTL_QUERY_REGISTRY_TABLE    paramTable[] = {
        // QueryRoutine     Flags                                               Name                            EntryContext                            DefaultType                                                     DefaultData                                 DefaultLength
        { NULL,   RTL_QUERY_REGISTRY_DIRECT | RTL_QUERY_REGISTRY_TYPECHECK, L"HostCaptureToneFrequency",        &m_ulHostCaptureToneFrequency,          (REG_DWORD << RTL_QUERY_REGISTRY_TYPECHECK_SHIFT) | REG_DWORD,  &m_ulHostCaptureToneFrequency,              sizeof(DWORD) },
        { NULL,   RTL_QUERY_REGISTRY_DIRECT | RTL_QUERY_REGISTRY_TYPECHECK, L"HostCaptureToneAmplitude",        &m_dwHostCaptureToneAmplitude,          (REG_DWORD << RTL_QUERY_REGISTRY_TYPECHECK_SHIFT) | REG_DWORD,  &m_dwHostCaptureToneAmplitude,              sizeof(DWORD) },
        { NULL,   RTL_QUERY_REGISTRY_DIRECT | RTL_QUERY_REGISTRY_TYPECHECK, L"HostCaptureToneDCOffset",         &m_dwHostCaptureToneDCOffset,           (REG_DWORD << RTL_QUERY_REGISTRY_TYPECHECK_SHIFT) | REG_DWORD,  &m_dwHostCaptureToneDCOffset,               sizeof(DWORD) },
        { NULL,   RTL_QUERY_REGISTRY_DIRECT | RTL_QUERY_REGISTRY_TYPECHECK, L"HostCaptureToneInitialPhase",     &m_dwHostCaptureToneInitialPhase,       (REG_DWORD << RTL_QUERY_REGISTRY_TYPECHECK_SHIFT) | REG_DWORD,  &m_dwHostCaptureToneInitialPhase,           sizeof(DWORD) },
        { NULL,   0,                                                        NULL,                               NULL,                                   0,                                                              NULL,                                       0 }
    };

    DriverObject = WdfDriverWdmGetDriverObject(WdfGetDriver());
    DriverKey = NULL;
    ntStatus = IoOpenDriverRegistryKey(DriverObject, 
                                 DriverRegKeyParameters,
                                 KEY_READ,
                                 0,
                                 &DriverKey);

    if (!NT_SUCCESS(ntStatus))
    {
        return ntStatus;
    }

    ntStatus = RtlQueryRegistryValues(RTL_REGISTRY_HANDLE,
                                  (PCWSTR) DriverKey,
                                  &paramTable[0],
                                  NULL,
                                  NULL);

    if (!NT_SUCCESS(ntStatus)) 
    {
        DPF(D_VERBOSE, ("RtlQueryRegistryValues failed, using default values, 0x%x", ntStatus));
        //
        // Don't return error because we will operate with default values.
        //
    }

    if (DriverKey)
    {
        ZwClose(DriverKey);
    }

    return ntStatus;
}

NTSTATUS
CMiniportWaveRTStream::Init
( 
    _In_ PCMiniportWaveRT           Miniport_,
    _In_ PPORTWAVERTSTREAM          PortStream_,
    _In_ ULONG                      Pin_,
    _In_ BOOLEAN                    Capture_,
    _In_ PKSDATAFORMAT              DataFormat_,
    _In_ GUID                       SignalProcessingMode
)
/*++

Routine Description:

  Initializes the stream object.

Arguments:

  Miniport_ -

  Pin_ -

  DataFormat -

  SignalProcessingMode - The driver uses the signalProcessingMode to configure
    driver and/or hardware specific signal processing to be applied to this new
    stream.

Return Value:

  NT status code.

--*/
{
    PAGED_CODE();

    PWAVEFORMATEX pWfEx = NULL;
    NTSTATUS ntStatus = STATUS_SUCCESS;

    m_pMiniport = NULL;
    m_ulPin = 0;
    m_bUnregisterStream = FALSE;
    m_bCapture = FALSE;
    m_ulDmaBufferSize = 0;
    m_pDmaBuffer = NULL;
    m_pDmaMdl = NULL;
    m_ulNotificationsPerBuffer = 0;
    m_KsState = KSSTATE_STOP;
    m_pTimer = NULL;
    m_pDpc = NULL;
    m_llPacketCounter = 0;
    m_ullPlayPosition = 0;
    m_ullWritePosition = 0;
    m_ullDmaTimeStamp = 0;
    m_hnsElapsedTimeCarryForward = 0;
    m_ullLastDPCTimeStamp = 0;
    m_hnsDPCTimeCarryForward = 0;
    m_ulDmaMovementRate = 0;
    m_byteDisplacementCarryForward = 0;
    m_bLfxEnabled = FALSE;
    m_pbMuted = NULL;
    m_plVolumeLevel = NULL;
    m_plPeakMeter = NULL;
    m_pWfExt = NULL;
    m_ullLinearPosition = 0;
    m_TranslateITCaptureReadFrame = 0;
    m_TranslateITCaptureSerial = 0;
    m_ullPresentationPosition = 0;
    m_ulContentId = 0;
    m_ulCurrentWritePosition = 0;
    m_ulLastOsReadPacket = ULONG_MAX;
    m_ulLastOsWritePacket = ULONG_MAX;
    m_IsCurrentWritePositionUpdated = 0;
    m_SignalProcessingMode = SignalProcessingMode;
    m_bEoSReceived = FALSE;
    m_bLastBufferRendered = FALSE;

    m_ulHostCaptureToneFrequency = IsEqualGUID(SignalProcessingMode, AUDIO_SIGNALPROCESSINGMODE_RAW) ? 1000 : 2000;
    m_dwHostCaptureToneAmplitude = 50;
    m_dwHostCaptureToneDCOffset = 0;
    m_dwHostCaptureToneInitialPhase = 0;

    m_pPortStream = PortStream_;
    InitializeListHead(&m_NotificationList);
    m_ulNotificationIntervalMs = 0;

    // Initialize the spinlock to synchronize position updates
    KeInitializeSpinLock(&m_PositionSpinLock);

    m_pNotificationTimer = ExAllocateTimer(
         TimerNotifyRT,
         this,
         EX_TIMER_HIGH_RESOLUTION
    );
    if (!m_pNotificationTimer)
    {
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    pWfEx = GetWaveFormatEx(DataFormat_);
    if (NULL == pWfEx) 
    { 
        return STATUS_UNSUCCESSFUL; 
    }

    m_pMiniport = reinterpret_cast<CMiniportWaveRT*>(Miniport_);
    if (m_pMiniport == NULL)
    {
        return STATUS_INVALID_PARAMETER;
    }
    m_pMiniport->AddRef();
    if (!NT_SUCCESS(ntStatus))
    {
        return ntStatus;
    }
    m_ulPin = Pin_;
    m_bCapture = Capture_;
    m_ulDmaMovementRate = pWfEx->nAvgBytesPerSec;

    m_pDpc = (PRKDPC)ExAllocatePool2(POOL_FLAG_NON_PAGED, sizeof(KDPC), MINWAVERTSTREAM_POOLTAG);
    if (!m_pDpc)
    {
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    m_pWfExt = (PWAVEFORMATEXTENSIBLE)ExAllocatePool2(POOL_FLAG_NON_PAGED, sizeof(WAVEFORMATEX) + pWfEx->cbSize, MINWAVERTSTREAM_POOLTAG);
    if (m_pWfExt == NULL)
    {
        return STATUS_INSUFFICIENT_RESOURCES;
    }
    RtlCopyMemory(m_pWfExt, pWfEx, sizeof(WAVEFORMATEX) + pWfEx->cbSize);

    m_pbMuted = (PBOOL)ExAllocatePool2(POOL_FLAG_NON_PAGED, m_pWfExt->Format.nChannels * sizeof(BOOL), MINWAVERTSTREAM_POOLTAG);
    if (m_pbMuted == NULL)
    {
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    m_plVolumeLevel = (PLONG)ExAllocatePool2(POOL_FLAG_NON_PAGED, m_pWfExt->Format.nChannels * sizeof(LONG), MINWAVERTSTREAM_POOLTAG);
    if (m_plVolumeLevel == NULL)
    {
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    m_plPeakMeter = (PLONG)ExAllocatePool2(POOL_FLAG_NON_PAGED, m_pWfExt->Format.nChannels * sizeof(LONG), MINWAVERTSTREAM_POOLTAG);
    if (m_plPeakMeter == NULL)
    {
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    // Capture-only: no test tone, render stream or debug audio file.

    //
    // Register this stream.
    //
    ntStatus = m_pMiniport->StreamCreated(m_ulPin, this);
    if (NT_SUCCESS(ntStatus))
    {
        m_bUnregisterStream = TRUE;
    }

    return ntStatus;
} // Init

//=============================================================================
#pragma code_seg("PAGE")
STDMETHODIMP_(NTSTATUS)
CMiniportWaveRTStream::NonDelegatingQueryInterface
( 
    _In_ REFIID  Interface,
    _COM_Outptr_ PVOID * Object 
)
/*++

Routine Description:

  QueryInterface

Arguments:

  Interface - GUID

  Object - interface pointer to be returned

Return Value:

  NT status code.

--*/
{
    PAGED_CODE();

    ASSERT(Object);

    if (IsEqualGUIDAligned(Interface, IID_IUnknown))
    {
        *Object = PVOID(PUNKNOWN(PMINIPORTWAVERTSTREAM(this)));
    }
    else if (IsEqualGUIDAligned(Interface, IID_IMiniportWaveRTStream))
    {
        *Object = PVOID(PMINIPORTWAVERTSTREAM(this));
    }
    else if (IsEqualGUIDAligned(Interface, IID_IMiniportWaveRTStreamNotification))
    {
        *Object = PVOID(PMINIPORTWAVERTSTREAMNOTIFICATION(this));
    }
    else if (IsEqualGUIDAligned(Interface, IID_IMiniportWaveRTInputStream) && (this->m_bCapture))
    {
        // This interface is supported only on capture streams
        *Object = PVOID(PMINIPORTWAVERTINPUTSTREAM(this));
    }
    else if (IsEqualGUIDAligned(Interface, IID_IMiniportWaveRTOutputStream))
    {
        // This interface is supported only on host render streams
        *Object = PVOID(PMINIPORTWAVERTOUTPUTSTREAM(this));
    }
    else if (IsEqualGUIDAligned(Interface, IID_IDrmAudioStream))
    {
        *Object = (PVOID)(IDrmAudioStream*)this;
    }
    else
    {
        *Object = NULL;
    }

    if (*Object)
    {
        PUNKNOWN(*Object)->AddRef();
        return STATUS_SUCCESS;
    }

    return STATUS_INVALID_PARAMETER;
} // NonDelegatingQueryInterface

//=============================================================================
#pragma code_seg() // Nonpaged: protects shared stream state with m_PositionSpinLock
NTSTATUS CMiniportWaveRTStream::AllocateBufferWithNotification
(
    _In_    ULONG               NotificationCount_,
    _In_    ULONG               RequestedSize_,
    _Out_   PMDL                *AudioBufferMdl_,
    _Out_   ULONG               *ActualSize_,
    _Out_   ULONG               *OffsetFromFirstPage_,
    _Out_   MEMORY_CACHING_TYPE *CacheType_
)
{

    ULONG ulBufferDurationMs = 0;

    if (!m_pWfExt || !m_pWfExt->Format.nBlockAlign
        || !RequestedSize_ || RequestedSize_ < m_pWfExt->Format.nBlockAlign)
    {
        return STATUS_INVALID_PARAMETER;
    }

    // The WaveRT notification contract admits one or two notifications
    // per cyclic buffer; other counts must never reach the time math.
    if ((NotificationCount_ != 1 && NotificationCount_ != 2)
        || RequestedSize_ % NotificationCount_ != 0)
    {
        return STATUS_INVALID_PARAMETER;
    }

    RequestedSize_ -= RequestedSize_ % (m_pWfExt->Format.nBlockAlign);
    if (!RequestedSize_ || RequestedSize_ > kMaxCaptureDmaBytes
        || RequestedSize_ % NotificationCount_ != 0
        || !m_ulDmaMovementRate)
    {
        return STATUS_INVALID_PARAMETER;
    }
    // An event interval of zero would disable the notification timer and
    // strand a capture client without any delivered packets.
    const ULONGLONG durationMs =
        ((ULONGLONG)RequestedSize_ * 1000ull) / m_ulDmaMovementRate;
    if (durationMs < NotificationCount_)
    {
        return STATUS_INVALID_PARAMETER;
    }

    if (!m_bCapture && (!g_DoNotCreateDataFiles))
    {
        NTSTATUS ntStatus;
        
        // Simple Audio Sample uses following buffer to hold data before writing to a file.
        // Allocating larger buffer will reduce File I/O operations.
        ntStatus = m_SaveData.SetMaxWriteSize(RequestedSize_ * 4);
        if (!NT_SUCCESS(ntStatus))
        {
            return ntStatus;
        }
    }

    PHYSICAL_ADDRESS highAddress;
    highAddress.HighPart = 0;
    highAddress.LowPart = MAXULONG;

    PMDL pBufferMdl = m_pPortStream->AllocatePagesForMdl (highAddress, RequestedSize_);

    if (NULL == pBufferMdl)
    {
        return STATUS_UNSUCCESSFUL;
    }

    // From MSDN: 
    // "Since the Windows audio stack does not support a mechanism to express memory access 
    //  alignment requirements for buffers, audio drivers must select a caching type for mapped
    //  memory buffers that does not impose platform-specific alignment requirements. In other 
    //  words, the caching type used by the audio driver for mapped memory buffers, must not make 
    //  assumptions about the memory alignment requirements for any specific platform.
    //
    //  This method maps the physical memory pages in the MDL into kernel-mode virtual memory. 
    //  Typically, the miniport driver calls this method if it requires software access to the 
    //  scatter-gather list for an audio buffer. In this case, the storage for the scatter-gather 
    //  list must have been allocated by the IPortWaveRTStream::AllocatePagesForMdl or 
    //  IPortWaveRTStream::AllocateContiguousPagesForMdl method. 
    //
    //  A WaveRT miniport driver should not require software access to the audio buffer itself."
    //   
    BYTE* mapped = (BYTE*)m_pPortStream->MapAllocatedPages(pBufferMdl, MmCached);
    if (!mapped)
    {
        m_pPortStream->FreePagesFromMdl(pBufferMdl);
        return STATUS_INSUFFICIENT_RESOURCES;
    }
    RtlZeroMemory(mapped, RequestedSize_);
    ulBufferDurationMs = (ULONG)durationMs;

    KIRQL oldIrql;
    KeAcquireSpinLock(&m_PositionSpinLock, &oldIrql);
    const BOOLEAN occupied = (m_pDmaMdl != NULL);
    if (!occupied)
    {
        m_pDmaMdl = pBufferMdl;
        m_pDmaBuffer = mapped;
        m_ulNotificationsPerBuffer = NotificationCount_;
        m_ulDmaBufferSize = RequestedSize_;
        m_ulNotificationIntervalMs = ulBufferDurationMs / NotificationCount_;
    }
    KeReleaseSpinLock(&m_PositionSpinLock, oldIrql);
    if (occupied)
    {
        m_pPortStream->UnmapAllocatedPages(mapped, pBufferMdl);
        m_pPortStream->FreePagesFromMdl(pBufferMdl);
        return STATUS_DEVICE_BUSY;
    }

    *AudioBufferMdl_ = pBufferMdl;
    *ActualSize_ = RequestedSize_;
    *OffsetFromFirstPage_ = 0;
    *CacheType_ = MmCached;

    return STATUS_SUCCESS;
}

//=============================================================================
#pragma code_seg("PAGE")
VOID CMiniportWaveRTStream::FreeBufferWithNotification
(
    _In_        PMDL    Mdl_,
    _In_        ULONG   Size_
)
{
    UNREFERENCED_PARAMETER(Size_);
    PAGED_CODE();
    ReleaseDmaBuffer(Mdl_);
}

//=============================================================================
#pragma code_seg() // Nonpaged: protects shared stream state with m_PositionSpinLock
NTSTATUS CMiniportWaveRTStream::RegisterNotificationEvent
(
    _In_ PKEVENT NotificationEvent_
)
{
    if (!NotificationEvent_)
    {
        return STATUS_INVALID_PARAMETER;
    }

    NotificationListEntry* added = (NotificationListEntry*)ExAllocatePool2(
        POOL_FLAG_NON_PAGED, sizeof(NotificationListEntry), MINWAVERTSTREAM_POOLTAG);
    if (!added)
    {
        return STATUS_INSUFFICIENT_RESOURCES;
    }
    added->NotificationEvent = NotificationEvent_;

    NTSTATUS status = STATUS_SUCCESS;
    KIRQL oldIrql;
    KeAcquireSpinLock(&m_PositionSpinLock, &oldIrql);
    // TimerNotifyRT traverses this exact list with m_PositionSpinLock held.
    // Check duplicates and publish entries atomically with that traversal.
    for (PLIST_ENTRY cursor = m_NotificationList.Flink;
         cursor != &m_NotificationList; cursor = cursor->Flink)
    {
        NotificationListEntry* item =
            CONTAINING_RECORD(cursor, NotificationListEntry, ListEntry);
        if (item->NotificationEvent == NotificationEvent_)
        {
            status = STATUS_OBJECT_NAME_COLLISION;
            break;
        }
    }
    if (NT_SUCCESS(status))
    {
        InsertTailList(&m_NotificationList, &added->ListEntry);
    }
    KeReleaseSpinLock(&m_PositionSpinLock, oldIrql);

    if (!NT_SUCCESS(status))
    {
        ExFreePoolWithTag(added, MINWAVERTSTREAM_POOLTAG);
    }
    return status;
}

//=============================================================================
#pragma code_seg() // Nonpaged: protects shared stream state with m_PositionSpinLock
NTSTATUS CMiniportWaveRTStream::UnregisterNotificationEvent
(
    _In_ PKEVENT NotificationEvent_
)
{
    if (!NotificationEvent_)
    {
        return STATUS_INVALID_PARAMETER;
    }

    NotificationListEntry* removed = NULL;
    KIRQL oldIrql;
    KeAcquireSpinLock(&m_PositionSpinLock, &oldIrql);
    for (PLIST_ENTRY cursor = m_NotificationList.Flink;
         cursor != &m_NotificationList; cursor = cursor->Flink)
    {
        NotificationListEntry* item =
            CONTAINING_RECORD(cursor, NotificationListEntry, ListEntry);
        if (item->NotificationEvent == NotificationEvent_)
        {
            RemoveEntryList(cursor);
            removed = item;
            break;
        }
    }
    KeReleaseSpinLock(&m_PositionSpinLock, oldIrql);

    if (!removed)
    {
        return STATUS_NOT_FOUND;
    }
    // The callback completed its protected list traversal before removal,
    // so the node can now be safely freed outside the spinlock.
    ExFreePoolWithTag(removed, MINWAVERTSTREAM_POOLTAG);
    return STATUS_SUCCESS;
}


//=============================================================================
#pragma code_seg("PAGE")
NTSTATUS CMiniportWaveRTStream::GetClockRegister
(
    _Out_ PKSRTAUDIO_HWREGISTER Register_
)
{
    UNREFERENCED_PARAMETER(Register_);

    PAGED_CODE();

    return STATUS_NOT_IMPLEMENTED;
}

//=============================================================================
#pragma code_seg("PAGE")
NTSTATUS CMiniportWaveRTStream::GetPositionRegister
(
    _Out_ PKSRTAUDIO_HWREGISTER Register_
)
{
    UNREFERENCED_PARAMETER(Register_);

    PAGED_CODE();

    return STATUS_NOT_IMPLEMENTED;
}

//=============================================================================
#pragma code_seg("PAGE")
VOID CMiniportWaveRTStream::GetHWLatency
(
    _Out_ PKSRTAUDIO_HWLATENCY  Latency_
)
{
    PAGED_CODE();

    ASSERT(Latency_);

    Latency_->ChipsetDelay = 0;
    Latency_->CodecDelay = 0;
    Latency_->FifoSize = 0;
}

//=============================================================================
#pragma code_seg("PAGE")
VOID CMiniportWaveRTStream::FreeAudioBuffer
(
_In_opt_    PMDL        Mdl_,
_In_        ULONG       Size_
)
{
    UNREFERENCED_PARAMETER(Size_);
    PAGED_CODE();
    ReleaseDmaBuffer(Mdl_);
}

// Exactly one DMA/MDL ownership owner for event and polling modes.
// The PortCls Free*Buffer callback owns release; the destructor owns only
// the notification timer and list. The callback may be pending, so detach
// every DMA pointer under the same spinlock used by TimerNotifyRT and
// UpdatePosition. PortCls mapping DDIs must run OUTSIDE this spinlock at
// PASSIVE_LEVEL.
#pragma code_seg() // Nonpaged: protects shared stream state with m_PositionSpinLock
VOID CMiniportWaveRTStream::ReleaseDmaBuffer(_In_opt_ PMDL Mdl_)
{
    if (!Mdl_) return;

    BYTE* mapped = NULL;
    KIRQL oldIrql;
    KeAcquireSpinLock(&m_PositionSpinLock, &oldIrql);
    if (m_pDmaMdl == Mdl_)
    {
        mapped = m_pDmaBuffer;
        m_pDmaBuffer = NULL;
        m_pDmaMdl = NULL;
        m_ulDmaBufferSize = 0;
        m_ulNotificationsPerBuffer = 0;
        m_ulNotificationIntervalMs = 0;
    }
    KeReleaseSpinLock(&m_PositionSpinLock, oldIrql);

    if (!mapped)
    {
        // Duplicate/mismatched Free*Buffer: do not free arbitrary MDLs.
        return;
    }
    if (m_pNotificationTimer)
    {
        ExCancelTimer(m_pNotificationTimer, NULL);
    }
    m_pPortStream->UnmapAllocatedPages(mapped, Mdl_);
    m_pPortStream->FreePagesFromMdl(Mdl_);
}

//=============================================================================
#pragma code_seg() // Nonpaged: protects shared stream state with m_PositionSpinLock
NTSTATUS CMiniportWaveRTStream::AllocateAudioBuffer
(
_In_    ULONG                   RequestedSize_,
_Out_   PMDL                   *AudioBufferMdl_,
_Out_   ULONG                  *ActualSize_,
_Out_   ULONG                  *OffsetFromFirstPage_,
_Out_   MEMORY_CACHING_TYPE    *CacheType_
)
{

    if (!m_pWfExt || !m_pWfExt->Format.nBlockAlign
        || !RequestedSize_ || RequestedSize_ < m_pWfExt->Format.nBlockAlign)
    {
        return STATUS_INVALID_PARAMETER;
    }

    RequestedSize_ -= RequestedSize_ % (m_pWfExt->Format.nBlockAlign);
    if (!RequestedSize_ || RequestedSize_ > kMaxCaptureDmaBytes
        || !m_ulDmaMovementRate)
    {
        return STATUS_INVALID_PARAMETER;
    }

    PHYSICAL_ADDRESS highAddress;
    highAddress.HighPart = 0;
    highAddress.LowPart = MAXULONG;

    PMDL pBufferMdl = m_pPortStream->AllocatePagesForMdl(highAddress, RequestedSize_);

    if (NULL == pBufferMdl)
    {
        return STATUS_UNSUCCESSFUL;
    }

    // From MSDN: 
    // "Since the Windows audio stack does not support a mechanism to express memory access 
    //  alignment requirements for buffers, audio drivers must select a caching type for mapped
    //  memory buffers that does not impose platform-specific alignment requirements. In other 
    //  words, the caching type used by the audio driver for mapped memory buffers, must not make 
    //  assumptions about the memory alignment requirements for any specific platform.
    //
    //  This method maps the physical memory pages in the MDL into kernel-mode virtual memory. 
    //  Typically, the miniport driver calls this method if it requires software access to the 
    //  scatter-gather list for an audio buffer. In this case, the storage for the scatter-gather 
    //  list must have been allocated by the IPortWaveRTStream::AllocatePagesForMdl or 
    //  IPortWaveRTStream::AllocateContiguousPagesForMdl method. 
    //
    //  A WaveRT miniport driver should not require software access to the audio buffer itself."
    //   
    BYTE* mapped = (BYTE*)m_pPortStream->MapAllocatedPages(pBufferMdl, MmCached);
    if (!mapped)
    {
        m_pPortStream->FreePagesFromMdl(pBufferMdl);
        return STATUS_INSUFFICIENT_RESOURCES;
    }
    RtlZeroMemory(mapped, RequestedSize_);

    KIRQL oldIrql;
    KeAcquireSpinLock(&m_PositionSpinLock, &oldIrql);
    const BOOLEAN occupied = (m_pDmaMdl != NULL);
    if (!occupied)
    {
        m_pDmaMdl = pBufferMdl;
        m_pDmaBuffer = mapped;
        m_ulDmaBufferSize = RequestedSize_;
        m_ulNotificationsPerBuffer = 0;
        m_ulNotificationIntervalMs = 0;
    }
    KeReleaseSpinLock(&m_PositionSpinLock, oldIrql);
    if (occupied)
    {
        m_pPortStream->UnmapAllocatedPages(mapped, pBufferMdl);
        m_pPortStream->FreePagesFromMdl(pBufferMdl);
        return STATUS_DEVICE_BUSY;
    }

    *AudioBufferMdl_ = pBufferMdl;
    *ActualSize_ = RequestedSize_;
    *OffsetFromFirstPage_ = 0;
    *CacheType_ = MmCached;

    return STATUS_SUCCESS;
}

//=============================================================================
#pragma code_seg()
NTSTATUS CMiniportWaveRTStream::GetPosition
(
    _Out_   KSAUDIO_POSITION    *Position_
)
{
    NTSTATUS ntStatus;

    KIRQL oldIrql;
    KeAcquireSpinLock(&m_PositionSpinLock, &oldIrql);

    if (m_KsState == KSSTATE_RUN)
    {
        //
        // Get the current time and update position.
        //
        LARGE_INTEGER ilQPC = KeQueryPerformanceCounter(NULL);
        UpdatePosition(ilQPC);
    }

    Position_->PlayOffset = m_ullPlayPosition;
    Position_->WriteOffset = m_ullWritePosition;

    KeReleaseSpinLock(&m_PositionSpinLock, oldIrql);

    ntStatus = STATUS_SUCCESS;
    
    return ntStatus;
}

//=============================================================================
// WaveRT GetReadPacket: one coherent snapshot of capture packet, DMA buffer,
// position, time and last-read state. PortCls can Free*Buffer concurrently,
// so none of the buffer divisors may be read after releasing the same lock.
#pragma code_seg()
static constexpr ULONGLONG CapturePacketFirstSampleByte(
    ULONGLONG completedPackets, ULONG packetSizeBytes)
{
    return (completedPackets - 1ull) * packetSizeBytes;
}
static_assert(CapturePacketFirstSampleByte(1, 960) == 0
    && CapturePacketFirstSampleByte(2, 960) == 960,
    "WaveRT capture packet timestamp refers to the FIRST sample");

// Convert an absolute 100-ns timestamp to raw QPC ticks. Multiplying the
// full 100-ns value by the QPC frequency before dividing overflows after
// sufficiently long uptime, even when the final answer fits in 64 bits.
// Split into whole seconds and fractional ticks, with checked arithmetic.
static BOOLEAN CaptureHnsToQpc(
    ULONGLONG hns, ULONGLONG frequency, ULONGLONG* result)
{
    constexpr ULONGLONG hnsPerSecond = 10000000ull;
    if (!result || !frequency || frequency > MAXULONGLONG / hnsPerSecond)
        return FALSE;
    const ULONGLONG seconds = hns / hnsPerSecond;
    const ULONGLONG fractionHns = hns % hnsPerSecond;
    if (seconds > MAXULONGLONG / frequency)
        return FALSE;
    const ULONGLONG wholeTicks = seconds * frequency;
    const ULONGLONG fractionalTicks =
        (fractionHns * frequency) / hnsPerSecond;
    if (wholeTicks > MAXULONGLONG - fractionalTicks)
        return FALSE;
    *result = wholeTicks + fractionalTicks;
    return TRUE;
}

_IRQL_requires_max_(PASSIVE_LEVEL)
NTSTATUS CMiniportWaveRTStream::GetReadPacket
(
    _Out_ ULONG* PacketNumber,
    _Out_ DWORD* Flags,
    _Out_ ULONG64* PerformanceCounterValue,
    _Out_ BOOL* MoreData
)
{
    if (!PacketNumber || !Flags || !PerformanceCounterValue || !MoreData)
        return STATUS_INVALID_PARAMETER;

    // No allocations, waiting, PortCls calls or pageable work while locked.
    // Free*Buffer uses m_PositionSpinLock to detach the MDL and clear these
    // fields; STOP also resets packet counters under this same lock.
    KIRQL oldIrql;
    KeAcquireSpinLock(&m_PositionSpinLock, &oldIrql);
    NTSTATUS status = STATUS_SUCCESS;
    if (!m_ulNotificationsPerBuffer)
    {
        status = STATUS_NOT_SUPPORTED;
    }
    else if (!m_pDmaMdl || !m_pDmaBuffer || !m_ulDmaBufferSize
        || !m_ulDmaMovementRate || m_ullPerformanceCounterFrequency.QuadPart <= 0
        || m_ulDmaBufferSize % m_ulNotificationsPerBuffer != 0)
    {
        status = STATUS_DEVICE_NOT_READY;
    }
    else if (m_KsState < KSSTATE_PAUSE)
    {
        status = STATUS_INVALID_DEVICE_STATE;
    }
    else if (m_llPacketCounter <= 0)
    {
        status = STATUS_DEVICE_NOT_READY;
    }
    else
    {
        const ULONGLONG completed = (ULONGLONG)m_llPacketCounter;
        const ULONG packetBytes = m_ulDmaBufferSize / m_ulNotificationsPerBuffer;
        if (!packetBytes || completed - 1ull > MAXULONGLONG / packetBytes)
        {
            status = STATUS_INTEGER_OVERFLOW;
        }
        else
        {
            const ULONG readyPacket = (ULONG)(completed - 1ull);
            if (readyPacket == m_ulLastOsReadPacket)
            {
                status = STATUS_DEVICE_NOT_READY;
            }
            else
            {
                const ULONGLONG packetStartBytes =
                    CapturePacketFirstSampleByte(completed, packetBytes);
                const ULONGLONG carryBytes =
                    (m_hnsElapsedTimeCarryForward * m_ulDmaMovementRate) / 10000000ull;
                if (m_ullLinearPosition > MAXULONGLONG - carryBytes)
                {
                    status = STATUS_INTEGER_OVERFLOW;
                }
                else
                {
                    const ULONGLONG currentBytes =
                        m_ullLinearPosition + carryBytes;
                    // When time advanced more than a complete cyclic buffer
                    // during suspend/stall, the producer discards stale DMA.
                    // No valid first-sample timestamp exists for the skipped
                    // packet: fail closed rather than underflowing arithmetic.
                    if (packetStartBytes > currentBytes
                        || currentBytes - packetStartBytes > m_ulDmaBufferSize)
                    {
                        status = STATUS_DATA_LATE_ERROR;
                    }
                    else
                    {
                        const ULONGLONG deltaBytes = currentBytes - packetStartBytes;
                        // deltaBytes is bounded to one DMA cycle (<= 96 kB),
                        // so this multiplication cannot overflow uint64.
                        const ULONGLONG deltaHns =
                            (deltaBytes * 10000000ull) / m_ulDmaMovementRate;
                        if (deltaHns > m_ullDmaTimeStamp)
                        {
                            status = STATUS_DATA_LATE_ERROR;
                        }
                        else
                        {
                            ULONGLONG qpcValue = 0;
                            const ULONGLONG firstSampleHns =
                                m_ullDmaTimeStamp - deltaHns;
                            if (!CaptureHnsToQpc(
                                firstSampleHns,
                                (ULONGLONG)m_ullPerformanceCounterFrequency.QuadPart,
                                &qpcValue))
                            {
                                status = STATUS_INTEGER_OVERFLOW;
                            }
                            else
                            {
                                *PacketNumber = readyPacket;
                                *Flags = 0;
                                *PerformanceCounterValue = qpcValue;
                                *MoreData = FALSE;
                                m_ulLastOsReadPacket = readyPacket;
                            }
                        }
                    }
                }
            }
        }
    }
    KeReleaseSpinLock(&m_PositionSpinLock, oldIrql);
    return status;
}

//=============================================================================
#pragma code_seg()
_IRQL_requires_max_(PASSIVE_LEVEL)
NTSTATUS CMiniportWaveRTStream::SetWritePacket
(
    _In_ ULONG      PacketNumber,
    _In_ DWORD      Flags,
    _In_ ULONG      EosPacketLength
)
{
    UNREFERENCED_PARAMETER(EosPacketLength);
    NTSTATUS ntStatus;

    // The call must be from event driven mode
    if (m_ulNotificationsPerBuffer == 0)
    {
        return STATUS_NOT_SUPPORTED;
    }

    ULONG oldLastOsWritePacket = m_ulLastOsWritePacket;

    // This function should not be called once EoS has been set.
    if (m_bEoSReceived)
    {
        return STATUS_INVALID_DEVICE_STATE;
    }

    KIRQL oldIrql;
    KeAcquireSpinLock(&m_PositionSpinLock, &oldIrql);
    // 1-based count of completed packets, 0-based packet number of current packet
    LONGLONG currentPacket = m_llPacketCounter;
    KeReleaseSpinLock(&m_PositionSpinLock, oldIrql);

    // If not running, the current packet hasn't actually started transfering so OS should be writing
    // to the current packet. If running, then the current packing is already transfering to hardware
    // so the OS should write the packet after the current packet.
    ULONG expectedPacket = LODWORD(currentPacket);
    if (m_KsState == KSSTATE_RUN)
    {
        expectedPacket++;
    }

    // Check if OS PacketNumber is behind or too far ahead of current packet
    LONG deltaFromExpectedPacket = PacketNumber - expectedPacket;   // Modulo arithemetic
    if (deltaFromExpectedPacket < 0)
    {
        return STATUS_DATA_LATE_ERROR;
    }
    else if (deltaFromExpectedPacket > 0)
    {
        return STATUS_DATA_OVERRUN;
    }

    ULONG packetSize = (m_ulDmaBufferSize / m_ulNotificationsPerBuffer);
    ULONG packetIndex = PacketNumber % m_ulNotificationsPerBuffer;
    ULONG ulCurrentWritePosition = packetIndex * packetSize;

    // Check if EOS flag was passed
    if (Flags & KSSTREAM_HEADER_OPTIONSF_ENDOFSTREAM)
    {
        return STATUS_INVALID_PARAMETER;
    }
    else
    {
        m_ulLastOsWritePacket = PacketNumber;

        // This function sets the current write position to the specified byte in the DMA buffer.
        // Will check if the write position is smaller than the DMA buffer size.
        // Will not return an error when the passed in parameter is 0.
        // Will also check if this function was called with the same write position(in event mode only)
        // Underruning will also be checked via timer mechanism
        KeAcquireSpinLock(&m_PositionSpinLock, &oldIrql);
        ntStatus = SetCurrentWritePositionInternal(ulCurrentWritePosition);
        KeReleaseSpinLock(&m_PositionSpinLock, oldIrql);
    }

    if (!NT_SUCCESS(ntStatus))
    {
        m_ulLastOsWritePacket = oldLastOsWritePacket;
    }

    return ntStatus;
}

//=============================================================================
#pragma code_seg()
_IRQL_requires_max_(PASSIVE_LEVEL)
NTSTATUS CMiniportWaveRTStream::GetOutputStreamPresentationPosition
(
    _Out_ KSAUDIO_PRESENTATION_POSITION *pPresentationPosition
)
{
    ASSERT (pPresentationPosition);
    
    // The call must be from event driven mode
    if(m_ulNotificationsPerBuffer == 0)
    {
        return STATUS_NOT_SUPPORTED;
    }
    
    return GetPresentationPosition(pPresentationPosition);
}

//=============================================================================
#pragma code_seg()
_IRQL_requires_max_(PASSIVE_LEVEL)
NTSTATUS CMiniportWaveRTStream::GetPacketCount
(
    _Out_ ULONG *pPacketCount
)
{
    ASSERT(pPacketCount);

    // The call must be from event driven mode
    if(m_ulNotificationsPerBuffer == 0)
    {
        return STATUS_NOT_SUPPORTED;
    }
    
    KIRQL oldIrql;
    KeAcquireSpinLock(&m_PositionSpinLock, &oldIrql);

    if (m_KsState == KSSTATE_RUN)
    {
        // Get the current time and update simulated position.
        LARGE_INTEGER ilQPC = KeQueryPerformanceCounter(NULL);
        UpdatePosition(ilQPC);
    }

    *pPacketCount = LODWORD(m_llPacketCounter);
    KeReleaseSpinLock(&m_PositionSpinLock, oldIrql);

    return STATUS_SUCCESS;
}

//linear and presentation positions
#pragma code_seg()
NTSTATUS CMiniportWaveRTStream::GetPositions(
    _Out_opt_  ULONGLONG* _pullLinearBufferPosition,
    _Out_opt_  ULONGLONG* _pullPresentationPosition,
    _Out_opt_  LARGE_INTEGER* _pliQPCTime
)
{
    DPF_ENTER(("[CMiniportWaveRTStream::GetPositions]"));

    NTSTATUS        ntStatus;
    LARGE_INTEGER   ilQPC;
    KIRQL           oldIrql;

    // Update *_pullLinearBufferPosition with the the number of bytes fetched from waveRT ever since a stream got set into RUN
    // state.
    // Once the stream is set to STOP state, any further read on this call would return zero.

    //
    // Get the current time and update position.
    //
    KeAcquireSpinLock(&m_PositionSpinLock, &oldIrql);
    ilQPC = KeQueryPerformanceCounter(NULL);
    if (m_KsState == KSSTATE_RUN)
    {
        UpdatePosition(ilQPC);
    }
    if (_pullLinearBufferPosition)
    {
        *_pullLinearBufferPosition = m_ullLinearPosition;
    }
    if (_pullPresentationPosition)
    {
        *_pullPresentationPosition = m_ullPresentationPosition;
    }
    KeReleaseSpinLock(&m_PositionSpinLock, oldIrql);
    if (_pliQPCTime)
    {
        *_pliQPCTime = ilQPC;
    }

    ntStatus = STATUS_SUCCESS;

    return ntStatus;
}

NTSTATUS CMiniportWaveRTStream::GetPresentationPosition(_Out_  KSAUDIO_PRESENTATION_POSITION* _pPresentationPosition)
{
    ASSERT(_pPresentationPosition);
    LARGE_INTEGER timeStamp;

    DPF_ENTER(("[CMiniportWaveRTStream::GetPresentationPosition]"));

    ULONGLONG ullLinearPosition = { 0 };
    ULONGLONG ullPresentationPosition = { 0 };
    NTSTATUS status = STATUS_SUCCESS;

    status = GetPositions(&ullLinearPosition, &ullPresentationPosition, &timeStamp);
    if (!NT_SUCCESS(status))
    {
        return status;
    }

    // A position in blocks is bytes / blockAlign; multiplication by the
    // sample rate can wrap 64-bit after long capture sessions.
    const ULONG blockAlign = m_pWfExt->Format.nBlockAlign;
    if (!blockAlign) return STATUS_INVALID_DEVICE_STATE;
    _pPresentationPosition->u64PositionInBlocks = ullPresentationPosition / blockAlign;
    _pPresentationPosition->u64QPCPosition = (UINT64)timeStamp.QuadPart;

    return STATUS_SUCCESS;
}

#pragma code_seg()
NTSTATUS CMiniportWaveRTStream::SetCurrentWritePositionInternal(_In_  ULONG _ulCurrentWritePosition)
{
    DPF_ENTER(("[CMiniportWaveRTStream::SetCurrentWritePositionInternal]"));

    ASSERT(m_bEoSReceived == FALSE);

    if (m_bEoSReceived)
    {
        return STATUS_INVALID_DEVICE_REQUEST;
    }

    if (_ulCurrentWritePosition > m_ulDmaBufferSize)
    {
        return STATUS_INVALID_DEVICE_REQUEST;
    }

    PADAPTERCOMMON pAdapterComm = m_pMiniport->GetAdapterCommObj();

    //Event type: eMINIPORT_SET_WAVERT_BUFFER_WRITE_POSITION
    //Parameter 1: Current linear buffer position    
    //Parameter 2: Previous WaveRtBufferWritePosition that the driver received    
    //Parameter 3: Target WaveRtBufferWritePosition received from portcls
    //Parameter 4: 0
    pAdapterComm->WriteEtwEvent(eMINIPORT_SET_WAVERT_BUFFER_WRITE_POSITION,
        m_ullLinearPosition, // replace with the correct "Current linear buffer position"    
        m_ulCurrentWritePosition,
        _ulCurrentWritePosition, // this is new write position
        0); // always zero

//
// Check for eMINIPORT_GLITCH_REPORT - Same WaveRT buffer write during event driven mode.
//
    if (m_ulNotificationIntervalMs > 0)
    {
        if (m_ulCurrentWritePosition == _ulCurrentWritePosition)
        {
            //Event type: eMINIPORT_GLITCH_REPORT
            //Parameter 1: Current linear buffer position 
            //Parameter 2: Previous WaveRtBufferWritePosition that the driver received 
            //Parameter 3: Major glitch code: 3: Received same WaveRT buffer twice in a row during event driven mode
            //Parameter 4: Minor code for the glitch cause
            pAdapterComm->WriteEtwEvent(eMINIPORT_GLITCH_REPORT,
                m_ullLinearPosition, // replace with the correct "Current linear buffer position"
                m_ulCurrentWritePosition,
                3, // received same WaveRT buffer twice in a row during event driven mode
                _ulCurrentWritePosition);
        }
    }

    m_ulCurrentWritePosition = _ulCurrentWritePosition;
    InterlockedExchange(&m_IsCurrentWritePositionUpdated, 1);

    return STATUS_SUCCESS;
}

//=============================================================================
#pragma code_seg()
NTSTATUS CMiniportWaveRTStream::SetState
(
    _In_    KSSTATE State_
)
{
    NTSTATUS        ntStatus        = STATUS_SUCCESS;
    KIRQL oldIrql;

    // Spew an event for a pin state change request from portcls
    //Event type: eMINIPORT_PIN_STATE
    switch (State_)
    {
        case KSSTATE_STOP:
            // A RUN -> STOP transition may bypass PAUSE. Cancel the WaveRT
            // notification timer before releasing DMA state or stream buffers.
            if (m_KsState == KSSTATE_RUN && m_ulNotificationIntervalMs > 0)
            {
                ExCancelTimer(m_pNotificationTimer, NULL);
                KeFlushQueuedDpcs();
            }
            KeAcquireSpinLock(&m_PositionSpinLock, &oldIrql);
            // Reset DMA
            m_llPacketCounter = 0;
            m_ullPlayPosition = 0;
            m_ullWritePosition = 0;
            m_ullLinearPosition = 0;
            m_TranslateITCaptureReadFrame = 0;
            m_TranslateITCaptureSerial = 0;
            m_ullPresentationPosition = 0;
            m_hnsElapsedTimeCarryForward = 0;
            m_hnsDPCTimeCarryForward = 0;
            m_byteDisplacementCarryForward = 0;
            if (m_bCapture && m_pDmaBuffer && m_ulDmaBufferSize)
            {
                // Stop revokes any sample bytes already stored in the
                // Windows cyclic buffer (not just the producer FIFO).
                RtlZeroMemory(m_pDmaBuffer, m_ulDmaBufferSize);
            }

            // Reset OS read/write positions
            m_ulLastOsReadPacket = ULONG_MAX;
            m_ulCurrentWritePosition = 0;
            m_ulLastOsWritePacket = ULONG_MAX;
            m_bEoSReceived = FALSE;
            m_bLastBufferRendered = FALSE;

            KeReleaseSpinLock(&m_PositionSpinLock, oldIrql);

            // Wait until all work items are completed.
            if (!m_bCapture && !g_DoNotCreateDataFiles)
            {
                m_SaveData.WaitAllWorkItems();
            }
            break;

        case KSSTATE_ACQUIRE:
            if (m_KsState == KSSTATE_STOP)
            {
                // Acquire stream resources
            }
            break;
            
        case KSSTATE_PAUSE:

            if (m_KsState > KSSTATE_PAUSE)
            {
                //
                // Run -> Pause
                //

                // Pause DMA
                if (m_ulNotificationIntervalMs > 0)
                {
                    ExCancelTimer(m_pNotificationTimer, NULL);
                    KeFlushQueuedDpcs(); 

                    // Pending notification carry is discarded on the next
                    // RUN; it must not accumulate suspended CPU time.
                }
            }
            // This call updates the linear buffer and presentation positions.
            GetPositions(NULL, NULL, NULL);
            break;

        case KSSTATE_RUN:
            if (m_bCapture && m_pDmaBuffer && m_ulDmaBufferSize)
            {
                // No stale microphone samples may appear before the first
                // admitted packet after resume/source switch.
                RtlZeroMemory(m_pDmaBuffer, m_ulDmaBufferSize);
            }
            // A resumed stream starts a new notification epoch, not a burst
            // of timer events accrued during a long OS pause.
            m_hnsDPCTimeCarryForward = 0;
            // Reset only THIS client's cursor, not every capture reader.
            m_TranslateITCaptureReadFrame = 0;
            m_TranslateITCaptureSerial = 0;
            // Start DMA
            LARGE_INTEGER ullPerfCounterTemp;
            ullPerfCounterTemp = KeQueryPerformanceCounter(&m_ullPerformanceCounterFrequency);
            m_ullLastDPCTimeStamp = m_ullDmaTimeStamp = KSCONVERT_PERFORMANCE_TIME(m_ullPerformanceCounterFrequency.QuadPart, ullPerfCounterTemp);

            if (m_ulNotificationIntervalMs > 0)
            {
                // Set timer for 1 ms. This will cause DPC to run every 1 ms but driver will send out 
                // notification events only after notification interval. This timer is used by Simple Audio Sample to 
                // emulate hardware and send out notification event. Real hardware should not use this
                // timer to fire notification event as it will drain power if the timer is running at 1 msec.
                ExSetTimer
                (
                    m_pNotificationTimer,
                    (-1) * HNSTIME_PER_MILLISECOND,
                    HNSTIME_PER_MILLISECOND, // 1 ms 
                    NULL
                 );

            }

            break;
    }

    m_KsState = State_;

    return ntStatus;
}

//=============================================================================
#pragma code_seg("PAGE")
NTSTATUS CMiniportWaveRTStream::SetFormat
(
    _In_    KSDATAFORMAT    *DataFormat_
)
{
    UNREFERENCED_PARAMETER(DataFormat_);

    PAGED_CODE();

    //if (!m_fCapture && !g_DoNotCreateDataFiles)
    //{
    //    ntStatus = m_SaveData.SetDataFormat(Format);
    //}

    return STATUS_NOT_SUPPORTED;
}

#pragma code_seg()

//=============================================================================
#pragma code_seg()
VOID CMiniportWaveRTStream::UpdatePosition
(
    _In_ LARGE_INTEGER ilQPC
)
{
    const LONGLONG converted = KSCONVERT_PERFORMANCE_TIME(
        m_ullPerformanceCounterFrequency.QuadPart, ilQPC);
    if (converted < 0) return;
    const ULONGLONG nowHns = (ULONGLONG)converted;
    if (nowHns < m_ullDmaTimeStamp)
    {
        // Clock discontinuity: do not cast a negative interval into a huge
        // forward displacement or replay buffered speech.
        m_ullDmaTimeStamp = nowHns;
        m_hnsElapsedTimeCarryForward = 0;
        m_byteDisplacementCarryForward = 0;
        if (m_bCapture)
        {
            if (m_pDmaBuffer && m_ulDmaBufferSize)
            {
                RtlZeroMemory(m_pDmaBuffer, m_ulDmaBufferSize);
            }
            TranslateITCaptureDiscardStaleFrames(
                &m_TranslateITCaptureReadFrame, &m_TranslateITCaptureSerial);
        }
        return;
    }
    if (!m_pDmaBuffer || !m_ulDmaBufferSize || !m_ulDmaMovementRate)
    {
        // Buffer allocation/release is not an audio event. Avoid modulo 0.
        m_ullDmaTimeStamp = nowHns;
        m_hnsElapsedTimeCarryForward = 0;
        m_byteDisplacementCarryForward = 0;
        return;
    }
    const ULONGLONG elapsedHns =
        nowHns - m_ullDmaTimeStamp + m_hnsElapsedTimeCarryForward;
    const ULONGLONG elapsedMs = elapsedHns / 10000ull;
    m_hnsElapsedTimeCarryForward = elapsedHns % 10000ull;

    const ULONGLONG byteNumerator = CaptureDisplacementNumerator(
        m_ulDmaMovementRate, elapsedMs, m_byteDisplacementCarryForward);
    const ULONGLONG advancedBytes = byteNumerator / 1000ull;
    m_byteDisplacementCarryForward = (ULONG)(byteNumerator % 1000ull);

    // Always advance the logical sample clock, even when a delayed tick has
    // skipped multiple cyclic buffers. Do NOT fill the skipped duration
    // under the spinlock: drop stale PCM and explicitly silence the complete
    // bounded buffer instead.
    m_ullPresentationPosition += advancedBytes;
    if (m_bCapture)
    {
        if (advancedBytes > m_ulDmaBufferSize)
        {
            RtlZeroMemory(m_pDmaBuffer, m_ulDmaBufferSize);
            TranslateITCaptureDiscardStaleFrames(
                &m_TranslateITCaptureReadFrame, &m_TranslateITCaptureSerial);
        }
        else
        {
            WriteBytes((ULONG)advancedBytes);
        }
    }
    const ULONG cyclicBytes = (ULONG)(advancedBytes % m_ulDmaBufferSize);
    m_ullPlayPosition = m_ullWritePosition =
        (m_ullWritePosition + cyclicBytes) % m_ulDmaBufferSize;
    m_ullLinearPosition += advancedBytes;
    m_ullDmaTimeStamp = nowHns;
}

//=============================================================================
#pragma code_seg()
VOID CMiniportWaveRTStream::WriteBytes
(
    _In_ ULONG ByteDisplacement
)
/*++

Routine Description:

This function reads the current producer into each capture DMA buffer

Arguments:

ByteDisplacement - # of bytes to process.

--*/
{
    if (m_pDmaBuffer == NULL || m_ulDmaBufferSize == 0) return;
    ULONG bufferOffset = m_ullLinearPosition % m_ulDmaBufferSize;

    // Normally this will loop no more than once for a single wrap, but if
    // many bytes have been displaced then this may loops many times.
    while (ByteDisplacement > 0)
    {
        ULONG runWrite = min(ByteDisplacement, m_ulDmaBufferSize - bufferOffset);
        
        TranslateITCaptureRead(m_pDmaBuffer + bufferOffset, runWrite,
            &m_TranslateITCaptureReadFrame, &m_TranslateITCaptureSerial);
           	
        bufferOffset = (bufferOffset + runWrite) % m_ulDmaBufferSize;
        ByteDisplacement -= runWrite;
    }
}

//=============================================================================
#pragma code_seg("PAGE")
STDMETHODIMP_(NTSTATUS) 
CMiniportWaveRTStream::SetContentId
(
    _In_  ULONG                   contentId,
    _In_  PCDRMRIGHTS             drmRights
)
/*++

Routine Description:

  Sets DRM content Id for this stream. Also updates the Mixed content Id.

Arguments:

  contentId - new content id

  drmRights - rights for this stream.

Return Value:

  NT status code.

--*/
{
    PAGED_CODE();

    DPF_ENTER(("[CMiniportWaveRT::SetContentId]"));

    NTSTATUS    ntStatus;
    ULONG       ulOldContentId = contentId;

    m_ulContentId = contentId;

    //
    // Miniport should create a mixed DrmRights.
    //
    ntStatus = m_pMiniport->UpdateDrmRights();

    //
    // Restore the passed-in content Id.
    //
    if (!NT_SUCCESS(ntStatus))
    {
        m_ulContentId = ulOldContentId;
    }

    //
    // Simple Audio Sample writes each stream seperately to disk. If the rights for this
    // stream indicates that the stream is CopyProtected, stop writing to disk.
    //
    m_SaveData.Disable(drmRights->CopyProtect);

    //
    // From MSDN:
    //
    // This sample doesn't forward protected content, but if your driver uses 
    // lower layer drivers or a different stack to properly work, please see the 
    // following info from MSDN:
    //
    // "Before allowing protected content to flow through a data path, the system
    // verifies that the data path is secure. To do so, the system authenticates
    // each module in the data path beginning at the upstream end of the data path
    // and moving downstream. As each module is authenticated, that module gives
    // the system information about the next module in the data path so that it
    // can also be authenticated. To be successfully authenticated, a module's 
    // binary file must be signed as DRM-compliant.
    //
    // Two adjacent modules in the data path can communicate with each other in 
    // one of several ways. If the upstream module calls the downstream module 
    // through IoCallDriver, the downstream module is part of a WDM driver. In 
    // this case, the upstream module calls the DrmForwardContentToDeviceObject
    // function to provide the system with the device object representing the 
    // downstream module. (If the two modules communicate through the downstream
    // module's COM interface or content handlers, the upstream module calls 
    // DrmForwardContentToInterface or DrmAddContentHandlers instead.)
    //
    // DrmForwardContentToDeviceObject performs the same function as 
    // PcForwardContentToDeviceObject and IDrmPort2::ForwardContentToDeviceObject." 
    //
    // Other supported DRM DDIs for down-level module validation are: 
    // DrmForwardContentToInterfaces and DrmAddContentHandlers.
    //
    // For more information, see MSDN's DRM Functions and Interfaces.
    //

    return ntStatus;
} // SetContentId

//=============================================================================
#pragma code_seg()
void
TimerNotifyRT
(
    _In_      PEX_TIMER    Timer,
    _In_opt_  PVOID        DeferredContext
)
{
    LARGE_INTEGER qpc;
    LARGE_INTEGER qpcFrequency;
    BOOL bufferCompleted = FALSE;

    UNREFERENCED_PARAMETER(Timer);

    _IRQL_limited_to_(DISPATCH_LEVEL);

    CMiniportWaveRTStream* _this = (CMiniportWaveRTStream*)DeferredContext;
    
    if (NULL == _this)
    {
        return;
    }

    // Must be declared before any goto End to avoid skipped initializers.
    PADAPTERCOMMON pAdapterComm = NULL;
    KIRQL oldIrql;
    KeAcquireSpinLock(&_this->m_PositionSpinLock, &oldIrql);

    qpc = KeQueryPerformanceCounter(&qpcFrequency);

    // Convert ticks to 100ns units.
    LONGLONG  hnsCurrentTime = KSCONVERT_PERFORMANCE_TIME(_this->m_ullPerformanceCounterFrequency.QuadPart, qpc);

    // Do not advance a capture packet or signal events on an in-flight
    // cancelled timer callback after PAUSE/STOP. The position spinlock is
    // the same one that protects transport state changes.
    if (_this->m_KsState != KSSTATE_RUN)
    {
        goto End;
    }

    // Use a nested scope so early goto End never skips initialization of
    // local C++ variables (MSVC C2362). Only one notification per callback.
    if (hnsCurrentTime >= 0 && _this->m_ulNotificationIntervalMs != 0)
    {
        const ULONGLONG nowHns = (ULONGLONG)hnsCurrentTime;
        if (nowHns < _this->m_ullLastDPCTimeStamp)
        {
            _this->m_ullLastDPCTimeStamp = nowHns;
            _this->m_hnsDPCTimeCarryForward = 0;
        }
        else
        {
            const ULONGLONG elapsedHns =
                nowHns - _this->m_ullLastDPCTimeStamp
                + _this->m_hnsDPCTimeCarryForward;
            const ULONGLONG periodHns =
                (ULONGLONG)_this->m_ulNotificationIntervalMs * 10000ull;
            if (elapsedHns >= periodHns)
            {
                // Discard whole missed notification intervals; retaining
                // them would spam clients after a long sleep or DPC stall.
                _this->m_hnsDPCTimeCarryForward = elapsedHns % periodHns;
                _this->m_ullLastDPCTimeStamp = nowHns;
                bufferCompleted = TRUE;
            }
        }
    }

    if (!bufferCompleted && !_this->m_bEoSReceived)
    {
        goto End;
    }

    _this->UpdatePosition(qpc);

    if (!_this->m_bEoSReceived)
    {
        _this->m_llPacketCounter++;
    }

    pAdapterComm = _this->m_pMiniport->GetAdapterCommObj();

    // Simple buffer underrun detection.
    if (!_this->IsCurrentWaveRTWritePositionUpdated() && !_this->m_bEoSReceived)
    {
        //Event type: eMINIPORT_GLITCH_REPORT
        //Parameter 1: Current linear buffer position 
        //Parameter 2: Previous WaveRtBufferWritePosition that the driver received 
        //Parameter 3: Major glitch code: 1:WaveRT buffer is underrun
        //Parameter 4: Minor code for the glitch cause
        pAdapterComm->WriteEtwEvent(eMINIPORT_GLITCH_REPORT, 
                                    _this->m_ullLinearPosition,
                                    _this->GetCurrentWaveRTWritePosition(),
                                    1,      // WaveRT buffer is underrun
                                    0); 
    }

    // Send buffer completion event if either of the following is true
    // 1. Driver consumed a complete buffer for this stream
    // 2. Driver consumed a partial buffer containing EoS for this stream

    if (!IsListEmpty(&_this->m_NotificationList) && 
        (bufferCompleted || _this->m_bLastBufferRendered))
    {
        PLIST_ENTRY leCurrent = _this->m_NotificationList.Flink;
        while (leCurrent != &_this->m_NotificationList)
        {
            NotificationListEntry* nleCurrent = CONTAINING_RECORD( leCurrent, NotificationListEntry, ListEntry);
            KeSetEvent(nleCurrent->NotificationEvent, 0, 0);

            leCurrent = leCurrent->Flink;
        }
    }

    if (_this->m_bLastBufferRendered)
    {
        ExCancelTimer(_this->m_pNotificationTimer, NULL);
    }

End:
    KeReleaseSpinLock(&_this->m_PositionSpinLock, oldIrql);
    return;
}
//=============================================================================

