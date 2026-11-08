<script lang="ts">
  import { ArrowLeft, Bug, RefreshCw } from "@lucide/svelte";
  import { runtimeApi, type AudioQualityReport } from "../../app/bridge/runtimeApi";
  import {
    parseWorkerCapabilities,
    type ProductRuntimeSnapshot,
    type ProductSetupAction,
  } from "../../app/bridge/runtimeProductFacade";
  import { sanitizeDiagnosticText } from "../../app/shared/diagnosticPrivacy";
  import MeetingPerformanceDiagnostics from "./MeetingPerformanceDiagnostics.svelte";
  import RuntimeReliabilityDiagnostics from "./RuntimeReliabilityDiagnostics.svelte";
  import CodexGatewayDiagnostics from "./CodexGatewayDiagnostics.svelte";

  let {
    snapshot,
    setupBusy = false,
    diagnosticsLoading = false,
    refreshRevision = 0,
    onRefreshDiagnostics,
    onSetupAction,
    onBack,
  }: {
    snapshot: ProductRuntimeSnapshot;
    setupBusy?: boolean;
    diagnosticsLoading?: boolean;
    refreshRevision?: number;
    onRefreshDiagnostics: () => void | Promise<void>;
    onSetupAction: (action: ProductSetupAction) => void | Promise<void>;
    onBack: () => void;
  } = $props();

  const workerDiagnostics = $derived(parseWorkerCapabilities(snapshot.workerStatus));
  const helperDiagnosticMessage = $derived(
    sanitizeDiagnosticText(snapshot.helper?.message, "Refresh status to check the local worker."),
  );
  const commandErrors = $derived(runtimeApi.getCommandErrors());
  let audioQuality = $state<AudioQualityReport | null>(null);
  let audioQualityLoading = $state(true);

  $effect(() => {
    const revision = refreshRevision;
    let active = true;
    audioQuality = null;
    audioQualityLoading = true;
    void runtimeApi.getAudioQuality().then((report) => {
      if (active && revision === refreshRevision) audioQuality = report;
    }).catch(() => {
      if (active && revision === refreshRevision) audioQuality = null;
    }).finally(() => {
      if (active && revision === refreshRevision) audioQualityLoading = false;
    });
    return () => { active = false; };
  });
</script>

