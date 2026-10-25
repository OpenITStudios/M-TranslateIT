import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(scriptDir, "..");
export const defaultBackendRoot = resolve(appRoot, "../../Backend");

const EXPECTED_PYTHON_VERSION = "3.12.10";
const EXPECTED_G2P_VERSION = "2.1.0";
const EXPECTED_FROZENDICT_VERSION = "2.4.7";
const EXPECTED_SOXR_VERSION = "1.1.0";
const EXPECTED_FSSPEC_VERSION = "2026.9.0";

const normalizeText = (text) => String(text).replace(/\r\n/g, "\n").replace(/\r/g, "\n").trimEnd() + "\n";
const requireFile = (path, label = path) => {
  if (!existsSync(path) || !statSync(path).isFile() || statSync(path).size <= 0) {
    throw new Error(`missing_notice_input:${label}`);
  }
  return path;
};
const requireDir = (path, label = path) => {
  if (!existsSync(path) || !statSync(path).isDirectory()) {
    throw new Error(`missing_notice_input:${label}`);
  }
  return path;
};

function walkDirectories(root, maxDepth = 6) {
  const result = [];
  const queue = [[root, 0]];
  while (queue.length) {
    const [current, depth] = queue.shift();
    if (!existsSync(current) || !statSync(current).isDirectory()) continue;
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const path = join(current, entry.name);
      result.push(path);
      if (depth < maxDepth && entry.name !== "__pycache__") queue.push([path, depth + 1]);
    }
  }
  return result;
}

function walkFiles(root) {
  const result = [];
  const queue = [root];
  while (queue.length) {
    const current = queue.shift();
    if (!existsSync(current) || !statSync(current).isDirectory()) continue;
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const path = join(current, entry.name);
      if (entry.isDirectory()) queue.push(path);
      else if (entry.isFile()) result.push(path);
    }
  }
  return result;
}

function parseMetadataHeaders(text) {
  const values = new Map();
  let currentKey = null;
  for (const raw of normalizeText(text).split("\n")) {
    if (!raw) break;
    if (/^[ \t]/.test(raw) && currentKey) {
      const items = values.get(currentKey) ?? [];
      items[items.length - 1] = `${items[items.length - 1]} ${raw.trim()}`.trim();
      values.set(currentKey, items);
      continue;
    }
    const index = raw.indexOf(":");
    if (index <= 0) continue;
    currentKey = raw.slice(0, index).trim().toLowerCase();
    const value = raw.slice(index + 1).trim();
    const items = values.get(currentKey) ?? [];
    items.push(value);
    values.set(currentKey, items);
  }
  return values;
}

const firstHeader = (headers, name) => (headers.get(name.toLowerCase()) ?? [""])[0] ?? "";
const allHeaders = (headers, name) => headers.get(name.toLowerCase()) ?? [];

function noticeLike(path) {
  const normalized = path.replaceAll("\\", "/").toLowerCase();
  const base = normalized.split("/").pop() ?? "";
  return normalized.includes("/licenses/") || /^(license|licence|copying|notice|copyright)([._-]|$)/i.test(base);
}

