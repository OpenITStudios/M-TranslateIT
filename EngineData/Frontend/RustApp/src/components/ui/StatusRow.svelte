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

<div class="flex min-h-[68px] items-center justify-between gap-5 px-5 py-3.5">
  <div class="min-w-0">
    <span class="block text-[12px] font-medium text-[var(--ti-text-muted)]">{label}</span>
    <strong class="mt-1 block truncate text-[14px] font-semibold text-[var(--ti-text)]" title={value}>{value}</strong>
    {#if detail}
      <small class="mt-1 block text-[12.5px] leading-[1.55] text-[var(--ti-text-soft)]">{detail}</small>
    {/if}
  </div>
  {#if status}
    <StatusBadge label={status} {tone} />
  {/if}
</div>
