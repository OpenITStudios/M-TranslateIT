import { existsSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(scriptDir, "..");
const tauriRoot = join(appRoot, "src-tauri");
const backendRoot = resolve(appRoot, "../../Backend");
const workerRoot = join(backendRoot, "LocalWorker", "WorkerRuntime");
const errors = [];
const fail = (message) => errors.push(message);
const readText = (path) => readFileSync(path, "utf8");
const readJson = (path) => JSON.parse(readText(path));
const requireFile = (path) => {
  if (!existsSync(path) || !statSync(path).isFile()) fail(`Missing required file: ${path}`);
};
const requireMarkers = (body, label, markers) => {
  for (const marker of markers) if (!body.includes(marker)) fail(`${label} marker missing: ${marker}`);
};
const forbidMarkers = (body, label, markers) => {
  for (const marker of markers) if (body.includes(marker)) fail(`${label} forbidden marker present: ${marker}`);
};
const compareExact = (actual, expected, label) => {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${label} drifted: actual=${JSON.stringify(actual)} expected=${JSON.stringify(expected)}`);
  }
};

const paths = {
  package: join(appRoot, "package.json"),
  config: join(tauriRoot, "tauri.release.conf.json"),
  manifest: join(workerRoot, "model_manifest.json"),
  build: join(scriptDir, "build_release.ps1"),
  stage: join(scriptDir, "stage_release_inputs_impl.ps1"),
  builder: join(scriptDir, "build_r3_external_payload.py"),
  hook: join(tauriRoot, "windows", "r3_payload_hooks.template.nsh"),
  helper: join(tauriRoot, "windows", "r3_payload_installer.ps1"),
};
for (const path of Object.values(paths)) requireFile(path);
if (errors.length) {
  for (const error of errors) console.error(`[release-package] ${error}`);
  process.exit(1);
}

const packageJson = readJson(paths.package);
const config = readJson(paths.config);
const manifest = readJson(paths.manifest);
const build = readText(paths.build);
const stage = readText(paths.stage);
const builder = readText(paths.builder);
const hook = readText(paths.hook);
const helper = readText(paths.helper);

const canonicalPayloadRoots = [
  "EngineData/Backend/LocalWorker/PythonRuntime",
  "EngineData/Backend/RuntimeAssets/ASR/ModelData",
  "EngineData/Backend/RuntimeAssets/Translation/ModelData",
  "EngineData/Backend/RuntimeAssets/NativeCuda",
  "EngineData/Backend/RuntimeAssets/Voice/BuiltInVoices",
  "EngineData/Backend/RuntimeAssets/Voice/GPTSoVITS",
  "EngineData/Backend/RuntimeAssets/AudioProvider/VBCABLE/Package",
];
const builderRootsMatch = builder.match(/PAYLOAD_ROOTS\s*=\s*\(([\s\S]*?)\)\s*\nREQUIRED_FILES/);
if (!builderRootsMatch) fail("payload builder PAYLOAD_ROOTS block is missing or unparsable.");
else {
  const roots = [...builderRootsMatch[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]);
  compareExact(roots, canonicalPayloadRoots, "payload builder root closure");
}
const helperRootsMatch = helper.match(/\$PayloadRoots=@\(([\s\S]*?)\)\s*\n/);
if (!helperRootsMatch) fail("installer helper PayloadRoots block is missing or unparsable.");
else {
  const roots = [...helperRootsMatch[1].matchAll(/'([^']+)'/g)].map((match) => match[1].replaceAll("\\", "/"));
  compareExact(roots, canonicalPayloadRoots, "installer helper root closure");
}

const expectedResources = {
  "../../../Backend/LocalWorker/WorkerRuntime/model_manifest.json": "EngineData/Backend/LocalWorker/WorkerRuntime/model_manifest.json",
  "../../../Backend/RuntimeAssets/ThirdPartyNotices/THIRD_PARTY_NOTICES.txt": "EngineData/Backend/RuntimeAssets/ThirdPartyNotices/THIRD_PARTY_NOTICES.txt",
  "../../../Backend/RuntimeAssets/AudioProvider/VBCABLE/NOTICE.txt": "EngineData/Backend/RuntimeAssets/AudioProvider/VBCABLE/NOTICE.txt",
};
// Tauri's release bundle must consume the explicit, complete icon family.
// File existence alone does not prove that raster/ICO assets match the approved SVG.
const expectedIcons = [
  "icons/32x32.png",
  "icons/128x128.png",
  "icons/128x128@2x.png",
  "icons/icon.icns",
  "icons/icon.ico",
];
compareExact(config.bundle?.icon, expectedIcons, "R3 Tauri bundle icon set");
for (const icon of expectedIcons) requireFile(join(tauriRoot, icon));

const resources = config.bundle?.resources;
if (!resources || Array.isArray(resources)) fail("R3 release must use the canonical source-to-target resource map.");
else {
  const actual = Object.entries(resources).sort(([a], [b]) => a.localeCompare(b));
  const expected = Object.entries(expectedResources).sort(([a], [b]) => a.localeCompare(b));
  if (JSON.stringify(actual) !== JSON.stringify(expected)) fail("R3 Setup resource map drifted from the small canonical closure.");
  for (const source of Object.keys(resources)) {
    if (/UserData|DevelopingData|\.venv|tests/i.test(source)) fail(`Development/user baggage in Setup resources: ${source}`);
  }
}
for (const largeSource of [
  "../../../Backend/LocalWorker/PythonRuntime/",
  "../../../Backend/RuntimeAssets/ASR/ModelData/",
  "../../../Backend/RuntimeAssets/Translation/ModelData/",
  "../../../Backend/RuntimeAssets/Voice/GPTSoVITS/",
  "../../../Backend/RuntimeAssets/AudioProvider/VBCABLE/Package/",
]) {
  if (resources && Object.prototype.hasOwnProperty.call(resources, largeSource)) fail(`Large payload re-embedded in Tauri resources: ${largeSource}`);
}

const nsis = config.bundle?.windows?.nsis ?? {};
if (nsis.installMode !== "perMachine") fail("R3 NSIS installMode must be perMachine.");
if (nsis.installerHooks !== "./target/translateit-r3-payload-hooks.generated.nsh") fail("R3 NSIS must use the generated payload hook.");
if (existsSync(join(tauriRoot, "target", "translateit-r3-payload-hooks.generated.nsh"))) fail("Generated R3 hook must stay out of source.");

requireMarkers(stage, "release staging", [
  "$ffmpegReleaseTag = 'autobuild-2026-10-25-13-00'",
  "$ffmpegReleaseId = '423406080'",
  "$ffmpegReleaseName = 'Auto-Build 2026-10-25 13:00'",
  "$ffmpegReleasePublishedAt = '2026-10-25T13:01:14Z'",
  "$ffmpegAssetId = '675057963'",
  "$ffmpegAssetName = 'ffmpeg-n9.0.2-36-g6882969ce9-win64-lgpl-9.0.zip'",
  "$ffmpegArchiveSha = '47526345cc4800f4163337d5db8048a2d06c78a53bb234532f55a30dbfb49868'",
  "releases/download/$ffmpegReleaseTag/$ffmpegAssetName",
  "Assert-Hash $ffmpegArchive $ffmpegArchiveSha 'FFmpeg pinned archive'",
  "integrity_source=pinned_release_asset_sha256",
  "--enable-version3",
]);
forbidMarkers(stage, "release staging moving FFmpeg source", [
  "BtbN/FFmpeg-Builds/releases/latest",
  "ffmpeg-n9.0-latest-win64-lgpl-9.0.zip",
  "$ffmpegRelease = Invoke-RestMethod",
]);

requireMarkers(builder, "payload builder", [
  'PAYLOAD_SCHEMA = "translateit.r3.external_payload.v2"',
  'INSTALLED_RUNTIME_SCHEMA = "translateit.installed_runtime.v2"',
  "TRANSLATEIT_PAYLOAD_CONTRACT.json",
  'TRANSFORMERS_VERSION = "4.57.6"',
  'TOKENIZERS_VERSION = "0.22.2"',
  "validate_no_filesystem_indirection",
  "validate_native_inputs", "TRANSLATEIT_NATIVE_DLL_PINS", "@@NATIVE_DLL_PINS_SHA256@@",
  "is_filesystem_indirection",
  "is_junction",
  '"-t7z"', '"-m0=LZMA2"', '"-mx=9"', "find_7zip", "find_tar", "expanded_bytes", "@@APP_VERSION@@", "@@PAYLOAD_EXPANDED_BYTES@@",
]);
forbidMarkers(builder.toLowerCase(), "payload builder network/bootstrap", ["urllib", "requests.get", "invoke-webrequest", "start-bitstransfer", "http://", "https://"]);

requireMarkers(hook, "NSIS lifecycle hook", [
  "@@PAYLOAD_SCHEMA@@", "@@INSTALLED_RUNTIME_SCHEMA@@", "@@APP_VERSION@@", "@@PAYLOAD_SHA256@@", "@@PAYLOAD_EXPANDED_BYTES@@",
  "@@NATIVE_DLL_PINS_SHA256@@", "-ExpectedNativeDllPinsSha256",
  "NSIS_HOOK_PREINSTALL", "NSIS_HOOK_POSTINSTALL", "NSIS_HOOK_PREUNINSTALL", "-Mode Verify", "-Mode Install", "-Mode VerifyInstalled", "/UPDATE", "app-update mode", "$0 == 3010", "SetRebootFlag true",
  "RMDir /r \"$INSTDIR\\EngineData\\Backend\\LocalWorker\\PythonRuntime\"",
  "RMDir /r \"$INSTDIR\\EngineData\\Backend\\RuntimeAssets\\Voice\\BuiltInVoices\"",
  "TRANSLATEIT_INSTALLED_RUNTIME.json",
]);
forbidMarkers(hook.toLowerCase(), "NSIS network/bootstrap", ["inetc::", "nsisdl::", "http://", "https://", "execshell"]);

requireMarkers(helper, "installer helper", [
  "[ValidateSet('Verify','Install','VerifyInstalled')]", "Verify-InstalledRuntime", "Assert-InstalledRuntimeManifest", "Get-FileHash -Algorithm SHA256", "Read-PayloadContract", "Ensure-FreeSpace", ".translateit-r3-stage", ".translateit-r3-backup",
  "Rollback-Payload", "Read-PythonMetadata", "pnputil.exe", "VBCABLE_Setup_x64.exe", "@('-i','-h')", "exit 3010",
  "ExpectedNativeDllPinsSha256", "Assert-NativeRuntime", "Get-NativePins",
  "ExpectedInstalledRuntimeSchema", "preserve_system_driver", "preserve_app_local_user_data",
  "TranslateITExitCode", "throw $failure", "$cleanupVerified", "Payload rollback incomplete; recovery files retained.",
  "$newManifest", "$savedManifest", "Rollback-Payload $root $backup $states",
  "BuiltInVoices\\MaleVoice\\reference.wav",
  "BuiltInVoices\\FemaleVoice\\reference.wav",
  "BuiltInVoices\\SOURCES.json",
]);
const failHelper = helper.slice(helper.indexOf("function Fail("), helper.indexOf("function NeedFile("));
if (!failHelper.includes("throw $failure") || /\bexit\b/i.test(failHelper.replace(/#.*$/gm, ""))) fail("Installer validation must unwind through rollback.");
const transaction = helper.slice(helper.indexOf("function Install-Payload("), helper.lastIndexOf("\ntry{"));
if (!transaction.includes("if($cleanupVerified)") || !transaction.includes("$state=[pscustomobject]@{relative=$InstalledManifest;")) fail("Installed receipt and backup cleanup must belong to the payload transaction.");
forbidMarkers(helper.toLowerCase(), "installer helper network/bootstrap", ["invoke-webrequest", "start-bitstransfer", "webclient", "http://", "https://", "pip install", "pnputil /delete-driver"]);

requireMarkers(build, "release build", [
  "build_r3_external_payload.py", "TranslateIT-Payload.7z", "TranslateIT-Setup.exe", "TranslateIT-Setup.exe.sig", "latest.json", "translateit-r3-payload-hooks.generated.nsh",
  "npm run preflight:tauri-package", "--config src-tauri/tauri.release.conf.json", "signed updater metadata", "windows-x86_64", "releases/download/v$AppVersion/TranslateIT-Setup.exe", "UTF8Encoding($false)",
]);

if (manifest.schema !== "translateit.local_model_inventory.v2" || manifest.inventory_scope !== "full_product_release_assets") fail("model_manifest.json release inventory contract drifted.");
const requiredIds = new Set((manifest.models ?? []).filter((item) => item.required === true).map((item) => item.model_id));
for (const id of ["qwen3-asr-1.7b-q8_0", "hy-mt2-1.8b-q6_k", "zonos3-mini-q8_0", "eres2netv2-forward3-f32"]) if (!requiredIds.has(id)) fail(`Required model missing: ${id}`);
for (const legacy of ["marianmt-id-en", "marianmt-en-id", "m2m100-418m", "milmmt-46-1b-v1.0", "faster-whisper-medium", "faster-whisper-large-v3-turbo", "gpt-sovits-v2proplus-voicelab"]) if (requiredIds.has(legacy)) fail(`Legacy translator remains required: ${legacy}`);
const mt = (manifest.models ?? []).filter((item) => item.stage === "translation_bidirectional_id_en");
if (mt.length !== 1 || mt[0].model_id !== "hy-mt2-1.8b-q6_k" || mt[0].revision !== "a0c709d9fac510f2c807aa3af52872340dc37a4a" || mt[0].cpu_fallback !== false) fail("Native MT identity drifted.");
const asr = (manifest.models ?? []).filter((item) => item.stage === "asr_primary");
if (asr.length !== 1 || asr[0].model_id !== "qwen3-asr-1.7b-q8_0" || asr[0].revision !== "3b2c29585c2180452ccb14d3b6b58127f352400d" || asr[0].cpu_fallback !== false) fail("Native ASR identity drifted.");
// Native-only source must not ship the retired Python inference worker.
for (const retired of ["realtime_local_worker.py","realtime_local_worker_base.py","worker_runtime_common.py","worker_io_runtime.py","voice_lab_gpt_sovits.py","voice_lab_upstream_stage.py"]) {
  if (existsSync(join(workerRoot, retired))) fail(`Retired Python inference file remains: ${retired}`);
}
if (!build.includes("Native My Voice release is held") || !stage.includes("Native packaging is held")) {
  fail("Incomplete native packaging must remain blocked.");
}

const scripts = packageJson.scripts ?? {};
if (!String(scripts["preflight:tauri-package"] ?? "").includes("validate_release_package_contract.mjs")) fail("Tauri package preflight must include the R3 release contract.");
if (!String(scripts["validate:source-contracts"] ?? "").includes("preflight:tauri-package")) fail("Source validation must include R3 package preflight.");

if (errors.length) {
  for (const error of errors) console.error(`[release-package] ${error}`);
  process.exit(1);
}
console.log("[release-package] R3 source contract PASS: pinned FFmpeg release asset identity, exact builder/installer payload-root closure, version/hash-bound external payload, app-only /UPDATE preserves only a fully verified compatible installed runtime, no filesystem-indirection payload roots, transactional full runtime replacement including built-in Meeting voice references, symmetric uninstall cleanup, native-only inference source closure, Setup-owned VB-CABLE install/restart, user-data and system-driver preservation policy, and small Tauri resource closure are aligned.");
