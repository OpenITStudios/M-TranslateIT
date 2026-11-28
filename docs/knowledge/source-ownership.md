---
id: document.knowledge.source-ownership
class: DOCUMENT
domain: knowledge
role: CONTRACT
authority: CANONICAL
lifecycle: ACTIVE
---

# TranslateIT Source Ownership

This file maps semantic responsibility to the current owner. It does not carry proof outcomes or historical implementation detail.

## Governance

| Responsibility | Owner |
|---|---|
| GitHub branch/ref/history, CI security, retry/STOP | `GITHUB_RULES.md` |
| Agent execution/routing | `AGENTS.md` |
| External reverse-engineering investigation method / REA evidence scope | `docs/knowledge/operations/rea-investigation.md` (conditional developer tooling; domain source owners and existing proof gates remain authoritative) |
| REA opt-in CLI readiness and private JavaScript Evidence capture | `tools/repository_rea.py` + `tools/tests/test_repository_rea.py` (no provider registry or product dependency) |
| Skill identity/classification | `.agents/skill-registry.json` |
| Advisory agent path/action preflight | `.agents/permissions/permission-policy.json` + `tools/repository_permissions.py` |
| Permission regression corpus | `.agents/evals/permission-cases.json` + `tools/tests/test_repository_permissions.py` |
| Agent routing/procedure expectations + receipt scorer | `.agents/evals/` + `tools/repository_agent_evals.py` |
| Skill package static admission | `tools/repository_skill_admission.py` |
| Proof taxonomy / development discipline | `docs/knowledge/development-discipline.md` |
| Toolchain policy | `tooling/windows/config/requirements.json` |
| Unified Windows developer routing | `manage.cmd` → `tooling/windows/manage.ps1` and `tooling/windows/modules/Workspace.psm1` |
| Stable product facts | `CONTEXT.md` |
| Product/system law | `docs/foundation/` |
| Active continuation | `planning/development.md` |
| Proof interpretation | `docs/knowledge/current-validation.md` |
| Repository static governance | `tools/verify_repository.py` |
| Document identity/typed graph/path witnesses/section lookup and Obsidian/Pele source export | `docs/**/*.md` metadata + links → `tools/repository_knowledge.py` (derived; navigation architecture in `docs/system/knowledge-architecture.md`) |
| Cross-language contract/test binding | `tools/interop-contracts.json` + `tools/repository_contracts.py` |
| Changed-path impact/test planning | `tools/repository_impact.py` |
| Opt-in affected verification execution | `tools/repository_verification.py` (fixed existing test/validator commands only; execution never automatic) |
| Offline audio fixture/test harness (not a runtime owner) | `tools/audio_validation/scenarios.json` owns acoustic cases and independent regression profiles; `harness.py` owns fixture identity; `worker_runner.py` owns execution/transport; `pipeline_checks.py` owns pipeline decisions shared with offline scoring; `vad_runner.py` invokes test-only `engine/audio/audio_fixture_replay_tests.rs`. ASR/translation/TTS grading remains owned by the existing quality corpora/evaluators; canonical PCM/VAD remains owned by `engine/audio/` |
| Derived source-import relationships | `tools/repository_dependencies.py` (ephemeral Rust/TS/Python graph; partial, not semantic authority) |
| Task-mode context projection / handoff | `tools/repository_context.py` (read-only composition; mode policy remains in `AGENTS.md`) |
| Frontend source/bridge contracts | `EngineData/Frontend/RustApp/scripts/` validators |

## Desktop product/runtime

