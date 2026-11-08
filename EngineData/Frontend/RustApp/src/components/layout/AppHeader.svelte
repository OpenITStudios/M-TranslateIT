<script lang="ts">
  import type { ProductRuntimeSnapshot } from "../../app/bridge/runtimeProductFacade";
  import type { AppRoute } from "../../app/shared/types";
  import { Zap, Hash, Info, TriangleAlert, CircleAlert } from "@lucide/svelte";
  import type { AppNotice } from "../../app/runtime/applicationNotice";
  import UpdateAction from "../runtime/UpdateAction.svelte";
  import { toggleQuickAccess } from "../../app/runtime/quickAccessRuntime";

  let {
    snapshot,
    route,
    meetingSection,
    notice,
    myVoiceRecording,
    onNavigate,
    onNotice,
  }: {
    snapshot: ProductRuntimeSnapshot;
    route: AppRoute;
    meetingSection: "live" | "last" | "saved";
    notice: AppNotice;
    myVoiceRecording: boolean;
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
      ? meetingSection === "live" ? "live-meeting" : meetingSection === "last" ? "last-meeting" : "saved-transcripts"
      : route === "my-voice"
        ? "My Voice"
        : "Settings",
  );
  const showBanner = $derived(notice.kind === "error" || notice.kind === "attention" || notice.kind === "unverified");
</script>

<header class="ti-appbar flex min-h-[58px] shrink-0 flex-wrap items-center gap-x-4 gap-y-2 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-2">
  <div class="flex min-w-0 shrink-0 items-center gap-3">
    {#if route === "meeting"}<Hash size={19} class="shrink-0 text-[var(--ti-text-soft)]" aria-hidden="true"/>{/if}
    <strong class="text-[15px] font-semibold text-[var(--ti-text)]">{routeTitle}</strong>
  </div>

  {#if !showBanner}
    <p class="m-0 flex min-w-[140px] flex-1 items-center gap-2 truncate text-[12px] text-[var(--ti-text-muted)]"
      role="status" aria-live="polite" aria-atomic="true" title={notice.message}>
      <span class={"inline-block size-1.5 shrink-0 rounded-full " + (notice.kind === "success" ? "bg-[var(--ti-success)]" : "bg-[var(--ti-text-soft)]")} aria-hidden="true"></span>
      <span class="truncate">{notice.message}</span>
    </p>
  {/if}

  <div class="ml-auto flex shrink-0 flex-wrap items-center gap-2">
    <button
      type="button"
      class="ti-button ti-button-secondary min-h-9 px-3 text-[12px]"
      aria-label="Open Quick Access"
      title="Quick Access (Ctrl+K)"
      onclick={() => void toggleQuickAccess().catch(() => onNotice("Quick Access couldn't be opened."))}
    >
      <Zap size={15} />
      Quick Access
      <span class="ml-1 hidden rounded bg-[var(--ti-surface-soft)] px-1 py-0.5 text-[11px] text-[var(--ti-text-muted)] xl:inline" aria-hidden="true">Ctrl+K</span>
    </button>
    <UpdateAction
      audioLocked={snapshot.resources.audio_locked}
      audioOwnerKind={snapshot.resources.owner_kind}
      voiceBuildActive={snapshot.voiceBuildActive}
      {myVoiceRecording}
      {onNotice}
    />
    {#if snapshot.meeting.applicationOwned && snapshot.meeting.hasSession || meetingNeedsAttention}
      <button
        type="button"
        class={"flex min-h-9 shrink-0 items-center gap-2 rounded-[var(--ti-radius-md)] border px-3 text-left " + (meetingNeedsAttention
          ? "border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)]"
          : "border-[var(--ti-success-border)] bg-[var(--ti-success-surface)]")}
        aria-label="Open Meeting status"
        onclick={() => { onNavigate("meeting"); }}
      >
        <span class={"size-2 rounded-full " + (meetingNeedsAttention ? "bg-[var(--ti-warning)]" : "bg-[var(--ti-success)]")} aria-hidden="true"></span>
        <strong class={"text-[12px] font-semibold " + (meetingNeedsAttention ? "text-[var(--ti-warning)]" : "text-[var(--ti-success)]")}>{snapshot.meeting.label}</strong>
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
