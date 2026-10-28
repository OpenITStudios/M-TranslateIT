<script lang="ts">
  import type { MeetingSessionStatus } from "../../app/bridge/runtimeApi";
  import { sanitizeDiagnosticText } from "../../app/shared/diagnosticPrivacy";

  let { status }: { status: MeetingSessionStatus | null } = $props();

  const timing = $derived(status?.outbound?.timing ?? null);
  const outbound = $derived(status?.outbound ?? null);

  function isMeasuredDuration(value: number | null | undefined): value is number {
    return typeof value === "number" && Number.isFinite(value) && value >= 0;
  }

  // Worker stage totals already include this scheduler admission wait.
  // The residual is transport + runtime + inference, NOT isolated model time.
  // A missing or contradictory admission receipt must stay unmeasured.
  function postSchedulerElapsed(
    stageMs: number | null | undefined,
    admissionWaitMs: number | null | undefined,
  ): number | null {
    if (!isMeasuredDuration(stageMs) || !isMeasuredDuration(admissionWaitMs) || admissionWaitMs > stageMs) {
      return null;
    }
    return stageMs - admissionWaitMs;
  }

  const postScheduler = $derived({
    asr: postSchedulerElapsed(timing?.asr_ms, timing?.asr_scheduler_wait_ms),
    translation: postSchedulerElapsed(timing?.translation_ms, timing?.translation_scheduler_wait_ms),
    tts: postSchedulerElapsed(timing?.tts_ms, timing?.tts_scheduler_wait_ms),
  });

  function formatTiming(value: number | null | undefined): string {
    if (!isMeasuredDuration(value)) return "Not measured";
    return `${Math.round(value)} ms`;
  }

  function formatCount(value: number | null | undefined): string {
    return typeof value === "number" && Number.isFinite(value) && value >= 0
      ? String(Math.round(value))
      : "Not measured";
  }

  function formatRate(value: number | null | undefined): string {
    return typeof value === "number" && Number.isFinite(value) && value >= 0
      ? `${value.toFixed(1)} tok/s`
      : "Not measured";
  }

  const diagnosis = $derived.by(() => {
    if (!status?.has_session) return "No active Meeting session. Start a real session to collect timing evidence.";
    if ((outbound?.overflow_dropped_utterance_count ?? 0) > 0 ||
        (outbound?.evicted_pending_utterance_count ?? 0) > 0) {
      return "The runtime reports dropped or evicted finalized phrases. Inspect queue pressure before changing ASR or VAD settings.";
    }
    if (outbound?.blocker) {
      return `Latest outbound blocker: ${sanitizeDiagnosticText(outbound.blocker)}. Check the affected stage before changing models.`;
    }
    if (timing?.outbound_latency_ms == null) {
      return "First translated playback has not been measured for this phrase. Do not infer end-to-end latency from partial stages.";
    }
    // Compare disjoint observed intervals. Ranking inclusive ASR/MT/TTS totals
    // alongside their waits would count scheduler contention as model slowness.
    const stages: Array<[string, number | null | undefined]> = [
      ["Finalized queue", timing.queue_ms],
      ["Audio preparation", timing.audio_prepare_ms],
      ["ASR scheduler wait", timing.asr_scheduler_wait_ms],
      ["ASR post-admission", postScheduler.asr],
      ["Translation scheduler wait", timing.translation_scheduler_wait_ms],
      ["Translation post-admission", postScheduler.translation],
      ["TTS scheduler wait", timing.tts_scheduler_wait_ms],
      ["TTS post-admission", postScheduler.tts],
      ["Playback queue", timing.playback_queue_ms],
      ["Delivery", timing.delivery_ms],
    ];
    const measured = stages.filter((stage): stage is [string, number] => isMeasuredDuration(stage[1]));
    if (!measured.length) return "No individual processing interval has valid timing evidence yet.";
    const largest = measured.reduce((best, stage) => stage[1] > best[1] ? stage : best);
    return `Largest recorded stage: ${largest[0]} (${formatTiming(largest[1])}). Intervals exclude measured scheduler wait where available; missing admission evidence is not estimated. This is a measurement, not proof of the underlying bottleneck.`;
  });
</script>

