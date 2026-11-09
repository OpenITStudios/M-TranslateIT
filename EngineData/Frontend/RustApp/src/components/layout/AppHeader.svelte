<script lang="ts">
  import { tick, untrack } from "svelte";
  import type { ProductRuntimeSnapshot } from "../../app/bridge/runtimeProductFacade";
  import type { AppRoute } from "../../app/shared/types";
  import { Zap, Hash, Info, TriangleAlert, CircleAlert, Captions, ArrowLeftRight, MoreHorizontal } from "@lucide/svelte";
  import type { AppNotice } from "../../app/runtime/applicationNotice";
  import UpdateAction from "../runtime/UpdateAction.svelte";
  import { toggleQuickAccess } from "../../app/runtime/quickAccessRuntime";
  import { toggleFloatingCaptions } from "../../app/runtime/translationOverlayRuntime";
  import { runtimeApi } from "../../app/bridge/runtimeApi";
  import { MEETING_SOURCE_LABELS } from "../ui/meetingSourceLabels";

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

  const meetingNeedsAttention = $derived((snapshot.meeting.hasSession && !snapshot.meeting.live && !snapshot.meeting.busy)
    || snapshot.meeting.lifecycle === "cleanup_incomplete");
  const meetingStopNeeded = $derived(snapshot.meeting.lifecycle === "cleanup_incomplete");

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
      ? meetingSection === "live" ? snapshot.meeting.live ? selectedSessionName ?? "Live Meeting" : "Live Meeting"
        : meetingSection === "session" ? selectedSessionName ?? "Meeting"
        : meetingSection === "last" ? "Last Meeting" : "Saved Meeting"
      : route === "my-voice" ? "My Voice" : "Settings",
  );
  const showBanner = $derived(notice.kind === "error" || notice.kind === "attention" || notice.kind === "unverified");

  let sourceChanging = $state(false);
  let currentSource = $state<"original" | "translateit">("translateit");
  let moreOpen = $state(false);
  let captionBusy = $state(false);
  let moreRoot = $state<HTMLDivElement>();
  let moreButton = $state<HTMLButtonElement>();
  const sourceEditable = $derived(Boolean(route === "meeting" && meetingSection === "live"
    && snapshot.meetingSession && (snapshot.meeting.live ||
      (!snapshot.meeting.hasSession && snapshot.meeting.lifecycle === "idle"))));
  $effect(() => {
    if (!sourceChanging && snapshot.meetingSession) {
      currentSource = snapshot.meetingSession.audio_source === "original" ? "original" : "translateit";
    }
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
  // The header persists between pages. A menu from the previous page must not
  // remain open after navigation.
  $effect(() => {
    route;
    meetingSection;
    untrack(() => { moreOpen = false; });
  });
  function closeMore(): void { moreOpen = false; }
  async function toggleMore(): Promise<void> {
    moreOpen = !moreOpen;
    if (moreOpen) {
      await tick();
      moreRoot?.querySelector<HTMLButtonElement>('[role="menuitem"]')?.focus();
    }
  }
  function handleGlobalClick(event: MouseEvent): void {
    if (moreOpen && event.target instanceof Node && !moreRoot?.contains(event.target)) closeMore();
  }
  function handleGlobalKey(event: KeyboardEvent): void {
    if (moreOpen && event.key === "Escape") {
      event.preventDefault();
      closeMore();
      void tick().then(() => moreButton?.focus());
    }
  }
  async function changeFloatingCaptions(): Promise<void> {
    if (captionBusy || previewReadOnly) return;
    captionBusy = true;
    closeMore();
    await tick();
    moreButton?.focus();
    try {
      const result = await toggleFloatingCaptions();
      onNotice(result === "shown" ? "Floating Captions shown."
        : result === "hidden" ? "Floating Captions hidden."
        : "Floating Captions state could not be verified.");
    } catch {
      onNotice("Floating Captions state could not be verified.");
    } finally { captionBusy = false; }
  }
  function openQuickAccess(): void {
    if (previewReadOnly) return;
    closeMore();
    void toggleQuickAccess().catch(() => onNotice("Quick Access couldn't be opened."));
  }
</script>

<svelte:window onclick={handleGlobalClick} onkeydown={handleGlobalKey} />

<header class="ti-appbar flex min-h-[58px] shrink-0 items-center gap-3 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-4 py-2" aria-label="TranslateIT workspace header">
  <div class="flex min-w-0 items-center gap-2">
    {#if route === "meeting"}<Hash size={19} class="shrink-0 text-[var(--ti-text-soft)]" aria-hidden="true"/>{/if}
    <strong class="min-w-0 max-w-[220px] truncate text-[15px] font-semibold" title={routeTitle}>{routeTitle}</strong>
    {#if route === "meeting" && meetingSection === "live" && snapshot.meeting.live}
      <span class="ml-1 inline-flex min-h-6 items-center rounded-[6px] border border-[var(--ti-success-border)] bg-[var(--ti-success-surface)] px-2 text-[11px] font-semibold text-[var(--ti-success)]" title="Meeting session is active">Live</span>
      {#if snapshot.meetingSession?.muted}
        <span class="inline-flex min-h-6 items-center rounded-[6px] border border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] px-2 text-[11px] font-semibold text-[var(--ti-warning)]" role="status" title="Audio sent to the meeting is muted">Muted</span>
      {/if}
    {/if}
  </div>

  <div class="ml-auto flex shrink-0 items-center gap-2">
    {#if route === "meeting" && meetingSection === "live"}
      {#if sourceEditable}
        <label class="flex min-h-9 items-center gap-1.5 rounded-[var(--ti-radius-sm)] border border-[var(--ti-border)] bg-[var(--ti-surface-raised)] px-2.5">
          <ArrowLeftRight size={15} class="text-[var(--ti-text-muted)]"/>
          <span class="hidden text-[12px] text-[var(--ti-text-muted)] xl:inline">Send to call</span>
          <select aria-label="Audio sent to the call: translated or original microphone" title="Choose translated audio or your original microphone for the call" class="min-w-0 max-w-[150px] bg-transparent text-[12px] font-medium"
            value={currentSource} disabled={sourceChanging || snapshot.meeting.busy || previewReadOnly}
            onchange={(event) => void selectSource(event.currentTarget.value === "original" ? "original" : "translateit")}>
            <option value="translateit">{MEETING_SOURCE_LABELS.translateit}</option>
            <option value="original">{MEETING_SOURCE_LABELS.original}</option>
          </select>
        </label>
      {:else if snapshot.meetingSession}
        <span class="text-[12px] text-[var(--ti-text-muted)]">
          Sending: {snapshot.meetingSession.audio_source === "original" ? "Original Microphone" : "TranslateIT"}
        </span>
      {/if}
    {/if}
    <div class="relative shrink-0" bind:this={moreRoot}>
      <button type="button" bind:this={moreButton}
        class="ti-sidebar-icon-button"
        aria-label="More meeting controls" title="Meeting utilities"
        aria-haspopup="menu" aria-expanded={moreOpen}
        aria-controls={moreOpen ? "ti-meeting-more-menu" : undefined}
        onclick={() => void toggleMore()}>
        <MoreHorizontal size={18} aria-hidden="true"/>
      </button>
      {#if moreOpen}
        <div id="ti-meeting-more-menu"
          class="ti-user-popover absolute right-0 top-full z-40 mt-2 w-[224px] p-1"
          role="menu" aria-label="Meeting utilities">
          {#if route === "meeting"}
            <button type="button" role="menuitem" class="ti-user-menu-option"
              disabled={captionBusy || previewReadOnly} onclick={() => void changeFloatingCaptions()}>
              <Captions size={16} aria-hidden="true"/>
              <span class="min-w-0 flex-1 text-left">Floating Captions</span>
            </button>
          {/if}
          <button type="button" role="menuitem" class="ti-user-menu-option"
            disabled={previewReadOnly} onclick={openQuickAccess}>
            <Zap size={16} aria-hidden="true"/>
            <span class="min-w-0 flex-1 text-left">Quick Access</span>
            <span class="text-[10px] text-[var(--ti-text-soft)]">Ctrl+K</span>
          </button>
        </div>
      {/if}
    </div>
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
        {meetingStopNeeded ? "Stop Needed" : meetingNeedsAttention ? "Check Meeting" : "Live Meeting"}
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
