import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const files = {
  meeting: resolve(root, "src-tauri/src/commands/meeting_session.rs"),
  meetingLifecycle: resolve(root, "src-tauri/src/commands/meeting_session/lifecycle.rs"),
  runtimeCommands: resolve(root, "src-tauri/src/commands/runtime.rs"),
  outboundPipeline: resolve(root, "src-tauri/src/commands/meeting_session/outbound_pipeline.rs"),
  playbackRuntime: resolve(root, "src-tauri/src/commands/meeting_session/playback_runtime.rs"),
  route: resolve(root, "src-tauri/src/commands/virtual_mic_route.rs"),
  pairSelection: resolve(root, "src-tauri/src/commands/virtual_mic_route/pair_selection.rs"),
  nativeInf: resolve(root, "src-tauri/windows/virtual-audio/Source/Main/TranslateITMic.inx"),
  nativeProtocol: resolve(root, "src-tauri/windows/virtual-audio/Source/Main/capture_control_protocol.h"),
  nativeControl: resolve(root, "src-tauri/windows/virtual-audio/Source/Main/capture_control_device.cpp"),
  nativePcm: resolve(root, "src-tauri/windows/virtual-audio/Source/Main/capture_pcm.cpp"),
  nativePcmHeader: resolve(root, "src-tauri/windows/virtual-audio/Source/Main/capture_pcm.h"),
  nativeAdapter: resolve(root, "src-tauri/windows/virtual-audio/Source/Main/adapter.cpp"),
  nativeMiniPairs: resolve(root, "src-tauri/windows/virtual-audio/Source/Filters/minipairs.h"),
  nativeCaptureTopology: resolve(root, "src-tauri/windows/virtual-audio/Source/Filters/micarray1toptable.h"),
  nativeCaptureTopologyImpl: resolve(root, "src-tauri/windows/virtual-audio/Source/Filters/micarraytopo.cpp"),
  nativeCaptureTopologyHeader: resolve(root, "src-tauri/windows/virtual-audio/Source/Filters/micarraytopo.h"),
  nativeProject: resolve(root, "src-tauri/windows/virtual-audio/Source/Main/Main.vcxproj"),
  nativeRust: resolve(root, "src-tauri/src/engine/audio/native_capture_transport.rs"),
  nativeRustPipe: resolve(root, "src-tauri/src/engine/audio/native_capture_pipe.rs"),
  nativeRustPacing: resolve(root, "src-tauri/src/engine/audio/native_capture_pacing.rs"),
  brokerProtocol: resolve(root, "src-tauri/windows/virtual-audio/Broker/broker_protocol.h"),
  brokerDevice: resolve(root, "src-tauri/windows/virtual-audio/Broker/broker_driver.cpp"),
  brokerIdentity: resolve(root, "src-tauri/windows/virtual-audio/Broker/broker_identity.cpp"),
  brokerIdentityHeader: resolve(root, "src-tauri/windows/virtual-audio/Broker/broker_identity.h"),
  brokerIo: resolve(root, "src-tauri/windows/virtual-audio/Broker/broker_io.cpp"),
  brokerService: resolve(root, "src-tauri/windows/virtual-audio/Broker/broker_service.cpp"),
  brokerProject: resolve(root, "src-tauri/windows/virtual-audio/Broker/TranslateITMicBroker.vcxproj"),
  nativeSolution: resolve(root, "src-tauri/windows/virtual-audio/TranslateITMic.sln"),
  nativeStream: resolve(root, "src-tauri/windows/virtual-audio/Source/Main/minwavertstream.cpp"),
  nativeStreamHeader: resolve(root, "src-tauri/windows/virtual-audio/Source/Main/minwavertstream.h"),
  nativeCaptureFormat: resolve(root, "src-tauri/windows/virtual-audio/Source/Filters/micarraywavtable.h"),
  meetingOutput: resolve(root, "src-tauri/src/engine/audio/meeting_output.rs"),
  meetingOutputRuntime: resolve(root, "src-tauri/src/engine/audio/meeting_output_runtime.rs"),
  audioMod: resolve(root, "src-tauri/src/engine/audio/mod.rs"),
  commandsMod: resolve(root, "src-tauri/src/commands/mod.rs"),
  registry: resolve(root, "src-tauri/src/commands/registry.rs"),
};
const retired = [
  resolve(root, "src-tauri/src/commands/virtual_audio_route_runtime.rs"),
  resolve(root, "../../Backend/LocalWorker/WorkerRuntime/virtual_audio_route_provider.py"),
  resolve(root, "src-tauri/windows/virtual-audio/Source/Main/pcm_bridge.cpp"),
  resolve(root, "src-tauri/windows/virtual-audio/Source/Main/pcm_bridge.h"),
  resolve(root, "src-tauri/windows/virtual-audio/Source/Main/TranslateITVA.inx"),
];

const source = {};
for (const [name, path] of Object.entries(files)) {
  if (!existsSync(path)) throw new Error(`Missing ${name}: ${path}`);
  source[name] = readFileSync(path, "utf8");
}
for (const path of retired) if (existsSync(path)) throw new Error(`Retired Python/command route owner still exists: ${path}`);