| Responsibility | Owner |
|---|---|
| App composition shell | `src/App.svelte` |
| Windows single-instance admission before runtime bootstrap | `src-tauri/src/app_bootstrap/single_instance.rs`, called by `src-tauri/src/main.rs` |
| Product pages | `src/pages/` |
| Product runtime composition | `src/app/bridge/runtimeProductFacade.ts` |
| Product audio / setup actions | `productAudioFacade.ts`, `productSetupFacade.ts` |
| Meeting product mapping | `productMeetingState.ts` |
| Product readiness mapping | `productReadinessState.ts` |
| Worker capability parsing | `workerCapabilities.ts` |
| Product DTOs | `runtimeProductTypes.ts` |
| Tauri bridge calls/types | `runtimeApi.ts`, `applicationRuntimeApi.ts`, `myVoiceApi.ts`, `myVoiceBuildApi.ts` |
| Tauri window permissions | `src-tauri/capabilities/default.json` + `translation-overlay.json` |
| Frontend application state sequencing | `applicationController.ts` |
| Meeting event/reconciliation state | `meetingLiveController.ts` + `meetingReconcileReader.ts` + `reliabilityMonitor.ts` |
| Background My Voice build event reconciliation | `applicationRuntimeApi.ts` + `myVoiceBuildApi.ts` + `engine/runtime_events.rs`; terminal notices in `applicationNotice.ts` |
| Native close flow | `closeController.ts` + `nativeCloseRuntime.ts` + `closePolicy.ts` |
| Translation overlay presentation | `translationOverlay*.ts` + `pages/TranslationOverlay.svelte` |
| First-setup navigation/resume | `SetupNavigation.svelte` + `setupFlow.ts` |
| Translation feedback | `translationFeedbackState.ts` |
| One-shot updater frontend | `appUpdateApi.ts` + `UpdateAction.svelte` |
| Native updater installation and cross-resource exclusion | `commands/app_update.rs` + `engine/runtime_state.rs` (one admission gate; Meeting/Voice keep their existing domain lifecycles) |

## Application runtime authority

Cross-feature coordination is owned by:

`EngineData/Frontend/RustApp/src-tauri/src/commands/application_runtime/`

| Responsibility | Owner |
|---|---|
| Typed public contracts | `contract.rs` |
| Cheap read-only snapshot | `snapshot.rs` |
| Typed subsystem projection | `summaries.rs` |
| Global lifecycle/capabilities/resources/problems | `lifecycle.rs`, `capabilities.rs`, `resources.rs`, `problems.rs` |
| Parameterless product intents | `intents.rs` |
| Typed parameterized mutations | `mutations.rs` |
| Runtime events | `events.rs` |
| Safe app shutdown coordination | `shutdown.rs` |

Canonical shared-resource owners:

- `translateit_application_meeting`
- `translateit_mic_test`
- `translateit_voice_recording`

Domain algorithms remain in their domain modules; ApplicationRuntime coordinates but does not absorb them.

## Meeting / audio

| Responsibility | Owner |
|---|---|
| Meeting public status facade | `commands/meeting_session.rs` |
| Start/Stop lifecycle transactions | `commands/meeting_session/lifecycle.rs` + `engine/runtime_state.rs` |
| Local translated route admission and retired preference compatibility | `commands/meeting_session.rs` owns strict Local admission; legacy Codex or unverified settings do not authorize Local AI |
| Meeting preflight/status | `meeting_session/preflight.rs` + `session_state.rs` |
| Outbound/incoming pipelines | `meeting_session/outbound_pipeline.rs`, `incoming_pipeline.rs`, `incoming_deferred.rs` |
| Meeting consumer ownership | `meeting_session/consumer_runtime.rs` |
| Committed turns / last-session review | `meeting_session/committed_turns.rs` |
| Persistent saved Meeting entries / manual deletion | `meeting_session/transcript_archive.rs` (storage adapter; committed_turns remains transcript authority) |
| Self-contained HTML transcript output / shell file actions | `meeting_session/transcript_export.rs` |
| Physical capture / VAD | `engine/audio/` |
| Canonical speech presence / callback-to-CPU transport | `engine/audio/silero.rs`, `finalized_utterance.rs` + `speech_ingress.rs`, `pcm_handoff.rs`; `vad.rs` energy diagnostics do not own production speech admission |
| Optional Meeting Sound capture | `engine/audio/meeting_sound_capture.rs` |
| Meeting output delivery | `engine/audio/meeting_output*.rs` |
| Virtual microphone route / session binding | `commands/virtual_mic_route.rs` (one lifecycle owner); pure approved-pair selection in `commands/virtual_mic_route/pair_selection.rs` |
| Candidate native Windows PCM client | `engine/audio/native_capture_transport.rs`, not admitted by normal Meeting |
| Candidate normal-user IPC and paced delivery | `engine/audio/native_capture_pipe.rs` (SCM-backed IPC) and `native_capture_pacing.rs` (existing Meeting worker helper); no new output owner |
| Candidate privileged Windows broker transport | `src-tauri/windows/virtual-audio/Broker/` (`broker_identity`, `broker_io`, `broker_driver`, `broker_service`); not registered as a Windows service in this source state |
| Candidate native capture-only kernel and privileged control device | `src-tauri/windows/virtual-audio/Source/Main/capture_pcm.{h,cpp}`, `capture_control_protocol.h`, `capture_control_device.{h,cpp}` |
| Device persistence, Local engine recovery and in-session locks | `commands/settings.rs` + `engine/settings.rs`; `components/settings/MeetingTranslationEngine.svelte` exposes explicit recovery only |

