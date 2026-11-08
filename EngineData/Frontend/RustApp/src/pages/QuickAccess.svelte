<script lang="ts">
  import "../styles/quick-access.css";
  import { onMount, tick } from "svelte";
  import { Captions, ExternalLink, Languages, Mic, Play, Square, X } from "@lucide/svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { applicationRuntimeApi, type ApplicationSnapshot } from "../app/bridge/applicationRuntimeApi";
  import { runtimeApi } from "../app/bridge/runtimeApi";
  import {
    hideQuickAccess, openTranslateIt, captionWindowIsVisible, changeCaptionVisibility,
  } from "../app/runtime/quickAccessRuntime";
  import { APPLICATION_MEETING_OWNER_ID } from "../app/shared/types";
  import { MEETING_SOURCE_LABELS } from "../components/ui/meetingSourceLabels";
  import TranslateITMark from "../components/brand/TranslateITMark.svelte";

  type Source = "original" | "translateit";
  type ErrorArea = "source" | "meeting" | "mute" | "captions" | "window";
  let snapshot = $state<ApplicationSnapshot | null>(null);
  let choice = $state<Source>("original");
  let captionsVisible = $state<boolean | null>(null);
  let busy = $state(false);
  let confirmingStop = $state(false);
  let errorMessage = $state("");
  let errorArea = $state<ErrorArea>("meeting");
  let resultMessage = $state("");
  let confirmStopButton = $state<HTMLButtonElement>();
  let primaryActionButton = $state<HTMLButtonElement>();
  let feedbackRegion: HTMLDivElement | undefined;
  let disposed = false;
  let actionRevision = 0;
  let closeTask: ReturnType<typeof setTimeout> | null = null;

  const meeting = $derived(snapshot?.meeting ?? null);
  const owned = $derived(meeting?.owner_id === APPLICATION_MEETING_OWNER_ID);
  const live = $derived(Boolean(owned && meeting?.authority_active && meeting?.lifecycle === "live"));
  const stopping = $derived(meeting?.lifecycle === "starting" || meeting?.lifecycle === "stopping");
  const foreignOwner = $derived(Boolean(meeting?.has_session && !owned));
  // Foreign-owned runtime resources must be handled by their own feature.
  const needsStop = $derived(Boolean(meeting?.lifecycle === "cleanup_incomplete"
    && (!meeting?.has_session || owned)));
  const canSelectSource = $derived(Boolean(meeting && !stopping && !needsStop
    && !foreignOwner && (live || (!meeting.has_session && meeting.lifecycle === "idle"))));
  const canStart = $derived(Boolean(snapshot && snapshot.revision > 0
    && meeting && !meeting.has_session && meeting.lifecycle === "idle"
    && !snapshot.resources.audio_locked
    && meeting.preflight.microphone_ready && meeting.preflight.meeting_route_ready
    && (choice === "original" || meeting.preflight.start_eligible)));
  const canStop = $derived(Boolean(meeting && !stopping && !foreignOwner
    && meeting.lifecycle !== "unavailable"
    && meeting.runtime_claim !== "frontend_bridge_unavailable"
    && ((meeting.has_session && owned) || needsStop)));
  const sourceLabel = $derived(
    !snapshot || snapshot.revision <= 0 || !meeting ? "Source unavailable"
      : meeting.audio_source === "original" ? MEETING_SOURCE_LABELS.original
      : meeting.audio_source === "translateit" ? MEETING_SOURCE_LABELS.translateit
      : "Source unavailable"
  );
  const sourceSummary = $derived(
    live || canSelectSource ? `${live ? "Active" : "Selected"}: ${sourceLabel}` : "Audio source status unavailable"
  );
  // Presentation only: the native snapshot remains the source of Meeting truth.
  const statusPresentation = $derived.by((): { label: string; tone: "good" | "warning" | "danger" | "neutral" } => {
    if (!snapshot || snapshot.revision <= 0 || !meeting) return { label: "Checking", tone: "neutral" };
    if (needsStop) return { label: "Stop Needed", tone: "warning" };
    if (meeting.lifecycle === "unavailable" || meeting.runtime_claim === "frontend_bridge_unavailable") return { label: "Unavailable", tone: "danger" };
    if (stopping) return { label: meeting.lifecycle === "starting" ? "Starting" : "Stopping", tone: "neutral" };
    if (foreignOwner) return { label: "In Use", tone: "neutral" };
    if (live) return { label: meeting.muted ? "Muted" : "Live", tone: meeting.muted ? "neutral" : "good" };
    if (canStart) return { label: "Ready", tone: "good" };
    if (meeting.lifecycle === "idle" && !meeting.has_session) return { label: "Setup Needed", tone: "warning" };
    return { label: "Check Status", tone: "neutral" };
  });
  const statusLabel = $derived(statusPresentation.label);
  // Guide a failed action toward its owning UI, without guessing whether
  // the native operation actually changed state.
  const recoveryInstruction = $derived(
    errorArea === "source" ? "Open TranslateIT → Meeting to check the selected audio source before retrying."
      : errorArea === "mute" ? "Open TranslateIT → Meeting to check the microphone mute state before retrying."
      : errorArea === "captions" ? "Open TranslateIT → Settings → Meeting Captions to check caption visibility before retrying."
      : errorArea === "window" ? "Try opening TranslateIT again. Your Meeting action was not changed by this attempt."
      : "Open TranslateIT → Meeting to check whether the microphone is live or stopping before retrying.",
  );
  const statusDotClass = $derived(
    statusPresentation.tone === "good" ? "bg-[var(--ti-success)]"
      : statusPresentation.tone === "warning" ? "bg-[var(--ti-warning)]"
      : statusPresentation.tone === "danger" ? "bg-[var(--ti-danger)]"
      : "bg-[var(--ti-text-soft)]"
  );

  function syncFromNative(next: ApplicationSnapshot): void {
    // Native application snapshots carry a monotonic observation revision.
    // A delayed poll or event cannot overwrite a newer confirmed source state.
    if (snapshot && next.revision < snapshot.revision) return;
    snapshot = next;
    if (!next.meeting.has_session && next.meeting.lifecycle !== "cleanup_incomplete") {
      confirmingStop = false;
    }
    if ((next.meeting.lifecycle === "live"
        && next.meeting.owner_id === APPLICATION_MEETING_OWNER_ID)
      || (next.meeting.lifecycle === "idle" && !next.meeting.has_session)) {
      choice = next.meeting.audio_source === "original" ? "original" : "translateit";
    }
  }

  async function refresh(): Promise<void> {
    const [current, visible] = await Promise.all([
      applicationRuntimeApi.getSnapshot(), captionWindowIsVisible(),
    ]);
    if (!disposed && !busy) {
      syncFromNative(current);
      captionsVisible = visible;
    }
  }

  function cancelAutoClose(): void {
    actionRevision += 1;
    if (closeTask !== null) { clearTimeout(closeTask); closeTask = null; }
  }

  function closeAfterSuccess(): void {
    // A background action can finish after the user moved back to the call.
    // Never arm an old timer capable of closing a later popup invocation.
    if (!document.hasFocus()) return;
    const token = ++actionRevision;
    if (closeTask !== null) clearTimeout(closeTask);
    closeTask = setTimeout(() => {
      closeTask = null;
      if (!disposed && token === actionRevision && !busy) void hideQuickAccess();
    }, 850);
  }

  async function hide(): Promise<void> {
    cancelAutoClose();
    confirmingStop = false;
    await hideQuickAccess();
  }

  async function selectSource(requested: Source): Promise<void> {
    if (busy || !canSelectSource || !meeting || choice === requested) return;
    errorArea = "source";
    cancelAutoClose();
    busy = true;
    errorMessage = "";
    resultMessage = "Switching audio source...";
    try {
      const result = live
        ? await runtimeApi.selectMeetingAudioSource(requested)
        : meeting.lifecycle === "idle" && !meeting.has_session
          ? await runtimeApi.selectMeetingPreferredAudioSource(requested)
          : null;
      const native = await applicationRuntimeApi.getSnapshot();
      if (!disposed) syncFromNative(native);
      const actual = snapshot?.meeting ?? native.meeting;
      if (!result?.ok || actual.audio_source !== requested
        || (live ? actual.lifecycle !== "live" : actual.lifecycle !== "idle")) {
        errorMessage = result?.message ?? "Could not confirm the selected audio source.";
        resultMessage = "";
      } else {
        resultMessage = `${MEETING_SOURCE_LABELS[requested]} selected.`;
        closeAfterSuccess();
      }
    } catch {
      errorMessage = "Audio source could not be verified. Check the current Meeting status.";
      resultMessage = "";
    } finally { busy = false; await focusErrorIfLost(); }
  }

  async function cancelStopConfirmation(): Promise<void> {
    confirmingStop = false;
    await tick();
    if (!disposed && document.hasFocus()) primaryActionButton?.focus();
  }

  // A disabled control or removed confirmation should not strand keyboard focus.
  async function focusErrorIfLost(): Promise<void> {
    if (disposed || !errorMessage || !document.hasFocus()) return;
    await tick();
    if (!disposed && document.activeElement === document.body) feedbackRegion?.focus();
  }

  async function meetingAction(): Promise<void> {
    if (busy || stopping || foreignOwner || (!canStart && !canStop) || !meeting) return;
    errorArea = "meeting";
    const stop = canStop;
    if (stop && !confirmingStop) {
      confirmingStop = true;
      await tick();
      if (!disposed && document.hasFocus()) confirmStopButton?.focus();
      return;
    }
    if (stop && confirmingStop && document.hasFocus()) feedbackRegion?.focus();
    confirmingStop = false;
    cancelAutoClose();
    busy = true;
    errorMessage = "";
    resultMessage = stop ? "Stopping microphone..." : "Starting microphone...";
    try {
      const result = await applicationRuntimeApi.dispatchIntent(
        stop ? "stop_meeting" : choice === "original" ? "start_original_meeting" : "start_meeting",
      );
      if (!disposed) syncFromNative(result.snapshot);
      const actual = snapshot?.meeting ?? result.snapshot.meeting;
      if (!result.ok
        || (stop && (actual.has_session || actual.lifecycle !== "idle"))
        || (!stop && (actual.lifecycle !== "live"
          || actual.audio_source !== choice))) {
        errorMessage = result.message || "Meeting state was not confirmed. Try again.";
        resultMessage = "";
      } else {
        resultMessage = stop ? "Microphone stopped." : "Microphone is live.";
        closeAfterSuccess();
      }
    } catch {
      errorMessage = "The Meeting action could not be confirmed. Check its status before retrying.";
      resultMessage = "";
    } finally { busy = false; await focusErrorIfLost(); }
  }

  async function toggleMute(): Promise<void> {
    if (busy || !live || !meeting) return;
    errorArea = "mute";
    cancelAutoClose();
    busy = true;
    errorMessage = "";
    const desired = !meeting.muted;
    resultMessage = desired ? "Muting microphone..." : "Unmuting microphone...";
    try {
      const response = await runtimeApi.setMeetingOutputMuted(desired);
      const native = await applicationRuntimeApi.getSnapshot();
      if (!disposed) syncFromNative(native);
      const actual = snapshot?.meeting ?? native.meeting;
      if (!response?.ok || actual.muted !== desired
        || actual.lifecycle !== "live") {
        errorMessage = response?.message ?? "Mute status could not be confirmed.";
        resultMessage = "";
      } else {
        resultMessage = desired ? "Microphone muted." : "Microphone unmuted.";
        closeAfterSuccess();
      }
    } catch {
      errorMessage = "Mute could not be verified. Check Meeting status.";
      resultMessage = "";
    } finally { busy = false; await focusErrorIfLost(); }
  }

  async function toggleCaptions(): Promise<void> {
    if (busy) return;
    errorArea = "captions";
    cancelAutoClose();
    busy = true;
    errorMessage = "";
    const desired = captionsVisible === false;
    try {
      const result = await changeCaptionVisibility(desired);
      const actual = await captionWindowIsVisible();
      if (!disposed) captionsVisible = actual;
      if (!result.ok || actual !== desired) {
        errorMessage = result.message || "Floating Captions visibility could not be confirmed.";
        resultMessage = "";
      } else {
        resultMessage = actual ? "Floating Captions shown." : "Floating Captions hidden.";
        closeAfterSuccess();
      }
    } catch {
      errorMessage = "Floating Captions could not be verified. Open Settings for recovery.";
      resultMessage = "";
    } finally { busy = false; await focusErrorIfLost(); }
  }

  async function openMain(): Promise<void> {
    try { await openTranslateIt(); }
    catch {
      errorArea = "window";
      errorMessage = "TranslateIT window could not be opened.";
    }
  }

  onMount(() => {
    disposed = false;
    const currentWindow = getCurrentWindow();
    let unlistenRuntime: (() => void) | null = null;
    const connect = async () => {
      try {
        const unlisten = await applicationRuntimeApi.subscribe((event) => {
          if (!disposed && !busy && event.snapshot.revision > 0) syncFromNative(event.snapshot);
        });
        if (disposed) unlisten();
        else unlistenRuntime = unlisten;
      } catch { /* Focus refresh remains authoritative. */ }
    };
    const onFocus = () => {
      cancelAutoClose();
      confirmingStop = false;
      if (!busy) resultMessage = "";
      // Preserve an unacknowledged error across close/reopen. New actions
      // explicitly clear it, so the user never loses a failed-switch reason.
      void refresh();
    };
    const onBlur = () => {
      // Never hold a floating panel on top of a call while AI is preparing.
      // Hiding does not cancel or alter the authoritative Meeting action.
      void hide();
    };
    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); void hide(); }
    };
    let closeUnlisten: (() => void) | null = null;
    void currentWindow.onCloseRequested((event) => {
      event.preventDefault();
      void hide();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else closeUnlisten = unlisten;
    }).catch(() => {});
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    window.addEventListener("keydown", onKeydown);
    const polling = window.setInterval(() => {
      if (!disposed && !busy && document.hasFocus()) void refresh();
    }, 5000);
    void connect();
    void refresh();
    return () => {
      disposed = true;
      cancelAutoClose();
      window.clearInterval(polling);
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("keydown", onKeydown);
      unlistenRuntime?.();
      closeUnlisten?.();
    };
  });
