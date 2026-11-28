import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { buildThirdPartyNoticeBundle, thirdPartyNoticeOutputPath, defaultBackendRoot } from "./generate_third_party_notices.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "../../../..");
// One owner verifies native model, DLL, ORT/VAD, built-in and provider bytes.
const result = spawnSync("python", ["-B", "-s", join(scriptDir, "build_r3_external_payload.py"),
  "--repo-root", repoRoot, "--verify-inputs"], { stdio: "inherit", windowsHide: true });
if (result.error || result.status !== 0) throw new Error("native_release_input_verification_failed");
const expected = buildThirdPartyNoticeBundle().content;
const actual = readFileSync(thirdPartyNoticeOutputPath(defaultBackendRoot), "utf8").replace(/\r\n/g, "\n").trimEnd() + "\n";
if (actual !== expected) throw new Error("third_party_notice_bundle_stale");
// A byte match is not release authorization. Keep the existing release hold.
throw new Error("native_package_closure_not_reviewed");
