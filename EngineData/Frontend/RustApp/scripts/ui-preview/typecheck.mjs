// Strict preview typecheck using the already-locked Svelte toolchain. No install.
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const require = createRequire(import.meta.url);
let checker;
try {
  checker = resolve(dirname(require.resolve("svelte-check/package.json")), "bin/svelte-check");
} catch {
  console.error("[ui-preview:typecheck] NOT_EXECUTED: the locked svelte-check dependency is not installed locally.");
  process.exit(2);
}
const result = spawnSync(process.execPath, [checker, "--tsconfig", "./scripts/ui-preview/tsconfig.json"], {
  cwd: appRoot,
  encoding: "utf8",
  shell: false,
});
if (result.stdout) process.stdout.write(result.stdout);
if (result.stderr) process.stderr.write(result.stderr);
if (result.error || result.status !== 0) {
  console.error("[ui-preview:typecheck] Failed or unavailable. No browser/native proof claimed.");
  process.exit(result.status || 1);
}
const summary = `${result.stdout ?? ""}\n${result.stderr ?? ""}`.match(/svelte-check found\s+(\d+)\s+errors?\s+and\s+(\d+)\s+warnings?/i);
if (!summary || Number(summary[1]) !== 0 || Number(summary[2]) !== 0) {
  console.error("[ui-preview:typecheck] Strict acceptance requires the exact summary: 0 errors, 0 warnings.");
  process.exit(1);
}
console.log("[ui-preview:typecheck] SOURCE TYPECHECK: 0 errors / 0 warnings; browser and native NOT_EXECUTED.");