<section class="grid min-w-0 gap-4">
  <header class="ti-page-header flex-wrap">
    <div>
      <h3 class="m-0 text-xl font-semibold tracking-[-0.02em]">Diagnostics</h3>
      <p class="mb-0 mt-1.5 text-[13px] text-[var(--ti-text-muted)]">Check current status and retry only what needs attention. Technical details are available below.</p>
    </div>
    <button type="button" class="ti-button ti-button-secondary" onclick={onBack}><ArrowLeft size={15} /> Back</button>
  </header>

  <article class="ti-panel min-w-0 p-5" aria-label="Current worker status">
    <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 2xl:grid-cols-3">
      <div class="ti-state-card min-w-0"><span class="ti-field-label">Worker</span><strong class="mt-2 block text-[13px]">{snapshot.helper?.state ?? "Not checked"}</strong></div>
      <div class="ti-state-card min-w-0"><span class="ti-field-label">Outbound provider</span><strong class="mt-2 block text-[13px]">{!snapshot.helper || snapshot.helper.state === "unavailable" ? "Not verified" : snapshot.helper.provider_ready ? "Available" : "Setup Needed"}</strong></div>
      <div class="ti-state-card min-w-0"><span class="ti-field-label">Execution device</span><strong class="mt-2 block text-[13px]">{snapshot.helper?.cuda_ready ? "CUDA" : snapshot.helper?.degraded_mode ? "CPU / degraded" : "Not verified"}</strong></div>
    </div>

    <p class="mb-0 mt-4 text-[13px] leading-5 text-[var(--ti-text-muted)]">{helperDiagnosticMessage}</p>

    <div class="ti-action-row mt-4" role="group" aria-label="Diagnostics recovery actions">
      <button type="button" class="ti-button ti-button-secondary" aria-busy={setupBusy || diagnosticsLoading} disabled={setupBusy || diagnosticsLoading} onclick={() => void onRefreshDiagnostics()}><RefreshCw size={15} /> {diagnosticsLoading || setupBusy ? "Refreshing..." : "Check Again"}</button>
      <button type="button" class="ti-button ti-button-secondary" disabled={setupBusy || diagnosticsLoading} onclick={() => void onSetupAction("verify-models")}><Bug size={15} /> Check AI Files</button>
      {#if snapshot.helper?.state === "blocked"}
        <button type="button" class="ti-button ti-button-secondary" disabled={setupBusy || diagnosticsLoading || snapshot.meetingSession?.has_session || snapshot.voiceBuildActive} onclick={() => void onSetupAction("retry-worker")}><RefreshCw size={15} /> Retry Worker</button>
      {/if}
    </div>
  </article>

  <RuntimeReliabilityDiagnostics {refreshRevision} />
  <article class="ti-panel p-5" aria-busy={audioQualityLoading}>
    <span class="ti-field-label">Microphone quality</span>
    <strong class="mt-2 block text-[13px]">{audioQualityLoading ? "Checking..." : audioQuality?.label ?? "Not verified"}</strong>
    <p class="mb-0 mt-2 break-words text-[13px] leading-5 text-[var(--ti-text-muted)]"
      role="status">{audioQualityLoading ? "Reading the current microphone buffer." : sanitizeDiagnosticText(audioQuality?.note, "Microphone quality has not been verified. Select Check Again above to retry.")}</p>
    <p class="mb-0 mt-2 text-[13px] leading-5 text-[var(--ti-text-soft)]">Uses existing Mic Test or Meeting capture. Checking quality does not start the microphone.</p>
  </article>

  <article class="ti-panel p-5">
    <div class="flex flex-wrap items-end justify-between gap-4">
      <div>
        <span class="ti-field-label">Troubleshooting</span>
        <h4 class="mb-0 mt-1.5 text-[14px] font-semibold">Recent technical errors</h4>
      </div>
      <span class="ti-pill">{commandErrors.length} recent</span>
    </div>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need focus to scroll this named region.) -->
    <div class="mt-4 grid max-h-[320px] gap-2 overflow-y-auto pr-1" role="region" aria-label="Recent technical errors" tabindex={commandErrors.length > 0 ? 0 : undefined}>
      {#if commandErrors.length === 0}
        <p class="m-0 text-[13px] text-[var(--ti-text-muted)]">No recent technical errors.</p>
      {:else}
        {#each commandErrors as error (`${error.occurred_at}-${error.command}`)}
          <div class="ti-subtle-card min-w-0 px-4 py-3">
            <strong class="block break-all text-[13px] font-semibold">{error.command}</strong>
            <p class="mb-0 mt-1 break-words text-[13px] leading-5 text-[var(--ti-text-muted)]">{error.message}</p>
          </div>
        {/each}
      {/if}
    </div>
  </article>
  <details class="ti-panel min-w-0" aria-label="Local AI technical details">
    <summary class="min-h-11 cursor-pointer px-5 py-4 text-[14px] font-semibold hover:bg-[var(--ti-surface-soft)]">
      Local AI technical details
      <span class="mt-1 block text-[13px] font-normal leading-5 text-[var(--ti-text-muted)]">ASR, translation, Meeting voice and memory details</span>
    </summary>
  <div class="border-t border-[var(--ti-border)] p-5">
    <div>
      <span class="ti-field-label">Technical components</span>
      <h4 class="mb-0 mt-1.5 text-[14px] font-semibold">Local AI status</h4>
    </div>
    <div class="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-2 2xl:grid-cols-3">
      <div class="ti-state-card min-w-0">
        <span class="ti-field-label">ASR</span>
        <strong class="mt-2 block break-words text-[13px] leading-5">{workerDiagnostics.asrDisplay}</strong>
      </div>
      <div class="ti-state-card min-w-0">
        <span class="ti-field-label">Translation</span>
        <strong class="mt-2 block break-words text-[13px] leading-5">{workerDiagnostics.translationDisplay}</strong>
      </div>
      <div class="ti-state-card min-w-0">
        <span class="ti-field-label">Meeting voice</span>
        <strong class="mt-2 block break-words text-[13px] leading-5">{workerDiagnostics.voiceDisplay}</strong>
      </div>
    </div>
    <p class="mb-0 mt-3 text-[13px] leading-5 text-[var(--ti-text-soft)]">Processing mode: {workerDiagnostics.executionDisplay}. This information is only needed for troubleshooting.</p>
    <div class="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2">
      <div class="ti-state-card min-w-0">
        <span class="ti-field-label">PyTorch CUDA allocator</span>
        <strong class="mt-2 block break-words text-[13px] leading-5">{workerDiagnostics.torchMemoryDisplay}</strong>
      </div>
      <div class="ti-state-card min-w-0">
        <span class="ti-field-label">CUDA device memory</span>
        <strong class="mt-2 block break-words text-[13px] leading-5">{workerDiagnostics.deviceMemoryDisplay}</strong>
      </div>
    </div>
    <p class="mb-0 mt-3 text-[13px] leading-5 text-[var(--ti-text-soft)]">
      Read-only snapshot from Check Again. PyTorch values are current allocator usage and lifetime peaks; device free/total is not attributable to a model. Neither value measures CTranslate2-only memory, transient whole-device peaks, or Meeting stability.
    </p>
  </div>

  </details>

  <details class="min-w-0">
    <summary class="min-h-11 cursor-pointer rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-4 py-3 text-[14px] font-semibold hover:bg-[var(--ti-surface-hover)]">
      Codex Gateway details
      <span class="mt-1 block text-[13px] font-normal leading-5 text-[var(--ti-text-muted)]">Account connection and technical gateway checks</span>
    </summary>
    <div class="mt-3">
      <CodexGatewayDiagnostics />
    </div>
  </details>
  <details class="min-w-0">
    <summary class="min-h-11 cursor-pointer rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-4 py-3 text-[14px] font-semibold hover:bg-[var(--ti-surface-hover)]">
      Meeting performance details
      <span class="mt-1 block text-[13px] font-normal leading-5 text-[var(--ti-text-muted)]">Detailed timing metrics from recorded Meeting activity</span>
    </summary>
    <div class="mt-3">
      <MeetingPerformanceDiagnostics status={snapshot.meetingSession} />
    </div>
  </details>

</section>
