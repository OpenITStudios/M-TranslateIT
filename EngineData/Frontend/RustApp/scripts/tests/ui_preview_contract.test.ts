import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const appRoot = new URL("../../", import.meta.url);
const repoRoot = new URL("../../../../../", import.meta.url);
const src = (path: string): string => readFileSync(new URL(path, appRoot), "utf8");
const root = (path: string): string => readFileSync(new URL(path, repoRoot), "utf8");

const entry = src("scripts/ui-preview/index.html");
const frameHtml = src("scripts/ui-preview/frame.html");
const parentMain = src("scripts/ui-preview/main.ts");
const frameMain = src("scripts/ui-preview/frame.ts");
const controls = src("scripts/ui-preview/PreviewApp.svelte");
const frame = src("scripts/ui-preview/FrameApp.svelte");
const fixtures = src("scripts/ui-preview/fixtures.ts");
const renderer = src("scripts/ui-preview/render.mjs");
const typecheck = src("scripts/ui-preview/typecheck.mjs");
const tsconfig = JSON.parse(src("scripts/ui-preview/tsconfig.json"));
const header = src("src/components/layout/AppHeader.svelte");
const protocol = root(".agents/skills/desktop-ui-design-development/references/ui-preview-protocol.md");
const skill = root(".agents/skills/desktop-ui-design-development/SKILL.md");
const registry = JSON.parse(root(".agents/skill-registry.json"));
const packageJson = JSON.parse(src("package.json"));

