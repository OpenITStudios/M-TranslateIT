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
| Rust native_capture_transport.rs | Privileged diagnostic DeviceIoControl adapter | Not used by normal Meeting |
| TranslateITMicBroker.exe | Local SYSTEM service mediating restricted kernel writes | Source only; not installed |
| Rust native_capture_pipe.rs | Non-admin client with SCM server-PID verification | Source only; no Meeting caller |
| Rust native_capture_pacing.rs | 20 ms paced PCM helper for the existing Meeting worker | Source only; no Meeting caller |

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

A normal non-elevated TranslateIT application **cannot** open the kernel
control device. A restricted Windows service broker source is now available
to mediate this access, but it is not signed, installed, provisioned,
or admitted into the normal Meeting path. It must never weaken the driver
DACL or use a shared static password as a substitute for client identity.

## Candidate non-admin broker (not an installed Windows service)

The C++ `Broker/TranslateITMicBroker.vcxproj` is part of the native
`TranslateITMic.sln` source solution. It is a Windows service intended to
run as SYSTEM under a separately hardened machine installation. It only
forwards authenticated, bounded PCM to the privileged kernel control device.
It does not translate, mix, record or own Meeting sessions.

- Pipe: `\\.\pipe\TranslateITMicBrokerV1`, local clients only.
- Pipe SDDL: `D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;0x0012008B;;;AU)`.
  Authenticated users get read/write-data only, not the pipe-create right.
- The broker impersonates the connected pipe client and checks user SID,
  logon authentication ID, Windows session and process identity.
- It requires the actual client executable path inside a machine-protected
  Program Files installation and a SHA-256 match against
  `HKLM\SOFTWARE\OpenITStudios\TranslateIT\ApprovedDesktopSha256`
  (32-byte REG_BINARY). Missing hash or mismatched executable denies access.
- The ordinary Rust client checks the running Windows SCM service PID
  against the actual pipe server PID before transmitting PCM.
- Server I/O is cancelable during Stop; the initial handshake and packet
  bodies have finite deadlines. Rust IPC uses overlapped operations and
  per-transaction deadlines. The broker limits PCM burst rate while the
  existing Meeting worker can use its 20-ms pacing helper.
- Disconnecting a pipe releases the broker's kernel FILE_OBJECT,
  triggering scoped writer cleanup. Neither native Mute nor Stop may
  claim success without the relevant FLUSH acknowledgement.

No installer currently provisions the approved executable hash, registers
the Windows service or installs this broker executable. Consequently
the new IPC path remains a source candidate, not a usable microphone.
The trusted package installer must first verify its signed payload and
enforce a non-user-writable installation directory. Hash checking does
not protect against malicious code injection into an authorized process;
service privileges, SCM access, signer identity, update invalidation,
reparse/hardlink threats, session isolation and device lifecycle remain
native-security acceptance requirements.
## Required higher-context proof

All remain **UNKNOWN / NOT EXECUTED**: WDK x64/ARM64 compiler/link, WDK
code analysis, INF/CAT syntax and Microsoft-required driver signing,
broker SCM registration, installed trust hash and non-admin access proof, actual PnP root-device provisioning,
Windows 10/11 endpoint name, WASAPI capture/driver IOCTL roundtrip,
simultaneous capture clients, device lifecycle, drift/latency, Mute/Stop
tail, physical mic isolation, crash/restart/rollback/uninstall, and real
Zoom/Teams receiver reception. These checks belong to the existing single
integrated TARGET_WINDOWS acceptance runbook, not to GitHub CI or per-commit
manual user testing.

The native candidate is never deployed in the existing
`TranslateIT-Setup.exe` + `TranslateIT-Payload.7z` flow until separately
approved and genuinely signed/tested. VB-Audio's device/brand is not renamed.
