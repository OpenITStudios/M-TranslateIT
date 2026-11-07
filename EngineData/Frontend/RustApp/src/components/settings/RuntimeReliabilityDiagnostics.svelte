<script lang="ts">
  import { sanitizeDiagnosticText } from "../../app/shared/diagnosticPrivacy";
  import {
    exportDiagnosticSupportBundle,
    getDeviceLossGuardStatus,
    getLongSessionHealthStatus,
    getRuntimeWatchdogStatus,
    getStartupRecoveryStatus,
    getRecentRuntimeIncidents,
    type DeviceLossGuardStatus,
    type LongSessionHealthStatus,
    type RuntimeIncidentSnapshot,
    type RuntimeWatchdogStatus,
    type StartupRecoveryReport,
  } from "../../app/bridge/reliabilityApi";
  import StatusBadge from "../ui/StatusBadge.svelte";

  let { refreshRevision = 0 }: { refreshRevision?: number } = $props();

  let watchdog = $state<RuntimeWatchdogStatus | null>(null);
  let recovery = $state<StartupRecoveryReport | null>(null);
  let devices = $state<DeviceLossGuardStatus | null>(null);
  let incidents = $state<RuntimeIncidentSnapshot | null>(null);
  let longSession = $state<LongSessionHealthStatus | null>(null);
  let exporting = $state(false);
  let exportMessage = $state("");
  let exportSucceeded = $state(false);
  let loading = $state(true);

  const watchdogLabel = $derived(
    loading ? "Checking" :
      !watchdog || watchdog.state === "unavailable" ? "Not verified" :
      watchdog.state === "idle" ? "Idle" :
      watchdog.healthy ? "Healthy" : "Needs attention",
  );
  const recoveryLabel = $derived(
    loading || !recovery ? "Not checked" :
      recovery.blocker ? "Needs attention" :
      !recovery.cleanup_ok ? "Not verified" :
      recovery.another_instance_detected ? "Another instance detected" :
      recovery.previous_unclean_shutdown ? "Recovered" : "Clean",
  );

  async function refresh(isCurrent: () => boolean): Promise<void> {
    const next = await Promise.all([
      getRuntimeWatchdogStatus().catch(() => null),
      getStartupRecoveryStatus().catch(() => null),
      getDeviceLossGuardStatus().catch(() => null),
      getRecentRuntimeIncidents().catch(() => null),
      getLongSessionHealthStatus().catch(() => null),
    ]);
    if (!isCurrent()) return;
    [watchdog, recovery, devices, incidents, longSession] = next;
    loading = false;
  }

  async function exportSupport(): Promise<void> {
    if (exporting) return;
    exporting = true;
    exportMessage = "";
    exportSucceeded = false;
    try {
      const result = await exportDiagnosticSupportBundle();
      exportSucceeded = result.ok && Boolean(result.file_path?.trim());
      exportMessage = exportSucceeded && result.file_path
        ? `${result.message} Saved to ${result.file_path}`
        : result.ok
          ? "Diagnostic export was acknowledged, but no saved file location was verified. Check the local Diagnostics folder before trying again."
          : `Diagnostic export could not be verified. ${result.message || "Check the local Diagnostics folder before trying again."}`;
    } catch {
      exportMessage = "Diagnostic export failed. Check the local Diagnostics folder and try again.";
    } finally {
      exporting = false;
    }
  }

  $effect(() => {
    const revision = refreshRevision;
    let active = true;
    loading = true;
    watchdog = null;
    recovery = null;
    devices = null;
    incidents = null;
    longSession = null;
    // A disposed or superseded Diagnostics read must not replace newer data.
    void refresh(() => active && revision === refreshRevision);
    return () => { active = false; };
  });
</script>

