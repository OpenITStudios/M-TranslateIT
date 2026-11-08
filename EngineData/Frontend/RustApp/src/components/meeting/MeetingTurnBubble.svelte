<script lang="ts">
  import { Check, Copy, Mic, Volume2 } from "@lucide/svelte";
  import type { MeetingCommittedTurn } from "../../app/bridge/runtimeApi";
  import { languageName } from "../../app/shared/state";
  import { meetingSpeakerDescription, meetingSpeakerLabel, meetingTime, meetingTranslationDisplay } from "../../app/runtime/meetingTranscriptPresentation";

  let {
    turn, statusLabel = "", onCopy, copied = false,
  }: {
    turn: MeetingCommittedTurn;
    statusLabel?: string;
    onCopy?: () => void | Promise<void>;
    copied?: boolean;
  } = $props();
  const own = $derived(turn.lane === "you");
</script>

<div class={"flex w-full " + (own ? "justify-end" : "justify-start")}>
  <article class={"ti-chat-bubble group relative " + (own ? "ti-chat-bubble-own" : "ti-chat-bubble-incoming")}
    aria-label={meetingSpeakerLabel(turn.lane)}>
    <header class="mb-3 flex min-w-0 items-center justify-between gap-2">
      <div class="flex min-w-0 items-center gap-2">
        <span class="grid size-6 shrink-0 place-items-center rounded-full bg-[var(--ti-surface-soft)] text-[var(--ti-text-muted)]"
          aria-hidden="true">
          {#if own}<Mic size={13}/>{:else}<Volume2 size={14}/>{/if}
        </span>
        <strong class="truncate text-[13px] font-semibold" title={meetingSpeakerDescription(turn.lane)}>
          {meetingSpeakerLabel(turn.lane)}
        </strong>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <span data-chat-timestamp class="text-[12px] font-medium tabular-nums text-[var(--ti-text-muted)]">{meetingTime(turn.created_unix_ms)}</span>
        {#if onCopy}
          <button type="button" class="ti-chat-copy-button grid size-8 place-items-center rounded-md text-[var(--ti-text-muted)]"
            aria-label={`Copy bilingual phrase ${turn.sequence}`}
            title="Copy source and translation" onclick={() => void onCopy?.()}>
            {#if copied}<Check size={15}/>{:else}<Copy size={15}/>{/if}
          </button>
        {/if}
      </div>
    </header>
    <div class="grid min-w-0 gap-1">
      <span class="ti-chat-meta">{languageName(turn.source_language)} · Original</span>
      <p class="ti-chat-text m-0" lang={turn.source_language}>{turn.source_text}</p>
    </div>
    <div class="ti-chat-divider mt-3 min-w-0 pt-2.5">
      <span class="ti-chat-meta">{languageName(turn.target_language)} · Translation</span>
      <p class={"ti-chat-text mb-0 mt-1 " +
        (turn.translation_state === "translated" ? "font-medium" :
          turn.translation_state === "failed" ? "text-[var(--ti-warning)]" : "text-[var(--ti-text-muted)]")}
        lang={turn.translation_state === "translated" ? turn.target_language : undefined}
        role={turn.translation_state === "failed" ? "status" : undefined}>
        {meetingTranslationDisplay(turn)}
      </p>
    </div>
    {#if statusLabel}
      <p class="mb-0 mt-2 text-right text-[11px] leading-5 text-[var(--ti-text-muted)]">{statusLabel}</p>
    {/if}
  </article>
</div>
