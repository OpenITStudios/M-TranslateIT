
/*++

Copyright (c) Microsoft Corporation All Rights Reserved

Module Name:

    micarraytopo.h

Abstract:

    Declaration of mic array topology miniport.

--*/

#ifndef _SIMPLEAUDIOSAMPLE_MICARRAYTOPO_H_
#define _SIMPLEAUDIOSAMPLE_MICARRAYTOPO_H_

#include "basetopo.h"

//=============================================================================
// Classes
//=============================================================================

///////////////////////////////////////////////////////////////////////////////
// CMicArrayMiniportTopology 
//   

#pragma code_seg()
class CMicArrayMiniportTopology :
    public CMiniportTopologySimpleAudioSample,
    public IMiniportTopology,
    public CUnknown
{
public:
    DECLARE_STD_UNKNOWN();
    CMicArrayMiniportTopology
    (
        _In_opt_    PUNKNOWN                UnknownOuter,
        _In_        PCFILTER_DESCRIPTOR* FilterDesc,
        _In_        USHORT                  DeviceMaxChannels,
        _In_        eDeviceType             DeviceType
    )
        : CUnknown(UnknownOuter),
        CMiniportTopologySimpleAudioSample(FilterDesc, DeviceMaxChannels),
        m_DeviceType(DeviceType)
    {
        ASSERT(m_DeviceType == eMicArrayDevice1);
    }

    ~CMicArrayMiniportTopology();

    IMP_IMiniportTopology;

    // Only the generic volume/mute topology is offered. A virtual
    // microphone cannot report physical array geometry or jack placement.

protected:
    eDeviceType     m_DeviceType;

};

typedef CMicArrayMiniportTopology* PCMicArrayMiniportTopology;


NTSTATUS
CreateMicArrayMiniportTopology(
    _Out_           PUNKNOWN* Unknown,
    _In_            REFCLSID,
    _In_opt_        PUNKNOWN                                UnknownOuter,
    _In_            POOL_FLAGS                              PoolFlags,
    _In_            PUNKNOWN                                UnknownAdapter,
    _In_opt_        PVOID                                   DeviceContext,
    _In_            PENDPOINT_MINIPAIR                      MiniportPair
);

NTSTATUS PropertyHandler_MicArrayTopology(_In_ PPCPROPERTY_REQUEST PropertyRequest);

#endif // _SIMPLEAUDIOSAMPLE_MICARRAYTOPO_H_
