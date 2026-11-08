# ChatGPT UI Preview Protocol

This is the operating procedure for `desktop-ui-design-development`. It does not activate an agent, install a plugin, grant write permission, or create a second frontend runtime. Root `AGENTS.md`, `GITHUB_RULES.md`, user-selected branch and product privacy policy take precedence.

## Purpose and invariants

**Request:** "preview UI", "preview Meeting", "show actual layout", "compare before and after", "check responsive" or similar.

**Output contract:** the smallest useful preview for the request, clearly marked by fidelity and source SHA. A preview is never automatically a screenshot of an installed application.

1. **No generative-image tool** for source-based UI previews. Avoid invented layouts as source claims.
2. **One identity:** repository, exact commit SHA, component closure, scenario revision, viewport, theme, locale, timezone, renderer version. If any source changed mid-run, discard the mixed evidence and re-pin once; do not claim fresh proof from stale artifacts.
3. **One source:** real production Svelte/Tailwind tokens and approved SVG are authoritative. Never maintain hand-copied frontend clones as a parallel UI owner.
4. **One boundary:** no Tauri/native calls, real microphone, cloud model, updater, external audio, hidden user data, network fonts/assets or persistent app mutations inside the preview. Never bypass the native gate.
5. **One output by default:** one page, one standard scenario and one viewport, with a compact QA summary. More screens/states only if needed by the user's request or to resolve a material risk.

## Fidelity levels and capability gates

| Label | Actual evidence | Allowed claim |
|---|---|---|
| `F0_SOURCE` | GitHub/current source review only | Components/tokens/handlers present in source |
| `F1_RECONSTRUCTED` | Manually constructed inline HTML/diagram, explicitly identified | Visual design discussion, *not* proof of production rendering |
| `F2_BROWSER_COMPONENT` | Exact-revision production Svelte components rendered by the existing Vite stack with typed synthetic fixture and browser screenshot | Frontend visual composition for this fixture/browser/viewport; not Tauri/Windows/audio |
| `F3_TARGET_WINDOWS` | Exact installed Tauri/Windows target observation | Native UI at inspected resolution and state; audio/device outcomes require their own tests |

Do not call a standalone mockup or recreated HTML `F2`. GitHub `Local` access without an actual compatible checkout and an executable frontend toolchain is `F0`; optional `F1` is explicitly marked. Do not dispatch paused CI or ask users for incremental PC tests.

## Automatic routing — least work first

- "Preview UI now": latest pinned `Local`, current/relevant page, standard desktop 1600×940, one state. Resolve from current request context before asking.
- "Preview [component]": render the owning page with only the component's dependent state, not a full suite.
- "Before/after": require two named SHAs, same scenario/viewport/browser and distinct source provenance on both sides. If old SHA is unavailable, explain instead of synthesizing "before".
- "Responsive": 1280×760, 1600×940 and 1920×1080, with no layout rewriting between screenshots.
- "Bug/overflow": add one discriminating synthetic edge scenario and inspect geometry, visibility, clipping and keyboard focus if the preview interaction allows it.
- "Full review": batch relevant pages/states; do not render everything by default.
- "Preview without a real frontend toolchain": report F0 or F1 without inventing F2.

## TranslateIT browser fixture — source contract

Canonical entry is `EngineData/Frontend/RustApp/scripts/ui-preview/index.html`, served **only by existing Vite** from a clean exact-SHA checkout with already available dependencies. The outer UI holds scenario and viewport selectors. It loads `frame.html` in a same-origin, script-enabled **isolated iframe viewport**: its `FrameApp.svelte` mounts the production `Sidebar.svelte`, `AppHeader.svelte`, `Meeting.svelte` and their imported transcript/bubble components, using the exact `src/styles/app.css`. This iframe is required because CSS media queries, `100vh`, overflow and fixed controls must follow the *selected viewport*, not the preview toolbar's browser window. The iframe isolates **layout viewport semantics**, not security authority: `allow-scripts allow-same-origin` is not a strong sandbox against trusted same-origin source. The safety boundary is no Tauri host, an `inert` production view, no mutable callbacks, no updater mount, synthetic-only fixtures and the renderer's denied external network routes. Audit imported components for automatic mount-time side effects before adding any new page. Do not flatten this into a resized `<div>` or clone the production markup. Production `src/main.ts` is untouched; no new frontend/runtime owner is introduced.

The first fixture covers only **Live Meeting**: `ready`, `live`, `muted`, `cleanup`, `translation-failed` and `long-transcript`. No actual saved sessions, native device data, profile pictures or identifiable speaker claims. Typed fixture state is derived through existing product state mapping; it is not a native response receipt.

To avoid native effects, the production updater component is suppressed by a presentation-only default-on `AppHeader` prop in this preview, while the previewed app surface **inside the iframe** is `inert` (no browser clicks/keys can trigger runtime actions). Only scenario and viewport selectors outside that surface are interactive. **Keyboard navigation, focus order, control click behavior, actual runtime/mute/Stop and real Tauri mutations are NOT TESTED by this read-only F2 harness**. Test interactive controls separately only after a safe, explicitly reviewed mock/adaptor prevents all native effects; never remove `inert` just to make a screenshot appear interactive. Do not mount `FirstSetup`, `MyVoice` or `Saved Transcripts` until their own read-only lifecycle and data-isolation seams have been reviewed. Do not replace unavailable runtime evidence with a successful-looking mock.

