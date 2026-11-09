<script lang="ts">
  import { Hash, Mic, PhoneOff } from "@lucide/svelte";
  import { runtimeApi, type MeetingCommittedTurnsSnapshot, type MeetingSessionStatus } from "../app/bridge/runtimeApi";
  import type { AppRoute } from "../app/shared/types";
  import type { ProductRuntimeSnapshot } from "../app/bridge/runtimeProductFacade";
  import { meetingControlState } from "../app/bridge/productMeetingState";
  import MeetingActivity from "../components/meeting/MeetingActivity.svelte";
  import MeetingSessionReview from "../components/meeting/MeetingSessionReview.svelte";
  import MeetingTranscriptExport from "../components/meeting/MeetingTranscriptExport.svelte";
  import MeetingChannel from "../components/meeting/MeetingChannel.svelte";
  import type { MeetingLibrarySession } from "../app/bridge/runtimeApi";

  let {
    section = "live", snapshot, meetingStatus, meetingTurns, actionBusy = false, previewReadOnly = false,
    selectedSession = null, onLibraryRefresh, onMeetingAction, onRefresh, onNavigate,
  }: {
    section?: "live" | "last" | "saved" | "session";
    selectedSession?: MeetingLibrarySession | null;
    onLibraryRefresh?: () => void | Promise<void>;
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
  let currentSource = $state<"original" | "translateit">("translateit");
  $effect(() => {
    if (meetingStatus) {
      currentSource = meetingStatus.audio_source === "original" ? "original" : "translateit";
    }
  });
  const meeting = $derived(snapshot.meeting);
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
  // Native action admission still owns Start/Stop. Header source selection is
  // asynchronous; Stop remains visible when the source control is updating.
  const actionEnabled = $derived(!resumeBusy && !meeting.busy && !actionBusy
    && (primaryControl.enabled || originalAvailable || needsSetup || needsVoice || needsSettings));
  const activityVisible = $derived(Boolean(meetingStatus && meeting.applicationOwned && meeting.live));
  const primaryLabel = $derived(
    primaryControl.action === "stop"
      ? meeting.lifecycle === "cleanup_incomplete" ? primaryControl.label : "Stop Meeting"
      : currentSource === "original" && originalAvailable ? "Start Meeting"
      : currentSource === "original" && !meeting.hasSession ? "Check Audio Setup"
      : meeting.busy || actionBusy ? primaryControl.label
      : resumeBusy ? "Opening setup..."
      : needsSetup ? "Finish Setup"
      : needsVoice ? "Choose Voice"
      : needsSettings ? "Open Settings"
      : "Start Meeting"
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
</script>
<section class="flex h-full min-h-0 flex-col bg-[var(--ti-bg)]" aria-label="Meeting workspace">
  {#if section === "live"}
    <div class="min-h-0 flex-1 overflow-hidden px-5 py-4 sm:px-7" role="region" aria-label="Live Meeting transcript and status">
      {#if activityVisible && meetingStatus && meetingStatus.audio_source !== "original"}
        <MeetingActivity status={meetingStatus} turns={meetingTurns} {previewReadOnly} />
      {:else}
        <div class="flex min-h-[280px] flex-col items-center justify-center gap-3 text-center">
          <Hash size={28} class="text-[var(--ti-text-soft)]" aria-hidden="true"/>
          <h3 class="m-0 text-[19px] font-semibold">{meeting.live ? "Original Microphone is live" : "Ready to Translate"}</h3>
          <p class="m-0 max-w-[520px] text-[13px] leading-6 text-[var(--ti-text-muted)]" role="status" aria-live="polite">
            {meeting.live ? statusMessage
              : originalAvailable ? "Original Microphone sends your voice without translation. Select Start to begin."
              : snapshot.readiness.meetingReady && !needsSetup && !needsVoice
                ? "Select TranslateIT as your microphone in Discord or Zoom, then select Start."
                : statusMessage}
          </p>
          {#if meeting.live && meetingStatus?.audio_source === "original"}
            <p class="m-0 text-[12px] text-[var(--ti-text-soft)]">Original Microphone bypasses translation; no translated transcript is generated for your outbound speech.</p>
          {/if}
        </div>
      {/if}
    </div>
  {:else if section === "session"}
    {#if selectedSession}
      <MeetingChannel session={selectedSession} onNotice={(message) => void onRefresh(message)} {previewReadOnly}/>
    {:else}
      <div class="flex min-h-0 flex-1 items-center justify-center p-6 text-center text-[13px] text-[var(--ti-text-muted)]">
        This Meeting is no longer in the saved library. Open Saved Meeting to refresh the list.
      </div>
    {/if}
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
        onDeleted={() => { savedDeletionRevision += 1; void onLibraryRefresh?.(); }} />
    </div>
  {/if}

  {#if section === "live" || meeting.hasSession || meeting.canStop}
    <footer class="ti-call-dock flex min-h-[70px] shrink-0 items-center justify-center px-5 py-3"
      aria-label="Meeting main action">
      <button type="button"
        class={"ti-button min-h-11 min-w-[172px] px-5 " + (primaryControl.action === "stop" ? "ti-call-stop" : "")}
        title={primaryControl.action === "stop" ? "Stop TranslateIT audio without leaving your Discord or Zoom call" : "Start TranslateIT meeting audio; this does not join a Discord or Zoom call"}
        disabled={!actionEnabled} onclick={handlePrimary}>
        {#if primaryControl.action === "stop"}<PhoneOff size={18} aria-hidden="true"/>{:else}<Mic size={18} aria-hidden="true"/>{/if}
        {primaryLabel}
      </button>
    </footer>
  {/if}
</section>