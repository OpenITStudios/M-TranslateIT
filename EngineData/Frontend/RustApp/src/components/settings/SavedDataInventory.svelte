<script lang="ts">
  import { onMount } from "svelte";
  import { runtimeApi, type StorageInventory } from "../../app/bridge/runtimeApi";

  let report = $state<StorageInventory | null>(null);
  let loading = $state(false);
  let mounted = true;

  async function refresh(): Promise<void> {
    if (loading) return;
    loading = true;
    try {
      const result = await runtimeApi.getStorageInventory();
      if (mounted) report = result;
    } catch {
      if (mounted) report = { ok: false, categories: [], message: "Storage inventory could not be verified." };
    } finally {
      if (mounted) loading = false;
    }
  }

  function sizeLabel(bytes: number | null): string {
    if (bytes === null || !Number.isFinite(bytes) || bytes < 0) return "Unknown";
    if (bytes < 1024) return `${bytes} B`;
    const units = ["KB", "MB", "GB", "TB"];
    let value = bytes / 1024;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit++; }
    return `${value.toFixed(value >= 10 ? 1 : 2)} ${units[unit]}`;
  }

  onMount(() => {
    mounted = true;
    void refresh();
    return () => { mounted = false; };
  });
</script>

<section class="ti-panel mt-4 overflow-hidden" aria-label="Saved Data storage inventory">
  <header class="flex flex-wrap items-start justify-between gap-3 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
    <div>
      <h3 class="m-0 text-[16px] font-semibold">Saved Data</h3>
      <p class="mb-0 mt-1 text-[13px] leading-5 text-[var(--ti-text-muted)]">Read-only storage overview. No files are changed or deleted.</p>
    </div>
    <button type="button" class="ti-button ti-button-secondary min-h-9 px-3 text-[12px]" disabled={loading} onclick={() => void refresh()}>
      {loading ? "Checking..." : "Refresh storage"}
    </button>
  </header>
  {#if report?.categories.length}
    <dl class="divide-y divide-[var(--ti-border)]">
      {#each report.categories as category (category.name)}
        <div class="flex flex-wrap items-center justify-between gap-3 px-5 py-4">
          <div class="min-w-0">
            <dt class="text-[13px] font-semibold">{category.name}</dt>
            <dd class="m-0 mt-1 text-[12px] text-[var(--ti-text-muted)]">
              {category.complete ? `${category.files} files` : "Scan incomplete"}
              {#if category.note} · {category.note}{/if}
            </dd>
          </div>
          <span class="text-[14px] font-semibold tabular-nums" aria-label={`${category.name} size ${sizeLabel(category.bytes)}`}>
            {sizeLabel(category.bytes)}
          </span>
        </div>
      {/each}
    </dl>
  {:else}
    <p class="m-0 px-5 py-4 text-[13px] text-[var(--ti-text-muted)]" role="status">
      {loading ? "Checking storage..." : report?.message ?? "Storage information is not yet available."}
    </p>
  {/if}
  {#if report}
    <p class="m-0 border-t border-[var(--ti-border)] px-5 py-3 text-[12px] text-[var(--ti-text-muted)]" role="status">{report.message}</p>
  {/if}
</section>
