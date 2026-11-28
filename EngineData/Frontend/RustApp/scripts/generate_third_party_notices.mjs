import { lstatSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
export const defaultBackendRoot = resolve(scriptDir, "../../../Backend");
const normalizeText = (text) => String(text).replace(/\r\n/g, "\n").replace(/\r/g, "\n").trimEnd() + "\n";
const requireFile = (path, label = path) => {
  const meta = lstatSync(path);
  if (!meta.isFile() || meta.isSymbolicLink() || meta.size <= 0 || meta.size > 4 * 1024 * 1024) {
    throw new Error(`missing_or_invalid_notice_input:${label}`);
  }
  return path;
};
function renderMaterial(label, path) {
  return [`----- BEGIN ${label} -----`, normalizeText(readFileSync(path, "utf8")).trimEnd(), `----- END ${label} -----`, ""].join("\n");
}

function builtInVoiceAttribution(backendRoot) {
  const sourcePath = requireFile(
    join(backendRoot, "RuntimeAssets", "Voice", "BuiltInVoices", "SOURCES.json"),
    "Voice/BuiltInVoices/SOURCES.json",
  );
  const payload = JSON.parse(readFileSync(sourcePath, "utf8").replace(/^\uFEFF/, ""));
  if (payload.schema !== "translateit.builtin_voice_sources.v1") {
    throw new Error("invalid_builtin_voice_sources_schema");
  }
  const voices = Array.isArray(payload.voices) ? payload.voices : [];
  if (voices.length !== 2 || new Set(voices.map(voice => voice.voice_id)).size !== 2 ||
      !["MaleVoice", "FemaleVoice"].every(id => voices.some(voice => voice.voice_id === id))) {
    throw new Error("builtin_voice_sources_inventory_mismatch");
  }

  return voices.map((voice) => {
    const required = [
      "voice_id",
      "speaker_id",
      "utterance",
      "source_url",
      "wav_sha256",
      "license",
      "reference_text",
    ];
    for (const key of required) {
      if (!String(voice?.[key] ?? "").trim()) throw new Error(`builtin_voice_source_missing:${key}`);
    }
    if (!/^[a-f0-9]{64}$/.test(voice.wav_sha256)) throw new Error("builtin_voice_invalid_sha256");
    if (voice.license !== "CC-BY-4.0") {
      throw new Error(`builtin_voice_license_mismatch:${voice.voice_id}:${voice.license}`);
    }
    return [
      `Component: TranslateIT built-in voice ${voice.voice_id}`,
      "Source corpus: LibriSpeech dev-clean (OpenSLR SLR12)",
      `Speaker: ${voice.speaker_id}`,
      `Utterance: ${voice.utterance}`,
      `Source: ${voice.source_url}`,
      `Reference WAV SHA-256: ${voice.wav_sha256}`,
      `Declared license: ${voice.license}`,
      `Reference text: ${voice.reference_text}`,
    ].join("\n");
  }).join("\n\n");
}


export function thirdPartyNoticeOutputPath(backendRoot = defaultBackendRoot) {
  return join(backendRoot, "RuntimeAssets", "ThirdPartyNotices", "THIRD_PARTY_NOTICES.txt");
}

export function buildThirdPartyNoticeBundle({ backendRoot = defaultBackendRoot } = {}) {
  const manifestPath = requireFile(join(backendRoot, "LocalWorker", "WorkerRuntime", "model_manifest.json"));
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  if (manifest.schema !== "translateit.local_model_inventory.v2" || manifest.native_payload?.schema !== "translateit.r3.external_payload.v3") {
    throw new Error("invalid_native_notice_inventory");
  }
  const verifier = manifest.models.find(model => model.model_id === "eres2netv2-forward3-f32");
  if (verifier?.pin_status !== "reviewed_export" || !/^[a-f0-9]{64}$/.test(verifier.artifact_sha256 ?? "") ||
      !Number.isSafeInteger(verifier.artifact_size_bytes) || verifier.artifact_size_bytes <= 0) {
    throw new Error("speaker_verifier_export_pin_pending");
  }
  const models = manifest.models.map(model => [
    `Component: ${model.model_id}`,
    `Source: ${model.repo_id}`,
    `Revision: ${model.revision ?? model.source_revision}`,
    `Declared license: ${model.license}`,
    ...Object.entries(model.artifact_hashes ?? {}).sort().map(([name, hash]) => `Artifact: ${name} SHA-256 ${hash}`),
    model.source_checkpoint_sha256 ? `Source checkpoint SHA-256: ${model.source_checkpoint_sha256}` : null,
    model.artifact_sha256 ? `Export SHA-256: ${model.artifact_sha256}` : null,
  ].filter(Boolean).join("\n")).sort().join("\n\n");
  const assets = join(backendRoot, "RuntimeAssets");
  const sections = [
    "TranslateIT Third-Party Notices and Source References\n",
    "Native R3 v3 controlled input notices. These declarations do not independently establish redistribution rights or runtime acceptance.\n",
    "===== CONTROLLED MODEL / VOICE INVENTORY =====\n\n" + models,
    "===== BUILT-IN VOICE ATTRIBUTION =====\n\n" + builtInVoiceAttribution(backendRoot),
    renderMaterial("NATIVE CUDA AND GGUF MODEL LICENSE / PROVENANCE", requireFile(join(assets, "NativeCuda", "THIRD_PARTY_NOTICES.txt"))),
    renderMaterial("NATIVE ORT, SILERO AND SPEAKER VERIFIER LICENSE / PROVENANCE", requireFile(join(assets, "NativeOrt", "THIRD_PARTY_NOTICES.txt"))),
    renderMaterial("VB-CABLE NOTICE", requireFile(join(assets, "AudioProvider", "VBCABLE", "NOTICE.txt"))),
  ];
  return { content: normalizeText(sections.join("\n\n")) };
}

export function writeThirdPartyNoticeBundle({ backendRoot = defaultBackendRoot } = {}) {
  // Do not emit a release notice during the unreviewed native package hold.
  throw new Error("Native packaging is held: native notice publication is not release-admitted.");
  const outputPath = thirdPartyNoticeOutputPath(backendRoot);
  const built = buildThirdPartyNoticeBundle({ backendRoot });
  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, built.content, "utf8");
  return { ...built, outputPath };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const mode = process.argv[2] ?? "--write";
  if (mode === "--write") {
    const result = writeThirdPartyNoticeBundle();
    console.log(`[third-party-notices] wrote ${result.outputPath}`);
  } else if (mode === "--check") {
    const expected = buildThirdPartyNoticeBundle().content;
    const actual = normalizeText(readFileSync(requireFile(thirdPartyNoticeOutputPath()), "utf8"));
    if (actual !== expected) throw new Error("third_party_notice_bundle_stale");
    console.log("[third-party-notices] Native notice bytes match; legal review remains separate.");
  } else {
    throw new Error(`unsupported_notice_mode:${mode}`);
  }
}
