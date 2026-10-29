/*++

Copyright (c) Microsoft Corporation All Rights Reserved

Module Name:

    micarraytopo.cpp

Abstract:

    Implementation of topology miniport for the mic array.

--*/

#pragma warning (disable : 4127)

#include "definitions.h"
#include "endpoints.h"
#include "common.h"
#include "kshelper.h"
#include "micarraytopo.h"
#include "micarray1toptable.h"

#pragma code_seg("PAGE")

//=============================================================================
NTSTATUS
CreateMicArrayMiniportTopology
(
    _Out_           PUNKNOWN* Unknown,
    _In_            REFCLSID,
    _In_opt_        PUNKNOWN                                UnknownOuter,
    _In_            POOL_FLAGS                              PoolFlags,
    _In_            PUNKNOWN                                UnknownAdapter,
    _In_opt_        PVOID                                   DeviceContext,
    _In_            PENDPOINT_MINIPAIR                      MiniportPair
)
/*++

Routine Description:

    Creates a new topology miniport.

Arguments:

  Unknown -

  RefclsId -

  UnknownOuter -

  PoolType -

  UnknownAdapter -

  DeviceContext -

  MiniportPair -

Return Value:

  NT status code.

--*/
{
    PAGED_CODE();

    ASSERT(Unknown);
    ASSERT(MiniportPair);

    UNREFERENCED_PARAMETER(UnknownAdapter);
    UNREFERENCED_PARAMETER(DeviceContext);

    CMicArrayMiniportTopology* obj =
        new (PoolFlags, MINTOPORT_POOLTAG)
        CMicArrayMiniportTopology(UnknownOuter,
            MiniportPair->TopoDescriptor,
            MiniportPair->DeviceMaxChannels,
            MiniportPair->DeviceType);
    if (NULL == obj)
    {
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    obj->AddRef();

    *Unknown = reinterpret_cast<IUnknown*>(obj);

    return STATUS_SUCCESS;
} // CreateMicArrayMiniportTopology

//=============================================================================
CMicArrayMiniportTopology::~CMicArrayMiniportTopology
(
    void
)
/*++

Routine Description:

  Topology miniport destructor

Arguments:

Return Value:

  NT status code.

--*/
{
    PAGED_CODE();

    DPF_ENTER(("[CMicArrayMiniportTopology::~CMicArrayMiniportTopology]"));
} // ~CMicArrayMiniportTopology

//=============================================================================
NTSTATUS
CMicArrayMiniportTopology::DataRangeIntersection
(
    _In_        ULONG                   PinId,
    _In_        PKSDATARANGE            ClientDataRange,
    _In_        PKSDATARANGE            MyDataRange,
    _In_        ULONG                   OutputBufferLength,
    _Out_writes_bytes_to_opt_(OutputBufferLength, *ResultantFormatLength)
    PVOID                   ResultantFormat     OPTIONAL,
    _Out_       PULONG                  ResultantFormatLength
)
/*++

Routine Description:

  The DataRangeIntersection function determines the highest quality
  intersection of two data ranges.

Arguments:

  PinId - Pin for which data intersection is being determined.

  ClientDataRange - Pointer to KSDATARANGE structure which contains the data range
                    submitted by client in the data range intersection property
                    request.

  MyDataRange - Pin's data range to be compared with client's data range.

  OutputBufferLength - Size of the buffer pointed to by the resultant format
                       parameter.

  ResultantFormat - Pointer to value where the resultant format should be
                    returned.

  ResultantFormatLength - Actual length of the resultant format that is placed
                          at ResultantFormat. This should be less than or equal
                          to OutputBufferLength.

Return Value:

  NT status code.

--*/
{
    PAGED_CODE();

    return
        CMiniportTopologySimpleAudioSample::DataRangeIntersection
        (
            PinId,
            ClientDataRange,
            MyDataRange,
            OutputBufferLength,
            ResultantFormat,
            ResultantFormatLength
        );
} // DataRangeIntersection

//=============================================================================
STDMETHODIMP
CMicArrayMiniportTopology::GetDescription
(
    _Out_ PPCFILTER_DESCRIPTOR* OutFilterDescriptor
)
/*++

Routine Description:

  The GetDescription function gets a pointer to a filter description.
  It provides a location to deposit a pointer in miniport's description
  structure. This is the placeholder for the FromNode or ToNode fields in
  connections which describe connections to the filter's pins.

Arguments:

  OutFilterDescriptor - Pointer to the filter description.

Return Value:

  NT status code.

--*/
{
    PAGED_CODE();

    ASSERT(OutFilterDescriptor);

    return CMiniportTopologySimpleAudioSample::GetDescription(OutFilterDescriptor);
} // GetDescription

//=============================================================================
STDMETHODIMP
CMicArrayMiniportTopology::Init
(
    _In_ PUNKNOWN                 UnknownAdapter,
    _In_ PRESOURCELIST            ResourceList,
    _In_ PPORTTOPOLOGY            Port_
)
/*++

Routine Description:

  The Init function initializes the miniport. Callers of this function
  should run at IRQL PASSIVE_LEVEL

Arguments:

  UnknownAdapter - A pointer to the IUnknown interface of the adapter object.

  ResourceList - Pointer to the resource list to be supplied to the miniport
                 during initialization. The port driver is free to examine the
                 contents of the ResourceList. The port driver will not be
                 modify the ResourceList contents.

  Port - Pointer to the topology port object that is linked with this miniport.

Return Value:

  NT status code.

--*/
{
    UNREFERENCED_PARAMETER(ResourceList);

    PAGED_CODE();

    ASSERT(UnknownAdapter);
    ASSERT(Port_);

    DPF_ENTER(("[CMicArrayMiniportTopology::Init]"));

    NTSTATUS                    ntStatus;

    ntStatus =
        CMiniportTopologySimpleAudioSample::Init
        (
            UnknownAdapter,
            Port_
        );

    return ntStatus;
} // Init

//=============================================================================
STDMETHODIMP
CMicArrayMiniportTopology::NonDelegatingQueryInterface
(
    _In_         REFIID                  Interface,
    _COM_Outptr_ PVOID* Object
)
/*++

Routine Description:

  QueryInterface for MiniportTopology

Arguments:

  Interface - GUID of the interface

  Object - interface object to be returned.

Return Value:

  NT status code.

--*/
{
    PAGED_CODE();

    ASSERT(Object);

    if (IsEqualGUIDAligned(Interface, IID_IUnknown))
    {
        *Object = PVOID(PUNKNOWN(this));
    }
    else if (IsEqualGUIDAligned(Interface, IID_IMiniport))
    {
        *Object = PVOID(PMINIPORT(this));
    }
    else if (IsEqualGUIDAligned(Interface, IID_IMiniportTopology))
    {
        *Object = PVOID(PMINIPORTTOPOLOGY(this));
    }
    else
    {
        *Object = NULL;
    }

    if (*Object)
    {
        // We reference the interface for the caller.
        PUNKNOWN(*Object)->AddRef();
        return(STATUS_SUCCESS);
    }

    return(STATUS_INVALID_PARAMETER);
} // NonDelegatingQueryInterface

//=============================================================================
NTSTATUS
PropertyHandler_MicArrayTopology
(
    _In_ PPCPROPERTY_REQUEST      PropertyRequest
)
/*++

Routine Description:

  Redirects property request to miniport object

Arguments:

  PropertyRequest -

Return Value:

  NT status code.

--*/
{
    PAGED_CODE();

    ASSERT(PropertyRequest);

    DPF_ENTER(("[PropertyHandler_MicArrayTopology]"));

    // PropertryRequest structure is filled by portcls. 
    // MajorTarget is a pointer to miniport object for miniports.
    //
    PCMicArrayMiniportTopology pMiniport = (PCMicArrayMiniportTopology)PropertyRequest->MajorTarget;

    return pMiniport->PropertyHandlerGeneric(PropertyRequest);
} // PropertyHandler_MicArrayTopology

#pragma code_seg()
