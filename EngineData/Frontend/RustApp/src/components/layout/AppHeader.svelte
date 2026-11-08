<script lang="ts">
  import { onMount } from "svelte";
  import type { ProductRuntimeSnapshot } from "../../app/bridge/runtimeProductFacade";
  import type { AppRoute } from "../../app/shared/types";
  import { Zap, Hash, Info, TriangleAlert, CircleAlert, Captions, ArrowLeftRight } from "@lucide/svelte";
  import type { AppNotice } from "../../app/runtime/applicationNotice";
  import UpdateAction from "../runtime/UpdateAction.svelte";
  import { toggleQuickAccess } from "../../app/runtime/quickAccessRuntime";
  import { runtimeApi } from "../../app/bridge/runtimeApi";
  import { MEETING_SOURCE_LABELS } from "../ui/meetingSourceLabels";
  import {
    DEFAULT_TRANSCRIPT_PREFERENCES, readTranscriptPreferences,
    subscribeTranscriptPreferences, updateTranscriptPreferences,
    type TranscriptPreferences, type TranscriptTextMode,
  } from "../meeting/transcriptPreferences";

  let {
    snapshot,
    route,
    meetingSection,
    notice,
    myVoiceRecording,
    selectedSessionName = null, previewReadOnly = false,
    showUpdateAction = true, onRefresh,
    onNavigate,
    onNotice,
  }: {
    snapshot: ProductRuntimeSnapshot;
    route: AppRoute;
    meetingSection: "live" | "last" | "saved" | "session";
    selectedSessionName?: string | null;
    previewReadOnly?: boolean;
    onRefresh?: (message?: string) => void | Promise<void>;
    notice: AppNotice;
    myVoiceRecording: boolean;
    showUpdateAction?: boolean;
    onNavigate: (route: AppRoute) => void;
    onNotice: (message: string) => void;
  } = $props();

  const meetingNeedsAttention = $derived(snapshot.meeting.hasSession && !snapshot.meeting.live
    || snapshot.meeting.lifecycle === "cleanup_incomplete");

  const noticeSurface = $derived(
    notice.kind === "error"
      ? "border-[var(--ti-danger-border)] bg-[var(--ti-danger-surface)]"
      : notice.kind === "attention"
        ? "border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)]"
        : notice.kind === "success"
          ? "border-[var(--ti-success-border)] bg-[var(--ti-success-surface)]"
          : "border-[var(--ti-border)] bg-[var(--ti-surface-soft)]"
  );

  const routeTitle = $derived(
    route === "meeting"
      ? meetingSection === "live" ? snapshot.meeting.live ? selectedSessionName ?? "Live Meeting" : "Start Meeting"
        : meetingSection === "session" ? selectedSessionName ?? "Meeting"
        : meetingSection === "last" ? "Last Meeting" : "Saved Meeting"
      : route === "my-voice" ? "My Voice" : "Settings",
  );
  const showBanner = $derived(notice.kind === "error" || notice.kind === "attention" || notice.kind === "unverified");

  let sourceChanging = $state(false);
  let currentSource = $state<"original" | "translateit">("translateit");
  let appearance = $state<TranscriptPreferences>({ ...DEFAULT_TRANSCRIPT_PREFERENCES });
  const sourceEditable = $derived(Boolean(route === "meeting" && meetingSection === "live"
    && snapshot.meetingSession && (snapshot.meeting.live ||
      (!snapshot.meeting.hasSession && snapshot.meeting.lifecycle === "idle"))));
  $effect(() => {
    if (!sourceChanging && snapshot.meetingSession) {
      currentSource = snapshot.meetingSession.audio_source === "original" ? "original" : "translateit";
    }
  });
  onMount(() => {
    if (previewReadOnly) return;
    appearance = readTranscriptPreferences();
    return subscribeTranscriptPreferences((next) => { appearance = next; });
  });
  async function selectSource(source: "original" | "translateit"): Promise<void> {
    if (!sourceEditable || sourceChanging || currentSource === source || previewReadOnly) return;
    sourceChanging = true;
    try {
      const result = snapshot.meeting.live
        ? await runtimeApi.selectMeetingAudioSource(source)
        : await runtimeApi.selectMeetingPreferredAudioSource(source);
      if (result?.ok) {
        currentSource = result.source;
        await onRefresh?.(result.message);
      } else {
        await onRefresh?.(result?.message ?? "Audio Sent could not be changed.");
      }
    } catch {
      await onRefresh?.("Audio Sent selection could not be verified. The previous source was kept.");
    } finally { sourceChanging = false; }
  }
  function changeDisplay(mode: TranscriptTextMode): void {
    const result = updateTranscriptPreferences({ textMode: mode });
    appearance = result.preferences;
    onNotice(result.ok ? "Transcript display updated." : "Transcript display could not be saved.");
  }
</script>

