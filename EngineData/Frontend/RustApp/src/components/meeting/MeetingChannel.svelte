<script lang="ts">
  import { onMount, tick } from "svelte";
  import { Check, Copy, Download, Eye, MoreHorizontal, RefreshCw, AlertTriangle, Hash, Search } from "@lucide/svelte";
  import { runtimeApi, type MeetingCommittedTurnsSnapshot, type MeetingLibrarySession } from "../../app/bridge/runtimeApi";
  import { languageName } from "../../app/shared/state";
  import { meetingSpeakerLabel, meetingTime, meetingTranslationDisplay } from "../../app/runtime/meetingTranscriptPresentation";
  import {
    readTranscriptPreferences, subscribeTranscriptPreferences,
    DEFAULT_TRANSCRIPT_PREFERENCES, type TranscriptPreferences,
  } from "./transcriptPreferences";
  import MeetingTurnBubble from "./MeetingTurnBubble.svelte";

  let { session, onNotice, previewReadOnly = false }: {
    session: MeetingLibrarySession;
    onNotice: (message: string) => void;
    previewReadOnly?: boolean;
  } = $props();

  let snapshot = $state<MeetingCommittedTurnsSnapshot | null>(null);
  let loading = $state(false);
  let failed = $state("");
  let copied = $state(false);
  let exporting = $state(false);
  let exportRevision = 0;
  let htmlPreviewRevision = 0;
  let previewBusy = $state(false);
  let actionsOpen = $state(false);
  let actionsPosition = $state("visibility:hidden");
  let actionsTrigger = $state<HTMLButtonElement | null>(null);
  let actionsPanel = $state<HTMLDivElement | null>(null);
  let htmlPreview = $state<{ html: string; count: number } | null>(null);
  let exportResult = $state<{ path: string; count: number; incomplete: boolean } | null>(null);
  let query = $state("");
  const visibleTurns = $derived((snapshot?.turns ?? []).filter(turn => {
    const needle = query.trim().toLowerCase();
    return !needle || `${turn.source_text} ${turn.translated_text}`.toLowerCase().includes(needle);
  }));
  let appearance = $state<TranscriptPreferences>({ ...DEFAULT_TRANSCRIPT_PREFERENCES });
  let readRevision = 0;

  function closeActions(restoreFocus = false): void {
    if (!actionsOpen) return;
    actionsOpen = false;
    actionsPosition = "visibility:hidden";
    if (restoreFocus) void tick().then(() => actionsTrigger?.focus());
  }
  async function toggleActions(): Promise<void> {
    if (actionsOpen) { closeActions(true); return; }
    actionsOpen = true;
    actionsPosition = "visibility:hidden";
    await tick();
    if (!actionsOpen || !actionsTrigger || !actionsPanel) return;
    const rect = actionsTrigger.getBoundingClientRect();
    const height = actionsPanel.getBoundingClientRect().height;
    const width = Math.min(220, Math.max(160, window.innerWidth - 16));
    const usableHeight = Math.max(0, window.innerHeight - 16);
    const shownHeight = Math.min(height, usableHeight);
    const above = rect.top - 8;
    const below = window.innerHeight - rect.bottom - 8;
    const preferAbove = below < shownHeight && above > below;
    const proposedTop = preferAbove ? rect.top - shownHeight - 6 : rect.bottom + 6;
    const top = Math.max(8, Math.min(window.innerHeight - shownHeight - 8, proposedTop));
    const left = Math.max(8, Math.min(window.innerWidth - width - 8, rect.right - width));
    actionsPosition = `top:${Math.round(top)}px;left:${Math.round(left)}px;width:${width}px;max-height:${Math.round(usableHeight)}px;overflow-y:auto`;
    await tick();
    if (actionsOpen) actionsPanel?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }
  onMount(() => {
    function onKey(event: KeyboardEvent): void {
      if (event.key === "Escape" && actionsOpen) {
        event.preventDefault();
        closeActions(true);
      }
    }
    function onPointer(event: PointerEvent): void {
      if (!actionsOpen || !(event.target instanceof Node)) return;
      if (actionsTrigger?.contains(event.target) || actionsPanel?.contains(event.target)) return;
      closeActions();
    }
    function onScroll(): void { closeActions(); }
    function onResize(): void { closeActions(); }
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onPointer, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onPointer, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onResize);
    };
  });
  onMount(() => {
    if (previewReadOnly) return;
    appearance = readTranscriptPreferences();
    const unsubscribe = subscribeTranscriptPreferences((next) => { appearance = next; });
    return () => { readRevision++; unsubscribe(); };
  });
  $effect(() => {
    const id = session.session_id;
    // A search belongs to the selected channel; never carry it into another archive.
    query = "";
    copied = false;
    exportResult = null;
    exportRevision++;
    htmlPreviewRevision++;
    htmlPreview = null;
    previewBusy = false;
    actionsOpen = false;
    actionsPosition = "visibility:hidden";
    exporting = false;
    if (!previewReadOnly) void load(id);
  });
  async function load(id: string): Promise<void> {
    const revision = ++readRevision;
    loading = true;
    failed = "";
    snapshot = null;
    try {
      const result = await runtimeApi.getSavedMeetingTranscriptPreview(id);
      if (revision !== readRevision || session.session_id !== id) return;
      if (!result.ok || result.session_id !== id) {
        failed = result.note || "The saved Meeting could not be verified.";
      } else {
        snapshot = result;
      }
    } catch {
      if (revision === readRevision) failed = "Saved Meeting read failed; the archive was not changed.";
    } finally {
      if (revision === readRevision) loading = false;
    }
  }
  async function toggleHtmlPreview(): Promise<void> {
    if (previewBusy || previewReadOnly) return;
    if (htmlPreview) { htmlPreview = null; htmlPreviewRevision++; return; }
    previewBusy = true;
    const id = session.session_id;
    const revision = ++htmlPreviewRevision;
    try {
      const result = await runtimeApi.previewSavedMeetingTranscriptHtml(id);
      if (revision !== htmlPreviewRevision || session.session_id !== id) return;
      if (!result.ok) {
        onNotice(result.message || "HTML document preview is unavailable.");
        return;
      }
      htmlPreview = { html: result.html, count: result.entry_count };
    } catch {
      if (revision === htmlPreviewRevision && session.session_id === id) {
        onNotice("HTML document preview could not be rendered.");
      }
    } finally {
      if (revision === htmlPreviewRevision) previewBusy = false;
    }
  }
  async function exportHtml(): Promise<void> {
    if (exporting || previewReadOnly) return;
    exporting = true;
    exportResult = null;
    const revision = ++exportRevision;
    const id = session.session_id;
    try {
      const result = await runtimeApi.exportSavedMeetingTranscriptHtml(id);
      if (session.session_id !== id || revision !== exportRevision) return;
      if (!result.ok || !result.file_path) {
        onNotice(result.message || "HTML transcript export could not be verified.");
        return;
      }
      exportResult = { path: result.file_path, count: result.turn_count, incomplete: result.incomplete };
      onNotice(result.incomplete
        ? "HTML exported, but archive completeness could not be verified."
        : "HTML transcript exported.");
    } catch {
      if (session.session_id === id && revision === exportRevision) onNotice("HTML transcript export failed; saved conversation was not changed.");
    } finally {
      if (revision === exportRevision) exporting = false;
    }
  }
  async function copyAll(): Promise<void> {
    if (!snapshot?.turns.length) return;
    try {
      await navigator.clipboard.writeText(snapshot.turns.map(t =>
        `${meetingSpeakerLabel(t.lane)} · ${meetingTime(t.created_unix_ms)}\n${languageName(t.source_language)}: ${t.source_text}\n${languageName(t.target_language)}: ${meetingTranslationDisplay(t)}`
      ).join("\n\n"));
      copied = true;
      onNotice("Displayed transcript entries copied (up to 240).");
    } catch { onNotice("Could not copy saved Meeting entries."); }
  }
