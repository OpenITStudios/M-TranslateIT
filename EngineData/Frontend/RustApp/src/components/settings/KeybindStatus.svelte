<script lang="ts">
  import { onMount } from "svelte";
  import { Keyboard, ShieldCheck, RefreshCw, LockKeyhole } from "@lucide/svelte";
  import { runtimeApi, type NativeKeybindSettingsResult } from "../../app/bridge/runtimeApi";

  let report = $state<NativeKeybindSettingsResult | null>(null);
  let loading = $state(false);
  let mounted = true;

  async function refresh(): Promise<void> {
    if (loading) return;
    loading = true;
    try {
      const result = await runtimeApi.loadKeybindSettings();
      if (mounted) report = result;
    } catch {
      if (mounted) report = {
        ok: false, configuration: null,
        message: "Keybind preferences could not be verified.",
      };
    } finally {
      if (mounted) loading = false;
    }
  }

  onMount(() => {
    mounted = true;
    void refresh();
    return () => { mounted = false; };
  });

  const labels: Record<string, string> = {
    "toggle-microphone": "Toggle Microphone",
    "toggle-deafen": "Toggle Deafen",
    "push-to-talk": "Push to Talk",
    "toggle-main-window": "Show / Hide TranslateIT",
    "toggle-floating-captions": "Floating Captions",
    "start-stop-meeting": "Start / Stop Meeting",
  };
</script>

<div class="grid gap-4">
  <section class="ti-panel overflow-hidden" aria-labelledby="keybind-overview-heading">
    <header class="flex flex-wrap items-start justify-between gap-4 border-b border-[var(--ti-border)] px-5 py-5">
      <div class="flex min-w-0 flex-1 items-start gap-3">
        <span class="flex h-10 w-10 shrink-0 items-center justify-center rounded-[var(--ti-radius-md)] bg-[var(--ti-surface-raised)] text-[var(--ti-text-muted)]">
          <Keyboard size={20} strokeWidth={1.8} aria-hidden="true" />
        </span>
        <div class="min-w-0">
          <h3 id="keybind-overview-heading" class="m-0 text-[16px] font-semibold">Keyboard Shortcuts</h3>
          <p class="mb-0 mt-1 max-w-[62ch] text-[13px] leading-5 text-[var(--ti-text-muted)]">
            Review available actions and saved assignments. Recording shortcuts is coming after native support is verified.
          </p>
        </div>
      </div>
      <button type="button" class="ti-button ti-button-secondary inline-flex min-h-10 items-center gap-2 px-3 text-[12px]"
        disabled={loading} aria-label="Refresh saved keyboard shortcut status" onclick={() => void refresh()}>
        <RefreshCw size={15} strokeWidth={1.8} aria-hidden="true" />
        {loading ? "Checking..." : "Refresh"}
      </button>
    </header>

    <div class="flex flex-wrap items-start gap-3 border-b border-[var(--ti-border)] bg-[var(--ti-surface-raised)] px-5 py-3">
      <ShieldCheck size={17} class="mt-0.5 shrink-0 text-[var(--ti-text-muted)]" aria-hidden="true" />
      <div class="min-w-0 flex-1">
        <p class="m-0 text-[13px] font-medium">Shortcut editing is not yet available</p>
        <p class="mb-0 mt-1 text-[12px] leading-5 text-[var(--ti-text-muted)]">
          No keyboard shortcuts are registered by this page. Existing meeting and audio controls remain unchanged.
        </p>
      </div>
    </div>

    {#if report?.ok && report.configuration}
      <div class="px-5 py-3">
        <p class="m-0 text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--ti-text-muted)]">
          Actions <span class="font-normal normal-case tracking-normal">· {report.configuration.assignments.length} available</span>
        </p>
      </div>
      <dl class="divide-y divide-[var(--ti-border)] border-t border-[var(--ti-border)]">
        {#each report.configuration.assignments as assignment (assignment.action)}
          <div class="flex flex-wrap items-center justify-between gap-3 px-5 py-3.5">
            <dt class="min-w-0 flex-1 text-[13px] font-medium">{labels[assignment.action] ?? assignment.action}</dt>
            <dd class="m-0 flex items-center gap-2 text-[12px] text-[var(--ti-text-muted)]">
              <LockKeyhole size={14} aria-hidden="true" />
              <span class="rounded-[var(--ti-radius-sm)] border border-[var(--ti-border)] bg-[var(--ti-surface-raised)] px-3 py-1.5">
                {assignment.chord === null ? "Not assigned" : "Registration unverified"}
              </span>
            </dd>
          </div>
        {/each}
      </dl>
    {:else}
      <div class="px-5 py-6" aria-busy={loading}>
        <p class="m-0 text-[13px] leading-5 text-[var(--ti-text-muted)]" role="status">
          {loading ? "Checking saved keyboard shortcuts..." : report?.message ?? "Checking keyboard shortcut availability..."}
        </p>
      </div>
    {/if}
    <footer class="border-t border-[var(--ti-border)] px-5 py-3">
      <p class="m-0 text-[12px] leading-5 text-[var(--ti-text-muted)]" role="status">
        {report?.message ?? "Saved shortcut preferences have not been verified yet."}
      </p>
    </footer>
  </section>
</div>
