<script lang="ts">
  import { onMount } from "svelte";
  import { Check, Copy, Download, RefreshCw, AlertTriangle, Hash, Search } from "@lucide/svelte";
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
  let exportResult = $state<{ path: string; count: number; incomplete: boolean } | null>(null);
  let query = $state("");
  const visibleTurns = $derived((snapshot?.turns ?? []).filter(turn => {
    const needle = query.trim().toLowerCase();
    return !needle || `${turn.source_text} ${turn.translated_text}`.toLowerCase().includes(needle);
  }));
  let appearance = $state<TranscriptPreferences>({ ...DEFAULT_TRANSCRIPT_PREFERENCES });
  let readRevision = 0;

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
      onNotice("Available transcript entries copied.");
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
        <button type="button" class="ti-button ti-button-secondary min-h-9 px-3 text-[12px]"
          disabled={loading} onclick={() => void load(session.session_id)} aria-label="Refresh saved Meeting">
          <RefreshCw size={14} aria-hidden="true"/> Refresh
        </button>
        <button type="button" class="ti-button ti-button-secondary min-h-9 px-3 text-[12px]"
          disabled={!snapshot?.turns.length} onclick={() => void copyAll()}>
          {#if copied}<Check size={14}/>{:else}<Copy size={14}/>{/if} Copy all
        </button>
        <button type="button" class="ti-button ti-button-secondary min-h-9 px-3 text-[12px]"
          disabled={exporting || previewReadOnly} onclick={() => void exportHtml()}
          aria-label="Export complete saved Meeting as HTML">
          <Download size={14} aria-hidden="true"/> {exporting ? "Exporting..." : "Export HTML"}
        </button>
      </div>
    </div>
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
