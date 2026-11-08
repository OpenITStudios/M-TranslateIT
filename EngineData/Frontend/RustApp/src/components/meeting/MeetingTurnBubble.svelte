<script lang="ts">
  import { Check, Copy, Mic, Volume2 } from "@lucide/svelte";
  import type { MeetingCommittedTurn } from "../../app/bridge/runtimeApi";
  import { languageName } from "../../app/shared/state";
  import { meetingSpeakerDescription, meetingSpeakerLabel, meetingTime, meetingTranslationDisplay } from "../../app/runtime/meetingTranscriptPresentation";
  import { DEFAULT_TRANSCRIPT_PREFERENCES, type TranscriptPreferences } from "./transcriptPreferences";

  let {
    turn, statusLabel = "", onCopy, copied = false,
    preferences = DEFAULT_TRANSCRIPT_PREFERENCES,
  }: {
    turn: MeetingCommittedTurn;
    statusLabel?: string;
    onCopy?: () => void | Promise<void>;
    copied?: boolean;
    preferences?: Readonly<TranscriptPreferences>;
  } = $props();

  const own = $derived(turn.lane === "you");
  const original = $derived({
    kind: "original" as const,
    title: `${languageName(turn.source_language)} · Original`,
    text: turn.source_text,
  });
  const translation = $derived({
    kind: "translation" as const,
    title: `${languageName(turn.target_language)} · Translation`,
    text: meetingTranslationDisplay(turn),
  });
  const blocks = $derived(preferences.textMode === "translation-only"
    ? turn.translation_state === "translated" && translation.text !== "Translation unavailable"
      ? [translation] : [original, translation]
    : preferences.textOrder === "translation-first"
      ? [translation, original] : [original, translation]);
  // A user's display preference must never conceal a failed translation/output.
  const criticalStatus = $derived(turn.translation_state === "failed"
    || turn.delivery_state === "output_failed" || turn.delivery_state === "interrupted");
</script>

<div class={"flex w-full " + (own ? "justify-end" : "justify-start")}>
  <article class={"ti-chat-bubble group relative " +
    (own ? "ti-chat-bubble-own" : "ti-chat-bubble-incoming") +
    (preferences.density === "compact" ? " ti-chat-bubble-compact" : "")}
    aria-label={meetingSpeakerLabel(turn.lane)}>
    <header class="ti-chat-bubble-header flex min-w-0 items-center justify-between gap-2">
      <div class="flex min-w-0 items-center gap-2">
        {#if preferences.showSpeakerIcons}
          <span class="grid size-6 shrink-0 place-items-center rounded-full bg-[var(--ti-surface-soft)] text-[var(--ti-text-muted)]"
            aria-hidden="true">
            {#if own}<Mic size={13}/>{:else}<Volume2 size={14}/>{/if}
          </span>
        {/if}
        <strong class="truncate text-[13px] font-semibold" title={meetingSpeakerDescription(turn.lane)}>
          {meetingSpeakerLabel(turn.lane)}
        </strong>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <span data-chat-timestamp
          class={"ti-chat-timestamp text-[12px] font-medium tabular-nums text-[var(--ti-text-muted)] " +
            (preferences.timestamps === "hover" && onCopy ? "ti-chat-timestamp-hover" : "")}>
          {meetingTime(turn.created_unix_ms)}
        </span>
        {#if onCopy}
          <button type="button" class="ti-chat-copy-button grid size-8 place-items-center rounded-md text-[var(--ti-text-muted)]"
            aria-label={`Copy bilingual phrase ${turn.sequence}`}
            title="Copy source and translation" onclick={() => void onCopy?.()}>
            {#if copied}<Check size={15}/>{:else}<Copy size={15}/>{/if}
          </button>
        {/if}
      </div>
    </header>

    {#each blocks as block, index (block.kind)}
      <div class={index === 0 ? "grid min-w-0 gap-1" : "ti-chat-divider min-w-0"}>
        <span class="ti-chat-meta">{block.title}</span>
        <p class={"ti-chat-text m-0 " +
          (block.kind === "translation" && turn.translation_state === "translated" ? "font-medium" :
           block.kind === "translation" && turn.translation_state === "failed" ? "text-[var(--ti-warning)]" :
           block.kind === "translation" ? "text-[var(--ti-text-muted)]" : "")}
          lang={block.kind === "original" ? turn.source_language :
            turn.translation_state === "translated" ? turn.target_language : undefined}
          role={block.kind === "translation" && turn.translation_state === "failed" ? "status" : undefined}>
          {block.text}
        </p>
      </div>
    {/each}

    {#if statusLabel && (preferences.showPlaybackStatus || criticalStatus)}
      <p class="mb-0 mt-2 text-right text-[11px] leading-5 text-[var(--ti-text-muted)]">{statusLabel}</p>
    {/if}
  </article>
</div>