</script>

<section class="min-h-0 flex h-full flex-col" aria-label="Saved Meeting channel">
  {#if !session.complete || session.storage_warning}
    <p class="mx-5 mb-0 mt-3 flex items-center gap-2 text-[12px] leading-5 text-[var(--ti-warning)]" role="status">
      <AlertTriangle size={15} class="shrink-0"/> Archive finalization is unverified. Some recognized entries may be missing.
    </p>
  {/if}
  <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4 sm:px-7" aria-label="Saved Meeting transcript">
    <div class="mx-auto w-full max-w-[920px]">
    <div class="mb-6 border-b border-[var(--ti-border)] pb-5">
      <Hash size={27} class="mb-3 text-[var(--ti-text-soft)]" aria-hidden="true"/>
      <h2 class="m-0 truncate text-[22px] font-semibold" title={session.display_name}>{session.display_name}</h2>
      <p class="mb-0 mt-2 text-[13px] leading-5 text-[var(--ti-text-muted)]">Saved conversation · You on the right, Meeting Sound on the left. Read-only transcript.</p>
    </div>
    <div class="mb-4 flex flex-wrap items-center justify-between gap-3 border-b border-[var(--ti-border)] pb-4" role="group" aria-label="Saved conversation tools">
      <span class="text-[12px] text-[var(--ti-text-muted)]">
        {session.entry_count} saved phrase{session.entry_count === 1 ? "" : "s"}
        · {session.complete ? "Finalized" : "Archive needs review"}
      </span>
      <div class="flex min-w-0 flex-1 flex-wrap items-center justify-end gap-2">
        <label class="relative min-w-[190px] flex-1 sm:max-w-[320px]">
          <Search size={16} class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[var(--ti-text-soft)]" aria-hidden="true"/>
          <input class="ti-field min-h-9 w-full pl-10 pr-3 text-[13px]" bind:value={query} aria-label="Search this Meeting transcript" placeholder="Search conversation..."/>
        </label>
        <div class="relative shrink-0">
          <button type="button" bind:this={actionsTrigger} class="ti-button ti-button-secondary flex min-h-9 items-center gap-2 px-3 text-[12px]"
            aria-label="More conversation actions" aria-haspopup="true" aria-expanded={actionsOpen}
            onclick={() => void toggleActions()}>
            <MoreHorizontal size={16} aria-hidden="true"/> More
          </button>
          {#if actionsOpen}
          <div bind:this={actionsPanel} class="fixed z-50 grid gap-1 rounded-[var(--ti-radius-md)] border border-[var(--ti-border-strong)] bg-[var(--ti-sidebar)] p-1 shadow-lg"
            style={actionsPosition} role="group" aria-label="Saved conversation actions">
            <button type="button" class="ti-button ti-button-secondary min-h-9 w-full justify-start px-3 text-[12px]"
              disabled={loading} onclick={() => { actionsOpen = false; void load(session.session_id); }} aria-label="Refresh saved Meeting">
              <RefreshCw size={14} aria-hidden="true"/> Refresh
            </button>
            <button type="button" class="ti-button ti-button-secondary min-h-9 w-full justify-start px-3 text-[12px]"
              disabled={!snapshot?.turns.length} onclick={() => { actionsOpen = false; void copyAll(); }}>
              {#if copied}<Check size={14}/>{:else}<Copy size={14}/>{/if} Copy displayed
            </button>
            <button type="button" class="ti-button ti-button-secondary min-h-9 w-full justify-start px-3 text-[12px]"
              disabled={previewBusy || previewReadOnly} onclick={() => { actionsOpen = false; void toggleHtmlPreview(); }}
              aria-label="Preview exported HTML document">
              <Eye size={14} aria-hidden="true"/> {previewBusy ? "Loading..." : htmlPreview ? "Hide preview" : "Preview HTML"}
            </button>
            <button type="button" class="ti-button ti-button-secondary min-h-9 w-full justify-start px-3 text-[12px]"
              disabled={exporting || previewReadOnly} onclick={() => { actionsOpen = false; void exportHtml(); }}
              aria-label="Export complete saved Meeting as HTML">
              <Download size={14} aria-hidden="true"/> {exporting ? "Exporting..." : "Export HTML"}
            </button>
          </div>
          {/if}
        </div>
      </div>
    </div>
    {#if htmlPreview}
      <section class="mb-5 grid gap-2 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-3" aria-label="HTML export document preview">
        <p class="m-0 text-[12px] text-[var(--ti-text-muted)]">Preview of {htmlPreview.count} recent archived entries. Export HTML to include all available entries.</p>
        <iframe title="Offline HTML transcript document preview" sandbox=""
          srcdoc={htmlPreview.html} class="h-[min(52vh,480px)] w-full rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-bg)]"></iframe>
      </section>
    {/if}
    {#if exportResult}
      <p class="mb-4 break-words text-[12px] text-[var(--ti-text-muted)]" role="status">
        Exported {exportResult.count} entries to {exportResult.path}{exportResult.incomplete ? " · Archive completeness unverified" : ""}
      </p>
    {/if}
    {#if loading}
      <p class="text-[13px] text-[var(--ti-text-muted)]" role="status">Loading Meeting...</p>
    {:else if failed}
      <p class="text-[13px] text-[var(--ti-warning)]" role="alert">{failed}</p>
    {:else if snapshot}
      {#if snapshot.turns.length === 0}
        <p class="text-[13px] text-[var(--ti-text-muted)]">No recognized phrases in this Meeting.</p>
      {:else}
        <div class="grid gap-3">
          {#each visibleTurns as turn (turn.sequence)}
            <MeetingTurnBubble {turn} preferences={appearance}/>
          {/each}
        </div>
        {#if visibleTurns.length === 0}
          <p class="py-8 text-center text-[13px] text-[var(--ti-text-muted)]" role="status">No messages match this search.</p>
        {/if}
        <p class="mt-5 text-center text-[12px] text-[var(--ti-text-soft)]">
          Display shows up to 240 saved phrases. Export HTML above to include all available archived entries.
        </p>
      {/if}
    {:else}
      <p class="text-[13px] text-[var(--ti-text-muted)]">Select Refresh to check this Meeting.</p>
    {/if}
    </div>
  </div>
</section>
