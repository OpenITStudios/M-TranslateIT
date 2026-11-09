---
name: desktop-ui-design-development
description: TranslateIT Svelte UI specialist for source-first ChatGPT preview, visual layout, tokens, spacing, typography, component states, accessibility, responsive composition and rendered acceptance. Do not use for native Windows/audio truth, AI/runtime/package behavior or architecture migration.
---

# Desktop UI Design Development

## Current visual stack

```text
Svelte 5
+ Tailwind CSS 4
+ CSS custom-property semantic tokens
+ selective Bits UI
+ Lucide Svelte
```

This stack is already current. Do not describe vanilla→Svelte migration as pending.

## Owns

- visual hierarchy/composition;
- spacing/density/alignment;
- typography and readable contrast;
- semantic colors/radii/elevation/tokens;
- visible interaction states;
- responsive/resizable desktop composition;
- visual accessibility and reduced-motion behavior;
- rendered visual acceptance.

## Does not own

Navigation semantics, readiness truth, AI/audio behavior, persistence, installer/package behavior or product requirements.

## Working modes

`ALIGN` when a coherent visual language exists; `ESTABLISH` only when the surface genuinely lacks one or the user explicitly approves replacement.

For external references separate:

```text
OBSERVED
INFERRED
ADOPTED
```

A screenshot is evidence, not automatic policy.

## Rules

Prioritize task/state hierarchy before decoration. Reject generic AI gradients/glow, gratuitous glassmorphism, unnecessary card grids, component inflation, animation without a job and large UI-library adoption without a current need.

Material states should cover only states that can actually occur: default, focus, active/selected, checking/loading, ready/success, degraded/setup-needed/error, disabled/unavailable.

## ChatGPT visual preview — source-faithful HTML only

**Do not use generative-image tools** to make UI screenshots, wireframes, mockups, preview imagery, or Before/After artwork. User screenshots may be inspected as reference evidence, not modified into fabricated product captures.

- **INSPECT / PLAN / DISCUSS:** inspect and discuss source and design in text. No automatic rendering or source changes from an observation-only request.
- **PREVIEW:** explicit requests such as "preview UI", "tampilkan UI", "lihat tampilannya", or "buat preview HTML" authorize `HTML_SOURCE_PREVIEW`. This is the **only ChatGPT visual preview method**. It is not gated on local Svelte, Vite, Playwright, Tauri, native Windows, Codex, CI, or PC tests. Do not stop at source-only analysis simply because those environments are unavailable. No extra permission for "conceptual reconstruction" is required.
- **DEVELOPMENT:** change production UI/source only on a development request. Creating HTML for review does not itself authorize production redesign or backend changes.
- **VALIDATION:** source inspection, HTML visual review, actual compiled Svelte behavior and native Windows/audio testing are distinct evidence. Never upgrade HTML to executable/native proof.

## Source-grounded HTML preview procedure

Follow [the UI preview protocol](references/ui-preview-protocol.md) when producing or maintaining a preview.

1. Pin `OpenITStudios/M-TranslateIT`, the user-selected `Local` HEAD SHA, the requested page and deterministic synthetic scenario. Read the owning Svelte components, imported UI, `src/styles/app.css`, `src/styles/tokens.css` and applicable fixture/state mapping. Use actual source as the visual authority.
2. Translate only Svelte-specific template behavior into **standalone HTML/CSS**. Preserve production hierarchy, visible content, labels, font, spacing, colors, sizing, controls, scroll behavior and state. Prefer exact CSS/token values; isolate one fixture for conditional states. No invented feature, generic mockup, off-brand redesign or parallel production frontend.
3. Default to the current Meeting page in the existing TranslateIT two-column shell, a standard desktop 1600×940 viewport and a safe `ready` fixture unless the user specifies another page, viewport or state. If the source lacks an element or its behavior cannot be represented, label the limitation; never fill the gap with speculative content.
4. Deliver an actual HTML preview within ChatGPT when possible, otherwise a **standalone .html file**. Include `HTML_SOURCE_PREVIEW`, repository/ref/SHA, page, fixture, viewport, source coverage and genuine approximations. A file or rendered preview is more useful than a plan to make one.
5. Keep it self-contained, synthetic and offline: **no** Tauri/native command, microphone, audio, worker, external network, inference, updater, installs, filesystem side effects, credentials or personal data. Static controls must not claim functional Meeting/voice behavior.
6. Audit visual source coverage, hierarchy, token fidelity, text overflow and visible controls. Mark actual Svelte build/browser/Windows/audio validation `NOT_EXECUTED` unless those separate checks genuinely ran. Do not require those checks for HTML review.

The existing production Svelte 5/Tauri application, browser fixture scripts, CI policy and integrated native acceptance gates remain untouched. This change only simplifies how ChatGPT shows a **source-grounded visual preview**; it is not a claim of pixel-perfect or functional equivalence.

## Procedure

1. Ground the current page, user task and real interaction states in Svelte source.
2. Reuse existing semantic tokens, component patterns and accessibility boundaries before adding UI structure.
3. Separate visual styling and state presentation from navigation, readiness and backend ownership.
4. Change only the owning UI surfaces; inspect responsive, focus, disabled, degraded and reduced-motion cases as relevant.
5. Use rendered evidence at the required window sizes for visual claims; static CSS inspection is not visual acceptance.
6. Stop after the scoped visual acceptance. If runtime truth is wrong, document evidence for a separate desktop-runtime handoff instead of rewriting it here.

## Proof

Root `GITHUB_RULES.md` limits GitHub to **source/version control only**: no cloud compute, hosted CI, remote build, or cloud inference. Keep build/model/device proof `UNKNOWN / NOT EXECUTED` until the grouped local build-and-native acceptance phase; never request per-commit tests on the user's PC.

Static proof can establish style ownership/tokens/markup. Claims such as visual match, responsive composition, motion feel or rendered accessibility require rendered evidence at relevant dimensions/context.