function requireMarkers(body, label, markers) {
  for (const marker of markers) if (!body.includes(marker)) throw new Error(`${label} marker missing: ${marker}`);
}
function forbidMarkers(body, label, markers) {
  for (const marker of markers) if (body.includes(marker)) throw new Error(`${label} forbidden marker found: ${marker}`);
}

requireMarkers(source.audioMod, "Windows audio ownership", ["pub mod meeting_output;"]);
forbidMarkers(source.commandsMod, "retired command-layer route owner", ["pub mod virtual_audio_route_runtime;"]);
requireMarkers(source.meeting, "Meeting session output orchestration facade", [
  "mod lifecycle;",
  "mod outbound_pipeline;",
  "mod playback_runtime;",
  "process_outbound_wav",
]);
requireMarkers(source.runtimeCommands, "Fresh Meeting route preparation owner", [
  "clear_prepared_virtual_mic_route_selection",
  "prepare_current_virtual_mic_route_for_meeting",
  "meeting_session::start_meeting_translation",
]);
requireMarkers(source.meetingLifecycle, "Meeting lifecycle output orchestration", [
  "bind_prepared_virtual_mic_route_to_generation",
  "prepare_meeting_output_device",
  "cancel_meeting_output_for_generation",
  "get_virtual_mic_route_selection",
]);
forbidMarkers(source.meetingLifecycle, "duplicate Meeting route preparation", [
  "prepare_current_virtual_mic_route_for_meeting",
]);
requireMarkers(source.outboundPipeline, "Meeting outbound AI preparation ownership", [
  "pub(super) fn process_outbound_wav(",
  "enqueue_meeting_playback",
  "PreparedPlaybackJob",
]);
requireMarkers(source.playbackRuntime, "Meeting bounded playback ownership", [
  "PLAYBACK_QUEUE_CAPACITY",
  "mpsc::sync_channel::<PreparedPlaybackJob>(PLAYBACK_QUEUE_CAPACITY)",
  "deliver_meeting_output_wav",
  "get_bound_virtual_mic_output_device",
]);
forbidMarkers(`${source.meeting}\n${source.meetingLifecycle}\n${source.outboundPipeline}\n${source.playbackRuntime}`, "retired Python route ownership", [
  "TRANSLATEIT_ENABLE_VIRTUAL_AUDIO_ROUTE_PROVIDER",
  "dispatch_meeting_virtual_audio_route_provider",
  "prepare_meeting_virtual_audio_route_provider",
  "route_execution_guard_ready",
]);
requireMarkers(source.meetingOutput, "guarded Meeting output facade", [
  "mod runtime;",
  "pub fn deliver_meeting_output_wav(",
  "validate_source_riff_boundary",
  "meeting_output:wav_riff_size_mismatch",
  "runtime::deliver_meeting_output_wav",
]);
requireMarkers(source.meetingOutputRuntime, "native Meeting output runtime", [
  "pub fn prepare_meeting_output_device(",
  "pub fn deliver_meeting_output_wav(",
  "pub fn cancel_meeting_output_for_generation(",
  ".build_output_stream(",
  "runtime_generation_is_authoritative",
  "meeting_output:delivery_deadline_exceeded",
  "decode_wav_bytes",
  "prepare_output_samples",
]);
requireMarkers(source.route, "one lifecycle owner and extracted pure selector", [
  "pub fn prepare_current_virtual_mic_route_for_meeting()",
  "pub fn bind_prepared_virtual_mic_route_to_generation",
  "pub fn get_virtual_mic_route_selection()",
  "pub fn get_virtual_mic_route_contract_status()",
  'mod pair_selection;',
  "select_route_pair(&devices.output_devices, &devices.input_devices)",
]);
requireMarkers(source.pairSelection, "approved provider selection", [
  "fn matched_pair_identity(",
  "fn matched_pair_candidates(",
  "fn primary_vb_cable_pair(",
  "fn reject_ambiguous_pair(",
  "let selected = if primary.len() == 1",
  "virtual_mic:unaccepted_native_candidate",
  "prototype_names_can_never_activate_the_native_route",
]);
forbidMarkers(source.pairSelection, "native prototype cannot be selected by name", [
  "TRANSLATEIT_NATIVE_PAIR_ID",
  'return Some(TRANSLATEIT_NATIVE_PAIR_ID.to_string())',
]);
forbidMarkers(source.pairSelection, "unexpected virtual provider auto-fallback", [
  'normalized.contains("voicemeeter")',
  '|| normalized.contains("virtual cable")',
  "else if candidates.len() == 1",
]);
forbidMarkers(source.route, "unowned persisted route preference/stub", [
  "VirtualMicRoutePreference",
  "virtual_mic_route_preference.json",
  "set_preferred_virtual_mic_route_devices",
  "prepare_virtual_mic_output_route_runtime_stub",
]);
requireMarkers(source.registry, "read-only route diagnostics surface", [
  "crate::commands::virtual_mic_route::get_virtual_mic_route_contract_status",
]);
forbidMarkers(source.registry, "mutating/manual route surface", [
  "set_preferred_virtual_mic_route_devices",
  "prepare_virtual_mic_output_route_runtime_stub",
  "dispatch_guarded_virtual_audio_route_provider",
]);

