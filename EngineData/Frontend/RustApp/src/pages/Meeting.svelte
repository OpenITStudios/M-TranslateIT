<script lang="ts">
  import { Hash, Mic, Captions, PhoneOff, ArrowLeftRight } from "@lucide/svelte";
  import { toggleQuickAccess } from "../app/runtime/quickAccessRuntime";
  import { runtimeApi, type MeetingCommittedTurnsSnapshot, type MeetingSessionStatus } from "../app/bridge/runtimeApi";
  import type { AppRoute } from "../app/shared/types";
  import type { ProductRuntimeSnapshot } from "../app/bridge/runtimeProductFacade";
  import { meetingControlState } from "../app/bridge/productMeetingState";
  import MeetingActivity from "../components/meeting/MeetingActivity.svelte";
  import MeetingSessionReview from "../components/meeting/MeetingSessionReview.svelte";
  import MeetingTranscriptExport from "../components/meeting/MeetingTranscriptExport.svelte";
  import StatusBadge from "../components/ui/StatusBadge.svelte";
  import { MEETING_SOURCE_LABELS } from "../components/ui/meetingSourceLabels";

  let {
    section = "live", snapshot, meetingStatus, meetingTurns, actionBusy = false, previewReadOnly = false,
    onMeetingAction, onRefresh, onNavigate,
  }: {
    section?: "live" | "last" | "saved";
    snapshot: ProductRuntimeSnapshot;
    meetingStatus: MeetingSessionStatus | null;
    meetingTurns: MeetingCommittedTurnsSnapshot | null;
    actionBusy?: boolean;
    previewReadOnly?: boolean;
    onMeetingAction: (source?: "original" | "translateit") => void | Promise<void>;
    onRefresh: (preferredNotice?: string) => void | Promise<void>;
    onNavigate: (route: AppRoute) => void;
  } = $props();

  let savedDeletionRevision = $state(0);
  let resumeBusy = $state(false);
  let sourceChanging = $state(false);
  let currentSource = $state<"original" | "translateit">("translateit");
  $effect(() => {
    // Both Meeting and Quick Access show the same Rust-owned pre-Start choice
    // and the same confirmed Live selection.
    if (!sourceChanging && meetingStatus) {
      currentSource = meetingStatus.audio_source === "original" ? "original" : "translateit";
    }
  });
  async function selectSource(source: "original" | "translateit"): Promise<void> {
    // Keep Stop/Retry Stop as the only control while cleanup is pending.
    if (sourceChanging || currentSource === source || !sourceChangeAllowed) return;
    sourceChanging = true;
    try {
      const result = meeting.live
        ? await runtimeApi.selectMeetingAudioSource(source)
        : !meeting.hasSession && meeting.lifecycle === "idle"
          ? await runtimeApi.selectMeetingPreferredAudioSource(source)
          : null;
      if (result?.ok) {
        currentSource = result.source;
        await onRefresh(result.message);
      } else {
        await onRefresh(result?.message ?? "Could not change microphone source. Check Meeting status.");
      }
    } catch {
      await onRefresh("Could not change microphone source. Your previous selection was kept.");
    } finally { sourceChanging = false; }
  }
  const meeting = $derived(snapshot.meeting);
  const sourceChangeAllowed = $derived(meeting.live || (!meeting.hasSession && meeting.lifecycle === "idle"));
  const needsSetup = $derived(!meeting.hasSession && !meeting.canStop
    && ["new", "deferred"].includes(snapshot.settings.meeting_setup_state));
  const needsVoice = $derived(!needsSetup && !meeting.hasSession && !meeting.canStop
    && snapshot.readiness.approvedVoiceReady === false);

  async function continueSetup(): Promise<void> {
    if (resumeBusy || actionBusy || meeting.hasSession || meeting.canStop) return;
    resumeBusy = true;
    try {
      const expected = {
        ...snapshot.settings,
        audio: { ...snapshot.settings.audio },
      };
      const settings = { ...expected, meeting_setup_state: "new" };
      const save = await runtimeApi.saveSettings(settings, expected);
      if (!save.ok) throw new Error("Setup could not be resumed");
      await onRefresh("Continue the initial Meeting setup.");
    } catch {
      await onRefresh("Couldn't resume setup. Check Settings and try again.");
    } finally {
      resumeBusy = false;
    }
  }

  function handlePrimary(): void {
    if (!actionEnabled) return;
    if (currentSource === "original" && !meeting.hasSession && !meeting.canStop) {
      if (originalAvailable) void onMeetingAction("original");
      else onNavigate("settings");
      return;
    }
    if (needsSetup) { void continueSetup(); return; }
    if (needsVoice) { onNavigate("my-voice"); return; }
    if (needsSettings) { onNavigate("settings"); return; }
    void onMeetingAction(currentSource);
  }
  const primaryControl = $derived(meetingControlState(
    meeting, snapshot.readiness.approvedVoiceReady, snapshot.settings.meeting_setup_state, actionBusy,
  ));
  const needsSettings = $derived(!needsSetup && !needsVoice && !meeting.hasSession && !meeting.canStop
    && !primaryControl.enabled && !meeting.busy && !actionBusy);
  const originalAvailable = $derived(
    currentSource === "original" && !meeting.hasSession && meeting.lifecycle === "idle"
    && snapshot.readiness.microphoneReady === true
    && snapshot.readiness.meetingRouteReady === true
  );
  const readyForSelectedSource = $derived(!meeting.hasSession && meeting.lifecycle === "idle"
    && (currentSource === "original" ? originalAvailable : meeting.canStart && !needsSetup && !needsVoice));
  // Stop/Retry Stop must remain available even if a source-switch command is
  // still pending. Only Start and setup actions wait for that operation.
  const actionEnabled = $derived(!resumeBusy && !meeting.busy && !actionBusy
    && (primaryControl.action === "stop"
      ? primaryControl.enabled
      : !sourceChanging && (primaryControl.enabled || originalAvailable || needsSetup || needsVoice || needsSettings)));
  const activityVisible = $derived(Boolean(meetingStatus && meeting.applicationOwned && meeting.live));
  const primaryLabel = $derived(
    primaryControl.action === "stop"
      ? meeting.lifecycle === "cleanup_incomplete" ? primaryControl.label : "Stop Meeting"
      : currentSource === "original" && originalAvailable ? "Start Original Microphone"
      : currentSource === "original" && !meeting.hasSession ? "Check Audio Setup"
      : meeting.busy || actionBusy ? primaryControl.label
      : resumeBusy ? "Opening setup..."
      : needsSetup ? "Finish Setup"
      : needsVoice ? "Choose Voice"
      : needsSettings ? "Open Settings"
      : "Start Translation"
  );
  const statusMessage = $derived(
    meeting.canStop || meeting.hasSession
      ? meeting.live && meetingStatus?.muted
        ? "Microphone muted. No voice is being sent to the meeting."
        : meeting.live && meetingStatus?.audio_source === "original"
          ? "Original microphone is live. Audio bypasses translation and AI."
          : meeting.message
    : currentSource === "original"
      ? originalAvailable
        ? "Original Microphone is ready. Select the virtual microphone in your calling app."
        : "Original Microphone needs a working microphone and virtual route. Check Voice & Audio in Settings."
    : snapshot.settings.meeting_setup_state === "deferred" || snapshot.settings.meeting_setup_state === "new"
      ? "Finish initial setup before starting."
    : snapshot.readiness.level === "unavailable"
      ? "Translation is unavailable. Check Help in Settings."
    : snapshot.readiness.approvedVoiceReady === false
      ? "Choose a voice in My Voice before starting."
    : !snapshot.readiness.microphoneReady
      ? "No microphone available. Check Voice & Audio in Settings."
    : !snapshot.readiness.meetingRouteReady
      ? "Meeting audio needs setup. Check Voice & Audio in Settings."
    : snapshot.readiness.meetingReady
      ? "Ready to translate. Start when your call is open."
      : "Checking translation readiness. If this persists, open Help in Settings."
  );
  const statusLabel = $derived(
    meeting.live && meetingStatus?.muted ? "Muted"
      : readyForSelectedSource ? "Ready" : meeting.label
  );
  const statusTone = $derived<"good" | "warning" | "danger" | "neutral">(
    meeting.lifecycle === "cleanup_incomplete" ? "warning"
      : meeting.live && meetingStatus?.muted ? "neutral"
      : meeting.live || readyForSelectedSource ? "good"
      : meeting.lifecycle === "unavailable" || snapshot.readiness.level === "unavailable" ? "danger"
      : needsSetup || needsVoice || needsSettings ? "warning" : "neutral"
  );