test("F2 mounts production Svelte components in their own real viewport", () => {
  assert.match(entry, /src="\.\/main\.ts"/);
  assert.match(frameHtml, /src="\.\/frame\.ts"/);
  assert.match(parentMain, /import PreviewApp from "\.\/PreviewApp\.svelte"/);
  assert.match(frameMain, /import FrameApp from "\.\/FrameApp\.svelte"/);
  assert.match(frameMain, /src\/styles\/app\.css/);
  assert.match(controls, /<iframe\b/);
  assert.match(controls, /src=\{frameUrl\}/);
  assert.match(controls, /sandbox="allow-scripts allow-same-origin"/);
  assert.match(controls, /data-preview-canvas/);
  assert.match(controls, /data-preview-viewport=\{viewport\}/);
  assert.doesNotMatch(controls, /<Sidebar|<Meeting|<AppHeader/);
  for (const component of ["Sidebar", "AppHeader", "Meeting"]) {
    assert.match(frame, new RegExp(`import ${component} from "\\.\\.\\/\\.\\.\\/src\\/`));
    assert.match(frame, new RegExp(`<${component}\\b`));
  }
  assert.match(frame, /class="flex h-screen min-h-0 w-screen/);
  assert.match(frame, /data-preview-frame-root/);
  assert.match(frame, /data-preview-synthetic="true"/);
});

test("native, updater, network and app interactions stay isolated from preview fixture", () => {
  assert.match(frame, /\sinert\s/);
  assert.match(frame, /showUpdateAction=\{false\}/);
  assert.match(header, /showUpdateAction = true/);
  assert.match(header, /\{#if showUpdateAction\}/);
  assert.match(frame, /function noOperation\(\): void/);
  assert.match(frame, /onMeetingAction=\{noOperation\}/);
  assert.match(frame, /onToggleMute=\{noOperation\}/);
  assert.match(frame, /onNavigate=\{noNavigation\}/);
  assert.doesNotMatch(frame + fixtures + controls, /\b(?:invoke|runCommand|fetch|install|localStorage|sessionStorage|Math\.random|Date\.now)\s*\(/);
  assert.doesNotMatch(frame + controls, /https?:\/\//);
  assert.doesNotMatch(frame, /<FirstSetup|<MyVoice|<MeetingTranscriptExport/);
});

test("six fixture states use canonical ProductMeetingState and stable synthetic time", () => {
  assert.match(fixtures, /mapProductMeetingState\(status\)/);
  assert.match(fixtures, /type ProductRuntimeSnapshot/);
  assert.match(fixtures, /type MeetingSessionStatus/);
  assert.match(fixtures, /type MeetingCommittedTurnsSnapshot/);
  assert.match(fixtures, /const FIXED_MS = Date\.UTC/);
  assert.match(fixtures, /synthetic_preview_fixture_not_native_proof/);
  for (const state of ["ready", "live", "muted", "cleanup", "translation-failed", "long-transcript"])
    assert.ok(fixtures.includes(`{ id: "${state}", label:`), state);
  assert.match(fixtures, /has_session: isLive/);
  assert.match(fixtures, /authority_active: isLive/);
  assert.match(fixtures, /const cleanup = scenario === "cleanup"/);
  assert.match(fixtures, /translation_state: failed \? "failed" : "translated"/);
  assert.match(fixtures, /Array\.from\(\{ length: 36 \}/);
  assert.match(fixtures, /source_text: source/);
  assert.match(fixtures, /lane: "you" \| "incoming"/);
});

test("responsive test is truly viewport-isolated, not only an outer canvas resize", () => {
  for (const [viewport, width, height] of [
    ["compact", 1280, 760], ["desktop", 1600, 940], ["large", 1920, 1080],
  ] as const)
    assert.match(fixtures, new RegExp(`${viewport}: \\{ width: ${width}, height: ${height} \\}`));
  assert.match(controls, /style=\{`width: \$\{size\.width\}px; height: \$\{size\.height\}px;\`\}/);
  assert.match(renderer, /window\.innerWidth/);
  assert.match(renderer, /window\.innerHeight/);
  assert.match(renderer, /matching_canvas_viewport/);
  assert.match(renderer, /no_horizontal_document_overflow/);
  assert.equal(tsconfig.extends, "../../tsconfig.json");
  assert.ok(tsconfig.include.includes("./**/*.svelte"));
  assert.ok(tsconfig.include.includes("../../src/**/*.svelte"));
});

test("renderer defaults to plan-only, pins exact clean Local and blocks remote hosts", () => {
  assert.match(renderer, /execute: false/);
  assert.match(renderer, /expectedSha/);
  assert.match(renderer, /head !== input\.expectedSha/);
  assert.match(renderer, /branch !== "Local"/);
  assert.match(renderer, /--porcelain/);
  assert.match(renderer, /--untracked-files=all/);
  assert.match(renderer, /if \(!input\.execute\)/);
  assert.match(renderer, /import\("vite"\)/);
  assert.match(renderer, /import\("playwright"\)/);
  assert.match(renderer, /"127\.0\.0\.1"/);
  assert.match(renderer, /return route\.abort\("blockedbyclient"\)/);
  assert.match(renderer, /context\.routeWebSocket\(/);
  assert.match(renderer, /socket\.close\(\{ code: 1008/);
  assert.match(renderer, /serviceWorkers: "block"/);
  assert.match(renderer, /acceptDownloads: false/);
  assert.match(renderer, /data-preview-frame-root/);
  assert.match(renderer, /footer\[aria-label="Meeting main action"\]/);
  assert.match(renderer, /F2_RENDERED_CHECKS_FAILED/);
  assert.match(renderer, /"receipt\.json"/);
  assert.match(renderer, /approved_logo_asset_loaded/);
  assert.match(renderer, /stylesheet_applied/);
  assert.match(renderer, /call_dock_not_clipped/);
  assert.match(renderer, /dock_controls_not_clipped/);
  assert.match(renderer, /lanes_distinct/);
  assert.match(renderer, /timestamp_to_seconds/);
  assert.match(renderer, /elapsedMs/);
  assert.match(renderer, /screenshotSha256/);
  assert.match(renderer, /createHash\("sha256"\)/);
  assert.match(renderer, /readFile\(png\)/);
  assert.match(renderer, /source_unchanged_during_render/);
  const screenshot = renderer.indexOf('.screenshot({ path: png');
  const sourceCheck = renderer.indexOf('checks.source_unchanged_during_render =');
  const networkCheck = renderer.lastIndexOf('checks.no_remote_requests = denied.length === 0');
  assert.ok(screenshot > 0 && sourceCheck > screenshot && networkCheck > screenshot,
    "A screenshot must not pass with post-render source or network errors");
  assert.match(renderer, /document\.fonts\.ready/);
  assert.match(renderer, /native_windows: "NOT_EXECUTED"/);
  assert.doesNotMatch(renderer + typecheck, /npm install|npx|ci-dispatch|cargo run/);
});

test("render request is explicit, local-only and cannot silently install Playwright", () => {
  const executionGate = renderer.indexOf("if (!input.execute)");
  const launch = renderer.indexOf('playwright.chromium.launch(');
  assert.ok(executionGate > 0 && launch > executionGate);
  assert.match(renderer, /LOCAL_CODE-only F2 renderer/);
  assert.match(renderer, /Playwright is not already available/);
  assert.doesNotMatch(renderer, /npm (?:ci|install)|npx|pnpm add|yarn add|curl -/);
});

test("two short package commands reuse existing tools without new dependencies", () => {
  assert.equal(packageJson.scripts["preview:typecheck"], "node scripts/ui-preview/typecheck.mjs");
  assert.equal(packageJson.scripts["preview:render"], "node scripts/ui-preview/render.mjs");
  assert.ok(!("playwright" in packageJson.dependencies));
  assert.ok(!("playwright" in packageJson.devDependencies));
});

test("existing typecheck is strict and never installs dependencies", () => {
  assert.match(typecheck, /require\.resolve\("svelte-check\/package\.json"\)/);
  assert.match(typecheck, /--tsconfig/);
  assert.match(typecheck, /0 errors \/ 0 warnings/);
  assert.match(typecheck, /NOT_EXECUTED/);
});

test("Preview remains inside existing specialist with explicit fidelity boundaries", () => {
  assert.equal(registry.skills.length, 6);
  assert.equal(registry.skills.filter((item: { id: string }) => item.id === "desktop-ui-design-development").length, 1);
  assert.match(skill, /references\/ui-preview-protocol\.md/);
  for (const fidelity of ["F0_SOURCE", "F1_RECONSTRUCTED", "F2_BROWSER_COMPONENT", "F3_TARGET_WINDOWS"])
    assert.ok(protocol.includes(fidelity), fidelity);
  assert.match(protocol, /NOT_EXECUTED/);
  assert.match(protocol, /No actual saved sessions/);
  assert.match(protocol, /viewport/);
});
