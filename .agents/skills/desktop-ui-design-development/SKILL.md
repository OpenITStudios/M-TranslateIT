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

## ChatGPT UI Preview — bounded, source-first workflow

When asked to preview UI, follow [the UI preview protocol](references/ui-preview-protocol.md) before making a new image, mockup, or HTML artifact. This is a **procedure within this specialist**, not a seventh Skill, a Tauri feature, or a hosted build service.

- **PIN:** Identify the exact repository/ref/HEAD and smallest page/component dependency closure. Never silently combine files from different SHAs or reuse preview assets whose source identity is unknown.
- **CAPABILITY:** F0 source inspection, F1 clearly labeled reconstructed inline preview, F2 actual frontend components rendered with synthetic fixture in a browser, F3 exact native Windows UI. Choose the strongest legitimately available evidence, not the most impressive label. F2/F3 require real execution in the appropriate context; GitHub-only is F0, optionally an explicitly labeled F1.
- **FAST PATH:** One requested page, one scenario, one desktop viewport, one main preview. Reuse the repository's `scripts/ui-preview` entry and typed scenarios when an exact-SHA checkout and frontend toolchain exist. The preview uses an isolated **iframe viewport** so `100vh` and responsive CSS reflect the requested dimensions. Do not reconstruct production Svelte markup by hand when this path is available.
- **ISOLATION:** Use synthetic, deterministic data; no personal transcripts/audio, real device discovery, updater checks, mutation, cloud inference, auto-install, Tauri commands, or runtime service. Preview-only controls are not proof of Meeting or audio behavior.
- **LOCAL F2 EXECUTION:** In an authorized LOCAL_CODE checkout only, first use `node scripts/ui-preview/typecheck.mjs`; the render command is plan-only unless `--execute --expected-sha <full-Local-SHA>` is supplied. The renderer never auto-installs Playwright/Vite and fails closed when either is missing. Read the protocol before invoking it. No command in this Skill authorizes hosted CI or user-PC testing.
- **GUARDS:** Pin asset/style imports; verify source coverage, stable viewport, synthetic badge, absence of native side effects, accessible visible Stop/Retry Stop, long-text containment, and actionable errors. Cache is valid only for matching SHA + dependency scope + scenario + viewport + render environment; do not presume cache survives chats.
- **DELIVERY:** Present inline when supported; otherwise use a browser screenshot of the actual preview (F2), or an F1 reconstruction with an explicit warning. Keep the entire response readable without the visual. One narrow fallback attempt; do not invent a successful screenshot, browser run, native validation, or sandbox file.
- **PROOF:** Tag each assertion F0/F1/F2/F3; never infer target-Windows correctness or audio delivery from F2. Emit a concise per-run receipt, not a second permanent status ledger. For a source-only session, executable and native checks remain `NOT_EXECUTED`.

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
