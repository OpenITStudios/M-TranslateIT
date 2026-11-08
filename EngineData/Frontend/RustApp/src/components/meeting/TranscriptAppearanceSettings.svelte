<script lang="ts">
  import { onMount } from "svelte";
  import {
    DEFAULT_TRANSCRIPT_PREFERENCES,
    readTranscriptPreferences,
    subscribeTranscriptPreferences,
    updateTranscriptPreferences,
    type TranscriptPreferences,
    type TranscriptDensity,
    type TranscriptTextMode,
    type TranscriptTextOrder,
    type TranscriptTimestamp,
  } from "./transcriptPreferences";

  let { onNotice }: { onNotice: (message: string) => void } = $props();

  let preferences = $state<TranscriptPreferences>({ ...DEFAULT_TRANSCRIPT_PREFERENCES });
  let feedback = $state("");

  onMount(() => {
    preferences = readTranscriptPreferences();
    return subscribeTranscriptPreferences((current) => { preferences = current; });
  });

  function update(patch: Partial<TranscriptPreferences>): void {
    const result = updateTranscriptPreferences(patch);
    preferences = result.preferences;
    feedback = result.ok
      ? "Transcript appearance saved on this device."
      : "The preference could not be saved. Your previous settings were kept.";
    onNotice(feedback);
  }

  function reset(): void {
    update({ ...DEFAULT_TRANSCRIPT_PREFERENCES });
  }
</script>

<div class="grid min-w-0 gap-5" aria-label="Transcript appearance preferences">
  <div class="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
    <label class="grid min-w-0 gap-2">
      <span class="ti-field-label">Transcript display</span>
      <select class="ti-field min-h-10 px-3" value={preferences.textMode}
        onchange={(event) => update({ textMode: event.currentTarget.value as TranscriptTextMode })}>
        <option value="both">Original + Translation</option>
        <option value="translation-only">Translation only</option>
      </select>
      <small class="text-[12px] leading-5 text-[var(--ti-text-muted)]">Original remains available when translation fails.</small>
    </label>
    <label class="grid min-w-0 gap-2">
      <span class="ti-field-label">Message density</span>
      <select class="ti-field min-h-10 px-3" value={preferences.density}
        onchange={(event) => update({ density: event.currentTarget.value as TranscriptDensity })}>
        <option value="cozy">Cozy · recommended</option>
        <option value="compact">Compact</option>
      </select>
      <small class="text-[12px] leading-5 text-[var(--ti-text-muted)]">Changes bubble padding and spacing, not text size.</small>
    </label>
    <label class="grid min-w-0 gap-2">
      <span class="ti-field-label">Text order</span>
      <select class="ti-field min-h-10 px-3" value={preferences.textOrder}
        onchange={(event) => update({ textOrder: event.currentTarget.value as TranscriptTextOrder })}>
        <option value="original-first">Original first</option>
        <option value="translation-first">Translation first</option>
      </select>
      <small class="text-[12px] leading-5 text-[var(--ti-text-muted)]">Both languages remain visible in every bubble.</small>
    </label>
    <label class="grid min-w-0 gap-2">
      <span class="ti-field-label">Timestamps</span>
      <select class="ti-field min-h-10 px-3" value={preferences.timestamps}
        onchange={(event) => update({ timestamps: event.currentTarget.value as TranscriptTimestamp })}>
        <option value="always">Always show</option>
        <option value="hover">On hover or keyboard focus</option>
      </select>
      <small class="text-[12px] leading-5 text-[var(--ti-text-muted)]">Still shown on touchscreens and when a bubble receives focus.</small>
    </label>
  </div>

  <div class="divide-y divide-[var(--ti-border)] border-y border-[var(--ti-border)]">
    <label class="flex min-h-[60px] items-start justify-between gap-4 py-3">
      <span class="min-w-0 flex-1">
        <strong class="block text-[13px] font-semibold">Show speaker icons</strong>
        <small class="mt-1 block text-[12px] leading-5 text-[var(--ti-text-muted)]">Keep speaker names even when icons are hidden.</small>
      </span>
      <input type="checkbox" class="mt-1 size-5 shrink-0 accent-[var(--ti-accent)]"
        checked={preferences.showSpeakerIcons}
        onchange={(event) => update({ showSpeakerIcons: event.currentTarget.checked })}/>
    </label>
    <label class="flex min-h-[60px] items-start justify-between gap-4 py-3">
      <span class="min-w-0 flex-1">
        <strong class="block text-[13px] font-semibold">Follow latest message</strong>
        <small class="mt-1 block text-[12px] leading-5 text-[var(--ti-text-muted)]">Follow new phrases only when already reading at the bottom. Jump to latest remains available.</small>
      </span>
      <input type="checkbox" class="mt-1 size-5 shrink-0 accent-[var(--ti-accent)]"
        checked={preferences.followLatest}
        onchange={(event) => update({ followLatest: event.currentTarget.checked })}/>
    </label>
    <label class="flex min-h-[60px] items-start justify-between gap-4 py-3">
      <span class="min-w-0 flex-1">
        <strong class="block text-[13px] font-semibold">Show local playback status</strong>
        <small class="mt-1 block text-[12px] leading-5 text-[var(--ti-text-muted)]">Only hides routine playback information. Errors remain visible.</small>
      </span>
      <input type="checkbox" class="mt-1 size-5 shrink-0 accent-[var(--ti-accent)]"
        checked={preferences.showPlaybackStatus}
        onchange={(event) => update({ showPlaybackStatus: event.currentTarget.checked })}/>
    </label>
  </div>

  <div class="flex flex-wrap items-center justify-between gap-3">
    <p class="m-0 max-w-[560px] text-[12px] leading-5 text-[var(--ti-text-muted)]">
      These display settings apply to Live Meeting, Last Meeting and Saved Transcripts. No conversation or audio content is stored with these preferences.
    </p>
    <button type="button" class="ti-button ti-button-secondary min-h-9 px-3 text-[12px]"
      onclick={reset}>Restore defaults</button>
  </div>
  {#if feedback}
    <p class="m-0 text-[12px] leading-5 text-[var(--ti-text-muted)]" role="status">{feedback}</p>
  {/if}
</div>
