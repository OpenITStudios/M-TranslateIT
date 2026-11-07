<script lang="ts">
  import StatusBadge from "./StatusBadge.svelte";

  type Tone = "neutral" | "good" | "warning" | "danger";

  let {
    label,
    value,
    detail = "",
    status = "",
    tone = "neutral",
  }: {
    label: string;
    value: string;
    detail?: string;
    status?: string;
    tone?: Tone;
  } = $props();
</script>

<div class="flex min-h-[68px] flex-wrap items-center justify-between gap-x-4 gap-y-2 px-4 py-3.5 sm:px-5">
  <div class="min-w-0 flex-[1_1_180px]">
    <span class="block text-[13px] font-medium text-[var(--ti-text-muted)]">{label}</span>
    <strong class="mt-1 block min-w-0 break-words text-[14px] font-semibold text-[var(--ti-text)]" title={value}>{value}</strong>
    {#if detail}
      <small class="mt-1 block break-words text-[13px] leading-[1.55] text-[var(--ti-text-muted)]">{detail}</small>
    {/if}
  </div>
  {#if status}
    <StatusBadge label={status} {tone} />
  {/if}
</div>
