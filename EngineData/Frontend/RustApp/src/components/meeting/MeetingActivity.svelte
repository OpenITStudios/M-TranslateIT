<script lang="ts">
  import { onMount, tick } from "svelte";

  import type { MeetingCommittedTurn, MeetingCommittedTurnsSnapshot, MeetingSessionStatus } from "../../app/bridge/runtimeApi";
  import { mapProductMeetingState } from "../../app/bridge/productMeetingState";
  import { languageName } from "../../app/shared/state";
  import { meetingSpeakerLabel, meetingTime, meetingTranslationDisplay } from "../../app/runtime/meetingTranscriptPresentation";
  import MeetingTurnBubble from "./MeetingTurnBubble.svelte";
  import {
    DEFAULT_TRANSCRIPT_PREFERENCES, readTranscriptPreferences, subscribeTranscriptPreferences,
    updateTranscriptPreferences, type TranscriptPreferences, type TranscriptTextMode,
  } from "./transcriptPreferences";

  let {
    status,
    turns,
    previewReadOnly = false,
  }: {
    status: MeetingSessionStatus;
    turns: MeetingCommittedTurnsSnapshot | null;
    previewReadOnly?: boolean;
  } = $props();

  let copiedSequence = $state<number | null>(null);
  let copyFailedSequence = $state<number | null>(null);
  let followingLatest = $state(true);
  let transcriptScroller = $state<HTMLDivElement>();
  let appearance = $state<TranscriptPreferences>({ ...DEFAULT_TRANSCRIPT_PREFERENCES });
  let displayError = $state("");

  onMount(() => {
    // Browser preview fixtures stay deterministic and never read saved preferences.
    if (previewReadOnly) return;
    appearance = readTranscriptPreferences();
    return subscribeTranscriptPreferences((next) => { appearance = next; });
  });

  const meeting = $derived(mapProductMeetingState(status));
  const orderedTurns = $derived(
    turns?.ok && turns.has_session && turns.session_id === status.session_id
      ? [...turns.turns].sort((a, b) => a.sequence - b.sequence)
      : [],
  );
  // Existing producer counters aggregate You + incoming Meeting Sound, not speakers.
  const overflowDropped = $derived(status.outbound.overflow_dropped_utterance_count ?? 0);
  const pendingEvicted = $derived(status.outbound.evicted_pending_utterance_count ?? 0);
  const missingSpeech = $derived(overflowDropped > 0 || pendingEvicted > 0);
  // Disk archive loss is distinct from microphone/incoming speech loss.
  const archiveErrors = $derived(
    turns?.ok && turns.has_session && turns.session_id === status.session_id
      ? turns.archive_write_errors ?? 0
      : 0,
  );

  const activity = $derived.by((): { label: string; detail: string; warning: boolean } => {
    if (meeting.busy) return { label: meeting.label, detail: "", warning: false };
    if (status.outbound.output_active) {
      return { label: "Speaking", detail: "", warning: false };
    }
    if (status.outbound.stage === "attention_needed") {
      return {
        label: "Needs attention",
        detail: "The last phrase could not be completed. Try speaking again.",
        warning: true,
      };
    }
    if (["transcribing", "translating", "synthesizing", "queued"].includes(status.outbound.stage)) {
      return { label: "Translating", detail: "", warning: false };
    }
    if (["listening", "idle"].includes(status.outbound.stage)) {
      return { label: "Listening", detail: "", warning: false };
    }
    return { label: "Audio status unavailable", detail: "Check Meeting status.", warning: true };
  });

  const latestSequence = $derived(orderedTurns[orderedTurns.length - 1]?.sequence ?? 0);
  $effect(() => {
    const sessionId = status.session_id;
    const sequence = latestSequence;
    const autoFollow = appearance.followLatest;
    const atLatest = followingLatest;
    if (!sequence) return;
    void tick().then(() => {
      if (status.session_id !== sessionId || !transcriptScroller) return;
      if (autoFollow && atLatest && followingLatest) {
        transcriptScroller.scrollTop = transcriptScroller.scrollHeight;
      } else {
        updateScrollPosition();
      }
    });
  });

  function changeDisplay(mode: TranscriptTextMode): void {
    if (previewReadOnly || mode === appearance.textMode) return;
    const result = updateTranscriptPreferences({ textMode: mode });
    appearance = result.preferences;
    displayError = result.ok ? "" : "Display preference could not be saved; the previous selection was kept.";
  }

  function updateScrollPosition(): void {
    if (!transcriptScroller) return;
    const distance = transcriptScroller.scrollHeight - transcriptScroller.scrollTop - transcriptScroller.clientHeight;
    followingLatest = distance < 72;
  }

  function jumpToLatest(): void {
    followingLatest = true;
    void tick().then(() => {
      if (transcriptScroller) transcriptScroller.scrollTop = transcriptScroller.scrollHeight;
    });
  }

  function deliveryLabel(turn: MeetingCommittedTurn): string {
    if (turn.translation_state === "failed") return "Translation unavailable";
    if (turn.translation_state === "pending") return "Translating";
    switch (turn.delivery_state) {
      case "speaking": return "Playing locally";
      case "output_complete": return "Local playback complete";
      case "output_failed": return "Not played";
      case "interrupted": return "Interrupted";
      default: return "Preparing";
    }
  }

  async function copyTurn(turn: MeetingCommittedTurn): Promise<void> {
    const value = `${meetingSpeakerLabel(turn.lane)} · ${meetingTime(turn.created_unix_ms)}\n${languageName(turn.source_language)}: ${turn.source_text}\n${languageName(turn.target_language)}: ${meetingTranslationDisplay(turn)}`;
    copyFailedSequence = null;
    try {
      if (!navigator.clipboard?.writeText) throw new Error("Clipboard unavailable");
      await navigator.clipboard.writeText(value);
      copiedSequence = turn.sequence;
      window.setTimeout(() => {
        if (copiedSequence === turn.sequence) copiedSequence = null;
      }, 1400);
    } catch {
      copiedSequence = null;
      copyFailedSequence = turn.sequence;
    }
  }
