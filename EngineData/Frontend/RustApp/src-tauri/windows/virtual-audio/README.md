# TranslateIT Virtual Audio — unaccepted paired-source prototype

**Status: historical design/reference only — NOT ADMITTED to Meeting routing or packaging.** This paired-render/capture source tree has never been built, signed, installed or tested as a native TranslateIT driver. The current product selects only the approved standard VB-CABLE provider. Do not bundle, auto-install or bypass Windows driver-security protections to run this prototype.

The accepted **next native target** is a *capture-only* Windows microphone endpoint backed by a single authorized Rust Meeting producer. It must not expose the separate `TranslateIT Microphone Feed` render endpoint. The capture-only implementation requires an explicit, access-controlled kernel/user-mode PCM transfer and generation-bound lifecycle, and is **not implemented in this prototype**. The paired sample remains only as reviewable source/provenance and may be retired when the replacement is complete.

## Provenance and licence

Derived from the official Microsoft `Windows-driver-samples/audio/simpleaudiosample` WaveRT code, pinned at `f4c1ff842b6a0001152f44cb61788b216a1c7627`. Original source is distributed under the **Microsoft Public License (MS-PL)**, reproduced unchanged in `LICENSE.MS-PL`. The upstream documentation is `UPSTREAM_README.md`; changes to this derivative are authored by OpenITStudios. A derivative source is not a Microsoft-certified/signed driver.

## Two prototype Windows endpoints (not supported)

| Endpoint | Type | Intended client |
| --- | --- | --- |
| `TranslateIT Microphone Feed` | Windows playback/render device | Existing Rust CPAL output (Original or English translation) |
| `TranslateIT Meeting Microphone` | Windows microphone/capture device | Zoom / Teams / Discord / meeting application |

Windows kernel source formats: **48,000 Hz, signed PCM16, 2 channels** on both endpoints. The paired WaveRT audio streams share a **bounded 200 ms, nonpaged, frame-aligned** PCM ring. Capture underflow returns silence, overflow discards oldest samples. The bridge has no user-mode IOCTL, cloud engine, independent AI service, or raw-audio file writer.

Rust does **not** admit these prototype endpoint names as a Meeting route. It may list their names for diagnostics, but the current supported legacy provider has exclusive route selection. A native audio endpoint cannot be accepted from a display-name match or a successful local callback alone.

## WDK solution / released-provider boundary

`TranslateITVA.sln` and the original WDK project files are source only. The intended production package is a properly Microsoft-trusted signed `TranslateITVA.sys`, generated `TranslateITVA.inf`, and `TranslateITVA.cat`. Generated, unsigned or test-signed outputs are not bundled in this Git commit.

Official Windows SDK/WDK + Visual Studio are required for a pinned native build. Exact Windows 10/11 support, driver verifier/static code analysis, HLK/required Microsoft signing route, INF/CAT installation, reboot/recovery/uninstall, real endpoint display names, both stream formats, long-session clock drift and lip/speech timing, Mute/Original/TranslateIT switching, speaker isolation, and actual Meeting receiver output **remain UNKNOWN / NOT EXECUTED**. Record proof only in the existing coordinated Windows acceptance owner.

The already approved standard VB-CABLE provider remains the only released installer payload and the only selected Meeting route. Source adoption does not authorize mixing drivers in setup or rebranding VB-Audio devices.

## Capture-only target contract (design; not executable proof)

- Exactly one Windows capture/recording endpoint, with stable Windows device identity independently of its friendly name; no native render/feed endpoint is registered.
- One Rust Meeting output owner writes bounded, checked PCM16 mono 48 kHz blocks over an authenticated, single-producer local channel. The native driver neither translates nor captures the physical microphone.
- A writer session is bound to the live Meeting generation and source epoch. Invalid, stale, duplicate or unowned writes are denied; Stop/Mute/Source Switch revokes that stream and flushes all queued native frames before success is acknowledged. Already delivered frames in another application's buffer cannot be retroactively recalled.
- Multiple legitimate Windows capture clients (e.g. two call apps) must consume the same audio independently; never let one client drain another client's samples.
- Absent producer or buffer underrun means silence. Overrun is bounded and explicitly observable, not a growing latency queue. No raw audio files, global publicly writable device access, cloud fallback, device name-based trust, or default speaker hijack.
- Windows PnP instance provisioning, signing, rollback/removal, clean-install and device-loss handling require separate verified implementation. This source tree is not that implementation.

