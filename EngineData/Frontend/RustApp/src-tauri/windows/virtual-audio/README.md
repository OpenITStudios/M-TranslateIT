# TranslateIT Virtual Audio — native source candidate

**Status: STATIC_SOURCE candidate, not yet built, signed, installed, or Windows-tested.** Do not bundle, auto-install or call this an operational virtual microphone until native acceptance succeeds. Never disable Windows security or test-signing protections on end-user systems to install it.

## Provenance and licence

Derived from the official Microsoft `Windows-driver-samples/audio/simpleaudiosample` WaveRT code, pinned at `f4c1ff842b6a0001152f44cb61788b216a1c7627`. Original source is distributed under the **Microsoft Public License (MS-PL)**, reproduced unchanged in `LICENSE.MS-PL`. The upstream documentation is `UPSTREAM_README.md`; changes to this derivative are authored by OpenITStudios. A derivative source is not a Microsoft-certified/signed driver.

## Two Windows endpoints

| Endpoint | Type | Intended client |
| --- | --- | --- |
| `TranslateIT Microphone Feed` | Windows playback/render device | Existing Rust CPAL output (Original or English translation) |
| `TranslateIT Meeting Microphone` | Windows microphone/capture device | Zoom / Teams / Discord / meeting application |

Windows kernel source formats: **48,000 Hz, signed PCM16, 2 channels** on both endpoints. The paired WaveRT audio streams share a **bounded 200 ms, nonpaged, frame-aligned** PCM ring. Capture underflow returns silence, overflow discards oldest samples. The bridge has no user-mode IOCTL, cloud engine, independent AI service, or raw-audio file writer.

The existing Rust Meeting route detects both native endpoint names, prevents ambiguous or partially installed device fallback, and retains one generation-bound selected route. It does not infer actual microphone reception merely from enumerated device names.

## WDK solution / released-provider boundary

`TranslateITVA.sln` and the original WDK project files are source only. The intended production package is a properly Microsoft-trusted signed `TranslateITVA.sys`, generated `TranslateITVA.inf`, and `TranslateITVA.cat`. Generated, unsigned or test-signed outputs are not bundled in this Git commit.

Official Windows SDK/WDK + Visual Studio are required for a pinned native build. Exact Windows 10/11 support, driver verifier/static code analysis, HLK/required Microsoft signing route, INF/CAT installation, reboot/recovery/uninstall, real endpoint display names, both stream formats, long-session clock drift and lip/speech timing, Mute/Original/TranslateIT switching, speaker isolation, and actual Meeting receiver output **remain UNKNOWN / NOT EXECUTED**. Record proof only in the existing coordinated Windows acceptance owner.

The previously approved standard VB-CABLE provider remains the only released installer payload; source adoption does not authorize mixing two drivers into the shipped setup or rebranding VB-Audio's devices.
