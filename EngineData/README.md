# EngineData

`EngineData` is the current implementation boundary for TranslateIT.

## Owns

```text
Frontend/RustApp
→ Tauri 2 desktop application
→ Svelte 5 + Vite + TypeScript product UI
→ Rust src-tauri runtime/commands/engine
→ source/build validation scripts that directly protect the product

Backend/LocalWorker
→ pinned model inventory and developer-only acquisition
→ retained historical dependency lock; no production Python worker

Backend/RuntimeAssets
→ controlled production ASR/translation/voice/audio release inputs
```

## Does not own

Do not place these here as persistent source:

```text
current task/progress reports
historical recovery documents
design/reference archives
local validation output
generated logs/cache
user-created or user-saved runtime data
scratch files
```

Generated development/test evidence belongs under ignored temporary/build-output paths. Runtime/user data belongs under `UserData`.

Historical development reports are recovered from Git history when genuinely needed; do not recreate `DevelopingData` or another parallel current-source hierarchy. If old material contains a still-valid requirement/implementation input, revalidate the bounded content and adopt it into the appropriate current owner.