requireMarkers(source.nativeInf, "one capture-only PnP endpoint", [
  'TRANSLATEITMIC.WaveMicArray1.szPname="TranslateIT Meeting Microphone"',
  "ROOT\\TranslateITCaptureMic",
  "Guid.TranslateITCapture",
  "MediaCategories",
]);
forbidMarkers(source.nativeInf, "paired render endpoint in capture-only INF", [
  "AddInterface=%KSCATEGORY_RENDER%", "KSNAME_WaveSpeaker",
  'WaveSpeaker.szPname="TranslateIT Microphone Feed"',
]);
requireMarkers(source.nativeMiniPairs, "one registered WaveRT capture minipair", [
  "g_CaptureEndpoints", "&MicArray1Miniports",
  "#define g_MaxMiniports  (g_cCaptureEndpoints * 2)",
]);
forbidMarkers(source.nativeMiniPairs, "obsolete render minipair", [
  "g_RenderEndpoints", "SpeakerMiniports",
]);
requireMarkers(source.nativeCaptureTopology, "truthful mono virtual microphone topology", [
  "&KSNODETYPE_MICROPHONE,",
  "MICARRAY1_CUSTOM_NAME",
  "MicArray1TopoMiniportFilterDescriptor",
  "NULL,                                          // No fabricated physical microphone properties",
  "MicArray1PropertiesMute",
  "MicArray1PropertiesVolume",
]);
forbidMarkers(source.nativeCaptureTopology, "physical microphone array and jack properties", [
  "KSNODETYPE_MICROPHONE_ARRAY",
  "KSPROPERTY_AUDIO_MIC_ARRAY_GEOMETRY",
  "KSPROPERTY_AUDIO_MIC_SNR",
  "KSPROPERTY_AUDIO_MIC_SENSITIVITY2",
  "KSPROPERTY_JACK_DESCRIPTION",
  "AutomationMicArray1TopoFilter",
]);
forbidMarkers(source.nativeCaptureTopologyImpl, "fictional mic-array geometry and sensor calibration", [
  "PropertyHandlerMicArrayGeometry",
  "PropertyHandlerMicProperties",
  "PropertyHandlerJackDescription",
  "MICARRAY_SENSITIVITY",
  "MICARRAY_SNR",
  "FloatToFixedPoint16_16",
  "PropertyHandler_MicArrayTopoFilter",
]);
forbidMarkers(source.nativeCaptureTopologyHeader, "obsolete physical microphone entrypoints", [
  "PropertyHandlerMicArrayGeometry",
  "PropertyHandlerMicProperties",
  "PropertyHandlerJackDescription",
  "PropertyHandler_MicArrayTopoFilter",
]);
requireMarkers(source.nativeCaptureTopologyImpl, "supported generic microphone topology properties", [
  "PropertyHandler_MicArrayTopology",
  "pMiniport->PropertyHandlerGeneric(PropertyRequest)",
]);
requireMarkers(source.nativeProject, "WDK driver and secure-device linker", [
  "wdmsec.lib", 'ClCompile Include="capture_pcm.cpp"',
  'ClCompile Include="capture_control_device.cpp"',
  "TranslateITMic",
]);
requireMarkers(source.nativeProtocol, "bounded PCM16 mono IOCTL ABI", [
  "TI_CAPTURE_PROTOCOL_VERSION 1u", "TI_CAPTURE_RATE_HZ 48000u",
  "TI_CAPTURE_CHANNELS 1u", "TI_CAPTURE_SAMPLE_BITS 16u",
  "TI_CAPTURE_MAX_FRAMES_PER_WRITE 960u",
  "TI_CAPTURE_FIFO_FRAMES 9600u",
  "METHOD_BUFFERED, FILE_WRITE_DATA", "METHOD_BUFFERED, FILE_READ_DATA",
  "C_ASSERT(sizeof(TI_CAPTURE_BEGIN) == 32)",
  "C_ASSERT(FIELD_OFFSET(TI_CAPTURE_WRITE, Data) == 32)",
  "C_ASSERT(sizeof(TI_CAPTURE_REVOKE) == 24)",
  "C_ASSERT(sizeof(TI_CAPTURE_STATUS) == 40)",
]);
requireMarkers(source.nativeControl, "privileged control admission and cleanup", [
  "IoCreateDeviceSecure(", "wdmsec.h",
  'D:P(A;;GA;;;SY)(A;;GA;;;BA)',
  "TI_CAPTURE_IOCTL_WRITE",
  "TranslateITCaptureCloseWriter(stack->FileObject)",
  "PcDispatchIrp(device, irp)", "input != sizeof(TI_CAPTURE_BEGIN)",
  "openedEpoch != currentEpoch",
]);
forbidMarkers(source.nativeControl, "publicly writable driver surface", [
  "(A;;GRGW;;;WD)", "(A;;GRGW;;;IU)", "FILE_ANY_ACCESS",
]);
requireMarkers(source.nativePcm, "session owned PCM and independent readers", [
  "g_writer != file", "g_lastGeneration",
  "request->SourceEpoch <= g_lastSourceEpoch",
  "request->Sequence != g_expectedSequence",
  "invalidateLocked(releaseWriter)",
  "*observedSerial == 0",
  "*observedSerial != g_serial",
  "RtlZeroMemory(destination, bytes)", "g_online",
  "g_deviceEpoch", "TranslateITCaptureDeviceEpoch()",
  "openedDeviceEpoch != g_deviceEpoch",
  "TranslateITCaptureRead(",
]);
requireMarkers(source.nativePcmHeader, "atomic PnP epoch passed to PCM operations", [
  "TranslateITCaptureBegin(_In_ PFILE_OBJECT file, _In_ ULONGLONG openedDeviceEpoch",
  "TranslateITCaptureWrite(_In_ PFILE_OBJECT file, _In_ ULONGLONG openedDeviceEpoch",
  "TranslateITCaptureRevoke(_In_ PFILE_OBJECT file, _In_ ULONGLONG openedDeviceEpoch",
  "TranslateITCaptureStatus(_In_ ULONGLONG openedDeviceEpoch",
]);
requireMarkers(source.nativeControl, "every IOCTL forwards its opened PnP epoch", [
  "TranslateITCaptureBegin(file, openedEpoch,",
  "TranslateITCaptureWrite(file, openedEpoch,",
  "TranslateITCaptureRevoke(file, openedEpoch,",
  "TranslateITCaptureStatus(openedEpoch,",
  "case TI_CAPTURE_IOCTL_STATUS: {",
  "NT_SUCCESS(status) ? sizeof(TI_CAPTURE_STATUS) : 0",
]);
// A marker anywhere in a file is insufficient: each operation must check
// the opened PnP generation *after* taking the same spinlock that governs
// Stop/Start revocation and PCM writer admission.
for (const operation of [
  "TranslateITCaptureBegin",
  "TranslateITCaptureWrite",
  "TranslateITCaptureRevoke",
  "TranslateITCaptureStatus",
]) {
  const start = source.nativePcm.indexOf(`NTSTATUS ${operation}(`);
  const lock = source.nativePcm.indexOf("KeAcquireSpinLock(&g_lock, &irql);", start);
  const unlock = source.nativePcm.indexOf("KeReleaseSpinLock(&g_lock, irql);", lock);
  if (start < 0 || lock < 0 || unlock < 0
      || !source.nativePcm.slice(lock, unlock).includes("openedDeviceEpoch != g_deviceEpoch")) {
    throw new Error(`Native PCM ${operation} must validate PnP epoch inside the writer spinlock`);
  }
}
forbidMarkers(source.nativeControl, "stale dispatch-only PnP epoch checks", [
  "const ULONGLONG currentEpoch = TranslateITCaptureDeviceEpoch();",
]);
requireMarkers(source.nativeAdapter, "driver install and PnP lifecycle", [
  "TranslateITCaptureInitialize();",
  "TranslateITControlInitialize(DriverObject)",
  "TranslateITCaptureSetOnline();",
  "TranslateITCaptureInvalidateAll();",
  "TranslateITControlShutdown();",
]);
forbidMarkers(source.nativeAdapter, "render-device admission", [
  "InstallAllRenderFilters(", "g_RenderEndpoints",
]);
requireMarkers(source.nativeStream, "WaveRT independent PCM capture", [
  "TranslateITCaptureRead(m_pDmaBuffer + bufferOffset, runWrite,",
  "&m_TranslateITCaptureReadFrame",
  "&m_TranslateITCaptureSerial",
]);
// Source-side regression contract for mono WaveRT after long PnP suspend,
// scheduler delay or a backwards QPC change. No executable Windows proof.
requireMarkers(source.nativeStream, "64-bit and bounded WaveRT DMA arithmetic", [
  "static_assert(kMaxCaptureDmaBytes == 96000u",
  "CaptureDisplacementNumerator(96000u, 60000u, 0u)",
  "const ULONGLONG elapsedMs = elapsedHns / 10000ull;",
  "const ULONGLONG advancedBytes = byteNumerator / 1000ull;",
  "if (!m_pDmaBuffer || !m_ulDmaBufferSize || !m_ulDmaMovementRate)",
  "if (advancedBytes > m_ulDmaBufferSize)",
  "RtlZeroMemory(m_pDmaBuffer, m_ulDmaBufferSize);",
  "TranslateITCaptureDiscardStaleFrames(",
  "m_hnsDPCTimeCarryForward = elapsedHns % periodHns;",
  "if (nowHns < _this->m_ullLastDPCTimeStamp)",
  "ullPresentationPosition / blockAlign",
  "RequestedSize_ > kMaxCaptureDmaBytes",
  "RtlZeroMemory(mapped, RequestedSize_);",
  "NotificationCount_ != 1 && NotificationCount_ != 2",
  "m_KsState == KSSTATE_RUN && m_ulNotificationIntervalMs > 0",
  "if (_this->m_KsState != KSSTATE_RUN)",
  "Stop revokes any sample bytes",
  "No stale microphone samples",
]);
forbidMarkers(source.nativeStream, "legacy overflowing DMA/notification computations", [
  "ULONG TimeElapsedInMS = (ULONG)",
  "m_ulDmaMovementRate * TimeElapsedInMS",
  "ullPresentationPosition * m_pWfExt->Format.nSamplesPerSec",
  "m_hnsDPCTimeCarryForward = hnsCurrentTime -",
]);
requireMarkers(source.nativePcm, "late WaveRT reader cannot replay buffered speech", [
  "void TranslateITCaptureDiscardStaleFrames(",
  "*nextFrame = g_written;",
  "*observedSerial = g_serial;",
]);
requireMarkers(source.nativePcmHeader, "WaveRT reader resync contract", [
  "void TranslateITCaptureDiscardStaleFrames(",
]);
if ((source.nativeStream.match(/RtlZeroMemory\(mapped, RequestedSize_\);/g) ?? []).length !== 2) {
  throw new Error("Both WaveRT DMA allocation paths must initialize mapped audio pages");
}
for (const transport of ["case KSSTATE_STOP:", "case KSSTATE_RUN:"]) {
  const index = source.nativeStream.indexOf(transport);
  const next = source.nativeStream.indexOf("case KSSTATE_", index + transport.length);
  const slice = source.nativeStream.slice(index, next < 0 ? undefined : next);
  if (index < 0 || !slice.includes("RtlZeroMemory(m_pDmaBuffer, m_ulDmaBufferSize);")) {
    throw new Error(`WaveRT ${transport} must silence old mapped audio`);
  }
}

