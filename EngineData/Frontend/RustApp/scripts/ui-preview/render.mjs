// Explicit LOCAL_CODE-only F2 renderer. Never install packages, run Tauri or dispatch CI.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const repoRoot = resolve(appRoot, "../../..");
const git = (...args) => execFileSync("git", args, { cwd: repoRoot, encoding: "utf8" }).trim();
const fail = (message) => { throw new Error(message); };

function options(argv) {
  const result = { execute: false, currentLocal: false, expectedSha: "", scenario: "live", viewport: "desktop" };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--execute") result.execute = true;
    else if (arg === "--current-local") result.currentLocal = true;
    else if (arg === "--expected-sha") result.expectedSha = argv[++i] ?? "";
    else if (arg === "--scenario") result.scenario = argv[++i] ?? "";
    else if (arg === "--viewport") result.viewport = argv[++i] ?? "";
    else fail("Unrecognized preview argument: " + arg);
  }
  if (result.currentLocal === Boolean(result.expectedSha))
    fail("Provide exactly one of --current-local or --expected-sha <40-hex SHA>.");
  if (result.expectedSha && !/^[0-9a-f]{40}$/.test(result.expectedSha))
    fail("The --expected-sha value must be the full 40-character pinned commit SHA.");
  if (!/^[a-z][a-z-]{0,48}$/.test(result.scenario))
    fail("Invalid scenario name.");
  if (!/^[a-z][a-z-]{0,32}$/.test(result.viewport))
    fail("Invalid viewport name.");
  return result;
}