<header class="ti-appbar flex min-h-[58px] shrink-0 items-center gap-3 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-4 py-2" aria-label="TranslateIT workspace header">
  <div class="flex min-w-0 items-center gap-2">
    {#if route === "meeting"}<Hash size={19} class="shrink-0 text-[var(--ti-text-soft)]" aria-hidden="true"/>{/if}
    <strong class="min-w-0 max-w-[220px] truncate text-[15px] font-semibold" title={routeTitle}>{routeTitle}</strong>
    {#if route === "meeting" && meetingSection === "live" && snapshot.meeting.live}
      <span class="ml-1 inline-flex min-h-6 items-center rounded-[6px] border border-[var(--ti-success-border)] bg-[var(--ti-success-surface)] px-2 text-[11px] font-semibold text-[var(--ti-success)]">Live</span>
    {/if}
  </div>

  <div class="ml-auto flex shrink-0 items-center gap-2">
    {#if route === "meeting" && meetingSection === "live"}
      {#if sourceEditable}
        <label class="flex min-h-9 items-center gap-1.5 rounded-[var(--ti-radius-sm)] border border-[var(--ti-border)] bg-[var(--ti-surface-raised)] px-2.5">
          <ArrowLeftRight size={15} class="text-[var(--ti-text-muted)]"/>
          <span class="hidden text-[12px] text-[var(--ti-text-muted)] xl:inline">Audio sent</span>
          <select aria-label="Audio sent to meeting" class="min-w-0 max-w-[150px] bg-transparent text-[12px] font-medium"
            value={currentSource} disabled={sourceChanging || snapshot.meeting.busy || previewReadOnly}
            onchange={(event) => void selectSource(event.currentTarget.value === "original" ? "original" : "translateit")}>
            <option value="translateit">{MEETING_SOURCE_LABELS.translateit}</option>
            <option value="original">{MEETING_SOURCE_LABELS.original}</option>
          </select>
        </label>
      {:else if snapshot.meetingSession}
        <span class="text-[12px] text-[var(--ti-text-muted)]">
          Audio: {snapshot.meetingSession.audio_source === "original" ? "Original" : "TranslateIT"}
        </span>
      {/if}
      <label class="flex min-h-9 items-center gap-1 rounded-[var(--ti-radius-sm)] border border-[var(--ti-border)] bg-[var(--ti-surface-raised)] px-2">
        <span class="hidden text-[12px] text-[var(--ti-text-muted)] xl:inline">Display</span>
        <select aria-label="Transcript display mode" class="max-w-[170px] bg-transparent text-[12px]"
          value={appearance.textMode} disabled={previewReadOnly}
          onchange={(event) => changeDisplay(event.currentTarget.value as TranscriptTextMode)}>
          <option value="both">Original + Translation</option>
          <option value="translation-only">Translation only</option>
        </select>
      </label>
      <button type="button" class="ti-sidebar-icon-button" aria-label="Floating Captions"
        title="Floating Captions in Quick Access"
        onclick={() => void toggleQuickAccess().catch(() => onNotice("Quick Access couldn't be opened."))}>
        <Captions size={18}/>
      </button>
    {/if}
    <button type="button" class="ti-sidebar-icon-button" aria-label="Open Quick Access"
      title="Quick Access (Ctrl+K)"
      onclick={() => void toggleQuickAccess().catch(() => onNotice("Quick Access couldn't be opened."))}>
      <Zap size={18}/>
    </button>
    {#if showUpdateAction}
      <UpdateAction
        audioLocked={snapshot.resources.audio_locked}
        audioOwnerKind={snapshot.resources.owner_kind}
        voiceBuildActive={snapshot.voiceBuildActive}
        {myVoiceRecording}
        {onNotice}
      />
    {/if}
    {#if meetingNeedsAttention || (snapshot.meeting.live && !(route === "meeting" && meetingSection === "live"))}
      <button type="button" class="flex min-h-9 shrink-0 items-center gap-2 rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] px-3 text-[12px] text-[var(--ti-warning)]"
        title="Return to active Meeting or Retry Stop" onclick={() => onNavigate("meeting")}>
        {meetingNeedsAttention ? "Stop Needed" : "Live Meeting"}
      </button>
    {/if}
  </div>
</header>

{#if showBanner}
  <div class={`ti-notice-bar flex min-h-10 shrink-0 items-start gap-2 border-b px-5 py-2 ${noticeSurface}`}>
    {#if notice.kind === "error"}
      <CircleAlert size={16} class="mt-0.5 shrink-0 text-[var(--ti-danger)]" aria-hidden="true" />
    {:else if notice.kind === "attention"}
      <TriangleAlert size={16} class="mt-0.5 shrink-0 text-[var(--ti-warning)]" aria-hidden="true" />
    {:else}
      <Info size={16} class="mt-0.5 shrink-0 text-[var(--ti-text-muted)]" aria-hidden="true" />
    {/if}
    {#if notice.kind === "error"}
      <p class="m-0 min-w-0 flex-1 break-words text-[13px] leading-5" role="alert" aria-atomic="true"><strong>Error. </strong>{notice.message}</p>
    {:else}
      <p class="m-0 min-w-0 flex-1 break-words text-[13px] leading-5" role="status" aria-live="polite" aria-atomic="true">
        <strong>{notice.kind === "attention" ? "Needs attention. " : "Status not confirmed. "}</strong>{notice.message}
      </p>
    {/if}
  </div>
{/if}