<article class="ti-panel min-w-0 p-5">
  <div class="flex items-start justify-between gap-4">
    <div>
      <span class="ti-field-label">Reliability</span>
      <h4 class="mb-0 mt-1.5 text-[15px] font-semibold">Runtime health</h4>
    </div>
    <StatusBadge
      label={watchdogLabel}
      tone={loading || !watchdog || watchdog.state === "unavailable" || watchdog.state === "idle"
        ? "neutral" : watchdog.action_required ? "danger" : watchdog.healthy ? "good" : "warning"}
    />
  </div>

  <div class="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-2 2xl:grid-cols-3">
    <div class="ti-state-card min-w-0">
      <span class="ti-field-label">Watchdog</span>
      <strong class="mt-2 block text-[13px]">{loading ? "Checking..." : watchdog?.state ?? "Not checked"}</strong>
      <p class="mb-0 mt-1 text-[13px] leading-5 text-[var(--ti-text-soft)]">{loading ? "Reading watchdog status..." : sanitizeDiagnosticText(watchdog?.note, "Watchdog status has not been verified.")}</p>
    </div>
    <div class="ti-state-card min-w-0">
      <span class="ti-field-label">Devices</span>
      <strong class="mt-2 block text-[13px]">{loading ? "Checking..." : devices?.state ?? "Not checked"}</strong>
      <p class="mb-0 mt-1 text-[13px] leading-5 text-[var(--ti-text-soft)]">{loading ? "Reading device-loss status..." : sanitizeDiagnosticText(devices?.note, "Device-loss status has not been verified.")}</p>
    </div>
    <div class="ti-state-card min-w-0">
      <span class="ti-field-label">Previous shutdown</span>
      <strong class="mt-2 block text-[13px]">{loading ? "Checking..." : recoveryLabel}</strong>
      <p class="mb-0 mt-1 text-[13px] leading-5 text-[var(--ti-text-soft)]">{loading ? "Reading startup recovery status..." : sanitizeDiagnosticText(recovery?.note, "Startup recovery status has not been verified.")}</p>
    </div>
  </div>

  {#if longSession}
    <div class="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-2 2xl:grid-cols-4">
      <div class="ti-state-card min-w-0"><span class="ti-field-label">Session age</span><strong class="mt-1 block text-[13px]">{Math.round(longSession.session_age_ms / 60000)} min</strong></div>
      <div class="ti-state-card min-w-0"><span class="ti-field-label">Queue drops</span><strong class="mt-1 block text-[13px]">{longSession.outbound_overflow_dropped + longSession.outbound_evicted_pending}</strong></div>
      <div class="ti-state-card min-w-0"><span class="ti-field-label">Transcript drops</span><strong class="mt-1 block text-[13px]">{longSession.transcript_dropped_turns}</strong></div>
      <div class="ti-state-card min-w-0"><span class="ti-field-label">Meeting temp files</span><strong class="mt-1 block text-[13px]">{longSession.meeting_temp_file_count}</strong></div>
    </div>
  {/if}

  {#if !loading && incidents}
    <details class="mt-4 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-4">
      <summary class="min-h-9 cursor-pointer text-[13px] font-semibold">
        Recent issue map · {incidents.coverage === "UNAVAILABLE" ? "Not verified" : `${incidents.issue_areas?.length ?? 0} areas`}
      </summary>
      <p class="mb-3 mt-2 text-[12px] leading-5 text-[var(--ti-text-muted)]">
        Retained runtime incident records only. Repeated notifications may be suppressed; this is not a full failure count or a confirmed root-cause report.
        {incidents.coverage === "AT_CAPACITY" ? " The 20-record limit has been reached; earlier issues may be omitted." : ""}
      </p>
      {#if incidents.coverage === "UNAVAILABLE"}
        <p class="m-0 text-[13px] text-[var(--ti-warning)]" role="status">Incident history could not be verified. Check Again or export the current diagnostics.</p>
      {:else if (incidents.issue_areas?.length ?? 0) === 0}
        <p class="m-0 text-[13px] text-[var(--ti-text-muted)]">No incident records in the retained local window; runtime health is checked separately.</p>
      {:else}
        <div class="grid gap-3">
          {#each incidents.issue_areas as area (`${area.category}:${area.component}:${area.build_source_sha ?? "unknown"}`)}
            <section class="ti-subtle-card min-w-0 p-3">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <strong class="break-words text-[13px]">{sanitizeDiagnosticText(area.category)} · {sanitizeDiagnosticText(area.component)}</strong>
                <span class="text-[12px] text-[var(--ti-text-muted)]">
                  {area.retained_records} retained · {area.build_source_sha ? `Build ${area.build_source_sha.slice(0, 12)}` : "Build unknown"}
                </span>
              </div>
              <ul class="mb-0 mt-2 grid gap-1 pl-4 text-[12px] leading-5">
                {#each area.blockers as blocker (blocker)}
                  <li><code class="break-all">{sanitizeDiagnosticText(blocker)}</code></li>
                {/each}
              </ul>
            </section>
          {/each}
        </div>
      {/if}
    </details>
  {/if}

  {#if incidents?.incidents[0]}
    <div class="mt-4 ti-subtle-card px-4 py-3">
      <span class="ti-field-label">Last incident · {incidents.incidents[0].category}</span>
      <strong class="mt-1 block break-words text-[13px] font-semibold">{sanitizeDiagnosticText(incidents.incidents[0].blocker)}</strong>
      <p class="mb-0 mt-1 text-[13px] leading-5 text-[var(--ti-text-soft)]">{sanitizeDiagnosticText(incidents.incidents[0].note)}</p>
    </div>
  {/if}

  <div class="mt-4 flex flex-wrap items-center justify-between gap-4 border-t border-[var(--ti-border)] pt-4">
    <p class="m-0 text-[13px] leading-5 text-[var(--ti-text-soft)]">
      Support export contains redacted runtime metadata only—never transcript, audio, or voice reference.
    </p>
    <button type="button" class="ti-button ti-button-secondary shrink-0" aria-busy={exporting} disabled={exporting} onclick={() => void exportSupport()}>
      {exporting ? "Exporting..." : "Export Diagnostics"}
    </button>
  </div>
  {#if exportMessage}
    <p class={exportSucceeded
        ? "mb-0 mt-3 break-words text-[13px] leading-5 text-[var(--ti-text-muted)]"
        : "mb-0 mt-3 break-words text-[13px] leading-5 text-[var(--ti-warning)]"}
      role={exportSucceeded ? "status" : "alert"}>{exportMessage}</p>
  {/if}
</article>
