# RustApp Scripts

This directory contains TranslateIT's source validators and Windows release entrypoints. Acceptance is scenario-based per `docs/foundation/03-acceptance-scenarios.md`; the former all-in-one target-PC harness was retired (decision D-031).

## Source checks

```text
validate:source-contracts
├─ bridge contract
│  ├─ Tauri command registry ↔ frontend invocation parity
│  ├─ selected Rust response struct ↔ TypeScript field parity
│  └─ no explicit `any` escape at bridge boundaries
├─ frontend source reachability from src/main.ts
├─ source-size ownership budget
├─ Meeting virtual-route contract
├─ Rust/Tauri manifest preflight
└─ R3 Tauri/package source checks

validate:quick
└─ source checks + TypeScript/Svelte typecheck + frontend runtime-policy tests
```

The bridge contract is intentionally one owner. Do not add a second validator that separately re-checks command parity or bridge type escape rules. Frontend reachability catches orphan `.ts`/`.svelte` modules that normal unused-local checks cannot see.

Functional behavior coverage remains with the owning native runtime and its Rust/session/audio fixtures. Python acquisition fixtures cover developer tooling, not native inference. Static source checks do not claim target-Windows runtime behavior.

`check:tauri-rust-local` is explicit local compile verification and is not run merely to validate documentation/source routing.

## R3 release boundary — HELD

The desired offline release remains `TranslateIT-Setup.exe` plus `TranslateIT-Payload.7z`. The native R3 v3 builder, installer and hook source now use an exact runtime file contract instead of Python dependency metadata. Staging, legacy optimizer, notice generation and reviewed artifact inputs remain incomplete. All release entrypoints are blocked; do not bypass the holds.

Native release requires a coherent package and installer/update contract, reviewed DLL/model/ONNX identities, the independent speaker verifier export pin, complete third-party notices, and integrated target acceptance. The existing model manifest is the inventory owner. No package build or download is implied by source checks.

Developer model acquisition remains separate: `WorkerRuntime/prepare_model_assets.py` can reuse exact verified snapshots and reports the pending independent offline export separately. It does not authorize release.

## Acceptance

Acceptance runs one scenario at a time per `docs/foundation/03-acceptance-scenarios.md`. Installer-bound checks are deferred Group E scenarios that call `build_release.ps1` directly; its build evidence under ignored `src-tauri/target/` records the source commit plus both SHA-256 values. Headless runtime evidence lands under `UserData/LogData/RustAppValidation/`.

## CI ownership — paused

The project profile currently prohibits CI/test dispatch and hosted compute. The following describes the historical workflow layout, not active execution permission.

`.github/workflows/release-payload-verify.yml` is the single R3 release workflow owner:

- relevant pull requests to `Local` run the source-contract job;
- relevant pushes to `Local` run the source-contract job and the controlled Windows payload-proof job;
- the source-contract job parse-checks the Windows release/staging PowerShell helpers;
- canonical MiLMMT validation also preserves the release staging model-acquisition contract;
- there is no manual-dispatch path for the current branch model.

Worker dependency-lock consistency is owned separately by the read-only WorkerRuntime lock workflow. Do not create overlapping release or proof workflows.

## Proof boundary

Source/hosted verification can establish declarations, controlled staging, payload structure, build evidence, and acceptance-tool syntax. It does not prove actual Windows Setup execution, driver consent/restart, installed model execution, GPU/audio behavior, Meeting delivery, or clean-machine readiness.

## Rules

- Do not re-embed the large R3 payload in `tauri.release.conf.json`.
- Do not add network bootstrap/model downloads to installed Setup.
- Do not add a user-facing 7-Zip dependency, manual extraction flow, or second installer.
- Do not create another release pipeline merely to gather duplicate evidence.
- Keep generated hook/payload/build/acceptance evidence under ignored output paths.
