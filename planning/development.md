# TranslateIT — Meeting translation focus

## Current Status

**PCM drop diagnosis prepared (2026-10-26).** With explicit owner approval, the
existing Diagnostics export now separates ingress rejection from handoff lock
contention, full buffer, oversized chunks and poisoned lock. Only numeric
observations were added; capture/discontinuity/Stop, queue capacity, VAD and engine
behavior remain unchanged. Rust 339 passed/one ignored and six Diagnostics
privacy/source contracts passed on the delivery candidate. The historical two
microphone drops still have no proved cause; the new breakdown has not yet been
observed in native execution. Current Validation owns interpretation and limits.

**Capture-health classification repaired at source (2026-10-26).** Native callback
errors, PCM drop counts and speech-pipeline blockers are now separate in the
existing capture reports. Health keeps actual native-device failures actionable;
internal PCM loss is degraded capture, not a lost microphone. The existing notice
monitor surfaces degraded capture, incoming preserves pipeline-failure evidence,
and explicit Diagnostics export includes per-lane numeric counts without raw
callback text/device identity. Two production status-projection tests reproduced
the old conflation before repair. Rust tests passed 338/one ignored, frontend 408,
with source contracts and typechecking passing. Clean repair commit
`ba84abf18e0a155e0f7708a8d4ce4d1ceae7e05b` then passed those checks and the
locked compile gate. Native Diagnostics separated microphone 0 native errors /
2 PCM drops from incoming 1 native error / 0 PCM drops; the real incoming failure
remained visible. Normal UI Stop, finalized archive and normal app exit succeeded;
no matching canonical worker remained. Two phrases appeared without controlled
test speech; their origin is unresolved and not quality evidence. No PCM transport,
VAD threshold, engine/model/prompt, incoming toggle or retry behavior changed.

**No-speech Start/Stop exercised (2026-10-26).** Native candidate
`535a3daab89f61985abb4b9bfdc27ec83de86362` launched with Built-in Male and
reached Live, but the required-microphone callback warning reproduced with no
recognized phrases. Normal UI Stop returned to Ready; the empty archive finalized,
normal app exit succeeded and no matching canonical worker remained. Capture is
not accepted; the callback cause remains UNKNOWN. The current application has a
Meeting Sound device selector but no Off control, so incoming remained active and
this is not an outbound-only baseline. No control/configuration/engine was added
or changed. Current Validation owns this bounded native evidence; the runbook now
removes the unsupported instruction to switch incoming off.

**Source gate passed; first native run inconclusive (2026-10-26).** Candidate
`71a731efef24d255fb77dac00257adb8a1abf8b1` passed 407 frontend tests, worker
compile/static checks, 326 worker tests (two POSIX-only skips), four audio source
validators, frontend build and locked Rust compile check. Symlink tests required
a bounded administrator test process and fresh temporary directory; no source
change was needed. The native dev app actually launched with Built-in Male and
reached Live. Optional incoming was still Windows Default; the owner confirmed
YouTube was playing through speakers, so this was not a controlled outbound-only
baseline. Native callback errors appeared for incoming and later microphone;
their cause remains UNKNOWN. Mic Test reported Too quiet. Normal Stop could not
be confirmed because UI automation detected concurrent input; emergency closure
removed the owned app and worker. No controlled speech/receiver or full-Meeting
PASS is claimed. Current Validation owns exact proof and residual boundaries.

**Integrated Meeting runbook aligned (2026-10-26).** The existing
`docs/knowledge/operations/target-windows-performance.md` now groups the next
native baseline into five flows: readiness/Start, outbound receiver delivery,
source/Mute/Stop/restart, optional incoming coexistence, and transcript/export/app
restart. It links existing scenarios/fixtures, separates source compile checks
from actual app launch, removes retired Text testing and stale CI authorization,
and keeps My Voice/install/extended fault checks conditional. This is procedure
repair only; no native flow was executed and no engine/test implementation changed.

