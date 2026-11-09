# TranslateIT Desktop UI Consistency Standard

**Status:** ADOPTED source-design contract for `Local`; browser and Windows validation NOT_EXECUTED.
**Source baseline:** `9b8357c2f02f4e14cdea09b5034589d05e36d28d`.
**Provenance:** engineering principles adapted from M-LazyDesigner Local standards; product hierarchy, palette and icon library remain TranslateIT-specific.

## Principles

**Rapi:** one owner for each layout dimension, separator, icon role, spacing and component state.
**Jelas:** every label, status, action, disabled reason and destination must be truthful.
Do not add dead preview controls, duplicate state, fake native operations, or unnecessary decoration.

## Component owners

- App shell: `src/App.svelte`
- Sidebar: `src/components/layout/Sidebar.svelte`
- Voice Actor, mute and audio footer: `src/components/layout/SidebarUserControls.svelte`
- Recent Meetings and Projects: `src/components/layout/MeetingLibraryNavigation.svelte`
- Header and audio selector: `src/components/layout/AppHeader.svelte`
- Meeting and Start/Stop dock: `src/pages/Meeting.svelte`
- Tokens and styles: `src/styles/tokens.css`, `src/styles/app.css`

All paths above are relative to `EngineData/Frontend/RustApp/`.

## Shared geometry and seam rules

1. Sidebar footer and Meeting action dock MUST use `--ti-bottom-dock-height: 70px`, not independently maintained 62px/70px literals.
2. Both top borders use `1px solid var(--ti-border)` and must align in the same bottom shell track; avoid doubled borders or arbitrary offsets.
3. Outer height includes border and padding (`box-sizing: border-box`). Children must not expand the fixed dock height.
4. Sidebar navigation and Meeting body scroll independently; footer and dock stay pinned.
5. Popovers overlay rather than change footer height, anchor to their triggers, and stay within viewport.
6. The Meeting dock is conditional: it is not present on every Saved/Settings/My Voice route. Do not invent a fake dock merely to continue the divider.
7. Where both docks are visible, the top-border Y positions must differ by no more than **1 CSS pixel** in an actual rendered measurement. Check 100%, 125%, 150%, 200% scaling and high-DPI rasterization separately.

## Visual system

- Keep TranslateIT's dark Discord-like surfaces, blue accent, status colors, radii and current 248px sidebar with 224px responsive override.
- Reuse semantic `--ti-*` variables rather than literal colors. Prefer spacing increments 4/8/12/16/24px.
- Use the existing `@lucide/svelte` icon family, typically 16–18px in controls, with consistent stroke and accessible button labels. Decorative icons use `aria-hidden`.
- Align icon, label, badge and context-action columns consistently. Hover/focus/selected must not shift outer geometry.
- Long names truncate with an accessible full value; no horizontal overflow or unreadable critical labels.

## Interaction and status

- `Live` and `Muted` are independent statuses; both may appear simultaneously.
- `Stop Needed` means incomplete cleanup, not every retained or transitional session.
- Header audio source and Meeting action use the same runtime-owned source of truth. Native runtime retains Start/Stop authority.
- The same action in a row, menu, toolbar or shortcut must share eligibility, disabled reason and destination.
- One context menu/editor at a time, even when a Meeting appears in Recent and Project. Escape/Cancel returns focus; deleted triggers need a stable fallback.
- Async operations guard duplicate submission, stale focus, stale menu placement and navigation races.
- `Ready` stays concise; last-check time remains accessible via hover and keyboard.

## State and responsive matrix

For changed controls inspect idle, hover, focus-visible, selected, disabled, busy, empty, stale, error, cancellation and success where applicable. Support keyboard, accessible names, reduced motion, contrast and non-color status cues.

Check widths 1920, 1600, 1440, 1280, 1024 and narrowest supported; include short-height windows and scaling 100/125/150/200%. Verify seam coordinates, footer content fit, menu flip/clipping, long labels and zoom. An unrun measurement stays `NOT_EXECUTED`.

## HTML preview and evidence

`HTML_SOURCE_PREVIEW` must state exact Local SHA, page, viewport, synthetic fixture and approximations. Use actual Svelte/CSS/tokens; simulated controls are not native actions. Preview-only styling does not automatically repair production.

`SOURCE_REVIEWED` = pinned source inspected.
`SOURCE_AUTHORED` = source committed.
`VISUAL_VERIFIED` = real rendered evidence at stated dimensions.
`NATIVE_UNVERIFIED` = Windows/audio behavior not tested.
`PACKAGE_READY` requires separate release gates.

**Open QA:** shared footer/dock geometry is source-aligned but not measured; conditional routes' bottom edges need visual review; menu clipping, keyboard, browser, Svelte build and Windows acceptance remain unverified.

## Mandatory review checklist

1. Pin Local HEAD and inspect exact owners.
2. State the invariant and single token/component owner.
3. Inspect adjacent panels, routes and state variants.
4. Preserve product semantics, SVG family and native safety.
5. Review bounded source diff and document exceptions.
6. Report source, HTML preview, browser and native evidence separately.