requireMarkers(source.nativeStreamHeader, "single MDL owner for cyclic DMA", [
  "PMDL                        m_pDmaMdl;",
  "VOID ReleaseDmaBuffer(_In_opt_ PMDL Mdl_);",
]);
requireMarkers(source.nativeStream, "serialized WaveRT lifetime and callback list", [
  "ExDeleteTimer(m_pNotificationTimer, TRUE, TRUE, NULL);",
  "ASSERT(m_pDmaMdl == NULL);",
  "ReleaseDmaBuffer(Mdl_);",
  "m_pDmaMdl = pBufferMdl;",
  "m_pPortStream->UnmapAllocatedPages(mapped, Mdl_);",
  "m_pPortStream->FreePagesFromMdl(Mdl_);",
  "STATUS_OBJECT_NAME_COLLISION",
]);
function waveRtOwnerSlice(signature, nextSignature) {
  const from = source.nativeStream.indexOf(signature);
  const until = source.nativeStream.indexOf(nextSignature, from + signature.length);
  if (from < 0 || until <= from) throw new Error("Missing WaveRT owner: " + signature);
  return source.nativeStream.slice(from, until);
}
const streamDestructor = waveRtOwnerSlice(
  "CMiniportWaveRTStream::~CMiniportWaveRTStream", "} // ~CMiniportWaveRTStream");