**Owner accepts the six discussed translation limitations for continued development
(2026-10-26).** Keep MiLMMT and its natural current-source translation behavior.
Do not pursue these six outputs with phrase substitutions, added prompt rules,
context/history saving, style/glossary injection or a second corrective engine.
This is a bounded owner acceptance of the inspected outputs, not permission to
accept every future action/number/identity error. Current Validation records the
exact cases and evidence boundary. Retain the original 20 PASS / 2 REVIEW / 6 FAIL
diagnostics; they do not require runtime repair before continuing Meeting testing.
No automatic PASS override, promotion-gate bypass or release acceptance follows.

**Meaning-first evaluation adjusted (2026-10-26).** Owner clarification:
understandable, faithful translations may differ from reference wording; grammar
or lexical similarity is not an acceptance target. Existing grader/direct worker
reports now distinguish PASS / REVIEW / FAIL / UNVERIFIED. Missing cue/order
matches require review, while declared contrary meanings and explicitly strict
identity literals fail. REVIEW cannot produce zero-exit acceptance. Existing raw
critical-check fields remain conservative regression signals, not model-error
counts. No production model/prompt/context behavior changed.
Full comparison also requires candidate PASS; unchanged REVIEW/FAIL cannot
authorize promotion through the no-new-regression check alone.

Eleven existing contracts now accept clear variants and protect mixed meanings.
The prior 28 actual outputs regrade to 20 PASS / 2 REVIEW / 6 detected pattern or
literal FAIL; five lexical false alarms are removed. Meeting-sound wording and
the previous-version approval ellipsis remain REVIEW. All 191 requests, fast
membership and frozen held-out bytes are unchanged. This is revised measurement,
not improved model output or human acceptance. Evaluator self-tests, 39 source
contract tests and actual PASS/REVIEW/FAIL CLI controls pass. New inference was
unnecessary; current validation owns original/source/grader identity and receipts.

**Lean, effective testing-system goal delivered (2026-10-26).** Source candidate
`81a16a50d8ccfd4c51453e3ba69109081b1c4a7b` passed exact-source admission and
fresh model/fixture acceptance. Existing `worker_runner.py` is the execution owner:
`--translation-suite fast|full` for direct translation, `--profile source|core|endurance`
for source/audio/resource questions. Existing evaluators remain grading owners;
corpora/scenarios own stable expectations and selection. No new engine, dependency
or parallel test framework was added. The maintained gate/cost/maintenance policy
is in `tools/translation_quality/README.md`, not this continuation.

The 28-case fast selection has explicit reasons and risk checks in both directions;
all 191 regression and 40 held-out cases remain. Calibrated controls distinguish
faithful variants from reversed corrections, changed numeric values and local
negated forbidden phrases. Real-child code-point controls protect UTF-8 transport;
a failed earlier model run exposed that defect and remains preserved. Output,
EOS, source/model identity and child cleanup are enforced. Missing/invalid output
cannot pass; fast-only output cannot authorize full promotion.

Fresh fast execution completed all 28 requests on CUDA/BF16 in 15,781 ms including
cold loading and cleanup: 15 declared passes, 13 review flags, no missing evidence,
worker terminated. Fresh audio core passed all 13 scenarios × 2 rounds, with
current-source VAD/writer inputs. Offline regrading agreed exactly for both.
The independent completion audit verified unchanged 191 requests, unchanged
held-out and production source, identity-bound receipts and scope limits.
`current-validation.md` owns receipt hashes and requirement-by-requirement proof.

This closes the testing-system goal. MiLMMT's declared quality checks retain
their flags; the six inspected failures are owner-accepted development limitations
as described above, not a new engine-repair task. Full/held-out inference was not
repeated after unchanged model behavior. Human bilingual review, naturalness,
physical capture, installed-app and independent-receiver gates remain separate
product acceptance, not a reason to repeat source/endurance tests blindly.

**Standalone Text removed from the application (2026-10-26).** The owner has
chosen Meeting voice translation as the product focus and retained MiLMMT;
model-replacement research/download is no longer the active continuation.
Normal navigation is Meeting / My Voice / Settings. Text's page, hidden mount,
setup shortcut, status/readiness branches, frontend bridge/facade/cache/result
policy and registered Tauri command/module are removed. Caption admission
rejects retired Text payloads; Meeting captions remain. Settings schema 13 drops
Text-only source/target direction preferences on ordinary save/clone, retaining
Meeting incoming directions and audio/setup preferences. Private UserData was
not migrated or deleted by this repair.

