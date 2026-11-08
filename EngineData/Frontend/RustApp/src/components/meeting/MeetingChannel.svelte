<script lang="ts">
  import { onMount } from "svelte";
  import { Check, Copy, RefreshCw, AlertTriangle } from "@lucide/svelte";
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
    <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
      <span class="text-[12px] text-[var(--ti-text-muted)]">
        {session.entry_count} saved phrase{session.entry_count === 1 ? "" : "s"}
        · {session.complete ? "Finalized" : "Archive needs review"}
      </span>
      <div class="flex gap-2">
        <button type="button" class="ti-button ti-button-secondary min-h-9 px-3 text-[12px]"
          disabled={loading} onclick={() => void load(session.session_id)} aria-label="Refresh saved Meeting">
          <RefreshCw size={14} aria-hidden="true"/> Refresh
        </button>
        <button type="button" class="ti-button ti-button-secondary min-h-9 px-3 text-[12px]"
          disabled={!snapshot?.turns.length} onclick={() => void copyAll()}>
          {#if copied}<Check size={14}/>{:else}<Copy size={14}/>{/if} Copy
        </button>
      </div>
    </div>
    {#if loading}
      <p class="text-[13px] text-[var(--ti-text-muted)]" role="status">Loading Meeting...</p>
    {:else if failed}
      <p class="text-[13px] text-[var(--ti-warning)]" role="alert">{failed}</p>
    {:else if snapshot}
      {#if snapshot.turns.length === 0}
        <p class="text-[13px] text-[var(--ti-text-muted)]">No recognized phrases in this Meeting.</p>
      {:else}
        <div class="grid gap-3">
          {#each snapshot.turns as turn (turn.sequence)}
            <MeetingTurnBubble {turn} preferences={appearance}/>
          {/each}
        </div>
        <p class="mt-5 text-center text-[12px] text-[var(--ti-text-soft)]">
          Display shows up to 240 saved phrases. Use Saved Meeting to export all available entries.
        </p>
      {/if}
    {:else}
      <p class="text-[13px] text-[var(--ti-text-muted)]">Select Refresh to check this Meeting.</p>
    {/if}
  </div>
</section>
