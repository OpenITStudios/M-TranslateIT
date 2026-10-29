# TranslateIT Capture-Only Windows Microphone — Source Candidate

**State: STATIC_SOURCE only.** This repository contains an uncompiled,
unsigned, uninstalled native driver candidate. It does **not** change the
current TranslateIT Meeting route: the standard licensed VB-CABLE pair remains
the only selected and packaged provider. Do not auto-install this source or
ask end users to disable Secure Boot / Windows driver protections.

## Provenance

Adapted from Microsoft's Windows driver sample
`Windows-driver-samples/audio/simpleaudiosample` pinned at commit
`f4c1ff842b6a0001152f44cb61788b216a1c7627`. Original and derivative
source is governed by `LICENSE.MS-PL`; the unedited original sample README
is `UPSTREAM_README.md`. Microsoft does not sign or endorse this derivative.

The previous *paired render/capture* prototype is preserved in Git history
(commit `6085c839925a83ae35426749e558252033c8391e`), **not** exposed as
another active driver. The current candidate has a different PnP identity and
service/package identity to avoid accidental upgrade collisions.

## Endpoint and authority

| Object | Source | Audience |
| --- | --- | --- |
| TranslateIT Meeting Microphone | WaveRT capture-only mic | Meeting applications via normal Windows capture |
| TranslateITMicControl | Separate secured device object | Currently SYSTEM / elevated Administrators only |
| Rust native_capture_transport.rs | Windows DeviceIoControl client | Unaccepted candidate; not used by production Meeting |

No virtual speaker, no virtual render/feed endpoint, no second translation
worker and no file-backed meeting audio. Normal audio applications can
open the Windows microphone; they must not gain a PCM writer handle from the
microphone endpoint.

Driver build entry: `TranslateITMic.sln`. The WDK project outputs
`TranslateITMic.sys`; `Source/Main/TranslateITMic.inx` generates the
matching INF and package catalog `TranslateITMic.cat`. Hardware ID:
`ROOT\TranslateITCaptureMic`. The exact endpoint friendly name is declared
using the custom capture topology GUID and INF MediaCategories registration;
the actual Windows display name must still be observed on a target host.

## PCM contract

- One privileged writer handles BEGIN / WRITE / FLUSH / END / STATUS.
- Version 1, **48,000 Hz, signed PCM16, mono** (2 bytes per frame).
- Each WRITE contains at most **960 frames / 20 ms**; no pointer, size,
  generation, epoch or sequence from user mode is trusted without validation.
- Ring capacity is **9,600 frames / 200 ms**; it is a maximum bound, not
  a guaranteed 200 ms latency. Underflow yields silence. Oldest history is
  overwritten on overflow, without allocating in audio callbacks.
- Up to eight capture streams use separate logical cursors into the same
  timestamp-ordered sample history. Closing one reader does not drain or
  reset audio for another.
- Wrong writer file, generation, source epoch or packet sequence is rejected;
  FLUSH revokes the current admission and makes queued frames unreadable.
  Re-BEGIN on the same control handle requires an advanced epoch. Closing the
  writer or stopping/removing PnP ownership revokes it independently.
  A control handle is bound to the PnP device epoch observed at CREATE;
  after device STOP/START, an already-open handle cannot reopen the writer.
- The driver cannot retract buffers already handed to Windows or other
  applications. The native acceptance phase must measure the remaining tail
  after Mute/Stop and ensure no subsequent old frames re-enter.

The input layer may contain other formats, but conversion/resampling to the
exact PCM16 mono contract must occur *before* this backend. The Rust client
does **not** schedule or resample audio by itself and is intentionally not
connected to the production playback path until its native acceptance gate.

## Privilege and product blocker

The control device uses `IoCreateDeviceSecure` with a **custom** class GUID
and DACL `D:P(A;;GA;;;SY)(A;;GA;;;BA)`. There is no write grant to
World, Authenticated Users, Interactive Users or a public pipe. Control IOCTLs
also require FILE_WRITE_DATA and METHOD_BUFFERED.

A normal non-elevated TranslateIT application **cannot** open this control
device. This is deliberate fail-closed behavior in the source candidate, **not
a completed end-user solution**. The next product security boundary is a
reviewed least-privilege broker/authorization transport under the existing
single Rust Meeting output owner. Do not lower the DACL or embed shared static
secrets in the app to simulate authorization.

## Required higher-context proof

All remain **UNKNOWN / NOT EXECUTED**: WDK x64/ARM64 compiler/link, WDK
code analysis, INF/CAT syntax and Microsoft-required driver signing,
non-admin authorization broker, actual PnP root-device provisioning,
Windows 10/11 endpoint name, WASAPI capture/driver IOCTL roundtrip,
simultaneous capture clients, device lifecycle, drift/latency, Mute/Stop
tail, physical mic isolation, crash/restart/rollback/uninstall, and real
Zoom/Teams receiver reception. These checks belong to the existing single
integrated TARGET_WINDOWS acceptance runbook, not to GitHub CI or per-commit
manual user testing.

The native candidate is never deployed in the existing
`TranslateIT-Setup.exe` + `TranslateIT-Payload.7z` flow until separately
approved and genuinely signed/tested. VB-Audio's device/brand is not renamed.
