# TranslateIT Windows Developer Tooling

`manage.cmd` is the **optional developer convenience** launcher, **not a step ChatGPT assigns** to a user's PC during routine source development.

```text
manage.cmd
  -> tooling/windows/manage.ps1
     -> tooling/windows/modules/Workspace.psm1
     -> tooling/windows/modules/Toolchain.psm1
     -> tooling/windows/modules/PortableToolchain.psm1
     -> tooling/windows/modules/Readiness.psm1
     -> existing npm, Cargo, uv and signed release owners
```

The only authoritative development requirements are
`tooling/windows/config/requirements.json` and the existing Node, Cargo
and Python lockfiles. These are review/pin inputs, not download destinations.

| Command | Current meaning |
|---|---|
| `manage.cmd status` | Read-only tool inventory with workspace/external origins and asset-marker presence |
| `manage.cmd prepare` | Install pinned Node/npm, MinGit, uv, managed Python and Rustup/Rust in `.workspace/tools`; then install locked developer dependencies |
| `manage.cmd verify` | Execute source contracts and save ignored `.workspace/reports/readiness.json`; native device/model proof remains unknown |
| `manage.cmd build` | Existing exact-SHA frontend and locked Tauri Rust compile gate |
| `manage.cmd run` | Existing Tauri development app, needing real local models/audio |
| `manage.cmd test` | Existing frontend, Rust and worker regression suites |
| `manage.cmd package "<quality-report.json>"` | Controlled signed R3 release, separately gated; not a development setup shortcut |

The `Toolchain.psm1` module selects complete existing workspace bundles (Node/npm, Git, Python, uv and Rust) before any modifying developer command; incomplete bundles fail closed and external fallback is reported explicitly. This module prefers the local bundles. The separate `PortableToolchain.psm1` provisions pinned Node, MinGit, uv-managed Python and Rust toolchains only on explicit `prepare`, verifies downloaded hashes and refuses incomplete installations; it never installs Windows drivers or MSVC automatically. All missing tool archives are hash-verified before the first live tool directory is published; a download-only failure frees the reservation for safe retry, while a failure after installation starts preserves its recovery lock. The single `Readiness.psm1` module owns the read-only toolchain/asset inventory. Its report is local diagnostic evidence only, never an automatically promoted release-readiness receipt or native acceptance proof.

The reviewed toolchain is Node 24 LTS, npm 12.2.1, Rust 1.99.1, rustup 1.29.2,
uv 0.13.5, MinGit 2.56.0.windows.3 and Python 3.12.10. Exact archive identities
remain in `requirements.json`. npm is installed from its SHA-256-verified
archive into the portable Node prefix, with lifecycle scripts disabled and no
global Windows installation. Existing mismatched tool groups still require
explicit developer maintenance; `status` never repairs them. Python compatibility
ceilings are explained in the [worker dependency owner](../EngineData/Backend/LocalWorker/WorkerRuntime/README.md#dependencies).

Process-scoped cache settings do not reinstall Rustup, Windows SDK, WebView2
or VB-CABLE, and do not make installed app data portable. The unchanged R3
staging flow still needs a separate safe environment/retry migration before
`prepare` can include its downloads. Do not delete or reset any
`UserData/SavedProject` content during build, verification or preparation.

[Workspace ownership](../docs/system/windows-workspace-boundaries.md) ·
[Windows acceptance](../docs/knowledge/operations/target-windows-performance.md) ·
[Repository tools](../tools/README.md).
