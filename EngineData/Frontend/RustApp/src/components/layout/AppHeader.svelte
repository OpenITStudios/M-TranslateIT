<script lang="ts">
  import type { ProductRuntimeSnapshot } from "../../app/bridge/runtimeProductFacade";
  import type { AppRoute } from "../../app/shared/types";
  import { Zap, Info, TriangleAlert, CircleAlert, Check } from "@lucide/svelte";
  import type { AppNotice } from "../../app/runtime/applicationNotice";
  import UpdateAction from "../runtime/UpdateAction.svelte";
  import { toggleQuickAccess } from "../../app/runtime/quickAccessRuntime";

  let {
    snapshot,
    route,
    notice,
    myVoiceRecording,
    onNavigate,
    onNotice,
  }: {
    snapshot: ProductRuntimeSnapshot;
    route: AppRoute;
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
      ? "Meeting"
      : route === "my-voice"
        ? "My Voice"
        : "Settings",
  );
</script>

<header class="ti-appbar flex min-h-16 shrink-0 flex-wrap items-center gap-x-3 gap-y-2 border-b border-[var(--ti-border)] bg-[var(--ti-bg)] px-4 py-2 sm:px-6">
  <div class="min-w-0 shrink-0">
    <div class="flex items-baseline gap-2.5">
      <strong class="text-[14px] font-semibold tracking-[-0.01em]">{routeTitle}</strong>
    </div>
  </div>

  <button
    type="button"
    class="ti-button ti-button-secondary min-h-10 shrink-0 px-3 text-[13px]"
    aria-label="Open Quick Access"
    title="Quick Access (Ctrl+K)"
    onclick={() => void toggleQuickAccess().catch(() => onNotice("Quick Access couldn't be opened."))}
  >
    <Zap size={15} />
    Quick Access
    <span class="ml-1 hidden rounded bg-[var(--ti-surface-soft)] px-1.5 py-0.5 text-[12px] text-[var(--ti-text-muted)] lg:inline" aria-hidden="true">Ctrl+K</span>
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
      class={`flex min-h-10 shrink-0 items-center gap-2 rounded-[var(--ti-radius-md)] border px-3 text-left ${meetingNeedsAttention
        ? "border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)]"
        : "border-[var(--ti-success-border)] bg-[var(--ti-success-surface)]"}`}
      aria-label="Open Meeting status"
      onclick={() => { onNavigate("meeting"); }}
    >
      <span class={`size-2 rounded-full ${meetingNeedsAttention ? "bg-[var(--ti-warning)]" : "bg-[var(--ti-success)]"}`} aria-hidden="true"></span>
      <strong class={`text-[13px] font-semibold ${meetingNeedsAttention ? "text-[var(--ti-warning)]" : "text-[var(--ti-success)]"}`}>{snapshot.meeting.label}</strong>
    </button>
  {/if}
</header>

<!-- A single, persistent notice surface; semantic severity comes from owned evidence, never message text. -->
<div class={`ti-notice-bar flex min-h-9 shrink-0 items-start gap-2 border-b px-4 py-2 sm:px-6 ${noticeSurface}`}>
  {#if notice.kind === "error"}
    <CircleAlert size={15} class="mt-0.5 shrink-0 text-[var(--ti-danger)]" aria-hidden="true" />
  {:else if notice.kind === "attention"}
    <TriangleAlert size={15} class="mt-0.5 shrink-0 text-[var(--ti-warning)]" aria-hidden="true" />
  {:else if notice.kind === "unverified"}
    <Info size={15} class="mt-0.5 shrink-0 text-[var(--ti-text-muted)]" aria-hidden="true" />
  {:else if notice.kind === "success"}
    <Check size={15} class="mt-0.5 shrink-0 text-[var(--ti-success)]" aria-hidden="true" />
  {/if}
  {#if notice.kind === "error"}
    <p class="m-0 min-w-0 flex-1 break-words text-[13px] leading-[1.5] text-[var(--ti-text)]"
      role="alert" aria-atomic="true"><strong>Error. </strong>{notice.message}</p>
  {:else}
    <p class="m-0 min-w-0 flex-1 break-words text-[13px] leading-[1.5] text-[var(--ti-text)]"
      role="status" aria-live="polite" aria-atomic="true">
      {#if notice.kind === "attention"}<strong>Needs attention. </strong>{:else if notice.kind === "unverified"}<strong>Status not confirmed. </strong>{/if}{notice.message}
    </p>
  {/if}
</div>