function sourceUrls(headers) {
  const urls = new Set();
  for (const key of ["home-page", "download-url"]) {
    for (const value of allHeaders(headers, key)) if (/^https?:\/\//i.test(value)) urls.add(value);
  }
  for (const value of allHeaders(headers, "project-url")) {
    const comma = value.indexOf(",");
    const candidate = (comma >= 0 ? value.slice(comma + 1) : value).trim();
    if (/^https?:\/\//i.test(candidate)) urls.add(candidate);
  }
  return [...urls].sort();
}

function renderMaterial(label, path) {
  return [
    `----- BEGIN ${label} -----`,
    normalizeText(readFileSync(path, "utf8")).trimEnd(),
    `----- END ${label} -----`,
    "",
  ].join("\n");
}

function collectPythonDistributions(pythonRoot) {
  const distInfos = walkDirectories(pythonRoot, 5)
    .filter((path) => path.toLowerCase().endsWith(".dist-info"))
    .sort((a, b) => a.localeCompare(b));
  if (!distInfos.length) throw new Error("python_runtime_has_no_dist_info");

  const distributions = [];
  for (const distInfo of distInfos) {
    const metadataPath = requireFile(join(distInfo, "METADATA"), `${relative(pythonRoot, distInfo)}/METADATA`);
    const headers = parseMetadataHeaders(readFileSync(metadataPath, "utf8"));
    const name = firstHeader(headers, "name").trim();
    const version = firstHeader(headers, "version").trim();
    if (!name || !version) throw new Error(`invalid_python_distribution_metadata:${relative(pythonRoot, distInfo)}`);
    const material = walkFiles(distInfo).filter(noticeLike).sort((a, b) => a.localeCompare(b));
    if (!material.length) throw new Error(`python_distribution_missing_license_material:${name}==${version}`);
    const licenseExpression = firstHeader(headers, "license-expression").trim();
    const declaredLicense = firstHeader(headers, "license").trim();
    distributions.push({
      name,
      normalizedName: name.toLowerCase().replaceAll("_", "-"),
      version,
      licenseExpression,
      declaredLicense,
      sourceUrls: sourceUrls(headers),
      material,
      distInfo,
    });
  }
  distributions.sort((a, b) => `${a.normalizedName}==${a.version}`.localeCompare(`${b.normalizedName}==${b.version}`));
  return distributions;
}

function requireDistribution(distributions, name, version) {
  const found = distributions.find((item) => item.normalizedName === name && item.version === version);
  if (!found) throw new Error(`required_python_distribution_missing:${name}==${version}`);
  return found;
}

function materialText(distribution) {
  return distribution.material.map((path) => readFileSync(path, "utf8")).join("\n").toLowerCase();
}

function checkPythonLicenseBoundary(distributions) {
  if (distributions.some((item) => item.normalizedName === "distance")) {
    throw new Error("excluded_distance_distribution_present");
  }
  requireDistribution(distributions, "g2p-en", EXPECTED_G2P_VERSION);
  const frozendict = requireDistribution(distributions, "frozendict", EXPECTED_FROZENDICT_VERSION);
  const soxr = requireDistribution(distributions, "soxr", EXPECTED_SOXR_VERSION);
  requireDistribution(distributions, "fsspec", EXPECTED_FSSPEC_VERSION);
  if (!materialText(frozendict).includes("lesser general public license")) {
    throw new Error("frozendict_lgpl_material_missing");
  }
  if (!materialText(soxr).includes("lesser general public license")) {
    throw new Error("soxr_lgpl_material_missing");
  }
  const combined = distributions.flatMap((item) => item.material).map((path) => readFileSync(path, "utf8")).join("\n");
  if (!/Permission is hereby granted, free of charge/i.test(combined)) {
    throw new Error("mit_license_terms_missing_from_python_runtime");
  }
  if (!/Apache License[\s\S]{0,120}Version 2\.0/i.test(combined)) {
    throw new Error("apache_2_license_terms_missing_from_python_runtime");
  }
}

function modelPresent(backendRoot, model) {
  const prefix = "EngineData/Backend/";
  const expectedPath = String(model.expected_path ?? "");
  if (!expectedPath.startsWith(prefix)) return false;
  const target = join(backendRoot, ...expectedPath.slice(prefix.length).split("/"));
  if (!existsSync(target)) return false;
  if (statSync(target).isFile()) return statSync(target).size > 0;
  return walkFiles(target).some((path) => statSync(path).size > 0);
}

function renderModelInventory(backendRoot, manifest) {
  const rows = [];
  for (const model of manifest.models ?? []) {
    if (model.required !== true && !modelPresent(backendRoot, model)) continue;
    rows.push([
      `Component: ${model.model_id}`,
      `Source: ${model.repo_id ?? "<release-asset>"}`,
      `Revision: ${model.revision ?? "<not-recorded>"}`,
      `Declared license: ${model.license ?? "<not-recorded>"}`,
      model.asset_repo_id ? `Asset source: ${model.asset_repo_id}` : null,
      model.asset_revision ? `Asset revision: ${model.asset_revision}` : null,
    ].filter(Boolean).join("\n"));
  }
  return rows.sort().join("\n\n");
}

function fixedVoiceAttribution() {
  const entries = [
    ["GPT-SoVITS pretrained snapshot", "lj1995/GPT-SoVITS", "336b2ec4e8d4ac74740798dd40af44e74659ecaf", "MIT (repository declaration; nested origins below retain their own attribution)"],
    ["Chinese HuBERT base origin", "TencentGameMate/chinese-hubert-base", "release bytes pinned by the enclosing GPT-SoVITS snapshot", "MIT"],
    ["Chinese RoBERTa WWM Ext Large origin", "hfl/chinese-roberta-wwm-ext-large", "release bytes pinned by the enclosing GPT-SoVITS snapshot", "Apache-2.0"],
    ["ERes2NetV2 speaker-model code origin", "alibaba-damo-academy/3D-Speaker", "vendored source header in pinned GPT-SoVITS revision", "Apache-2.0"],
    ["NLTK averaged_perceptron_tagger", "nltk/nltk_data", "packaged NLTK data resource", "MIT"],
    ["NLTK averaged_perceptron_tagger_eng", "nltk/nltk_data", "packaged NLTK data resource", "MIT"],
  ];
  return entries.map(([component, source, revision, license]) => [
    `Component: ${component}`,
    `Source: ${source}`,
    `Revision/provenance: ${revision}`,
    `Declared license: ${license}`,
  ].join("\n")).join("\n\n");
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
  if (voices.length !== 2) throw new Error("builtin_voice_sources_count");

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
  const pythonRoot = requireDir(join(backendRoot, "LocalWorker", "PythonRuntime"), "LocalWorker/PythonRuntime");
  const pythonExe = requireFile(join(pythonRoot, "python.exe"), "LocalWorker/PythonRuntime/python.exe");
  const pythonLicense = requireFile(join(pythonRoot, "LICENSE.txt"), "LocalWorker/PythonRuntime/LICENSE.txt");
  const pythonVersion = spawnSync(pythonExe, ["--version"], { encoding: "utf8", windowsHide: true });
  const versionText = `${pythonVersion.stdout ?? ""}\n${pythonVersion.stderr ?? ""}`.trim();
  if (pythonVersion.status !== 0 || !versionText.includes(`Python ${EXPECTED_PYTHON_VERSION}`)) {
    throw new Error(`python_runtime_version_mismatch:${versionText || pythonVersion.status}`);
  }

  const distributions = collectPythonDistributions(pythonRoot);
  checkPythonLicenseBoundary(distributions);

  const workerRoot = join(backendRoot, "LocalWorker", "WorkerRuntime");
  const manifest = JSON.parse(readFileSync(requireFile(join(workerRoot, "model_manifest.json"), "WorkerRuntime/model_manifest.json"), "utf8"));
  if (manifest.schema !== "translateit.local_model_inventory.v2") throw new Error("invalid_model_manifest_for_notice_bundle");

  const voiceRoot = join(backendRoot, "RuntimeAssets", "Voice", "GPTSoVITS", "Source");
  const gptLicense = requireFile(join(voiceRoot, "LICENSE"), "Voice/GPTSoVITS/Source/LICENSE");
  const ffmpegLicense = requireFile(join(voiceRoot, "FFMPEG_LICENSE.txt"), "Voice/GPTSoVITS/Source/FFMPEG_LICENSE.txt");
  const ffmpegSource = requireFile(join(voiceRoot, "FFMPEG_SOURCE.txt"), "Voice/GPTSoVITS/Source/FFMPEG_SOURCE.txt");
  const providerNotice = requireFile(join(backendRoot, "RuntimeAssets", "AudioProvider", "VBCABLE", "NOTICE.txt"), "AudioProvider/VBCABLE/NOTICE.txt");

  const packageIndex = distributions.map((item) => [
    `${item.name}==${item.version}`,
    item.licenseExpression ? `License-Expression: ${item.licenseExpression}` : null,
    item.declaredLicense ? `Declared-License: ${item.declaredLicense.replace(/\s+/g, " ").slice(0, 500)}` : null,
    ...item.sourceUrls.map((url) => `Source: ${url}`),
    ...item.material.map((path) => `Included material: ${relative(pythonRoot, path).replaceAll("\\", "/")}`),
  ].filter(Boolean).join("\n")).join("\n\n");

  const packageMaterials = [];
  const seenMaterial = new Set();
  for (const item of distributions) {
    for (const path of item.material) {
      const content = normalizeText(readFileSync(path, "utf8"));
      const key = content;
      if (seenMaterial.has(key)) continue;
      seenMaterial.add(key);
      packageMaterials.push(renderMaterial(`PYTHON RUNTIME MATERIAL: ${relative(pythonRoot, path).replaceAll("\\", "/")}`, path));
    }
  }

  const sections = [
    "TranslateIT Third-Party Notices and Source References\n",
    "This file is generated deterministically from the exact staged release payload. It is not a legal opinion or a declaration that every distribution right has been satisfied. VB-CABLE redistribution rights remain a separate external release gate.\n",
    `Python runtime authority: CPython ${EXPECTED_PYTHON_VERSION}\nSource: https://www.python.org/downloads/release/python-31210/\n`,
    renderMaterial("CPYTHON LICENSE", pythonLicense),
    "===== PYTHON RUNTIME DISTRIBUTION INDEX =====\n\n" + packageIndex + "\n",
    "===== PYTHON RUNTIME LICENSE / NOTICE MATERIAL =====\n\n" + packageMaterials.join(""),
    "===== CONTROLLED MODEL / VOICE INVENTORY =====\n\n" + renderModelInventory(backendRoot, manifest) + "\n\n" + fixedVoiceAttribution() + "\n",
    "===== BUILT-IN VOICE ATTRIBUTION =====\n\n" + builtInVoiceAttribution(backendRoot) + "\n",
    "===== CMUDICT ATTRIBUTION =====\n\nComponent: Carnegie Mellon Pronouncing Dictionary (CMUdict)\nSource: Carnegie Mellon University / nltk_data cmudict package\nUse status recorded by upstream: research and commercial use are unrestricted; acknowledgement of Carnegie Mellon origin is requested when the dictionary is used or redistributed.\n",
    renderMaterial("GPT-SOVITS SOURCE LICENSE", gptLicense),
    renderMaterial("FFMPEG LICENSE", ffmpegLicense),
    renderMaterial("FFMPEG SOURCE / BUILD PROVENANCE", ffmpegSource),
    "===== VB-CABLE NOTICE =====\n\nThe following notice is included for the staged standard VB-Audio VB-CABLE provider. Its presence does not prove concrete redistribution rights for a particular TranslateIT release.\n\n" + normalizeText(readFileSync(providerNotice, "utf8")),
  ];

  const content = normalizeText(sections.join("\n"));
  return {
    content,
    packageCount: distributions.length,
    uniquePythonMaterialCount: packageMaterials.length,
  };
}

export function writeThirdPartyNoticeBundle({ backendRoot = defaultBackendRoot } = {}) {
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
    console.log(`[third-party-notices] wrote ${result.outputPath}; Python distributions=${result.packageCount}; unique license/notice materials=${result.uniquePythonMaterialCount}`);
  } else if (mode === "--check") {
    const outputPath = thirdPartyNoticeOutputPath();
    requireFile(outputPath, "RuntimeAssets/ThirdPartyNotices/THIRD_PARTY_NOTICES.txt");
    const expected = buildThirdPartyNoticeBundle().content;
    const actual = normalizeText(readFileSync(outputPath, "utf8"));
    if (actual !== expected) throw new Error("third_party_notice_bundle_stale");
    console.log("[third-party-notices] staged notice bundle matches the exact current payload.");
  } else {
    throw new Error(`unsupported_notice_mode:${mode}`);
  }
}