<article class="ti-panel p-5">
  <div>
    <span class="ti-field-label">Latest outbound phrase</span>
    <h4 class="mb-0 mt-1.5 text-[14px] font-semibold">Meeting performance</h4>
  </div>

  <div class="mt-4 grid grid-cols-2 gap-3 lg:grid-cols-5">
    <div class="ti-state-card"><span class="ti-field-label">After finalization</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.outbound_latency_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">ASR</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.asr_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Translation</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.translation_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Voice TTS</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.tts_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Delivery</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.delivery_ms)}</strong></div>
  </div>

  <div class="mt-3 grid grid-cols-2 gap-3">
    <div class="ti-state-card"><span class="ti-field-label">Estimated speech onset → first audio</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.estimated_onset_to_first_audio_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Estimated speech end → first audio</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.estimated_end_to_first_audio_ms)}</strong></div>
  </div>
  <p class="mt-2 text-[11px] leading-5 text-[var(--ti-text-soft)]">Speech onset/end estimates include VAD's buffered speech span and trailing silence. They are not precise microphone or remote-participant timestamps.</p>

  <div class="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-3">
    <div class="ti-state-card"><span class="ti-field-label">ASR scheduler wait</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.asr_scheduler_wait_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Translation scheduler wait</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.translation_scheduler_wait_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">TTS scheduler wait</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.tts_scheduler_wait_ms)}</strong></div>
  </div>
  <div class="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-3">
    <div class="ti-state-card"><span class="ti-field-label">ASR post-admission</span><strong class="mt-2 block text-[12px]">{formatTiming(postScheduler.asr)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Translation post-admission</span><strong class="mt-2 block text-[12px]">{formatTiming(postScheduler.translation)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">TTS post-admission</span><strong class="mt-2 block text-[12px]">{formatTiming(postScheduler.tts)}</strong></div>
  </div>
  <div class="mt-3 grid grid-cols-2 gap-3">
    <div class="ti-state-card"><span class="ti-field-label">Playback queue wait</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.playback_queue_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Worker admission truth</span><strong class="mt-2 block text-[12px]">{postScheduler.asr != null && postScheduler.translation != null && postScheduler.tts != null ? "Complete" : "Partial / unmeasured"}</strong></div>
  </div>

  <div class="mt-3 grid grid-cols-2 gap-3">
    <div class="ti-state-card"><span class="ti-field-label">Output stream build</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.output_stream_build_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Output stream start → first callback</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.output_stream_start_to_first_audio_ms)}</strong></div>
  </div>

  <div class="mt-3 grid grid-cols-2 gap-3 lg:grid-cols-4">
    <div class="ti-state-card"><span class="ti-field-label">Translate tokenize</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.translation_tokenization_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Translate inference</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.translation_inference_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Translate decode</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.translation_decode_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Translate throughput</span><strong class="mt-2 block text-[12px]">{formatRate(timing?.translation_tokens_per_second)}</strong></div>
  </div>

  <div class="mt-3 grid grid-cols-[repeat(4,minmax(0,1fr))] gap-3">
    <div class="ti-state-card"><span class="ti-field-label">Speech boundary</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.speech_boundary_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Finalization</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.finalization_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Queue wait</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.queue_ms)}</strong></div>
    <div class="ti-state-card"><span class="ti-field-label">Audio prepare</span><strong class="mt-2 block text-[12px]">{formatTiming(timing?.audio_prepare_ms)}</strong></div>
  </div>

  <div class="mt-3 grid grid-cols-2 gap-3">
    <div class="ti-state-card">
      <span class="ti-field-label">Overflow-dropped utterances</span>
      <strong class="mt-2 block text-[12px]">{formatCount(outbound?.overflow_dropped_utterance_count)}</strong>
    </div>
    <div class="ti-state-card">
      <span class="ti-field-label">Evicted pending utterances</span>
      <strong class="mt-2 block text-[12px]">{formatCount(outbound?.evicted_pending_utterance_count)}</strong>
    </div>
  </div>

  <p class="mb-0 mt-3 text-[11.5px] leading-5 text-[var(--ti-text-muted)]" role="status">{diagnosis}</p>
  <p class="mb-0 mt-2 text-[11.5px] leading-5 text-[var(--ti-text-soft)]">
    Stage timings are observational evidence from this device only. ASR/translation/TTS totals already include scheduler wait, so do not add wait times to them. Post-admission elapsed also includes worker transport, preprocessing, and recovery; it is not isolated AI inference. Incomplete or inconsistent admission receipts remain Not measured. Measure whole-product VRAM with Windows/NVIDIA GPU tools (including browser and WDDM); no app-level VRAM figure is inferred.
  </p>
</article>