## Local AI / voice

| Responsibility | Owner |
|---|---|
| Native runtime lifecycle, task admission and readiness | `commands/helper_bridge.rs`, `helper_bridge_runtime.rs`, `helper_bridge_runtime/scheduler.rs`, `helper_bridge/functional_readiness.rs` |
| Native request transport and scoped dispatch | `commands/helper_bridge/transport.rs`, `helper_bridge/transport/native.rs`, `helper_bridge/request_policy.rs` |
| Canonical native ASR, MT and selected voice synthesis | `tools/translation_quality/native_cuda/` + `commands/helper_bridge/transport/native.rs` |
| Model inventory/staging | `model_manifest.json` + `prepare_model_assets*.py` |
| My Voice recording/review | Rust `commands/voice_lab_recording.rs` |
| My Voice candidate build/evaluation | `commands/voice_lab_build.rs` + `voice_lab_build/native_build.rs`, `native_evaluation.rs`, `native_speaker.rs`, `native_review.rs` |
| My Voice preview and native operation scope | `commands/voice_lab_preview/native.rs` + `my_voice_runtime.rs`; Meeting uses the same selected-voice native owner through the helper |
| Voice storage/package promotion | Rust `commands/voice_lab/storage.rs` |

## Release / packaging

| Responsibility | Owner |
|---|---|
| Release orchestration | `scripts/build_release.ps1` |
| Release input staging | `stage_release_inputs*.ps1` |
| Payload construction | `build_r3_external_payload.py` |
| Package/source validation | `validate_release_*.mjs`, `validate_tauri_package_preflight.mjs` |
| Installer behavior | `src-tauri/windows/` + Tauri release config |
| Updater signing/artifacts | `build_release.ps1` + `tauri.release.conf.json` |
| Third-party notices | `generate_third_party_notices.mjs` |

## Navigation questions

```text
Who owns this?        → source-ownership.md
What must it do?      → docs/foundation/
What is active?       → planning/development.md
What is proven?       → current-validation.md
Why was it chosen?    → decisions/
What happens now?     → current source + matching proof
```

Historical paths never become current owners merely because an old report references them.


### Bridge proof ownership

Command-name/response-shape parity is owned by `validate_bridge_contract.mjs`. Rust-command argument-key parity is owned by `validate_tauri_command_args.mjs`. Frontend bridge wrapper reachability is owned by `validate_runtime_api_usage.mjs`. These checks are complementary: a registered command can still be waste if its frontend wrapper has no consumer, and a command name can still match while its invocation arguments drift.


### Local worker protocol ownership

The single native task set and its deadlines are guarded by `worker_task_protocol_contract.test.ts`. Rust-projected native capability status ↔ TypeScript parsing is guarded by `worker_status_protocol_contract.test.ts`; request identity and native admission are guarded by `worker_request_metadata_contract.test.ts`. Rust owns transport and lifecycle; no production Python process or wire framing remains. Update the existing `tools/interop-contracts.json` binding when a contract owner or consumer changes.


### RuntimeSettings contract ownership

Rust persistence/sanitization remains owned by `src-tauri/src/engine/settings.rs`; frontend DTO/default projection remains owned by `src/app/shared/types.ts` and `src/app/shared/state.ts`. Rust↔TypeScript top-level field parity, schema version, and core defaults are guarded by `runtime_settings_contract.test.ts`.
