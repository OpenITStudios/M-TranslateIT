# Engine Translate Runtime Modules

This folder contains Rust runtime logic for Meeting, My Voice, Settings and their shared audio lifecycle.

## Current owners

- `audio/`: physical microphone capture, Meeting Sound capture, finalized speech/VAD and owned finalized PCM handoff. `audio/resample.rs` owns whole-clip mono FFT conversion shared with My Voice; one fixed-rate instance per non-16-kHz finalized Meeting producer, no extra worker. Rubato 5.0.1 `process_all` resets each clip and trims startup delay. VAD resampling and capture provenance remain unchanged; 16 kHz moves directly. Failure enters the existing speech ingress fault/cleanup path, never linear fallback. Native WER/CER and latency acceptance remain held.
- `capture_lifecycle.rs`: bounded Mic Test Start/Stop ownership only.
- `runtime_state.rs`: one application Meeting/capture session and generation authority.
- `runtime_settings.rs` + `settings.rs`: persisted runtime settings.
- `paths.rs`: repository-development and installed runtime/user-data path ownership.
- `logging.rs`: bounded runtime log writer used by current settings/runtime paths.
- `state.rs`: shared command/lifecycle result types.

ASR, translation and selected voice synthesis use one Rust-owned native CUDA runtime via `commands/helper_bridge*`. `audio/mod.rs` owns the shared bounded audio-thread exit observation; resource owners keep their own handles and cleanup.

## Maintenance rule

Keep only runtime logic with a current caller. Do not reintroduce dry-run planners, professional readiness gates, History/Chat persistence, transcript-save planning, native inference candidates, or generic service/domain scaffolding merely because older branches contained them.
