# Rust/Tauri Backend

This folder contains the desktop backend used by the current TranslateIT application.

## Current structure

```text
main.rs
-> process entrypoint and Tauri builder

app_bootstrap.rs
-> application path initialization
-> main-window setup
-> Windows lifecycle hooks

commands/
-> thin Tauri command boundary
-> Meeting, My Voice, Settings, audio, native inference, and diagnostics commands

engine/
-> reusable Rust runtime/domain logic
-> audio, capture lifecycle, settings, paths, logging, and runtime state
```

There is no separate `bridge/` source tree here. `commands/helper_bridge*` owns native inference lifecycle/admission and dispatch to the existing `tools/translation_quality/native_cuda` crate. There is no production Python process or second AI runtime.

## Rule

Keep `main.rs` small. Put Tauri-facing command wrappers in `commands/` and reusable runtime/domain behavior in `engine/`. Add another module only when a current responsibility and caller require it.
