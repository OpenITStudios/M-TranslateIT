<script lang="ts">
  import { onMount, tick } from "svelte";
  import { Check, Copy, Download, Eye, MoreHorizontal, RefreshCw, AlertTriangle, Search, ExternalLink, FolderOpen } from "@lucide/svelte";
  import { runtimeApi, type MeetingCommittedTurnsSnapshot, type MeetingLibrarySession } from "../../app/bridge/runtimeApi";
  import { filterMeetingTurns, formatMeetingTranscript } from "../../app/runtime/meetingTranscriptPresentation";
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
  const visibleTurns = $derived(filterMeetingTurns(snapshot?.turns ?? [], query));
  const archiveNeedsReview = $derived(!session.complete || session.storage_warning);
  const previewLimited = $derived(Boolean(snapshot && session.entry_count > snapshot.turns.length));
  let appearance = $state<TranscriptPreferences>({ ...DEFAULT_TRANSCRIPT_PREFERENCES });
  let readRevision = 0;

  function closeActions(restoreFocus = false): void {
    if (!actionsOpen) return;
    actionsOpen = false;
    actionsPosition = "visibility:hidden";
    if (restoreFocus) {
      const id = session.session_id;
      const trigger = actionsTrigger;
      void tick().then(() => {
        if (session.session_id === id && !actionsOpen && trigger?.isConnected) trigger.focus({ preventScroll: true });
      });
    }
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
    function closeForViewportChange(): void {
      closeActions(Boolean(actionsPanel?.contains(document.activeElement)));
    }
    function onScroll(event: Event): void {
      if (event.target instanceof Node && actionsPanel?.contains(event.target)) return;
      closeForViewportChange();
    }
    function onResize(): void { closeForViewportChange(); }
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
    return unsubscribe;
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
    return () => {
      readRevision++;
      exportRevision++;
      htmlPreviewRevision++;
    };
  });
  async function load(id: string): Promise<void> {
    if (previewReadOnly) return;
    const revision = ++readRevision;
    loading = true;
    copied = false;
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
    if (previewBusy || previewReadOnly || session.entry_count === 0) return;
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
    if (exporting || previewReadOnly || session.entry_count === 0) return;
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
    if (previewReadOnly || !visibleTurns.length) return;
    const id = session.session_id;
    const revision = readRevision;
    try {
      await navigator.clipboard.writeText(formatMeetingTranscript(visibleTurns));
      if (revision !== readRevision || session.session_id !== id) return;
      copied = true;
      onNotice("Matching loaded phrases copied with original and translated text.");
    } catch {
      if (revision === readRevision && session.session_id === id) onNotice("Could not copy saved Meeting entries.");
    }
  }

  async function openExport(folder: boolean): Promise<void> {
    if (!exportResult || previewReadOnly) return;
    const id = session.session_id;
    const revision = exportRevision;
    const fileName = exportResult.path.split("/").pop();
    if (!fileName) return;
    try {
      const result = await runtimeApi.openSavedMeetingHtml(id, fileName, folder);
      if (revision !== exportRevision || session.session_id !== id) return;
      if (!result.ok) onNotice(result.message);
    } catch {
      if (revision === exportRevision && session.session_id === id) onNotice("Could not open the exported HTML location.");
    }
  }

  async function copyExportPath(): Promise<void> {
    if (!exportResult || previewReadOnly) return;
    const id = session.session_id;
    const revision = exportRevision;
    try {
      await navigator.clipboard.writeText(exportResult.path);
      if (revision === exportRevision && session.session_id === id) onNotice("Transcript file location copied.");
    } catch {
      if (revision === exportRevision && session.session_id === id) onNotice("Could not copy the file location.");
    }
  }
</script>