Direct text translation remains an internal canonical worker mechanism for
isolating ASR vs model errors. Existing worker chunk/EOS/size bounds and both
language directions remain; the internal native verification scheduling tier is
still used by worker probes, not an application Text feature. No AI/audio/model,
dependency, voice, route or inference policy was changed.

Verification on the repair working tree: strict Svelte check (zero errors and
warnings), source contracts, production frontend build, native developer build and 407 frontend tests
passed; 335 Rust tests passed with one replay ignored; 75 affected worker tests
and 191-regression/40-held-out evaluator self-tests passed. This proves source
and fixture boundaries, not rendered native or installed-app acceptance. An
additional 16 impact-tool tests pass in a hash-recorded view of current tracked
source. Direct execution in the installed workspace hits the unchanged graph
capacity because environment/generated source files are scanned too; no analyzer
limit was raised or source failure hidden. Its temporary source-view cleanup was
rejected by automatic policy review (`blocked by policy`); the ignored copy stays
under `.workspace/tmp/remove-text-impact-source-view`, outside application/package.

**MiLMMT semantic fidelity remains FAIL / OPEN.** The construction-purpose
addition was previously reproduced from raw canonical inference as well as the
now-retired Text UI. Keeping MiLMMT and removing Text do not fix that output.
Beam search and FP32 retained the addition; sentence isolation introduced a
reference regression and was rejected. The repaired comparison gate catches new
errors inside already-failed cases; the native budget example is retained in the
191-case regression corpus. No rejected inference candidate was promoted.
Private receipts remain under `.workspace/reports/translation-fidelity-723fc247/`;
[current validation](../docs/knowledge/current-validation.md) owns proof limits.

## Retained native exit repair

**Native exit gate repaired (2026-10-26).** `application_runtime/shutdown.rs`
now rejects a failed Meeting Stop and re-reads runtime ownership plus the
canonical Meeting cleanup-pending fact before stopping the helper or clearing
the recovery marker. A missing session is no longer sufficient for clean exit.
Four targeted Rust shutdown tests, twelve close/recovery Node tests, the
ApplicationRuntime architecture validator and the frontend production build
passed locally. Controlled callbacks prove pending cleanup blocks shutdown,
verified release permits ordered shutdown and helper/marker failure refuses
clean exit; they do not prove actual native-window/device behavior.

No per-change tests on the user's PC. Resume native proof only within
one integrated local Windows test phase under the existing acceptance runbook.
GitHub default branch `main` remains reserved for explicitly approved final
outcomes; `Local` is the only development authority and CI remains paused.
Updater install concurrency remains a separate audit finding; no updater,
audio/AI algorithm, model, private data or release behavior changed here.

## Retained workspace outcome

