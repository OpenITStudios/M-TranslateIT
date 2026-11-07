<script lang="ts">
  import { Dialog } from "bits-ui";
  import type { CloseDialogAction } from "../../app/runtime/closePolicy";

  let {
    open = false,
    title,
    message,
    action,
    busy = false,
    primaryLabel,
    onKeepOpen,
    onPrimary,
    onOpenChange,
  }: {
    open?: boolean;
    title: string;
    message: string;
    action: CloseDialogAction;
    busy?: boolean;
    primaryLabel: string;
    onKeepOpen: () => void;
    onPrimary: () => void | Promise<void>;
    onOpenChange?: (open: boolean) => void;
  } = $props();
</script>

<Dialog.Root {open} onOpenChange={(nextOpen) => onOpenChange?.(nextOpen)}>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-50 bg-[var(--ti-overlay)] backdrop-blur-[2px]" />
    <Dialog.Content class="fixed left-1/2 top-1/2 z-50 max-h-[calc(100vh-24px)] w-[min(500px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-[var(--ti-radius-lg)] border border-[var(--ti-border-strong)] bg-[var(--ti-surface)] p-4 shadow-[var(--ti-shadow-dialog)] sm:p-6" aria-busy={busy}>
      <Dialog.Title class="text-xl font-semibold">{title}</Dialog.Title>
      <Dialog.Description class="mt-3 whitespace-pre-wrap break-words text-sm leading-6 text-[var(--ti-text-muted)]">{message}</Dialog.Description>
      {#if busy}
        <p class="mb-0 mt-3 text-[13px] leading-5 text-[var(--ti-text-muted)]" role="status" aria-live="polite">
          {action === "stop" ? "Stopping Meeting and checking that it is safe to close..." : "Checking whether TranslateIT can close safely..."}
        </p>
      {/if}
      <div class="mt-6 flex flex-wrap justify-end gap-3">
        <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={onKeepOpen}>Keep TranslateIT open</button>
        {#if action}
          <button type="button" class={`ti-button ${action === "stop" ? "ti-button-danger" : ""}`} disabled={busy} aria-label={action === "retry" ? "Check if TranslateIT can close safely" : undefined}
            onclick={() => void onPrimary()}>{action === "retry" ? "Check Again" : primaryLabel}</button>
        {/if}
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