</script>
<section class="flex h-full min-h-0 flex-col bg-[var(--ti-bg)]" aria-label="Meeting workspace">
  {#if section === "live"}
    <div class="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b border-[var(--ti-border)] px-5 py-3 sm:px-7"
      aria-label="Live Meeting toolbar">
      <div class="flex min-w-0 items-center gap-3">
        <p class="m-0 text-[13px] text-[var(--ti-text-muted)]">Indonesian speech → English voice</p>
        <StatusBadge label={statusLabel} tone={statusTone} />
      </div>
      <div class="flex min-w-0 flex-wrap items-center justify-end gap-2">
        {#if sourceChangeAllowed && meetingStatus}
          <label class="ti-meeting-source flex min-h-10 min-w-0 items-center gap-2 rounded-[var(--ti-radius-sm)] border border-[var(--ti-border)] bg-[var(--ti-surface-raised)] px-3">
            <ArrowLeftRight size={16} class="shrink-0 text-[var(--ti-text-muted)]" aria-hidden="true"/>
            <span class="hidden shrink-0 text-[12px] text-[var(--ti-text-muted)] md:inline">Audio sent</span>
            <select id="meeting-audio-source" class="min-h-9 min-w-0 max-w-[170px] bg-transparent text-[13px] font-medium text-[var(--ti-text)]"
              aria-label="Audio sent to meeting" title="Choose the audio sent through TranslateIT's virtual microphone"
              value={currentSource} disabled={sourceChanging || actionBusy || !sourceChangeAllowed}
              onchange={(event) => void selectSource(event.currentTarget.value === "original" ? "original" : "translateit")}>
              <option value="translateit">{MEETING_SOURCE_LABELS.translateit}</option>
              <option value="original">{MEETING_SOURCE_LABELS.original}</option>
            </select>
          </label>
        {:else if meetingStatus}
          <span class="text-[12px] text-[var(--ti-text-muted)]" title="The current audio mode cannot be changed during cleanup">
            Audio sent: {meetingStatus.audio_source === "original" ? MEETING_SOURCE_LABELS.original : MEETING_SOURCE_LABELS.translateit}
          </span>
        {/if}
        <button type="button" class="ti-button ti-button-secondary min-h-10 px-3 text-[13px]"
          aria-label="Open captions controls in Quick Access" title="Floating captions settings"
          onclick={() => void toggleQuickAccess().catch(() => onRefresh("Quick Access couldn't be opened."))}>
          <Captions size={17} aria-hidden="true"/> Captions
        </button>
      </div>
    </div>
    <div class="min-h-0 flex-1 overflow-hidden px-5 py-4 sm:px-7" role="region" aria-label="Live Meeting transcript and status">
      {#if activityVisible && meetingStatus && meetingStatus.audio_source !== "original"}
        <MeetingActivity status={meetingStatus} turns={meetingTurns} {previewReadOnly} />
      {:else}
        <div class="flex min-h-[280px] flex-col items-center justify-center gap-3 text-center">
          <Hash size={28} class="text-[var(--ti-text-soft)]" aria-hidden="true"/>
          <h3 class="m-0 text-[19px] font-semibold">{meeting.live ? "Original Microphone is live" : "Ready for your meeting"}</h3>
          <p class="m-0 max-w-[520px] text-[13px] leading-6 text-[var(--ti-text-muted)]" role="status" aria-live="polite">{statusMessage}</p>
          {#if meeting.live && meetingStatus?.audio_source === "original"}
            <p class="m-0 text-[12px] text-[var(--ti-text-soft)]">Original Microphone bypasses translation; no translated transcript is generated for your outbound speech.</p>
          {/if}
        </div>
      {/if}
    </div>
  {:else if section === "last"}
    <p class="m-0 shrink-0 border-b border-[var(--ti-border)] px-6 py-3 text-[13px] text-[var(--ti-text-muted)]">
      Read your last recognized conversation after the session ends.
    </p>
    <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4 sm:px-7">
      {#if meeting.hasSession}
        <p class="m-0 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-4 text-[13px] text-[var(--ti-text-muted)]">
          The last-session review becomes available after Meeting stops. The current session is still running; choose Live Meeting to view its transcript.
        </p>
      {:else}
        {#key savedDeletionRevision}
          <MeetingSessionReview sessionActive={meeting.hasSession} openByDefault={true} onNotice={(message) => void onRefresh(message)} />
        {/key}
      {/if}
    </div>
  {:else}
    <p class="m-0 shrink-0 border-b border-[var(--ti-border)] px-6 py-3 text-[13px] text-[var(--ti-text-muted)]">
      Choose a saved meeting to read, export, or delete its local archive.
    </p>
    <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4 sm:px-7">
      <MeetingTranscriptExport hasSession={meeting.hasSession}
        onNotice={(message) => void onRefresh(message)}
        onDeleted={() => { savedDeletionRevision += 1; }} />
    </div>
  {/if}

  {#if section === "live" || meeting.hasSession || meeting.canStop}
    <footer class="ti-call-dock flex min-h-[70px] shrink-0 items-center justify-center px-5 py-3"
      aria-label="Meeting main action">
      <button type="button"
        class={"ti-button min-h-11 min-w-[172px] px-5 " + (primaryControl.action === "stop" ? "ti-call-stop" : "")}
        title={primaryControl.action === "stop" ? "Stops TranslateIT audio; does not leave the Discord or Zoom call" : "Start or recover TranslateIT meeting audio"}
        disabled={!actionEnabled} onclick={handlePrimary}>
        {#if primaryControl.action === "stop"}<PhoneOff size={18} aria-hidden="true"/>{:else}<Mic size={18} aria-hidden="true"/>{/if}
        {primaryLabel}
      </button>
    </footer>
  {/if}
</section>