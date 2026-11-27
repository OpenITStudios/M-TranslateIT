import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync, openSync, readSync, closeSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(scriptDir, "..");
const backendRoot = resolve(appRoot, "../../Backend");
const workerRoot = join(backendRoot, "LocalWorker", "WorkerRuntime");
const manifestPath = join(workerRoot, "model_manifest.json");
const revisionMarker = ".translateit_model_revision";

const EXPECTED_TRANSLATION = {
  model_id: "milmmt-46-1b-v1.0",
  repo_id: "xiaomi-research/MiLMMT-46-1B-v1.0",
  revision: "4fc480b6c58dec29c159dcdf9fde0f6d5c354995",
  expected_path:
    "EngineData/Backend/RuntimeAssets/Translation/ModelData/xiaomi-research--MiLMMT-46-1B-v1.0",
};

const LEGACY_TRANSLATION_DIRS = [
  "m2m100-418m",
  "facebook--m2m100_418M",
  "marianmt-id-en",
  "marianmt-en-id",
];

const fail = (message) => {
  throw new Error(`release_model_revision:${message}`);
};

const requireFile = (path, label) => {
  if (!existsSync(path) || !statSync(path).isFile() || statSync(path).size <= 0) {
    fail(`missing_file:${label}`);
  }
};

// GGUF can exceed Node's single-buffer limit. Hash with bounded scratch.
const hashScratch = Buffer.allocUnsafe(1024 * 1024);
function sha256File(path) {
  const hash = createHash("sha256");
  const fd = openSync(path, "r");
  try {
    let count;
    while ((count = readSync(fd, hashScratch, 0, hashScratch.length, null)) > 0) {
      hash.update(hashScratch.subarray(0, count));
    }
    return hash.digest("hex");
  } finally {
    closeSync(fd);
  }
}

if (!existsSync(manifestPath)) fail("manifest_missing");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
if (
  manifest.schema !== "translateit.local_model_inventory.v2" ||
  manifest.inventory_scope !== "full_product_release_assets"
) {
  fail("manifest_contract");
}

const translations = (manifest.models ?? []).filter(
  (model) => model.stage === "translation_bidirectional_id_en",
);
if (translations.length !== 1) fail("translation_entry_count");
const translation = translations[0];
for (const [key, expected] of Object.entries(EXPECTED_TRANSLATION)) {
  if (translation[key] !== expected) fail(`translation_${key}`);
}
if (translation.backend !== "milmmt") fail("translation_backend");
if (translation.source_type !== "huggingface") fail("translation_source_type");

const prefix = "EngineData/Backend/";
for (const model of manifest.models ?? []) {
  if (model.required !== true || model.source_type !== "huggingface") continue;
  const expectedPath = String(model.expected_path ?? "");
  if (!expectedPath.startsWith(prefix)) fail(`invalid_expected_path:${model.model_id}`);
  const target = join(backendRoot, ...expectedPath.slice(prefix.length).split("/"));
  if (!existsSync(target) || !statSync(target).isDirectory()) {
    fail(`missing_model_dir:${model.model_id}`);
  }

  const marker = join(target, revisionMarker);
  requireFile(marker, `${model.model_id}/${revisionMarker}`);
  const actualRevision = readFileSync(marker, "utf8").trim();
  if (actualRevision !== model.revision) {
    fail(`revision_mismatch:${model.model_id}:${actualRevision}`);
  }

  for (const pattern of model.download_allow_patterns ?? []) {
    if (/[*?[\]]/.test(pattern)) continue;
    requireFile(join(target, ...pattern.split("/")), `${model.model_id}/${pattern}`);
  }

  const artifactHashes = model.artifact_hashes ?? {};
  if (Object.keys(artifactHashes).length === 0) {
    fail(`artifact_hashes_missing:${model.model_id}`);
  }
  for (const [relativePath, expectedHash] of Object.entries(artifactHashes)) {
    const path = join(target, ...relativePath.split("/"));
    requireFile(path, `${model.model_id}/${relativePath}`);
    const actual = sha256File(path);
    if (actual !== String(expectedHash).toLowerCase()) {
      fail(`artifact_hash_mismatch:${model.model_id}:${relativePath}:${actual}`);
    }
  }
}

const translationRoot = join(backendRoot, "RuntimeAssets", "Translation", "ModelData");
for (const legacy of LEGACY_TRANSLATION_DIRS) {
  if (existsSync(join(translationRoot, legacy))) fail(`legacy_translation_present:${legacy}`);
}

console.log(
  `[release-model-revision] canonical MiLMMT + required Hugging Face revision markers PASS (${EXPECTED_TRANSLATION.revision})`,
);