**Executable source-only commands (LOCAL_CODE only, explicit invocation):**

```sh
node scripts/ui-preview/typecheck.mjs
node scripts/ui-preview/render.mjs --expected-sha <exact-40-character-Local-SHA>
node scripts/ui-preview/render.mjs --execute --expected-sha <exact-40-character-Local-SHA> --scenario live --viewport desktop
```

Equivalent convenience commands from `EngineData/Frontend/RustApp` are `npm run preview:typecheck` and `npm run preview:render -- --expected-sha <SHA> [--execute --scenario live --viewport desktop]`; they only forward to the existing scripts and do not install or upgrade packages. The first command requires an already installed, locked `svelte-check` and rejects warnings; it never installs dependencies. The second is a **plan-only preflight**. Only `--execute` permits F2 screenshot rendering: the runner verifies the exact clean Local commit, uses the existing Vite API on loopback, and imports already available Playwright/Chromium. If Playwright or its browser is missing, mark F2 `NOT_EXECUTED`—do not install packages, launch CI, reach the public network or ask the user for a PC test. If executed, capture `[data-preview-canvas]` as an actual production-component browser screenshot and write an **ephemeral** JSON receipt with source identity, browser, locale, timestamp policy and QA checks. The runner rejects remote HTTP and WebSocket requests (including cross-origin attempts), disables service workers and downloads, and checks browser errors, missing logo assets, stylesheet loss, clipped dock **and all its controls**, mismatched iframe viewport, false action labels, swapped bubbles and invalid translated failure presentation. It records the screenshot's SHA-256 digest and repeats the source-cleanliness and network/error checks **after** capture; failure produces a nonzero exit and a failing receipt rather than a passing screenshot. It does **not** test real clicks, audio, voice, or saved data. Internal script behavior is never proven by simply reading its text.

The canonical renderer must use `en-GB` locale, `Asia/Jakarta` timezone, a stable synthetic fixture clock, reduced motion and no network-dependent assets. No automatic server start or dependency installation is authorized by this document; the explicit `--execute` option is a prerequisite. ChatGPT access to GitHub alone cannot run this renderer.

## Determinism and cache

Cache key: `commit SHA + transitive UI/style/assets SHA set + scenario version + viewport + locale + timezone + browser/version + deviceScaleFactor + font fingerprint`. The screenshot SHA-256 in each receipt attests to the artifact bytes, **not** source authenticity, runtime correctness or privacy approval. Invalidate on changes to shared tokens, CSS, font, shell, imported components, fixture DTO or renderer. A narrow component edit can reuse unaffected screenshot contexts only when dependency closure is demonstrated, not guessed. Cross-chat/cache persistence is never guaranteed.

Freeze fixture timestamps and data; do not depend on `Date.now()`, audio devices, network calls or random values. Browser animations should be disabled for screenshots, but UI controls and scroll must retain their real styles in the rendered source.

## QA minimum and failure policy

For F2, at minimum capture: exact SHA+clean-checkout source identity, correct iframe inner viewport and `[data-preview-canvas]` dimensions, approved Ti logo successfully loaded, actual CSS applied, no horizontal document overflow, title/transcript visible, persistent Start/Stop/Retry Stop dock within viewport, right/left bubbles not swapped, timestamps with seconds, missing translations explicitly labeled, and no browser console errors or remote requests. Native command calls are disabled by `inert` + guarded mounting; actual Windows invocation is not tested in F2. Test selected edge scenario for long source text, empty/unknown time, warning visibility and responsive bounds. Script presence does not mean checks ran.

- **P0:** wrong SHA/source, invalid fixture, hidden Stop or Retry Stop, native side effect, privacy leakage, false readiness.
- **P1:** overflow, clipped controls, illegible text, broken focus/keyboard navigation, missing error state.
- **P2:** subtle spacing, typography, contrast or cosmetic differences.
- Pixel diffs are *candidates*, not automatically bugs. State the exact reproduced scenario, viewport and source identity. Distinguish `CONFIRMED`, `SUSPECTED`, `UNVERIFIED`.
- On renderer failure, one bounded retry only if the specific cause changes. Otherwise use F0/F1 with an honest limitation; never fabricate Playwright/Windows evidence.
- Full native audio, Windows layout, and device ownership remain `NOT_EXECUTED` until the separate integrated acceptance phase.

## Per-response receipt (ephemeral)

```yaml
repository: OpenITStudios/M-TranslateIT
ref: Local
sha: <exact SHA>
page: Meeting
scenario: live
viewport: 1600x940
fidelity: F0_SOURCE | F1_RECONSTRUCTED | F2_BROWSER_COMPONENT | F3_TARGET_WINDOWS
source_coverage: [Sidebar.svelte, AppHeader.svelte, Meeting.svelte, MeetingActivity.svelte, MeetingTurnBubble.svelte, tokens.css]
rendered: NOT_EXECUTED | VERIFIED
native_windows: NOT_EXECUTED
limitations: <exact missing tool/evidence, if any>
```

Never store synthetic transcripts in `UserData/` or publish any recordings, audio/logs or user-identifying material. The existing Git history and canonical validation documents remain authoritative; this receipt is not a new progress database.
