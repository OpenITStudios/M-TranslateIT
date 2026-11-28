# Tauri Command Modules

This folder contains the Tauri command boundary for the current Meeting / Text / My Voice / Settings product.

## Active modules

- `meeting_session.rs` — public Meeting status/Start/Stop orchestration and lifecycle coordination; child modules own committed turns, consumer thread lifecycle, lane pipelines, deferred incoming work, and internal session status/preflight stores.
- `codex_gateway.rs` / `codex_gateway/` — opt-in, diagnostic-only owned Codex App Server child; bounded stdio JSON-RPC with one-slot owned writer, checked write receipts and bounded reader, ChatGPT account, source-derived CLI/version and read-only gateway OAuth provider admission in `compatibility.rs`. `transport/fake_app_server_tests.rs` supplies test-only in-memory scripted I/O for later offline verification; it is not a production worker. `realtime.rs` owns source-only role-aware transcript observations and a bounded PCM frame contract; `correlation.rs` holds a one-flight **unverified** source/assistant pairing lease; `realtime_rpc.rs` owns bounded offline V3 WebRTC Start/Audio/Stop JSON-RPC and audio ACK/source-clock contracts. None authorizes a Codex Voice session or Meeting output until Voice is proven.
- `helper_bridge.rs` / `helper_bridge_runtime.rs` — native local inference bridge and scheduler; routine lifecycle snapshot reads do not write trace logs (poisoned lock faults remain logged); the UI capability command uses an idle-only nonwaiting permit and lifecycle try-lock (busy means not checked), while explicit Start status retains normal admission and functional preparation refreshes capability within the native bind permit; `helper_bridge/functional_readiness.rs` owns required outbound functional readiness; `helper_bridge/request_policy.rs` owns Meeting request classification, stale-request, and outbound-pipeline admission policy; `helper_bridge_runtime/scheduler.rs` owns priority/admission/permit scheduling, including the original monotonic deferred incoming deadline through permit wait and pre-inference handoff.
- `audio.rs` — microphone/device status and device checks used by setup.
- `mic_test.rs` — Microphone Test Start/Stop wrappers.
- `runtime.rs` / `runtime_inventory.rs` — runtime readiness and explicit model-presence verification.
- `settings.rs` — runtime settings load/save and audio-device selection.
- `virtual_mic_route.rs` — managed virtual microphone used by the meeting application.
- `voice_lab.rs`, `voice_lab_recording.rs`, `voice_lab_build.rs` — legacy internal protocol/storage implementation identifiers for the product feature now named **My Voice**; `voice_lab/storage.rs` owns dataset freezing, actor-package validation, WAV inspection, and transactional candidate promotion. These names remain only where changing them requires an explicit compatibility migration; they are not product vocabulary.
- `diagnostic_trace.rs` — bounded diagnostics trace command support.
- `bridge_paths.rs` — packaged private Python/runtime path resolution.
- `registry.rs` — Tauri invoke registration list.

## Naming rule

- `my_voice_reference.rs` — shared Windows-native reference eligibility/ranking (one preview or up to three evaluation candidates) and gross synthesis checks. It owns no inference, profile mutation or approval; native Quick Preview uses the existing runtime.

- `my_voice_runtime.rs` — one exclusive native voice-operation scope shared by Quick Preview and the source-only `voice_lab_build/native_evaluation.rs` core; no independent model owner or approval path.

- `voice_lab_build/native_review.rs` — source-only unapproved native candidate package with reference/audio/model/score integrity, under the existing Candidate workspace; no selected voice, history or backup mutation.

- `voice_lab_build/native_build.rs` — explicit native candidate Create/Improve lifecycle over the existing build generation, frozen dataset, cancellation, cleanup and completion receipts. Review is available only after completed identity checks; approval saves an immutable native version; explicit selection uses the existing MyVoice directory and binds the verified reference at Meeting preparation.

Prefer names that describe the responsibility directly. New product-facing or semantic source must use **My Voice** (`MyVoice`, `myVoice`, or `my_voice` according to language convention). Do not introduce new `VoiceLab` product terminology.

Existing `voice_lab` command names, state/error identifiers, and on-disk storage paths may remain only where changing them would require an explicit compatibility migration. Product-facing bridges must translate any legacy display copy to **My Voice** without rewriting those machine-facing identifiers.

A source rename is complete only after the retained frontend, Python, Rust, repository, and MiLMMT checks pass on the candidate head. New command wrappers should be added only when a current product requirement and direct caller require them.