const eventRegister = waveRtOwnerSlice(
  "NTSTATUS CMiniportWaveRTStream::RegisterNotificationEvent",
  "NTSTATUS CMiniportWaveRTStream::UnregisterNotificationEvent");
const eventUnregister = waveRtOwnerSlice(
  "NTSTATUS CMiniportWaveRTStream::UnregisterNotificationEvent",
  "NTSTATUS CMiniportWaveRTStream::GetClockRegister");
const dmaRelease = waveRtOwnerSlice(
  "VOID CMiniportWaveRTStream::ReleaseDmaBuffer",
  "NTSTATUS CMiniportWaveRTStream::AllocateAudioBuffer");
function assertUnderPositionLock(body, operation) {
  const take = body.indexOf("KeAcquireSpinLock(&m_PositionSpinLock");
  const action = body.indexOf(operation);
  const unlock = body.indexOf("KeReleaseSpinLock(&m_PositionSpinLock");
  if (take < 0 || action <= take || unlock <= action) {
    throw new Error("Unprotected WaveRT operation: " + operation);
  }
}
assertUnderPositionLock(streamDestructor, "RemoveHeadList(&m_NotificationList)");
assertUnderPositionLock(eventRegister, "InsertTailList(&m_NotificationList");
assertUnderPositionLock(eventUnregister, "RemoveEntryList(cursor)");
assertUnderPositionLock(dmaRelease, "m_pDmaBuffer = NULL;");
if (streamDestructor.indexOf("ExDeleteTimer(m_pNotificationTimer") < 0
    || streamDestructor.indexOf("ExDeleteTimer(m_pNotificationTimer") >= streamDestructor.indexOf("m_pMiniport->StreamClosed")
    || streamDestructor.indexOf("ExFreePoolWithTag(item") <= streamDestructor.indexOf("KeReleaseSpinLock(&m_PositionSpinLock")
    || eventUnregister.indexOf("ExFreePoolWithTag(removed") <= eventUnregister.indexOf("KeReleaseSpinLock(&m_PositionSpinLock")
    || dmaRelease.indexOf("UnmapAllocatedPages(mapped, Mdl_)") <= dmaRelease.indexOf("KeReleaseSpinLock(&m_PositionSpinLock")
    || dmaRelease.indexOf("FreePagesFromMdl(Mdl_)") <= dmaRelease.indexOf("UnmapAllocatedPages(mapped, Mdl_)")) {
  throw new Error("WaveRT shutdown/free order is not safe");
}
if ((source.nativeStream.match(/ReleaseDmaBuffer\(Mdl_\);/g) || []).length !== 2
    || (source.nativeStream.match(/m_pDmaMdl = pBufferMdl;/g) || []).length !== 2) {
  throw new Error("Event and polling WaveRT APIs must share their DMA owner");
}
for (const signature of [
  "CMiniportWaveRTStream::~CMiniportWaveRTStream",
  "CMiniportWaveRTStream::AllocateBufferWithNotification",
  "CMiniportWaveRTStream::RegisterNotificationEvent",
  "CMiniportWaveRTStream::UnregisterNotificationEvent",
  "CMiniportWaveRTStream::ReleaseDmaBuffer",
  "CMiniportWaveRTStream::AllocateAudioBuffer",
]) {
  const at = source.nativeStream.indexOf(signature);
  const pragma = source.nativeStream.lastIndexOf("#pragma code_seg", at);
  if (at < 0 || pragma < 0
      || !source.nativeStream.slice(pragma, at).startsWith("#pragma code_seg()")
      || source.nativeStream.slice(at, at + 210).includes("PAGED_CODE();")) {
    throw new Error("Nonpaged WaveRT spinlock requirement failed: " + signature);
  }
}

