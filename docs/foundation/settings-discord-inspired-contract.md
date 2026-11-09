# TranslateIT Settings — Discord-inspired contract (design specification)

Status: DESIGN_APPROVED_IN_PRINCIPLE / IMPLEMENTATION_NOT_STARTED / NATIVE_QA_NOT_EXECUTED
Branch: Local
Scope: Settings IA, interaction safety, feature preservation. This is not a claim of runtime capability.

## Non-negotiable preservation
- Preserve existing Voice & Audio device selection, device validation, virtual microphone route check/repair, microphone test, translation engine and noise suppression controls.
- Preserve transcript appearance, floating caption preferences, diagnostics, voice selection, and existing audio resource locks.
- Do not delete, silently rename, or replace a working action with a mock.
- Do not present simulated hardware, volume control, Push-to-Talk, keybinds, or data management as native functionality.
- Keep Discord-inspired navigation, density, familiar icons and discoverability without copying branding.

## Information architecture (target)
1. Voice & Audio
2. Translation
3. Transcript
4. Keybinds
5. Saved Data
6. Appearance
7. Privacy
8. Advanced / Diagnostics
9. About

Only expose target tabs when a usable implementation exists. Existing four tabs remain operational during migration.

## Voice & Audio: one page, grouped sections
A. Microphone Input — native input device selection, Windows Default, missing/ambiguous endpoint warnings, microphone test.
B. Incoming Meeting Sound — separate native capture source for meeting audio, not necessarily the user's monitoring speaker.
C. Virtual Microphone / Call Routing — verify selected virtual endpoint, check route and repair setup; do not claim that Discord/Zoom selected it.
D. Monitoring Output — proposed; requires separate native capability validation.
E. Input profile, input/output volume, level meter, Push to Talk — proposed only; require backend audit and native tests.

Never conflate physical microphone, incoming meeting capture, virtual translated output and monitoring speaker. Changing devices while an audio resource is owned remains locked as currently implemented. Show the reason and recovery action.

## Keybinds (proposed)
- Actions: mute, deafen (if native), Push to Talk (if native), show/hide app, floating captions, optional meeting start/stop.
- Each shortcut has scope (local/global), Record, Clear, Reset, conflict checks, and accessible keyboard feedback.
- Do not bind Start/Stop Meeting by default. Global shortcut registration and conflict handling require explicit native support.
- Never intercept OS-reserved combinations.

## Saved Data (proposed)
- Show real on-disk sizes only when backend supplies verified data.
- Distinguish transcripts, export files, cache, diagnostics and settings.
- Per-channel HTML export and delete stay in channel Menu; bulk actions are separate.
- Destructive bulk deletion requires explicit confirmation, exclusion of active resources, and clear result/error.
- Clearing cache must not delete transcripts. No fictional cloud sync or retention guarantees.

## Translation / Transcript
- Translation controls change processing configuration and must respect active meeting locks.
- Transcript controls affect presentation: translated text primary, original secondary, speaker labels You/Other or Person 1 when unidentified, complete timestamp, density and visibility.
- Keep a single source of truth for transcript preferences across Live, Last and saved channels.
- Header Search stays visible; channel Menu is for Export as HTML, Rename, Move, Pin/Unpin and Delete. Do not remove settings access; place presentation preferences in Transcript settings.

## UI and accessibility
- Target desktop sidebar 240–260px; content max 900px; header 56–60px; controls 40–44px.
- Distinguish headings, descriptions, values and warnings. Consistent icon size and text baselines.
- Responsive scroll, no clipped flyouts; keyboard Escape, outside-click, focus restoration, visible focus, meaningful disabled descriptions.
- Device lists show both name and role, selection indicators, Windows Default and unavailable state.

## Acceptance / QA gates
1. Inventory and compare all existing actions before/after (zero unintended removals).
2. Validate each native control with real runtime API, errors and resource-lock scenarios.
3. Test keyboard, focus, popup bounds and long device names.
4. Check viewport widths 800, 1024, 1280, 1440, 1920 and Windows scaling.
5. Verify persistence across restart; failures must not be reported as saved.
6. No visual PASS without screenshots; no native PASS without native test.
