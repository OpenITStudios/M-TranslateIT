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
  nativeAdapter: resolve(root, "src-tauri/windows/virtual-audio/Source/Main/adapter.cpp"),
  nativeMiniPairs: resolve(root, "src-tauri/windows/virtual-audio/Source/Filters/minipairs.h"),
  nativeProject: resolve(root, "src-tauri/windows/virtual-audio/Source/Main/Main.vcxproj"),
  nativeRust: resolve(root, "src-tauri/src/engine/audio/native_capture_transport.rs"),
  nativeStream: resolve(root, "src-tauri/windows/virtual-audio/Source/Main/minwavertstream.cpp"),
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
  "TranslateITCaptureRead(",
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

console.log("[virtual-route] VB-CABLE remains the only admitted Meeting route; capture-only WDK source and privileged Rust IOCTL client are not installed, signed or native-accepted.");