<section class="min-h-0 flex h-full flex-col" aria-label="Saved Meeting channel">
  <header class="shrink-0 border-b border-[var(--ti-border)] px-5 py-3 sm:px-7">
    <div class="flex min-w-0 flex-wrap items-center justify-between gap-3" role="group" aria-label="Saved conversation tools">
      <div class="min-w-0">
        <h2 class="m-0 text-base font-semibold">Transcript</h2>
        <p class="mb-0 mt-1 text-[12px] leading-5 text-[var(--ti-text-muted)]">
          {session.entry_count} saved phrase{session.entry_count === 1 ? "" : "s"} · {archiveNeedsReview ? "Archive needs review" : "Finalized"} · Stored locally
        </p>
      </div>
      <div class="flex min-w-0 max-w-full flex-wrap items-center justify-end gap-2">
        <label class="relative min-w-0 flex-1 sm:min-w-[190px] sm:max-w-[320px]">
          <Search size={16} class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[var(--ti-text-soft)]" aria-hidden="true"/>
          <input class="ti-field min-h-9 w-full pl-10 pr-3 text-[13px]" bind:value={query} aria-label="Search loaded phrases in this Meeting" placeholder="Search loaded phrases..." title="Search the loaded preview, not the entire archive"/>
        </label>
        <button type="button" class="ti-button ti-button-secondary min-h-9 shrink-0 px-3 text-[12px]"
          disabled={exporting || previewReadOnly || session.entry_count === 0} onclick={() => void exportHtml()}
          aria-label="Export complete saved Meeting as HTML" title="Export all available archived entries without deleting the archive">
          <Download size={16} aria-hidden="true"/> {exporting ? "Exporting..." : "Export HTML"}
        </button>
        <div class="relative shrink-0">
          <button type="button" bind:this={actionsTrigger} class="ti-sidebar-icon-button"
            aria-label="More conversation actions" aria-controls={actionsOpen ? "ti-saved-conversation-actions" : undefined} aria-expanded={actionsOpen}
            title="More conversation actions" onclick={() => void toggleActions()}>
            <MoreHorizontal size={18} aria-hidden="true"/>
          </button>
          {#if actionsOpen}
            <div bind:this={actionsPanel} id="ti-saved-conversation-actions" class="ti-library-menu"
              style={actionsPosition} role="group" aria-label="Saved conversation actions">
              <button type="button" disabled={loading || previewReadOnly}
                onclick={() => { closeActions(true); void load(session.session_id); }} aria-label="Refresh saved Meeting">
                <RefreshCw size={14} aria-hidden="true"/> Refresh
              </button>
              <button type="button" disabled={previewReadOnly || !visibleTurns.length}
                onclick={() => { closeActions(true); void copyAll(); }} title="Copy original and translated text for matching loaded phrases">
                {#if copied}<Check size={14} aria-hidden="true"/>{:else}<Copy size={14} aria-hidden="true"/>{/if} Copy displayed
              </button>
              <button type="button" disabled={previewBusy || previewReadOnly || session.entry_count === 0}
                onclick={() => { closeActions(true); void toggleHtmlPreview(); }} aria-label="Preview exported HTML document">
                <Eye size={14} aria-hidden="true"/> {previewBusy ? "Loading..." : htmlPreview ? "Hide preview" : "Preview HTML"}
              </button>
            </div>
          {/if}
        </div>
      </div>
    </div>
    {#if snapshot}
      <p class="mb-0 mt-2 text-[12px] leading-5 text-[var(--ti-text-muted)]" role="status">
        {#if query.trim()}{visibleTurns.length} of {snapshot.turns.length} loaded phrases match. {:else}{snapshot.turns.length} phrases loaded. {/if}
        {#if previewLimited}The preview is partial. {/if}Search and copy use the loaded preview (up to 240 phrases). Export HTML includes all available archived entries.
      </p>
    {/if}
  </header>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need focus to scroll this named region.) -->
  <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4 sm:px-7" aria-label="Saved Meeting transcript" role="region" tabindex="0">
    {#if archiveNeedsReview}
      <p class="mb-4 mt-0 flex items-start gap-2 text-[12px] leading-5 text-[var(--ti-warning)]" role="status">
        <AlertTriangle size={15} class="mt-0.5 shrink-0" aria-hidden="true"/> Archive finalization is unverified. Some recognized entries may be missing.
      </p>
    {/if}
    {#if htmlPreview}
      <section class="mb-5 grid gap-2 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-3" aria-label="HTML export document preview">
        <p class="m-0 text-[12px] text-[var(--ti-text-muted)]">Preview of {htmlPreview.count} recent archived entries. Export HTML to include all available entries.</p>
        <iframe title="Offline HTML transcript document preview" sandbox=""
          srcdoc={htmlPreview.html} class="h-[min(52vh,480px)] w-full rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-bg)]"></iframe>
      </section>
    {/if}
    {#if exportResult}
      <div class="mb-4 grid gap-2">
        <p class="m-0 break-words text-[12px] text-[var(--ti-text-muted)]" role="status">
          Exported {exportResult.count} entries to {exportResult.path}{exportResult.incomplete ? " · Archive completeness unverified" : ""}
        </p>
        <div class="flex flex-wrap gap-2" role="group" aria-label="Exported transcript file actions">
          <button type="button" class="ti-button ti-button-secondary min-h-9 text-[12px]" onclick={() => void openExport(false)}><ExternalLink size={14}/>Open File</button>
          <button type="button" class="ti-button ti-button-secondary min-h-9 text-[12px]" onclick={() => void openExport(true)}><FolderOpen size={14}/>Open Folder</button>
          <button type="button" class="ti-button ti-button-secondary min-h-9 text-[12px]" onclick={() => void copyExportPath()}><Copy size={14}/>Copy Location</button>
        </div>
      </div>
    {/if}
    {#if loading}
      <p class="text-[13px] text-[var(--ti-text-muted)]" role="status">Loading Meeting...</p>
    {:else if failed}
      <p class="text-[13px] text-[var(--ti-warning)]" role="alert">{failed}</p>
    {:else if snapshot}
      {#if snapshot.turns.length === 0}
        <p class="text-[13px] text-[var(--ti-text-muted)]">{session.entry_count === 0 ? "No recognized phrases in this Meeting." : "No phrases could be loaded from this archive preview."}</p>
      {:else}
        <div class="grid gap-2">
          {#each visibleTurns as turn (turn.sequence)}
            <MeetingTurnBubble {turn} preferences={appearance}/>
          {/each}
        </div>
        {#if visibleTurns.length === 0}
          <p class="py-8 text-center text-[13px] text-[var(--ti-text-muted)]" role="status">No messages match this search.</p>
        {/if}
      {/if}
    {:else}
      <p class="text-[13px] text-[var(--ti-text-muted)]">Select Refresh to check this Meeting.</p>
    {/if}
  </div>
</section>
