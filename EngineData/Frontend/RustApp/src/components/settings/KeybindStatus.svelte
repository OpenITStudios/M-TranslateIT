<script lang="ts">
  import { onMount } from "svelte";
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

<section class="ti-panel overflow-hidden" aria-label="Keybind preferences status">
  <header class="flex flex-wrap items-start justify-between gap-3 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
    <div>
      <h3 class="m-0 text-[16px] font-semibold">Keybinds</h3>
      <p class="mb-0 mt-1 text-[13px] leading-5 text-[var(--ti-text-muted)]">
        Native shortcut registration is not available yet. This page only displays verified saved preferences.
      </p>
    </div>
    <button type="button" class="ti-button ti-button-secondary min-h-9 px-3 text-[12px]"
      disabled={loading} onclick={() => void refresh()}>
      {loading ? "Checking..." : "Refresh status"}
    </button>
  </header>
  {#if report?.ok && report.configuration}
    <dl class="divide-y divide-[var(--ti-border)]">
      {#each report.configuration.assignments as assignment (assignment.action)}
        <div class="flex items-center justify-between gap-4 px-5 py-3">
          <dt class="text-[13px]">{labels[assignment.action] ?? assignment.action}</dt>
          <dd class="m-0 text-[12px] text-[var(--ti-text-muted)]">
            {assignment.chord === null ? "Not assigned" : "Unavailable — registration not verified"}
          </dd>
        </div>
      {/each}
    </dl>
  {:else}
    <p class="m-0 px-5 py-4 text-[13px] text-[var(--ti-text-muted)]" role="status">
      {loading ? "Checking keybind preferences..." : report?.message ?? "No verified keybind status yet."}
    </p>
  {/if}
  <p class="m-0 border-t border-[var(--ti-border)] px-5 py-3 text-[12px] text-[var(--ti-text-muted)]" role="status">
    {report?.message ?? "No shortcuts are registered by this page."}
  </p>
</section>