</script>

<section class="flex h-full min-h-0 flex-col gap-3">
  {#if activity.warning}
    <p class="m-0 text-sm leading-6 text-[var(--ti-warning)]">{activity.detail}</p>
  {/if}
  {#if status.incoming.degraded}
    <p class="m-0 text-sm leading-6 text-[var(--ti-warning)]">Incoming captions are unavailable. Your voice translation can continue.</p>
  {/if}
  {#if archiveErrors > 0}
    <p class="m-0 rounded-[var(--ti-radius-sm)] border border-[var(--ti-warning-border)] px-3 py-2 text-sm leading-6 text-[var(--ti-warning)]"
      role="alert">
      Saved Transcript storage reported at least {archiveErrors} failed write or queue admission event{archiveErrors === 1 ? "" : "s"}.
      Some recognized entries may be missing from the saved archive. Meeting audio and translation can continue.
      Review Saved Transcripts after stopping. This does not measure missed speech recognition.
    </p>
  {/if}
  {#if missingSpeech}
    <p class="m-0 rounded-[var(--ti-radius-sm)] border border-[var(--ti-warning-border)] px-3 py-2 text-sm leading-6 text-[var(--ti-warning)]"
      role="status" aria-live="polite">
      Some meeting speech was not processed:
      {#if overflowDropped > 0}{overflowDropped} overlong speech segment{overflowDropped === 1 ? "" : "s"} skipped{/if}{#if overflowDropped > 0 && pendingEvicted > 0}; {/if}{#if pendingEvicted > 0}{pendingEvicted} pending speech segment{pendingEvicted === 1 ? "" : "s"} evicted when processing fell behind{/if}.
      These counts combine your microphone and incoming meeting audio; they do not identify a speaker.
      Important details may need repeating.
    </p>
  {/if}

  <section class="flex min-h-0 flex-1 flex-col border-t border-[var(--ti-border)] pt-3" aria-label="Translated conversation">
    <header class="flex shrink-0 flex-wrap items-center justify-between gap-3 pb-3">
      <div class="flex min-w-0 items-center gap-3">
        <h3 class="m-0 text-base font-semibold">Transcript</h3>
        {#if orderedTurns.length > 0}
          <span class="text-[12px] text-[var(--ti-text-muted)]">{orderedTurns.length} phrase{orderedTurns.length === 1 ? "" : "s"}</span>
        {/if}
        {#if activity.label !== "Listening" && !activity.warning}
          <span class="text-[12px] text-[var(--ti-text-muted)]" role="status">{activity.label}</span>
        {/if}
      </div>
      <div class="flex shrink-0 items-center gap-2">
        <label for="ti-transcript-display" class="text-[12px] text-[var(--ti-text-muted)]">Display</label>
        <select id="ti-transcript-display" class="min-h-9 rounded-[var(--ti-radius-sm)] border border-[var(--ti-border)] bg-[var(--ti-surface-raised)] px-2.5 text-[12px] font-medium text-[var(--ti-text)]"
          aria-label="Transcript display mode"
          title="Show both original and translation, or translated result only"
          value={appearance.textMode} disabled={previewReadOnly}
          onchange={(event) => changeDisplay(event.currentTarget.value as TranscriptTextMode)}>
          <option value="both">Both</option>
          <option value="translation-only">Translation</option>
        </select>
      </div>
    </header>
    {#if displayError}
      <p class="m-0 pb-2 text-[12px] text-[var(--ti-warning)]" role="alert">{displayError}</p>
    {/if}

    {#if !turns || !turns.ok || !turns.has_session || turns.session_id !== status.session_id}
      <p class="m-0 py-6 text-sm leading-6 text-[var(--ti-text-muted)]">Conversation is temporarily unavailable. You can still stop translation.</p>
    {:else}
      {#if turns.truncated || turns.dropped_turn_count > 0}
        <p class="m-0 border-b border-[var(--ti-warning-border)] py-3 text-sm leading-5 text-[var(--ti-warning)]">
          {turns.dropped_turn_count} earlier phrases are no longer in the Live memory buffer. Saved Transcripts may still contain them if local storage succeeded.
        </p>
      {/if}
      {#if orderedTurns.length === 0}
        <p class="m-0 py-7 text-sm leading-6 text-[var(--ti-text-muted)]">Recognized phrases from your microphone and meeting audio will appear here.</p>
      {:else}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need focus to scroll this named region.) -->
        <div bind:this={transcriptScroller} onscroll={updateScrollPosition}
          class="min-h-0 flex-1 overflow-y-auto pr-1" role="log" aria-label="Meeting conversation" aria-live="off" tabindex="0">
          {#each orderedTurns as turn, index (turn.sequence)}
            <div class={"ti-chat-message last:pb-0 " +
              (appearance.density === "compact" ? "pb-1 " : "pb-2 ") +
              (index > 0 && orderedTurns[index - 1]?.lane !== turn.lane
                ? appearance.density === "compact" ? "pt-2" : "pt-3" : "pt-1")}>
              <MeetingTurnBubble {turn} preferences={appearance}
                statusLabel={turn.lane === "you" ? deliveryLabel(turn) : ""}
                onCopy={() => void copyTurn(turn)} copied={copiedSequence === turn.sequence} />
              {#if copyFailedSequence === turn.sequence}
                <p class="m-0 mt-1 text-[13px] text-[var(--ti-warning)]" role="alert">Couldn't copy this phrase. Try again.</p>
              {/if}
            </div>
          {/each}
        </div>
        {#if !followingLatest && orderedTurns.length > 0}
          <div class="flex justify-end pt-3">
            <button type="button" class="ti-button ti-button-secondary min-h-10"
              onclick={jumpToLatest}>Jump to latest</button>
          </div>
        {/if}
      {/if}
    {/if}
  </section>
</section>