requireMarkers(source.nativeStream, "capture packet atomicity and correct first-sample QPC", [
  "CapturePacketFirstSampleByte(1, 960) == 0",
  "CapturePacketFirstSampleByte(2, 960) == 960",
  "static BOOLEAN CaptureHnsToQpc(",
  "const ULONGLONG seconds = hns / hnsPerSecond;",
  "const ULONGLONG fractionalTicks =",
  "m_ulDmaBufferSize % m_ulNotificationsPerBuffer != 0",
  "status = STATUS_DATA_LATE_ERROR;",
  "m_ulLastOsReadPacket = readyPacket;",
]);
forbidMarkers(source.nativeStream, "obsolete packet timestamp and unsigned underflow", [
  "packetCounter * (m_ulDmaBufferSize / m_ulNotificationsPerBuffer)",
  "timeOfAvailablePacketInHns * m_ullPerformanceCounterFrequency",
  "ullLinearPosition + carryForwardBytes - linearPositionOfAvailablePacket",
]);
// Free*Buffer resets cyclic DMA dimensions under m_PositionSpinLock. If
// GetReadPacket releases that lock before its packet-size division or
// last-read update, a concurrent free can trigger division by zero or
// advertise a stale packet even if its initial mode check succeeded.
const captureGetReadPacket = waveRtOwnerSlice(
  "NTSTATUS CMiniportWaveRTStream::GetReadPacket",
  "NTSTATUS CMiniportWaveRTStream::SetWritePacket");
const captureReadLock = captureGetReadPacket.indexOf("KeAcquireSpinLock(&m_PositionSpinLock");
const captureReadUnlock = captureGetReadPacket.indexOf("KeReleaseSpinLock(&m_PositionSpinLock");
for (const operation of [
  "m_ulDmaBufferSize % m_ulNotificationsPerBuffer",
  "m_ulDmaBufferSize / m_ulNotificationsPerBuffer",
  "CapturePacketFirstSampleByte(completed, packetBytes)",
  "m_ulLastOsReadPacket = readyPacket;",
]) {
  const at = captureGetReadPacket.indexOf(operation);
  if (captureReadLock < 0 || at <= captureReadLock
      || captureReadUnlock <= at) {
    throw new Error("GetReadPacket must keep DMA and packet state locked: " + operation);
  }
}
if (!captureGetReadPacket.includes("if (!m_ulNotificationsPerBuffer)")
    || !captureGetReadPacket.includes("!m_pDmaMdl || !m_pDmaBuffer")
    || !captureGetReadPacket.includes("CaptureHnsToQpc(")) {
  throw new Error("GetReadPacket lacks fail-closed buffer/clock admission");
}

forbidMarkers(source.nativeStream, "legacy paired-PCM/tone playback", [
  "TranslateITBridgeRead(", "TranslateITBridgeWrite(",
  "m_ToneGenerator.GenerateSine(m_pDmaBuffer",
  "m_ToneGenerator.Init(", "ReadBytes(ByteDisplacement);",
]);
requireMarkers(source.nativeCaptureFormat, "48 kHz mono PCM16 and multiple readers", [
  "MICARRAY_RAW_CHANNELS                   1",
  "MICARRAY_MAX_INPUT_STREAMS              8",
  "KSAUDIO_SPEAKER_MONO", "96000",
]);
requireMarkers(source.audioMod, "candidate client compiled but not admitted to Meeting", [
  "pub(crate) mod native_capture_transport;",
  "pub(crate) mod native_capture_pipe;",
  "pub(crate) mod native_capture_pacing;",
]);
requireMarkers(source.nativeRust, "user-mode client matches restricted wire ABI", [
  "DEVICE_PATH", "TranslateITMicControl",
  "fn DeviceIoControl(",
  "fn frame_bytes(",
  "pub fn submit_20ms(&mut self,",
  "pub fn flush(&mut self)",
  "pub fn resume(&mut self",
  "pub fn end(mut self)",
  "pub fn status(&self)",
  "ioctls_match_kernel_method_buffered_file_access",
]);
forbidMarkers(
  source.route + "\n" + source.pairSelection + "\n" + source.meetingLifecycle,
  "unaccepted capture driver cannot displace VB-CABLE",
  ["TranslateITMicControl", "IOCTL_BEGIN", "NativeCaptureSession::begin("],
);

