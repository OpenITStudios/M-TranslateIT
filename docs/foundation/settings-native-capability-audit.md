# Settings native capability audit — Keybinds and Saved Data

Branch: Local
Status: SOURCE_INSPECTED / NO_NATIVE_TESTS / NO_UI_FEATURE_IMPLEMENTATION
Date: 2026-11-10

## Evidence inspected
- EngineData/Frontend/RustApp/src/pages/Settings.svelte
- EngineData/Frontend/RustApp/src/components/layout/Sidebar.svelte
- EngineData/Frontend/RustApp/src/components/layout/SidebarUserControls.svelte
- EngineData/Frontend/RustApp/src/app/bridge/runtimeApi.ts
- EngineData/Frontend/RustApp/src/app/shared/types.ts
- EngineData/Frontend/RustApp/src/components/meeting/MeetingTranscriptExport.svelte

GitHub code search for keybind, shortcut, hotkey and storage returned incomplete/empty results; it cannot establish repository-wide absence. No backend-wide negative claim is made.

## Verified frontend capabilities
- Settings exposes four current tabs: Voice & Audio, Meeting & Transcript, Appearance, Help & Diagnostics.
- Native-facing device selection, device validation, route check/repair, microphone test and resource locking exist in Settings.
- Transcript appearance and floating caption preferences have existing components.
- Saved transcript API: listSavedMeetingTranscripts, deleteSavedMeetingTranscript, exportSavedMeetingTranscriptHtml, openSavedMeetingHtml.
- Legacy archive management component warns on unverified storage completeness and confirms deletion.

## Not yet verified / do not ship as working
- Global/local shortcut registration, persistence, conflict detection, Push to Talk.
- Independent monitoring speaker volume and microphone gain controls.
- Accurate on-disk storage totals and categorization, bulk export/delete, cache clearing and retention controls.
- Windows native audio runtime and visual rendering for new UI.

## Keybind implementation gate
1. Inventory native Tauri/Rust command registration and existing OS hooks, including app permissions.
2. Define key combinations, scope, ownership, collision rules and persistence schema.
3. Ensure meeting Start/Stop has no default global shortcut; active audio safety first.
4. Implement conflict-aware UI only after native registration and deregistration are confirmed.
5. Test foreground/background behavior, focus, restart and Windows permission failure.

## Saved Data implementation gate
1. Inventory native storage paths and distinguish transcript archives, export files, settings, caches and diagnostics.
2. Add verified size/count APIs before displaying usage totals.
3. Preserve per-channel HTML export/delete and integrity warnings.
4. Make cache clearing a separate native action that never deletes transcripts.
5. Gate bulk destructive actions with explicit confirmation, active-session lock, and verification of result.
6. Test incomplete archives, missing files, permission failures, concurrent writes and restart.

## Next safe UI step
Refine current Settings navigation and Voice & Audio layout without removing existing controls. New categories can be shown as explanatory planned capabilities only if explicitly labeled unavailable, but avoid dead interactive buttons.
