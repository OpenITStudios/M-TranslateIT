<script lang="ts">
  type Tone = "neutral" | "good" | "warning" | "danger";

  let {
    label,
    tone = "neutral",
  }: {
    label: string;
    tone?: Tone;
  } = $props();

  const toneClass: Record<Tone, string> = {
    neutral: "border-[var(--ti-border)] bg-[var(--ti-surface-soft)] text-[var(--ti-text-muted)]",
    good: "border-[var(--ti-success-border)] bg-[var(--ti-success-surface)] text-[var(--ti-success)]",
    warning: "border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] text-[var(--ti-warning)]",
    danger: "border-[var(--ti-danger-border)] bg-[var(--ti-danger-surface)] text-[var(--ti-danger)]",
  };
</script>

<span class={`inline-flex min-h-7 max-w-full shrink-0 items-center justify-center rounded-full border px-2.5 py-1 text-center text-[13px] font-semibold leading-5 break-words ${toneClass[tone]}`}>
  {label}
</span>
