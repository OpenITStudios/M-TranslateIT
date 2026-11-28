# App Bridge

The active frontend-to-runtime bridge is intentionally small:

```text
runtimeApi.ts
-> thin Tauri command calls used by current product/setup callers

runtimeProductFacade.ts
-> canonical application snapshot projection and high-level product actions

applicationRuntimeApi.ts
-> canonical native snapshot, product intents and events

myVoiceApi.ts
-> My Voice guided recording commands

myVoiceBuildApi.ts
-> My Voice build/evaluation commands
```

Do not recreate retired command-family subfolders, Audio Studio bridges, realtime scoring/reducer layers, or duplicate runtime APIs unless a current product requirement proves a distinct responsibility is needed.

Legacy `voice_lab_*` strings may appear only inside the My Voice bridge where they address retained Tauri compatibility commands. New frontend APIs and product terminology use My Voice naming.
