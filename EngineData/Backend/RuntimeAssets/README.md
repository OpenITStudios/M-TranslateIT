# RuntimeAssets

`RuntimeAssets` is TranslateIT's controlled local release/runtime asset boundary. Application orchestration remains in Tauri/Rust and LocalWorker owners; this tree contains models, voice references/runtime assets, audio-provider payload and generated notices only.

## Current layout

```text
RuntimeAssets/
├─ ASR/ModelData/
│  ├─ faster-whisper-large-v3-turbo/       # required release ASR
│  ├─ faster-whisper-medium/                # optional validated fallback
│  └─ qwen3-asr-1.7b/                       # pinned GGUF pair, migration input
├─ Translation/ModelData/
│  ├─ xiaomi-research--MiLMMT-46-1B-v1.0/ # current ID ↔ EN translator
│  └─ hy-mt2-1.8b/                          # pinned GGUF, migration input
├─ NativeCuda/
│  ├─ bin/                                 # exact reviewed private DLL closure
│  └─ THIRD_PARTY_NOTICES.txt               # reviewed license/build provenance
├─ Voice/
│  ├─ BuiltInVoices/
│  │  ├─ MaleVoice/reference.wav
│  │  ├─ FemaleVoice/reference.wav
│  │  └─ SOURCES.json                      # exact source/license/hash authority
│  └─ GPTSoVITS/Source/                    # pinned V2ProPlus runtime/build assets
├─ AudioProvider/VBCABLE/
│  ├─ NOTICE.txt
│  └─ Package/                              # reviewed vendor package; release-side only
└─ ThirdPartyNotices/THIRD_PARTY_NOTICES.txt
```

The machine-readable model inventory is `EngineData/Backend/LocalWorker/WorkerRuntime/model_manifest.json`. Required Hugging Face snapshots use immutable revisions and `.translateit_model_revision` markers.

Current production translation identity (native-only cutover pending):

```text
xiaomi-research/MiLMMT-46-1B-v1.0
revision 4fc480b6c58dec29c159dcdf9fde0f6d5c354995
license metadata: Gemma
```

## Native migration inputs

The existing release staging entrypoint accepts reviewed native artifacts under
`TRANSLATEIT_NATIVE_ARTIFACT_ROOT` (bin and notice bundle) and exact build-supplied
`TRANSLATEIT_NATIVE_DLL_PINS`. The same pin string is bound to Rust and the
offline Setup payload contract; app-only update verifies the existing DLL
closure against Setup. Qwen3-ASR and Hy-MT2 exact publisher revisions and GGUF
hashes are in the model manifest. Staging them does not activate the models.
MiLMMT and Faster Whisper remain staged until coordinated source cutover;
Faster Whisper evaluation and Silero assets have independent voice/audio
consumers and must not be removed as a side effect.

## Built-in Meeting voices

`Voice/BuiltInVoices/SOURCES.json` owns exact reference provenance for the two day-one Meeting voices. Both references come from LibriSpeech dev-clean / OpenSLR SLR12 and are recorded as **CC-BY-4.0**, with speaker ID, utterance, source URL and WAV SHA-256. They are not public-domain assets.

The Rust voice owner turns a selected built-in reference plus reviewed shared GPT-SoVITS pretrained weights into the same selected-Meeting-voice runtime contract used by the worker. My Voice remains the optional trained upgrade.

The canonical third-party notice generator reads `SOURCES.json` and includes this attribution in the staged release notice bundle.

## R3 packaging boundary

Large runtime bytes are ignored by Git and staged as controlled release inputs. They are packaged into `TranslateIT-Payload.7z`; they are **not** embedded in the Tauri resource map. `tauri.release.conf.json` carries only the small WorkerRuntime/control/notices closure used by `TranslateIT-Setup.exe`.

Release staging/validation is owned by `EngineData/Frontend/RustApp/scripts/`. Payload construction binds the external payload to app/schema identity and exact SHA-256. Installer source owns transactional runtime replacement, VB-CABLE vendor invocation/restart signaling and uninstall cleanup of TranslateIT-owned external runtime. User data and the system VB-CABLE driver are preserved by uninstall policy.

Presence in this tree is not runtime acceptance. It does not prove CUDA/model execution, translation quality, voice quality, Windows driver behavior, virtual-audio delivery or clean-machine installation.

## Provenance / notice boundary

| Input | Pinned source | Declared boundary |
|---|---|---|
| Faster Whisper large-v3-turbo | `dropbox-dash/faster-whisper-large-v3-turbo` @ `0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf` | MIT |
| MiLMMT-46-1B-v1.0 | `xiaomi-research/MiLMMT-46-1B-v1.0` @ `4fc480b6c58dec29c159dcdf9fde0f6d5c354995` | Gemma metadata/terms |
| GPT-SoVITS source | `RVC-Boss/GPT-SoVITS` @ `d523079fc05d9a8028d6085bffe4a2757c32abb6` | MIT source; nested assets retain separate obligations |
| Built-in Male/Female references | LibriSpeech dev-clean / OpenSLR SLR12; exact utterance/hash in `SOURCES.json` | CC-BY-4.0 |
| Standard VB-CABLE | reviewed official VB-Audio package | donationware / conditional distribution; preserve `NOTICE.txt` |

Exact Python/material hashes and exceptional license sources remain machine-owned by release staging/validation and generated notices rather than duplicated across governance docs.

## Rules

- Keep one canonical translator; do not restore M2M100/Marian as production fallback/router.
- Keep large model, GPT-SoVITS, FFmpeg, NLTK, VB-CABLE package and other runtime bytes out of Git.
- Built-in reference files intentionally tracked as small controlled assets must retain exact provenance/hash/license metadata.
- Do not put application orchestration under RuntimeAssets.
- Do not re-embed the large R3 payload into Tauri resources or add first-use core-model downloads.
- Preserve package-specific notices/provenance; presence is not legal clearance or runtime proof.