**First bounded cleanup delivered (2026-10-26).** `manage.cmd clean` now previews
renewable build outputs; `-Profile cache` previews npm/uv/pip and root pytest/Ruff
caches. Deletion requires `-Apply`. The Windows cleanup fixture passed dry-run,
tracked/ignored checks, junction rejection, active-build blocking, protected-data
and idempotence checks. Real build/cache cleanup reduced logical workspace size
from 35.71 to 16.71 GiB; observed drive free space increased by 16.86 GiB.
Protected path/size/mtime inventory fingerprints matched for UserData, runtime
assets, the worker environment and reports; this is metadata evidence, not a
new model/native acceptance claim. Models, active environments, accepted audio
fixtures and proof artifacts remain in place. Root `.cache` and staging remain
review candidates; one small failed-test fixture remains after an automatic
approval-review rejection of its removal. Three planning-text contract failures
were recorded at cleanup delivery; the current continuation restores those
canonical boundaries and repository verification now passes.
The [workspace contract](../docs/system/windows-workspace-boundaries.md#explicit-development-cleanup)
owns cleanup/retention rules. No source-root or installed-app path migration occurred.

## Retained automatic-regression acceptance

**The automatic-regression-system goal is complete (2026-10-26).** The existing audio harness now has one maintained entry point, `worker_runner.py --profile source|core|endurance`, with separate scenario/fixture, transport/execution and shared grading owners. Core membership and round policy live in `scenarios.json` independently of acoustic identity. ASR/translation/TTS expectations remain in their canonical evaluators/corpora; declared digit/word aliases do not require replacing fixtures or previous outputs. See [commands and maintenance boundaries](../tools/audio_validation/README.md#official-regression-entry-point).

**Accepted executable source:** clean `0d571fb472741df02179cdb739ea5379070ec6ef`. Actual source admission passed all four validators; core passed 13 scenarios × 2 rounds; endurance passed the same 13 × 20 rounds. Both model profiles performed fresh canonical VAD at that same source, with all 48 VAD expectations passing, then used one persistent worker per profile. Offline grading matched exactly. Seven explicit CLI controls accepted equivalent numeric forms and rejected incorrect or incomplete evidence with the required exit code. [Current Validation](../docs/knowledge/current-validation.md) owns receipt hashes, the completion audit and proof limits. No further test-system implementation is required for this bounded delivery.

## Active Boundary

The delivered regression package covers the fixed 13 vital scenarios, not all 48 downstream scenarios or all 28 planned faults. Perceptual naturalness, FemaleVoice, adaptive pace, native Stop, physical capture, installed-app and independent receiver acceptance remain separate. Short repeated runs and allocator samples do not prove leak-free long-duration behavior or native end-to-end latency.

Earlier Text/runtime and paused My Voice source-candidate evidence retains its own revision/scope. Do not restore private archived data or resume training automatically. No incremental user-PC testing, cloud inference, CI re-enable, main promotion or release follows from this completed goal. CI remains paused; Local remains the only development branch.

## Next Step

During the admitted integrated native phase, observe the new per-lane PCM drop breakdown through the existing explicit Diagnostics export in one bounded Start/Stop, without requiring user speech. Distinguish ingress rejection from handoff loss before changing transport policy; do not retrospectively attribute the historical two drops. Preserve normal Stop, exact-source identity and privacy. The incoming native error and uncontrolled recognized phrases remain separate unresolved evidence. No CI, model/context/style/glossary change, incoming toggle, queue enlargement, blind retry or corrective translation layer is authorized.

## Retained My Voice source candidate (paused)

**Status:** SOURCE_CANDIDATE / NOT EXECUTED. **Branch:** `Local`. **Date:** 2026-10-14.
**Source of history:** prior commits on `Local` and `docs/knowledge/proposals/2026-10-14-my-voice-safe-8gb-training.md`. This file is the CURRENT work handoff, not an ever-growing changelog.

## Scope and owners

- The system must **discover each user's supported NVIDIA CUDA GPU** using CUDA physical UUID and matching NVIDIA sensor data, and calculate memory budget from that device's actual free VRAM. RTX 3070 8 GiB is a reference only, not a hardcoded hardware requirement. Non-CUDA training is not silently substituted.
- **Rust/Tauri** owns the Create/Improve lifecycle, authorization, selected GPU, RAM/disk preflight, run-token/generation, Windows Job Object, Stop request, completion/review receipt and immutable recovery archive.
- **One Python worker** owns stage orchestration. The existing stage supervisor measures RAM/VRAM/disk and PID-scoped training evidence. No parallel supervisor, duplicate UI poller, cloud trainer, automatic retries, or automatic optimizer resume.
- Pinned GPT-SoVITS V2ProPlus source stays immutable on disk. Source-hash-bound single-GPU adapters handle compatibility and scoped checkpoint writers. Mixed-FP16 eligibility is a provisional CUDA compute-capability policy (major >= 7), not a universal GPU support claim.

## Source-candidate safety flow

1. **Admission:** reject unknown GPU identity/VRAM, incompatible precision, or inadequate RAM/disk. GPU batch size is currently 1; allocator fractions and resource reservations are conservative but cannot guarantee no OOM.
2. **Execution:** Windows worker/stage children are requested suspended, admitted to owned kill-on-close Jobs, then resumed. Global free VRAM, physical GPU identity and system headroom are observed via the already-running stage supervisor. Stable high usage is WATCH. Sustained PRESSURE requests checkpoint-boundary stop; CRITICAL/UNKNOWN triggers owned emergency termination. The initial pressure warmup and thresholds are provisional.
3. **Observability:** sparse heartbeat means supervisor responsiveness only. Stage-PID + fresh nonce + run-token/generation-bound small epoch and checkpoint receipts record actual epoch boundaries and checkpoint file publication, respectively. Stall warnings adapt to measured per-user epoch duration and NEVER auto-kill a merely slow GPU.
4. **Stop:** manual Stop uses a create-only, run/generation-bound signal read by that SAME supervisor. SoVITS/GPT only request cooperative stop at eligible saved-epoch boundaries; other stages stop directly. A bounded timeout falls back to owned termination. No stale/corrupt control record is silently treated as permission to continue.
5. **Checkpoint:** full SoVITS G/D optimizer checkpoints use distinct atomic per-file writes; a SHA-256 pair receipt binds a common global step and epoch. GPT full checkpoints use a scoped Lightning 2.6.5 CheckpointIO adapter with atomic file publication; inference candidates retain their pinned format. Four SoVITS pair-save epochs + five GPT full-save epochs bound I/O and fit the recovery count limit, avoiding redundant `last.ckpt` copies.
6. **Completion and recovery:** one Rust lifecycle mutex serializes **Stop versus completion receipt + Evaluating → idle**. Upon Python root exit, Rust checks the owned Windows Job's active-process count and refuses Review/recovery if descendant quiescence cannot be verified. Interrupted checkpoints are archived only after this gate, with free-disk admission and independent copy/destination hashes. Reuse source part hashes for G/D pair metadata instead of rereading GB files. Existing selected Meeting voice/recordings are not replaced on failure.
7. **Resume:** preserved checkpoints remain candidates for **developer-only integrity diagnostics**. A correct hash or saved optimizer file does not prove restored optimizer, scheduler, GradScaler, RNG, or training continuity. `resume_available` remains **false**. Any restored-training implementation must be separately validated and explicitly approved.

## Important limitations (do not claim completed product)

- No Rust/Node/Python suite, cargo check, build, Windows Job API run, GPU training, quality listening, installer inspection, simulated OOM, or hardware verification was performed as part of the source-only development. Source matches and authored test cases are NOT test passes.
- Device selection/allocator reservations are provisional. CUDA OOM and driver faults can occur between readings. Windows nested Job compatibility, thread suspension/release, graceful Stop, exact Lightning callback interactions and checkpoint saves must be tested on supported devices when the user authorizes validation.
- A G/D pair is two separately committed files, NOT a single atomic transaction. Checkpoint staging + pair hashes limit partial publication but cannot prove optimizer resume. The recovery archive may conservatively require manual attention when source files are incomplete.
- The stage PID and nonce are **correlation**, not a security boundary against intentionally malicious local processes.

## Generation handoff remains busy until owned process release — 2026-10-14

- Fixed a post-completion race: the small Review receipt is atomically published while the previous Python process may still be owned by the completion thread. Start/Approve/Meeting-change now treat that existing process ownership as busy until its PID/Job and generation are released.
- The canonical My Voice status defers `evaluation_ready` and `can_build` during this short finalization window, exposing a presentation-only `finalizing` status that disables repeated Stop requests while retaining the existing status polling. The underlying lifecycle remains unchanged. No new state field, worker, mutex, or polling loop is introduced.
- Source only. The window and UI behavior must be confirmed during eventual Windows acceptance.

## Source hardening: Windows accounting failure paths — 2026-10-14

- Cross-checked the owned Windows Job's `JOBOBJECT_BASIC_ACCOUNTING_INFORMATION` ABI against Microsoft's published structure, and authored a Windows-gated 48-byte compile-time layout assertion.
- When querying the active descendants fails, Rust now **attempts owned-Job termination but still returns the original accounting error**. It does not confuse a failed query with a stopped tree and does not start Review or checkpoint archiving. `KILL_ON_JOB_CLOSE` remains a fallback.
- Windows supports nested Job trees on current supported versions, but nested assignment/runtime compatibility is still unverified. No new process, background monitor, polling, or arbitrary external process termination was added.
- Changes are source-only; Windows compilation and Job API fault-injection remain disallowed until authorized.

## Latest optimizer candidate selection (source-only) — 2026-10-14

- Developer-only recovery audit previously used lexicographic labels (e.g. `gpt-10.ckpt` before `gpt-9.ckpt`) to select the last checkpoint for explicit tensor inspection. The latest candidate is now selected from the **original saved epoch and global step**, and from the original SoVITS G/D global step, not archive label order.
- Unrecognized historical checkpoint names (for example `last.ckpt` without trustworthy source epoch) block optional tensor inspection instead of silently selecting an arbitrary file. All byte/hash verification remains intact and `resume_available` remains **false**.
- Regression fixtures are authored but unexecuted, with no torch import or checkpoint tensor reads performed during this change.

## Windows Job handle lifetime before quiescence — 2026-10-14

- After `Child::try_wait`/`wait`, the Rust completion owner now explicitly drops the reaped `Child` handle **before** asking its owned Job for the active-process count. Windows process accounting can retain process-object references until handles are released; retaining the root handle through the query could falsely block legitimate completion/recovery.
- Descendant cleanup and fail-closed behavior are unchanged. No extra monitor, duplicate wait, or global process kill is added. Windows handle semantics remain to be confirmed during authorized runtime testing.

## CPython subprocess HANDLE lifetime before stage Job accounting — 2026-10-14

- Pinned CPython 3.12 `subprocess.Popen` retains a Windows process `Handle` object until it is closed/finalized. After the stage root has exited and its returncode is cached, the existing Python supervisor now explicitly closes that root HANDLE before querying its owned Job's active-process count. This prevents a retained Python handle from potentially causing a false `stage_descendants_still_running` failure on otherwise successful stages.
- The Job remains owned by the same supervisor and still kills true descendants if any remain; a missing/invalid HANDLE refuses to infer quiescence. No new subprocess, worker, polling loop or GPU allocation is introduced.
- Source-only, without Python execution on Windows. This relies on the currently pinned CPython 3.12 `Popen._handle` implementation, which should be reconfirmed if the Python runtime changes.

## Hard-stop timeout and cleanup attention gate — 2026-10-14

- Fixed an unbounded `Child::wait()` after forced cancellation / failed `try_wait`: the existing completion thread now makes only a bounded eight-second reaping attempt and **always classifies forced Stop as incomplete**, irrespective of the eventual child exit code.
- If Windows Job quiescence is unverifiable, the finalization path **keeps the old generation/Job handle in its existing process owner**, blocks new Start/Approve/Meeting changes, shows an explicit attention message, and refuses to clear the original Runtime work tree. It does not claim successful cancellation or archive integrity.
- No additional OS watcher, lock, process or progress poll was introduced. These code paths are still not runtime-tested and must remain fail-closed until Windows fault-injection is authorized.

## Unexecuted Windows failure acceptance matrix

These are future **acceptance scenarios**, not tests performed or runtime results. Test only after the user authorizes validation. In every scenario the existing Meeting voice, original recordings and immutable approved versions must remain unchanged.

| Scenario | Expected safe result | Existing source owner |
| --- | --- | --- |
| Stop during dataset preparation or before Python spawn | Do not launch GPU work; leave saved recordings untouched | Rust build lifecycle |
| Stop during SoVITS/GPT training | Request checkpoint-boundary stop using the existing supervisor; honor bounded deadline and never claim success | Python stage supervisor + Rust process owner |
| Trainer ignores Stop and Job termination succeeds | Forced stop classified **incomplete** regardless of eventual root exit code; preserve verified checkpoint candidates | Rust completion owner |
| TerminateJobObject fails or root cannot be reaped promptly | No unbounded wait; refuse Review; query owned Job quiescence before recovery | Rust Job owner |
| Job active-process accounting unavailable or descendants remain | Attempt owned termination, preserve existing runtime files, retain ownership gate, prevent new Create/Approve/Meeting changes | Rust Job/accounting + process store |
| Python stage exits while child process is still present | Reject successful stage; owned subprocess tree must stop before any next stage | Python stage supervisor |
| New SoVITS G/D or GPT checkpoint is partially written | Reject partial/pending artifacts; keep older checkpoint and original training work for attention | Atomic writer + Rust recovery |
| Checkpoint source changes while being copied | Reject archive, preserve original Runtime work; no optimizer resume | Rust immutable recovery |
| Stop races with completed evaluation | One existing lifecycle mutex decides Stop or Review; never publish a cancelled result | Rust lifecycle receipt gate |
| App closes abruptly while Job children exist | Owned kill-on-close must be verified on target Windows; no promise until tested | Windows Job Object |

**No speculative retry, auto-resume, GPU scaling or background monitors.** The existing active-process owner and canonical status remain the only runtime authorities.

## Windows suspended process admission cleanup — source-only

- A suspended child not yet admitted to a Job **cannot** be killed by terminating that empty Job. Both the Python stage supervisor and Rust top-level owner now attempt to terminate the **exact created process handle** on Job assignment failure and use a bounded reaping period. No PID enumeration, unrelated task kill, or extra worker is used.
- On the rare failure to prove that the suspended Rust process stopped, the native lifecycle stays in cancelling/attention state and prevents launching another training generation rather than claiming a clean failure. These exceptional paths cannot guarantee orphan prevention when Windows itself refuses termination; explicit diagnostics remain required.
- This is unexecuted source; do not claim fault containment is validated.

## Source-only POSIX process group ownership — 2026-10-14

- The stage supervisor no longer treats root PID exit as proof that all POSIX descendants exited. Because a POSIX stage starts in its own new session, the supervisor checks its specific stage process-group ID even after root exit and sends SIGKILL only to that dedicated group if descendants remain.
- Uncertain group ownership, permissions, and leftover descendants continue to fail closed. This **does not** enable My Voice training on Linux/macOS: the current native admission is Windows-only, and no cross-platform training claims are made. The main Rust non-Windows process owner needs separate hardening before any future non-Windows backend is supported.
- No new stage, process, watcher or polling channel was introduced. Source fixtures authored but not executed.

## Accurate attention status after unconfirmed cleanup — 2026-10-14

- When Rust cannot prove owned process cleanup, it retains the active cancellation gate and an explicit checkpoint/work preservation message. Both native My Voice status and the existing Svelte UI now **display this attention message**, rather than indefinitely claiming “Stopping safely”.
- The runtime event is emitted as a failed build when recovery/quiescence needs attention, even if Stop had been requested. This changes diagnostics only; it never claims Review or resume completed.
- No additional state owner, worker, polling or I/O channel. Source-only; not runtime tested.

## Repair of stale source regression contracts

- Previously authored Node source assertions referenced an earlier Rust expression (`!cancelling && exit...` and `let recovery = if ready`) that no longer matches the hardened lifecycle. Contracts now check the actual `owned_tree_quiescent`, `cancelling_before_review` and `recovery_failed` gates.
- This removes known future false-negative source failures without weakening the actual safety conditions. Tests were authored only, not executed, in accordance with the user's source-only boundary.

## POSIX subprocess group cleanup audit — source-only (2026-10-14)

- The Python stage supervisor already checks for surviving same-process-group children
  when the root exits and rejects stage success when they remain. A second issue
  existed during exceptional termination: after sending SIGKILL to the isolated
  process group, the cleanup path waited only for the root process.
- Cleanup now performs a bounded, post-root-wait group-existence check. If the
  original stage process group does not disappear, it raises an explicit
  `posix_group_cleanup_not_confirmed` failure instead of inferring cleanup
  from the root's return code. Regression cases use mocked process ownership;
  no process spawn, real signal, Python test suite, or native execution was run.
- POSIX process-group ownership does **not** track descendants that deliberately
  move to a separate session/group. It is not a general process-tree guarantee.
  This does not enable Linux/macOS My Voice training, change GPU admission,
  alter checkpoint publication, or imply Windows Job behavior was tested.

## Rust top-level non-Windows ownership audit — source-only (2026-10-14)

- Native Create and Improve share an early Windows-only admission guard before
  the dataset, accepted recording selection, GPU checks or child spawn. An
  available CUDA probe on Linux does not permit My Voice training.
- The previously unconditional non-Windows `owned_tree_quiescent = Ok(())`
  was unsafe: root exit and `kill -TERM <pid>` do not prove descendant
  cleanup. Its defensive fallback now remains `Err`, preserving Work files,
  preventing review publication and blocking another build rather than
  promoting unsupported cleanup to success. Any fallback attention text
  is explicitly non-Windows, not a claim that a Windows Job Object exists.
- The existing Rust source-contract suite now checks both entrypoints and the
  fail-closed completion/recovery gate. These checks were authored only;
  Node/Cargo, process execution and target-Windows validation were NOT RUN.
  No cross-platform training support or POSIX top-level process-group owner
  was introduced; Windows Job implementation remains unchanged.

## Windows My Voice process ownership and checkpoint finalization audit — source-only (2026-10-14)

- Found a definite exceptional-path hang: after Python spawn, a poisoned
  `process_store()` mutex could execute unbounded `child.wait()` despite
  the existing forced-stop timeout policy. It could also claim the child was
  stopped without verifying that result.
- The exception path now requests termination of its exact already-owned
  Windows Job, performs bounded root reaping, releases the root HANDLE and
  verifies Job descendant quiescence before describing the tree as stopped.
  Failure retains the cancellation/attention gate and preserves existing
  training files. Non-Windows remains unsupported and cannot claim cleanup.
- Review publication still requires `owned_tree_quiescent.is_ok()`, an
  authoritative ready-for-review worker receipt, and the existing
  cancellation-versus-completion lifecycle lock. Recovery copying is refused
  when owned child quiescence is not proven. The signed/verified checkpoint
  and automatic optimizer-resume policies were not changed.
- Added an exact-source regression contract covering the former unbounded
  wait and false-success branch. Tests were authored only; no Node, Cargo,
  Windows Job, model runtime or GPU checks were performed. An OS failure to
  terminate a genuinely unowned process remains an environment-level
  limitation, never a claimed success.

## Existing-engine inference efficiency — source-only (2026-10-15)

- **Product decision:** retain Faster Whisper/CTranslate2 ASR, MiLMMT/PyTorch
  translation, GPT-SoVITS/PyTorch TTS and the single Python worker. Research
  into alternative translation runtimes is historical only and **not an active
  candidate**; do not implement an engine swap, parallel provider, fallback
  router or model substitution for optimization.
- MiLMMT already uses persistent model reuse, BF16, deterministic generation,
  `torch.inference_mode()` and KV cache. Its `inference_ms` now includes the
  existing output-ID CPU materialization instead of stopping at potentially
  asynchronous CUDA dispatch. No extra GPU sync or speculative compiler/cache
  policy was introduced.
- Current GPT-SoVITS reuses its prompt/reference cache, including the
  TranslateIT speaker-embedding adapter. Source inspection of pinned upstream
  confirms per-`run()` `gc.collect()` and CUDA `empty_cache()` cleanup.
  Retain that lifecycle pending real multi-engine VRAM and warm-latency data;
  allocator cleanup changes are **not** proven safe from source alone.
- Optimized `voice_lab_gpt_sovits.py` PCM16 reference signal validation by
  combining silence/clipping/active-RMS/DC calculations in one traversal
  without allocating a separate active-sample list. Original quality
  thresholds and arithmetic are preserved; deterministic parity tests were
  authored but **not executed**. This affects actor/preview validation rather
  than the hot TTS acoustic generation.
- The hot-path `write_wav()` adapter no longer serializes an already
  contiguous PCM16 output with `.tobytes()`. CPython 3.12 `wave.writeframes`
  accepts a contiguous buffer directly; use `memoryview` and only make a
  contiguous copy for strided input. Existing int16/float conversion,
  mono sample format, WAV header, and empty-output rejection are unchanged.
  Source-authored byte-parity tests cover PCM16, strided and float inputs;
  these tests were not executed. Lower temporary output allocation is
  source-demonstrated, not a measured end-to-end latency improvement.
- Repeated actor-package readiness validation requires WAV duration and complete
  canonical PCM16 frames, but had called the full preview-quality analyzer
  (decode, silence, clipping, RMS and DC) each time. Both paths now share
  the original full-frame validation while duration-only reads no longer
  unpack or traverse individual samples. The same empty/truncated/noncanonical
  rejection remains. Authored negative/parity tests are not executed; this
  removes CPU work, not TTS model inference or safety validation.
- Section 5B of the **existing** target-Windows acceptance runbook now
  scopes eventual measurement to the current engines, stage latency, CUDA
  memory coexistence and selected voice output. No model/voice/GPU tests,
  cloud CI or REA runtime analysis was executed.