// Source contracts for the unaccepted non-admin broker and existing Meeting
// worker's pacing primitive. These are NOT proof of signature, SCM install,
// token identity at runtime, callback timing, or actual device delivery.
requireMarkers(source.nativeProtocol, "shared kernel/user-mode ABI", [
  "TRANSLATEIT_USER_MODE", "static_assert(offsetof(TI_CAPTURE_WRITE, Data)",
]);
requireMarkers(source.brokerProtocol, "fixed authenticated IPC framing", [
  "TI_BROKER_PIPE_NAME", "TI_BROKER_VERSION 1u",
  "TI_BROKER_MAX_PAYLOAD", "TI_BROKER_MAGIC",
  "static_assert(sizeof(TI_BROKER_REQUEST) == 16",
  "static_assert(sizeof(TI_BROKER_REPLY) == 16",
  "TI_CAPTURE_WRITE",
]);
requireMarkers(source.brokerDevice, "only privileged forwarding to captured PnP device", [
  "CreateFileW(kKernelControl", "TI_CAPTURE_IOCTL_BEGIN",
  "TI_CAPTURE_IOCTL_WRITE", "TI_CAPTURE_IOCTL_FLUSH",
  "TI_CAPTURE_IOCTL_END", "TI_CAPTURE_IOCTL_STATUS",
  "CloseHandle(handle_)", "producerActive_",
]);
requireMarkers(source.brokerIdentity, "SCM/install publisher-controlled admission", [
  "GetNamedPipeClientProcessId", "GetNamedPipeClientSessionId",
  "ImpersonateNamedPipeClient", "RevertToSelf",
  "OpenThreadToken", "OpenProcessToken",
  "SidAndLogonMatch", "QueryFullProcessImageNameW",
  "ImageHashApproved", "ApprovedDesktopSha256",
  "BrokerClientIdentity::IsLive", "BrokerClientIdentity::SenderTokenMatches",
  "TokenImpersonationLevel", "TokenIntegrityLevel", "SecurityImpersonation",
  "level < SecurityImpersonation",
  "EqualSid(senderLabel->Label.Sid, processLabel->Label.Sid)",
  "result.process_ = process.handle;", "WaitForSingleObject(process_, 0)",
  "SOFTWARE\\\\OpenITStudios\\\\TranslateIT",
  "CompareStringOrdinal",
]);
requireMarkers(source.brokerService, "single privileged transport worker only", [
  "StartServiceCtrlDispatcherW", "TranslateITMicBroker",
  "PIPE_REJECT_REMOTE_CLIENTS", "FILE_FLAG_FIRST_PIPE_INSTANCE",
  "FILE_FLAG_OVERLAPPED", "AuthenticatePipeClient(pipe, client)",
  "io.WatchPeerProcess(client.ProcessHandle())", "client.IsLive(pipe)",
  "BrokerDriver driver;", "BrokerPipeIo io(pipe.h, g_stop);",
  "TI_BROKER_REPLY", "ProducerClock",
  "const ULONGLONG boundedUs =", "if (!client.SenderTokenMatches(pipe))",
]);
// An authenticated connection is NOT authorization for every later packet.
// The named-pipe impersonation token follows the most recent server-side read;
// verify the sender after header and again after the body, before any privileged
// DeviceIoControl is forwarded. Header-only STATUS/FLUSH still gets a check.
// Preserve a single SHA-verified image HANDLE for the connection, rather
// than reopening by path and releasing it before privileged requests.
requireMarkers(source.brokerIdentityHeader, "pinned broker desktop image", [
  "HANDLE approvedImage_ = INVALID_HANDLE_VALUE;",
]);
requireMarkers(source.brokerIdentity, "hash-approved desktop image lifetime", [
  "bool Sha256File(HANDLE file,",
  "SetFilePointerEx(file, beginning, nullptr, FILE_BEGIN)",
  "bool ImageHashApproved(HANDLE file)",
  "Sha256File(file, actual)",
  "if (approvedImage_ != INVALID_HANDLE_VALUE) CloseHandle(approvedImage_)",
  "approvedImage_ == INVALID_HANDLE_VALUE",
  "image.handle = CreateFileW(expectedPath.c_str(), GENERIC_READ, FILE_SHARE_READ,",
  "ImageHashApproved(image.handle)",
  "result.approvedImage_ = image.handle;",
  "image.handle = INVALID_HANDLE_VALUE;",
]);
const imageAdmission = source.brokerIdentity.slice(
  source.brokerIdentity.indexOf("DWORD AuthenticatePipeClient("));