async function main() {
  const startedMs = performance.now();
  const input = options(process.argv.slice(2));
  const head = git("rev-parse", "HEAD");
  const branch = git("branch", "--show-current");
  if (branch !== "Local") fail("Preview source must be on the user-selected Local branch.");
  if (input.currentLocal) input.expectedSha = head;
  if (head !== input.expectedSha) fail("HEAD does not equal the requested source SHA; no screenshot was made.");
  const tracked = [
    "EngineData/Frontend/RustApp/src",
    "EngineData/Frontend/RustApp/scripts/ui-preview",
    "EngineData/Frontend/RustApp/index.html",
    "EngineData/Frontend/RustApp/vite.config.ts",
    "EngineData/Frontend/RustApp/package.json",
    "EngineData/Frontend/RustApp/package-lock.json",
  ];
  const dirty = git("status", "--porcelain", "--untracked-files=all", "--", ...tracked);
  if (dirty) fail("Preview dependency files are modified or untracked. Pin a clean exact-source checkout.");

  const receipt = {
    repository: "OpenITStudios/M-TranslateIT",
    branch: "Local",
    sha: head,
    scenario: input.scenario,
    viewport: input.viewport,
    fidelity: "F2_BROWSER_COMPONENT",
    synthetic: true,
    native_windows: "NOT_EXECUTED",
    audio_runtime: "NOT_EXECUTED",
  };
  if (!input.execute) {
    console.log(JSON.stringify({ ...receipt, status: "PLAN_ONLY", next: "Add --execute in an authorized LOCAL_CODE environment with existing Vite and Playwright." }, null, 2));
    return;
  }
  if (!existsSync(join(appRoot, "node_modules", "vite", "package.json")))
    fail("Local Vite dependency is not installed. F2 NOT_EXECUTED; no dependency installation was attempted.");
  let viteApi, playwright;
  try {
    viteApi = await import("vite");
  } catch {
    fail("Vite module cannot be imported. F2 NOT_EXECUTED.");
  }
  try {
    playwright = await import("playwright");
  } catch {
    fail("Playwright is not already available in this checkout. F2 NOT_EXECUTED; do not auto-install.");
  }
  if (!playwright.chromium) fail("Chromium renderer is unavailable in the existing Playwright installation.");

  let viteServer = null;
  let browser = null;
  try {
    viteServer = await viteApi.createServer({
      root: appRoot,
      configFile: join(appRoot, "vite.config.ts"),
      mode: "development",
      server: { host: "127.0.0.1", port: 0, strictPort: false, open: false },
      logLevel: "error",
    });
    await viteServer.listen();
    const address = viteServer.httpServer?.address();
    if (!address || typeof address === "string") fail("Vite did not bind a local TCP port.");
    const port = address.port;
    browser = await playwright.chromium.launch({ headless: true });
    const context = await browser.newContext({
      viewport: { width: 2060, height: 1290 },
      locale: "en-GB",
      timezoneId: "Asia/Jakarta",
      colorScheme: "dark",
      reducedMotion: "reduce",
      deviceScaleFactor: 1,
      serviceWorkers: "block",
      acceptDownloads: false,
    });
    const denied = [];
    const errors = [];
    // A page-level HTTP interceptor alone does not prevent outbound WebSockets.
    // Playwright routeWebSocket (v1.48+) is required for network-isolated F2.
    if (typeof context.routeWebSocket !== "function")
      fail("Playwright lacks WebSocket routing; cannot establish a network-isolated F2 proof.");
    await context.routeWebSocket("**/*", (socket) => {
      const url = new URL(socket.url());
      if (url.protocol === "ws:" && url.hostname === "127.0.0.1" && url.port === String(port)) {
        socket.connectToServer();
        return;
      }
      denied.push(url.origin);
      socket.close({ code: 1008, reason: "External websocket blocked in UI preview" });
    });
    await context.route("**/*", (route) => {
      const url = new URL(route.request().url());
      if (url.protocol === "http:" && url.hostname === "127.0.0.1" && url.port === String(port))
        return route.continue();
      denied.push(url.origin);
      return route.abort("blockedbyclient");
    });
    const page = await context.newPage();
    page.setDefaultTimeout(20_000);
    page.setDefaultNavigationTimeout(20_000);
    page.on("pageerror", (error) => errors.push(String(error.message)));
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    page.on("websocket", (ws) => {
      const url = new URL(ws.url());
      if (url.hostname !== "127.0.0.1" || url.port !== String(port))
        denied.push(url.origin);
    });
    const url = `http://127.0.0.1:${port}/scripts/ui-preview/index.html`;
    const response = await page.goto(url, { waitUntil: "domcontentloaded", timeout: 20_000 });
    if (!response?.ok()) fail("Preview HTML entry did not load successfully.");
    const selectScenario = page.locator('select[aria-label="Preview scenario"]');
    const selectViewport = page.locator('select[aria-label="Preview viewport"]');
    if (!(await selectScenario.locator(`option[value="${input.scenario}"]`).count()))
      fail("Requested synthetic scenario is not defined by this source revision.");
    if (!(await selectViewport.locator(`option[value="${input.viewport}"]`).count()))
      fail("Requested viewport is not defined by this source revision.");
    await selectScenario.selectOption(input.scenario);
    await selectViewport.selectOption(input.viewport);
    const frame = page.frameLocator("iframe[data-preview-frame]");
    const root = frame.locator(`[data-preview-frame-root][data-preview-scenario="${input.scenario}"]`);
    await root.waitFor({ state: "visible", timeout: 20_000 });
    // Wait for the exact fixture, fonts and approved logo to settle before checking pixels.
    await root.evaluate(async () => { await document.fonts.ready; });
    await frame.locator("aside img").first().evaluate(async (element) => {
      if (element instanceof HTMLImageElement) await element.decode().catch(() => undefined);
    });

    const dimensions = await page.locator("[data-preview-canvas]").evaluate((element) => ({
      width: Math.round(element.getBoundingClientRect().width),
      height: Math.round(element.getBoundingClientRect().height),
    }));
    const internal = await frame.locator('[data-preview-frame-root]').evaluate((element) => ({
      width: window.innerWidth,
      height: window.innerHeight,
      scrollWidth: document.documentElement.scrollWidth,
      rootInert: element.hasAttribute("inert"),
      fontFamily: getComputedStyle(document.body).fontFamily,
    }));
    const sidebar = frame.locator('aside[aria-label="TranslateIT navigation"]');
    const dock = frame.locator('footer[aria-label="Meeting main action"]');
    const dockGeometry = await dock.evaluate((element) => {
      const rect = element.getBoundingClientRect();
      const controls = [...element.querySelectorAll("button, select")]
        .filter((control) => getComputedStyle(control).display !== "none")
        .map((control) => control.getBoundingClientRect());
      return {
        top: rect.top, bottom: rect.bottom, width: rect.width,
        controlsUnclipped: controls.length > 0 && controls.every((c) =>
          c.width > 0 && c.height > 0
          && c.left >= 0 && c.right <= innerWidth + 1
          && c.top >= 0 && c.bottom <= innerHeight + 1),
      };
    });
    const assetAndCss = await sidebar.evaluate((element) => {
      const mark = element.querySelector("img");
      const color = getComputedStyle(element).backgroundColor;
      return {
        logoLoaded: mark instanceof HTMLImageElement && mark.complete && mark.naturalWidth > 0,
        sidebarStyled: color !== "rgba(0, 0, 0, 0)" && color !== "transparent",
      };
    });
    const checks = {
      matching_canvas_viewport: internal.width === dimensions.width && internal.height === dimensions.height,
      root_inert: internal.rootInert,
      no_horizontal_document_overflow: internal.scrollWidth <= internal.width,
      sidebar_present: await sidebar.count() === 1,
      approved_logo_asset_loaded: assetAndCss.logoLoaded,
      stylesheet_applied: assetAndCss.sidebarStyled,
      meeting_workspace_present: await frame.locator('section[aria-label="Meeting workspace"]').count() === 1,
      call_dock_visible: await dock.isVisible(),
      call_dock_not_clipped: dockGeometry.top >= 0 && dockGeometry.bottom <= internal.height + 1,
      dock_controls_not_clipped: dockGeometry.controlsUnclipped,
      no_remote_requests: denied.length === 0,
      no_browser_errors: errors.length === 0,
    };
    const primary = await frame.locator('footer[aria-label="Meeting main action"] button').last().textContent();
    const expectedPrimary = input.scenario === "cleanup" ? "Retry Stop"
      : input.scenario === "ready" ? "Start" : "Stop Meeting";
    checks.truthful_primary_action = (primary ?? "").includes(expectedPrimary);
    if (input.scenario !== "ready" && input.scenario !== "cleanup") {
      const incoming = frame.locator('article[aria-label="Meeting audio"]').first();
      const outgoing = frame.locator('article[aria-label="You"]').first();
      checks.incoming_left = await incoming.count() > 0;
      checks.outbound_right = await outgoing.count() > 0;
      if (checks.incoming_left && checks.outbound_right) {
        const incomingRect = await incoming.boundingBox();
        const outgoingRect = await outgoing.boundingBox();
        checks.lanes_distinct = Boolean(incomingRect && outgoingRect && outgoingRect.x > incomingRect.x);
        const timestamp = await outgoing.locator("[data-chat-timestamp]").first().textContent();
        checks.timestamp_to_seconds = /^\d{2}:\d{2}:\d{2}$/.test((timestamp ?? "").trim());
      }
    }
    if (input.scenario === "translation-failed")
      checks.translation_failure_disclosed = (await frame.locator('[aria-label="Meeting conversation"]').textContent())?.includes("Translation unavailable") === true;
    if (input.scenario === "long-transcript")
      checks.long_transcript_rendered = await frame.locator('article[aria-label="You"], article[aria-label="Meeting audio"]').count() >= 30;

    const dir = await mkdtemp(join(tmpdir(), "translateit-ui-preview-"));
    const png = join(dir, `meeting-${input.scenario}-${input.viewport}.png`);
    const json = join(dir, "receipt.json");
    await page.locator("[data-preview-canvas]").screenshot({ path: png, animations: "disabled" });
    const screenshotSha256 = createHash("sha256").update(await readFile(png)).digest("hex");
    // Recheck *after* screenshot: screenshot work itself can trigger loads or source edits.
    checks.source_unchanged_during_render =
      git("rev-parse", "HEAD") === head
      && git("status", "--porcelain", "--untracked-files=all", "--", ...tracked) === "";
    checks.no_remote_requests = denied.length === 0;
    checks.no_browser_errors = errors.length === 0;
    const success = Object.values(checks).every(Boolean);
    const full = {
      ...receipt,
      status: success ? "F2_RENDERED_CHECKS_PASS" : "F2_RENDERED_CHECKS_FAILED",
      dimensions,
      browser: "Chromium",
      browserVersion: browser.version(),
      locale: "en-GB",
      timezone: "Asia/Jakarta",
      deviceScaleFactor: 1,
      fontFamily: internal.fontFamily,
      elapsedMs: Math.round(performance.now() - startedMs),
      checks,
      deniedRequests: denied.length,
      browserErrors: errors.slice(0, 5),
      screenshot: png,
      screenshotSha256,
    };
    await writeFile(json, JSON.stringify(full, null, 2) + "\n", "utf8");
    console.log(JSON.stringify({ ...full, receipt: json }, null, 2));
    if (!success) process.exitCode = 1;
  } finally {
    await browser?.close();
    await viteServer?.close();
  }
}

main().catch((error) => {
  console.error(JSON.stringify({ status: "NOT_VERIFIED", message: String(error?.message ?? error), rendered: "NOT_CONFIRMED" }));
  process.exitCode = 1;
});
