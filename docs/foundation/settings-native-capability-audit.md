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

## Rust/Tauri follow-up — confirmed source findings (2026-11-10)

Inspected exact native files:
- `EngineData/Frontend/RustApp/src-tauri/Cargo.toml`: Tauri 2.12.5, Rust 1.99.1; `tauri-plugin-updater` present; no `tauri-plugin-global-shortcut` dependency listed.
- `EngineData/Frontend/RustApp/src-tauri/src/main.rs`: updater plugin initialized; no global shortcut plugin initialization in this file.
- `EngineData/Frontend/RustApp/src-tauri/capabilities/default.json`: main window permits core show/hide/destroy; no global-shortcut permissions.
- `EngineData/Frontend/RustApp/src-tauri/src/commands/registry.rs`: registered native saved transcript list/preview/delete/export commands.
- `EngineData/Frontend/RustApp/src-tauri/src/engine/paths.rs`: `user_cache_dir` uses `CacheData`; `user_saved_dir` uses `SavedProject`.
- `EngineData/Frontend/RustApp/src-tauri/src/commands/meeting_session/transcript_archive.rs`: session archive root is `SavedProject/Transcripts/Sessions` under the discovered user-data root.
- `EngineData/Frontend/RustApp/src-tauri/src/commands/meeting_session/transcript_export.rs`: HTML exports are written separately beneath the saved-data root; export has source-preservation and completeness warnings.

### Keybind native implementation prerequisites
1. Add and initialize the official Tauri 2 global-shortcut plugin, with least-privilege window capability permissions. Confirm OS and Rust version compatibility before changing dependencies.
2. Define actions and safe event ownership, including key pressed/released semantics, PTT edge handling, conflict validation, and shortcut lifecycle.
3. Persist shortcuts with versioned settings migration, never enabling Start/Stop globally by default.
4. Add native and frontend tests; only then expose active Record Shortcut UI.
Official reference: https://v2.tauri.app/plugin/global-shortcut/

### Saved Data native implementation prerequisites
1. Inventory every directory under `SavedProject` and `CacheData` before defining cleanup scopes.
2. Read-only storage inventory must reject symlinks, handle unreadable entries and overflow, and report unknown size rather than zero.
3. Do not count exports as transcript archives; do not delete export files when clearing cache.
4. Implement per-category destructive actions only after ownership/active-writer guards and clear confirmation contracts are verified.
5. Keep archive list/export/delete APIs intact; no bulk delete until a separate safe native API exists.

This is source audit only, not a build, Windows execution, or proof of repository-wide absence of alternate key handlers.
