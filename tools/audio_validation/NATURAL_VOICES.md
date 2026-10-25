# Natural Bahasa Indonesia voice comparison

## Current selected method: local neural listening preview

The owner selected separate **local neural fixture generation** on 2026-10-26. `local_neural_comparison.py` uses public Supertonic 3 presets M1/F1 for the same five public scripts A01/A03/A04/A06/A08. This produces ten synthetic utterances for listening review, not a second TranslateIT runtime provider. It does not start the application, record a microphone, clone a personal voice or change WorkerRuntime dependencies.

Model identity and SHA-256/size pins are owned by `local_neural_assets.json`: `supertone-oss-archive/supertonic-3` revision `aafc6e32416a594460b32413efc49d7fe4ce6d46`. Upstream code is archived and unsupported; its SDK code is MIT and model weights are OpenRAIL-M. The public model is used only in developer tooling. The model license accompanies the generated preview. No model or WAV is committed or added to the installer.

Create a separate Python 3.12 environment with the existing workspace uv (never run these installs against WorkerRuntime):

```powershell
.workspace/tools/uv/uv.exe venv --python .workspace/tools/python/cpython-3.12.10-windows-x86_64-none/python.exe .workspace/tools/neural-fixture-venv
.workspace/tools/uv/uv.exe pip install --python .workspace/tools/neural-fixture-venv/Scripts/python.exe -r tools/audio_validation/requirements-neural-fixtures.txt
.workspace/tools/neural-fixture-venv/Scripts/python.exe tools/audio_validation/local_neural_comparison.py plan
.workspace/tools/neural-fixture-venv/Scripts/python.exe tools/audio_validation/local_neural_comparison.py prepare --model-dir .workspace/tools/neural-fixture-model
.workspace/tools/neural-fixture-venv/Scripts/python.exe tools/audio_validation/local_neural_comparison.py generate --model-dir .workspace/tools/neural-fixture-model --destination .workspace/reports/neural-preview-001
```

Only the explicit `prepare` action downloads the pinned public assets. Generation verifies their size/hash before loading them, forces offline Hub settings and disables the SDK's automatic download. Existing destinations are refused. Direct package versions, generator/pin-file hashes, model revision, voice, seed, settings, audio hashes and public text are recorded in `comparison.json`. Seed 42 limits stochastic variation; cross-platform bitwise synthesis identity is not promised, so compare actual WAV hashes.

`listen.html` contains offline audio players. Listen to the preserved **44.1-kHz PCM16 originals** for naturalness; the separate **16-kHz PCM16** copies use anti-aliased polyphase resampling for eventual ASR input. There is no pitch shifting, automatic loudness normalization or eSpeak fallback. Silence, nonfinite samples and clipping fail generation. These numeric checks do not establish naturalness or faithful pronunciation.

Listening acceptance remains **PENDING** until the owner evaluates both voices: natural rhythm and pronunciation, intact “tidak”/“bukan”, “lima belas” versus “lima puluh”, clear English words, no missing/repeated speech, and usable pauses. An unacceptable voice is rejected rather than used to grade the application. The preview has its own `translateit.local_neural_comparison.v1` identity; it is not yet an approved 48-case `audio_fixtures.v1` suite and must not be fed to that runner as one. After listening acceptance, carry the approved source into the existing fixture generator and connected VAD→ASR acceptance work; do not mark those stages passed from this preview.

## Optional Azure comparison (not selected or executed)

The source of truth remains the **48-case canonical audio scenario suite**. The first neural voice comparison is deliberately limited to five public sentences **A01, A03, A04, A06 and A08**, spoken by two independent Indonesian voices: Microsoft Azure `id-ID-ArdiNeural` (male) and `id-ID-GadisNeural` (female). Ten audio files are generated only after explicit invocation. These voices are listed by Microsoft as supported Indonesian neural voices.

```powershell
python tools/audio_validation/natural_voice_comparison.py plan
```

The `plan` action needs no credentials, network or billing. To run the **optional billed, cloud-based syntheses** in a separately authorized local environment with a valid Azure Speech resource:

```powershell
# Set AZURE_SPEECH_KEY and AZURE_SPEECH_REGION securely in the environment.
python tools/audio_validation/natural_voice_comparison.py generate --destination UserData/CacheData/audio_validation/azure-voice-compare-v1
```

The generator never logs, commits or embeds the Speech key. It uses fixed Azure REST endpoints, constrained voice choices, bounded request count (10), SSML escaping, request timeout, SHA-256 manifests and WAV integrity checks. The provider produces 24 kHz mono PCM16 WAV; comparison WAVs are deterministically resampled to **16 kHz mono PCM16** for the existing replay input. The original high-resolution WAV hash is retained as provenance but the native sample file is not retained. Both voice identities and textual references remain visible in `comparison.json`.

**This is not automatic CI and does not alter TranslateIT's runtime architecture.** Cloud is for optional *fixture creation only*, never live ASR, translation, TTS, microphone or Meeting. Do not provide API keys in this chat. Check your organization's service/billing and audio-data reuse terms before invoking. A generated waveform is not proof of natural intelligibility, ASR quality or native delivery.

The comparison has a distinct schema `translateit.natural_voice_comparison.v1`. It must not be passed to the 48-case `translateit.audio_fixtures.v1` runner as if it were an identical canonical suite. After subjective quality review, an approved voice-profile and variant manifest can be linked to a second, explicitly versioned fixture suite without changing historical eSpeak hashes.