</script>

<div class="qa-shell">
  <section class="qa-panel" aria-label="TranslateIT Quick Access">
    <header class="flex shrink-0 items-center justify-between gap-3 px-4 py-3" data-tauri-drag-region>
      <div class="flex items-center gap-2" data-tauri-drag-region>
        <TranslateITMark size={20} />
        <strong class="text-[13px] font-semibold">Quick Access</strong>
      </div>
      <button type="button" class="grid size-8 place-items-center rounded-lg text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-soft)]"
        aria-label="Hide Quick Access" title="Close (Esc)" onclick={() => void hide()}><X size={16}/></button>
    </header>

    <div class="qa-section">
      <div class="flex items-center gap-2">
        <span class={`size-2 shrink-0 rounded-full ${statusDotClass}`} aria-hidden="true"></span>
        <div class="min-w-0 flex-1">
          <p class="m-0 text-[13px] font-semibold" role="status" aria-live="polite">{statusLabel}</p>
          <p class="m-0 mt-1 break-words text-[13px] leading-5 text-[var(--ti-text-muted)]">{sourceSummary}</p>
        </div>
      </div>
    </div>

    <div class="qa-section">
      <p class="m-0 mb-3 text-[13px] font-semibold text-[var(--ti-text-muted)]">Audio sent to meeting</p>
      <div class="grid grid-cols-2 gap-2">
        <button type="button" class="qa-choice" aria-pressed={choice === "original"}
          disabled={busy || !canSelectSource || !snapshot || snapshot.revision <= 0}
          onclick={() => void selectSource("original")}>
          <Mic size={18} aria-hidden="true" />
          <span>{MEETING_SOURCE_LABELS.original}</span>
        </button>
        <button type="button" class="qa-choice" aria-pressed={choice === "translateit"}
          disabled={busy || !canSelectSource || !snapshot || snapshot.revision <= 0}
          onclick={() => void selectSource("translateit")}>
          <Languages size={18} aria-hidden="true" />
          <span>{MEETING_SOURCE_LABELS.translateit}</span>
        </button>
      </div>
      <p class="qa-feedback mt-2">{choice === "original" ? "Direct microphone · No translation" : "Indonesian → English · Translation required"}</p>
    </div>

    <div class="qa-section flex items-center justify-between gap-3">
      <div class="flex items-center gap-2">
        <Mic size={17} aria-hidden="true" />
        <div>
          <p class="m-0 text-[13px] font-semibold">Microphone Mute</p>
          <p class="qa-feedback">{meeting?.muted ? "Virtual output muted" : live ? "Virtual output unmuted" : "Available during Meeting"}</p>
        </div>
      </div>
      <button type="button" class="qa-caption-toggle" role="switch"
        aria-checked={Boolean(meeting?.muted)}
        aria-label={meeting?.muted ? "Unmute microphone" : "Mute microphone"}
        disabled={busy || !live}
        onclick={() => void toggleMute()}>
        {meeting?.muted ? "Unmute" : "Mute"}
      </button>
    </div>

    <div class="qa-section flex items-center justify-between gap-3">
      <div class="flex items-center gap-2">
        <Captions size={17} aria-hidden="true" />
        <div>
          <p class="m-0 text-[13px] font-semibold">Floating Captions</p>
          <p class="qa-feedback">{captionsVisible === null ? "Status unavailable" : captionsVisible ? "Visible" : "Hidden"}</p>
        </div>
      </div>
      <button type="button" class="qa-caption-toggle" role="switch" aria-checked={captionsVisible === true}
        aria-label={captionsVisible ? "Hide Floating Captions" : "Show Floating Captions"} disabled={busy || captionsVisible === null}
        onclick={() => void toggleCaptions()}>
        {captionsVisible ? "Hide" : "Show"}
      </button>
    </div>

    <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need focus to scroll this named region.) -->
    <div bind:this={feedbackRegion} class="min-h-0 flex-1 overflow-y-auto px-4 py-2" role="region" aria-label="Quick Access status and recovery" tabindex="0">
      {#if errorMessage}
        <div class="grid min-w-0 gap-1.5" role="alert">
          <strong class="text-[13px] font-semibold text-[var(--ti-danger)]">Action not confirmed</strong>
          <p class="qa-feedback qa-error break-words">{errorMessage}</p>
          <p class="qa-feedback break-words">{recoveryInstruction}</p>
        </div>
      {:else if resultMessage}
        <p class="qa-feedback" role="status" aria-live="polite" aria-atomic="true">{resultMessage}</p>
      {:else if needsStop}
        <p class="qa-feedback qa-error" role="alert">Audio needs cleanup. Use Retry Stop to recover.</p>
      {:else if foreignOwner}
        <p class="qa-feedback">Another feature is using audio. Finish that action in TranslateIT before changing the source here.</p>
      {:else if stopping}
        <p class="qa-feedback">Meeting is {meeting?.lifecycle === "starting" ? "starting" : "stopping"}. Audio controls remain unavailable until its status is confirmed.</p>
      {:else if !snapshot || snapshot.revision <= 0}
        <p class="qa-feedback">Meeting status is not confirmed yet. Open TranslateIT → Meeting to check the current state if this continues.</p>
      {:else if !live && !canStart}
        <p class="qa-feedback">{choice === "original"
          ? "Connect a physical microphone and the virtual audio route."
          : "Check the microphone, virtual route, and selected translation voice."}</p>
      {/if}
    </div>

    <footer class="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t border-[var(--ti-border)] px-4 py-3">
      <button type="button" class="inline-flex min-h-10 items-center gap-1.5 rounded-[var(--ti-radius-sm)] px-1 text-[13px] font-medium text-[var(--ti-text-muted)] hover:text-[var(--ti-text)]"
        onclick={() => void openMain()}><ExternalLink size={13} aria-hidden="true" /> Open TranslateIT</button>
      {#if confirmingStop}
        <div class="flex flex-wrap items-center gap-2">
          <button type="button" class="ti-button ti-button-secondary min-h-10 px-2 text-[13px]"
            onclick={() => void cancelStopConfirmation()}>Cancel</button>
          <button type="button" class="ti-button ti-button-danger min-h-10 px-2 text-[13px]"
            bind:this={confirmStopButton} disabled={busy} onclick={() => void meetingAction()}>{needsStop ? "Confirm Retry Stop" : "Confirm Stop"}</button>
        </div>
      {:else}
        <button type="button" class={"ti-button min-h-10 px-3 text-[13px] " + (canStop ? "ti-button-danger" : "")}
          bind:this={primaryActionButton}
          aria-label={canStop ? needsStop ? "Retry Stop" : "Stop microphone" : choice === "original" ? "Start Original Microphone" : "Start Translation"}
          disabled={busy || (!canStart && !canStop)}
          onclick={() => void meetingAction()}>
          {#if canStop}<Square size={13} aria-hidden="true" /> {needsStop ? "Retry Stop" : "Stop"}
          {:else}<Play size={13} aria-hidden="true" /> Start{/if}
        </button>
      {/if}
    </footer>
  </section>
</div>
