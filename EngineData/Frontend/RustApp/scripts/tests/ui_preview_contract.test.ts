import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const source = (path: string): string => readFileSync(new URL(path, root), "utf8");

const entry = source("scripts/ui-preview/index.html");
const main = source("scripts/ui-preview/main.ts");
const screen = source("scripts/ui-preview/PreviewApp.svelte");
const fixture = source("scripts/ui-preview/fixtures.ts");
const tsconfig = JSON.parse(source("scripts/ui-preview/tsconfig.json"));
const header = source("src/components/layout/AppHeader.svelte");
const skill = readFileSync(new URL("../../../../../.agents/skills/desktop-ui-design-development/SKILL.md", import.meta.url), "utf8");
const protocol = readFileSync(new URL("../../../../../.agents/skills/desktop-ui-design-development/references/ui-preview-protocol.md", import.meta.url), "utf8");
const registry = JSON.parse(readFileSync(new URL("../../../../../.agents/skill-registry.json", import.meta.url), "utf8"));

test("preview is a separate Vite HTML entry using real production Svelte and CSS", () => {
  assert.match(entry, /<script type="module" src="\.\/main\.ts"><\/script>/);
  assert.match(main, /import "\.\.\/\.\.\/src\/styles\/app\.css"/);
  assert.match(main, /import PreviewApp from "\.\/PreviewApp\.svelte"/);
  for (const name of ["Sidebar", "AppHeader", "Meeting"]) {
    assert.match(screen, new RegExp(`<${name}\\b`));
    assert.match(screen, new RegExp(`import ${name} from "\\.\\.\\/\\.\\.\\/src\\/`));
  }
  assert.match(screen, /data-preview-canvas/);
  assert.match(screen, /data-preview-synthetic="true"/);
  assert.match(screen, /\sinert\s/);
  assert.match(screen, /showUpdateAction=\{false\}/);
  assert.match(header, /showUpdateAction = true/);
  assert.match(header, /\{#if showUpdateAction\}/);
});

test("no preview code triggers real Tauri actions, persistent user data, or network", () => {
  assert.doesNotMatch(screen, /\b(?:onMount|runtimeApi|applicationRuntimeApi|myVoiceApi|invoke|fetch|install|recordAudio|localStorage|sessionStorage)\s*\(/);
  assert.doesNotMatch(screen, /https?:\/\//);
  assert.doesNotMatch(fixture, /\b(?:invoke|fetch|Date\.now|Math\.random|localStorage|sessionStorage)\s*\(/);
  assert.match(screen, /function noOperation\(\): void/);
  assert.match(screen, /onMeetingAction=\{noOperation\}/);
  assert.match(screen, /onToggleMute=\{noOperation\}/);
  assert.match(screen, /onNavigate=\{noNavigation\}/);
});

test("fixture follows production types and includes six deterministic Meeting states", () => {
  assert.match(fixture, /type ProductRuntimeSnapshot/);
  assert.match(fixture, /type MeetingSessionStatus/);
  assert.match(fixture, /type MeetingCommittedTurnsSnapshot/);
  assert.match(fixture, /mapProductMeetingState\(status\)/);
  assert.match(fixture, /const FIXED_MS = Date\.UTC/);
  assert.match(fixture, /const cleanup = scenario === "cleanup"/);
  assert.match(fixture, /has_session: isLive/);
  assert.match(fixture, /authority_active: isLive/);
  assert.match(fixture, /meeting_setup_state: "completed"/);
  for (const state of ["ready", "live", "muted", "cleanup", "translation-failed", "long-transcript"]) {
    assert.ok(fixture.includes(`{ id: "${state}", label:`), state);
  }
  assert.match(fixture, /"frontend_bridge_unavailable"|synthetic_preview_fixture_not_native_proof/);
  assert.match(fixture, /lane: "you" \| "incoming"/);
  assert.match(fixture, /source_text: source/);
  assert.match(fixture, /translation_state: failed \? "failed" : "translated"/);
  assert.match(fixture, /Array\.from\(\{ length: 36 \}/);
});

test("viewport baseline and separate typecheck are bounded, not a new runtime", () => {
  assert.match(fixture, /compact: \{ width: 1280, height: 760 \}/);
  assert.match(fixture, /desktop: \{ width: 1600, height: 940 \}/);
  assert.match(fixture, /large: \{ width: 1920, height: 1080 \}/);
  assert.equal(tsconfig.extends, "../../tsconfig.json");
  assert.ok(tsconfig.include.includes("./**/*.svelte"));
  assert.ok(tsconfig.include.includes("../../src/**/*.svelte"));
  assert.doesNotMatch(entry + main, /cdn\.|https?:\/\//);
});

test("the existing six specialists remain the only registered skills and proof is honest", () => {
  assert.equal(registry.skills.length, 6);
  assert.equal(registry.skills.filter((item: { id: string }) => item.id === "desktop-ui-design-development").length, 1);
  assert.match(skill, /references\/ui-preview-protocol\.md/);
  assert.match(protocol, /F0_SOURCE/);
  assert.match(protocol, /F1_RECONSTRUCTED/);
  assert.match(protocol, /F2_BROWSER_COMPONENT/);
  assert.match(protocol, /F3_TARGET_WINDOWS/);
  assert.match(protocol, /NOT_EXECUTED/);
  assert.match(protocol, /No actual saved sessions/);
});
