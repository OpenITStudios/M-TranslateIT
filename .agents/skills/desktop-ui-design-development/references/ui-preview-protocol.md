# ChatGPT HTML UI Preview Protocol — TranslateIT

This is the canonical operating procedure for the `desktop-ui-design-development` specialist when the user requests a UI preview in ChatGPT. Root `AGENTS.md`, `GITHUB_RULES.md`, user-selected `Local` branch, runtime safety and privacy constraints remain authoritative.

## One supported ChatGPT preview method

**`HTML_SOURCE_PREVIEW` is the sole visual preview method within ChatGPT.** Reconstruct the *current production* Svelte UI as a standalone, source-faithful HTML/CSS document using the selected page, imported UI, current styles, semantic tokens and one documented synthetic fixture. Do not require or offer F2 browser-component or F3 target-Windows capture workflows as prerequisites or alternative preview tiers. Browser/native execution remains part of separate app validation, not this preview procedure.

Do **not** use image-generation tools to fabricate a screenshot, mockup, wireframe, product UI, audio/native state or image fallback. A provided screenshot is evidence to inspect, not authorization to synthesize a replacement.

An observation-only request (`amati`, audit) or planning/discussion request remains text-only and read-only. An **explicit UI preview request** authorizes HTML/CSS output immediately; there is **no second conceptual-mockup permission gate**. Ordinary design feedback alone does not authorize modifying the production application. A production UI edit requires its own development authority.

## Required source and visual fidelity

1. **PIN:** Use exactly `OpenITStudios/M-TranslateIT`, user-selected `Local`, current full HEAD SHA and smallest source closure for the requested page. Never mix SHAs or infer current UI from commit messages alone.
2. **READ:** Inspect production Svelte page/components under `EngineData/Frontend/RustApp/src/`, their imported visual dependencies, `src/styles/app.css`, `src/styles/tokens.css` and relevant typed fixture or state mapping. The existing product copy, tokens and UI hierarchy outrank invented examples.
3. **TRANSLATE:** Convert Svelte template constructs and conditional rendering for one selected fixture into static HTML/CSS. Preserve actual navigation/sidebar, header, panel proportions, typography, spacing, palette, borders, controls, labels, transcript presentation, alignment, empty/error/disabled states and scroll/overflow boundaries *as supported by source*. Do not redesign the product, invent controls, add generic dashboards or change the underlying Svelte app.
4. **DEFAULT:** If unspecified, preview the current Meeting page with synthetic `ready` state at 1600×940 in the existing two-column layout. Use only a supported `Local` fixture: Live Meeting (`ready`, `live`, `muted`, `cleanup`, `translation-failed`, `long-transcript`) or My Voice states when their relevant components are sourced. Do not guess native statuses or user audio/data.
5. **DELIVER:** Show HTML in ChatGPT if a suitable surface is available; otherwise create a self-contained standalone `.html` file. Include an explicit `HTML_SOURCE_PREVIEW` label, repo/ref/SHA, page, fixture, viewport, source files and approximations. Do not provide only a description when a visual preview was requested and HTML file creation is available.
6. **CHECK:** Verify the translated markup against the selected Svelte source, real token values, control labels, content containment, visible primary actions and selected fixture. Clearly distinguish *source-grounded approximation* from native/functional acceptance; do not claim pixel-perfect or successful Svelte/browser/Windows runs.

## Safety and isolation

The HTML artifact is a **visual discussion deliverable**, not a replacement frontend, runtime, installer or generated product UI module.

- Self-contained, deterministic and offline; no external CSS/fonts/scripts/images/network or model inference. Approved local/embedded static SVG graphics may be included if backed by source.
- No Tauri calls, real microphones, saved transcripts, personal voice data, updater, Python worker, device detection, persistent mutations or cloud services.
- Preview controls are visually represented only; they must not imply real speech translation, voice training, session history changes, or successful native actions. Never fake live data or readiness.
- Preserve the actual application architecture and the existing Vite/Svelte fixture harness without deleting or altering it just because the ChatGPT preview uses HTML.
- Keep GitHub Actions CI paused and preserve `Local`-only source policy. No user-PC tests, Codex, package installation, local Svelte build or Windows capture is required to produce this preview.

## Evidence and minimal receipt

Use this format in the preview reply or artifact metadata:

```yaml
method: HTML_SOURCE_PREVIEW
repository: OpenITStudios/M-TranslateIT
ref: Local
sha: <exact-40-character-SHA>
page: Meeting
fixture: ready
viewport: 1600x940
source_coverage: [<actual files inspected>]
approximations: [<genuine differences or unknowns>]
svelte_build: NOT_EXECUTED
native_windows_audio: NOT_EXECUTED
```

Source inspection can establish styling and markup fidelity; an HTML preview can support **visual feedback only**. A proper compiled Svelte app/browser and installed Tauri/Windows/audio integration have separate validation owners. Removing F2/F3 *from ChatGPT's preview procedure* does not remove their production test obligations.

## Failure policy

If the source cannot be accessed, disclose the specific missing files/permissions and present only supported evidence, not an invented design. If HTML rendering inside ChatGPT is unavailable, provide an offline `.html` file rather than falling back to a text-only F0 limitation. Do not generate imagery as a workaround. Do not add another preview tier, a new Skill, a second frontend implementation, a hosted CI task or an unsolicited UI redesign.
