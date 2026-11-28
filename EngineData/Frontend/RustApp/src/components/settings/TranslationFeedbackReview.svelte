<script lang="ts">
  import { ClipboardCopy, Trash2 } from "@lucide/svelte";
  import {
    clearTranslationFeedback,
    readTranslationFeedback,
    type TranslationFeedbackEntry,
  } from "../../app/runtime/translationFeedbackState";

  let { onNotice }: { onNotice: (message: string) => void } = $props();

  let report = $state(readTranslationFeedback());
  const entries = $derived(report.entries);
  let confirmClear = $state(false);

  function categoryLabel(value: TranslationFeedbackEntry["category"]): string {
    switch (value) {
      case "meaning_changed": return "Meaning changed";
      case "wrong_terminology": return "Wrong terminology";
      case "name_number": return "Name / number incorrect";
      case "incomplete": return "Incomplete";
      default: return "Unnatural wording";
    }
  }

  async function copyForReview(): Promise<void> {
    if (entries.length === 0) return;
    try {
      await navigator.clipboard.writeText(JSON.stringify({ version: 1, entries }, null, 2));
      onNotice("Translation feedback copied for quality review.");
    } catch {
      onNotice("Couldn't copy translation feedback.");
    }
  }

  function clearAll(): void {
    if (!clearTranslationFeedback()) {
      onNotice("Translation feedback couldn't be cleared from local storage.");
      return;
    }
    report = { state: "empty", entries: [] };
    confirmClear = false;
    onNotice("Translation feedback cleared.");
  }
</script>

{#if report.state !== "empty"}
<details class="ti-panel mt-4">
  <summary class="min-h-11 cursor-pointer px-5 py-4 text-sm font-medium hover:bg-[var(--ti-surface-soft)]">Past translation issue reports</summary>
  <article class="border-t border-[var(--ti-border)] overflow-hidden">
  <header class="flex flex-wrap items-start justify-between gap-4 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
    <div>
      <h3 class="m-0 text-[15px] font-semibold">Saved reports</h3>
      <p class="mb-0 mt-1 text-[13px] leading-5 text-[var(--ti-text-muted)]">
        New reports are no longer collected. Existing local reports are kept for manual review, copying or explicit clearing.
      </p>
    </div>
    {#if entries.length > 0}<span class="ti-pill">{entries.length} verified</span>{/if}
  </header>

  {#if report.state === "unavailable"}
    <p class="m-0 px-5 py-4 text-[13px] text-[var(--ti-warning)]" role="status">Saved report storage could not be read. No data was changed. Reopen this page to retry.</p>
  {:else if report.state === "attention"}
    <p class="m-0 px-5 py-4 text-[13px] text-[var(--ti-warning)]" role="status">Some stored report data could not be verified. Only the verified entries below can be copied. Original storage has not been changed.</p>
  {/if}
  {#if entries.length > 0}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need focus to scroll this named region.) -->
    <div class="max-h-[260px] divide-y divide-[var(--ti-border)] overflow-y-auto" role="region" aria-label="Saved translation issue reports" tabindex="0">
      {#each entries as entry}
        <div class="grid min-w-0 gap-1.5 px-5 py-3.5">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <strong class="text-[13px] font-semibold">{categoryLabel(entry.category)}</strong>
            <time class="text-[12px] text-[var(--ti-text-muted)]" datetime={new Date(entry.createdAt).toISOString()}>
              {new Date(entry.createdAt).toLocaleString()}
            </time>
          </div>
          <p class="m-0 whitespace-pre-wrap break-words text-[13px] leading-5 text-[var(--ti-text-muted)]">{entry.source}</p>
          <p class="m-0 whitespace-pre-wrap break-words text-[13px] leading-5 text-[var(--ti-text-muted)]">{entry.translation}</p>
        </div>
      {/each}
    </div>
  {/if}
  {#if report.state !== "unavailable"}
    <footer class="flex flex-wrap justify-end gap-2 border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-3.5">
      {#if entries.length > 0}<button type="button" class="ti-button ti-button-secondary min-h-10" onclick={() => void copyForReview()}><ClipboardCopy size={14} /> Copy review data</button>{/if}
      <button type="button" class="ti-button ti-button-secondary min-h-10" onclick={() => { confirmClear = true; }}><Trash2 size={14} /> Clear</button>
    </footer>
    {#if confirmClear}
      <div class="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--ti-border)] px-5 py-4" role="group" aria-label="Confirm clearing translation feedback">
        <p class="m-0 text-[13px] leading-5">Permanently clear all locally stored translation issue reports, including any unverified data?</p>
        <div class="flex flex-wrap gap-2">
          <button type="button" class="ti-button ti-button-secondary min-h-10" onclick={() => { confirmClear = false; }}>Cancel</button>
          <button type="button" class="ti-button ti-button-danger min-h-10" onclick={clearAll}>Clear reports</button>
        </div>
      </div>
    {/if}
  {/if}
  </article>
</details>
{/if}