const imageOpen = imageAdmission.indexOf("image.handle = CreateFileW(expectedPath.c_str(), GENERIC_READ, FILE_SHARE_READ,");
const sameFileHash = imageAdmission.indexOf("ImageHashApproved(image.handle)");
const retainImage = imageAdmission.indexOf("result.approvedImage_ = image.handle;");
const retainProcess = imageAdmission.indexOf("result.process_ = process.handle;");
if ([imageOpen, sameFileHash, retainImage, retainProcess].some(i => i < 0)
    || !(imageOpen < sameFileHash && sameFileHash < retainImage && retainImage < retainProcess)
    || source.brokerIdentity.includes("ImageHashApproved(expectedPath)")) {
  throw new Error("Broker must hash and retain the same approved image HANDLE before process admission");
}
const brokerHandlerStart = source.brokerService.indexOf("DWORD handleClient(");
const brokerHandlerEnd = source.brokerService.indexOf("DWORD WINAPI worker(", brokerHandlerStart);
const brokerHandler = source.brokerService.slice(brokerHandlerStart, brokerHandlerEnd);
const senderChecks = [...brokerHandler.matchAll(/if \(!client\.SenderTokenMatches\(pipe\)\)/g)]
  .map(x => x.index);
const hdrRead = brokerHandler.indexOf("if (!firstRequest && !io.ReadExactly(&request");
const bodyRead = brokerHandler.indexOf("if (!io.ReadExactly(payload.data(), request.PayloadBytes");
const privilegedWrite = brokerHandler.indexOf("status = driver.Send(request.Operation");
if (brokerHandlerStart < 0 || brokerHandlerEnd <= brokerHandlerStart
    || senderChecks.length !== 2
    || hdrRead < 0 || bodyRead < 0 || privilegedWrite < 0
    || !(hdrRead < senderChecks[0] && senderChecks[0] < bodyRead)
    || !(bodyRead < senderChecks[1] && senderChecks[1] < privilegedWrite)) {
  throw new Error("Broker must authenticate EACH named-pipe header and payload before kernel forwarding");
}
const senderValidation = source.brokerIdentity.slice(
  source.brokerIdentity.indexOf("bool BrokerClientIdentity::SenderTokenMatches"),
  source.brokerIdentity.indexOf("DWORD AuthenticatePipeClient("));
if (!senderValidation.includes("Impersonation impersonation;")
    || !senderValidation.includes("SidAndLogonMatch(senderToken.handle, processToken.handle)")
    || !senderValidation.includes("if (!impersonation.finish()) return false;")
    || !senderValidation.includes("return match && IsLive(pipe);")) {
  throw new Error("Broker sender validation must re-impersonate and restore security context");
}

requireMarkers(source.brokerIo, "cancelable bounded server IPC", [
  "WaitForMultipleObjects", "CancelIoEx",
  "GetOverlappedResult", "WaitForSingleObject(stop_, 0)",
  "BrokerPipeIo::ReadExactly", "BrokerPipeIo::WriteExactly",
  "const DWORD count = peer_ ? 3 : 2;",
]);
requireMarkers(source.brokerProject, "broker Win32 build target", [
  "TranslateITMicBroker", "broker_identity.cpp",
  "broker_driver.cpp", "broker_io.cpp",
  "broker_service.cpp", "TRANSLATEIT_USER_MODE",
  "bcrypt.lib", "advapi32.lib",
]);
requireMarkers(source.nativeSolution, "native driver and broker in one solution", [
  "Source\\Main\\Main.vcxproj",
  "Broker\\TranslateITMicBroker.vcxproj",
]);
requireMarkers(source.nativeRustPipe, "non-admin service identity before PCM send", [
  "OpenServiceW", "QueryServiceStatusEx",
  "GetNamedPipeServerProcessId", "verify_service_process(&file)?",
  "CLIENT_ACCESS: u32 = 0x0012_008B",
  "FILE_FLAG_OVERLAPPED", "SECURITY_SQOS_PRESENT",
  "SECURITY_IMPERSONATION", "GetOverlappedResult",
  ".custom_flags(FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IMPERSONATION)",
  "CancelIoEx", "fn transfer(",
  "pub fn submit(&mut self", "pub fn flush(&mut self)",
  "pub fn end(mut self)", "fn poison(&mut self)",
  "self.file.take()", "native_capture:stop_unverified",
  "failed_control_transport_is_sticky_and_never_acknowledges_stop",
  "native_capture:producer_state_mismatch",
  "a_mismatched_producer_status_cannot_confirm_this_session",
]);
requireMarkers(source.nativeRustPacing, "reuse one Meeting worker with bounded PCM", [
  "pub fn send_paced(", "FnMut(u64, u64) -> bool",
  "FRAMES_PER_PACKET: usize = 960",
  "MAX_LATENESS", "thread::sleep(",
  "if !authorized(generation, source_epoch) { return Err(interrupted()); }",
  "revoked_session_sends_no_audio",
  "revoked_during_last_packet_is_not_reported_as_success",
]);
forbidMarkers(source.brokerService, "broker must not become a second translation engine", [
  "start_helper_bridge(", "synthesize(", "launch_worker(",
  "process_outbound_wav(", "llama.cpp",
]);
forbidMarkers(
  [source.route, source.pairSelection, source.meetingLifecycle, source.playbackRuntime,
   source.outboundPipeline, source.runtimeCommands].join("\n"),
  "unaccepted broker cannot activate on ordinary Meeting",
  ["NativeCapturePipeSession::begin(", "TranslateITMicBrokerV1",
   "TranslateITMicControl", "native_capture_pacing::send_paced("],
);

console.log("[virtual-route] VB-CABLE remains the only admitted Meeting provider; capture-only kernel, authenticated broker and Rust pacing stay source candidates until native release acceptance.");
