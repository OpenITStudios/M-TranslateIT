<script lang="ts">
  import { onMount } from "svelte";
  import { toggleQuickAccess } from "./app/runtime/quickAccessRuntime";
  import { appUpdateApi } from "./app/update/appUpdateApi";
  import { runtimeApi, type MeetingSessionStatus, type MeetingLibrary, type MeetingLibraryChange } from "./app/bridge/runtimeApi";
  import {
    applicationRuntimeApi,
    type ApplicationSnapshot,
  } from "./app/bridge/applicationRuntimeApi";
  import {
    runtimeProductFacade,
    type ProductSetupAction,
  } from "./app/bridge/runtimeProductFacade";
  import { startMeetingRuntimeMonitors, type ReliabilityChannel, type ReliabilityNotice } from "./app/runtime/reliabilityMonitor";
import { appNotice, visibleAppNotice, type AppNotice, type AppNoticeKind } from "./app/runtime/applicationNotice";
  import { subscribeOverlayReady, subscribeQuickAccessCaptionRequests } from "./app/runtime/translationOverlayRuntime";
import { discardLegacyOverlayCaption } from "./app/runtime/translationOverlayState";
  import { startVoiceBuildRuntimeSync } from "./app/runtime/voiceBuildRuntimeSync";
  import { installNativeCloseGuard } from "./app/runtime/nativeCloseRuntime";
  import {
    createCloseController,
    type CloseControllerState,
  } from "./app/runtime/closeController";
  import { defaultSettings } from "./app/shared/state";
  import { meetingControlState } from "./app/bridge/productMeetingState";
  import {
    createApplicationController,
    type ApplicationControllerState,
  } from "./app/runtime/applicationController";
  import {
    createMeetingLiveController,
    type MeetingLiveViewState,
  } from "./app/runtime/meetingLiveController";
  import type {
    AppRoute,
    RuntimeSettings,
  } from "./app/shared/types";
  import Sidebar from "./components/layout/Sidebar.svelte";
  import TranslateITMark from "./components/brand/TranslateITMark.svelte";
  import NativeCloseDialog from "./components/runtime/NativeCloseDialog.svelte";
  import AppHeader from "./components/layout/AppHeader.svelte";
  import FirstSetup from "./pages/FirstSetup.svelte";
  import Meeting from "./pages/Meeting.svelte";
  import MyVoice from "./pages/MyVoice.svelte";
  import Settings from "./pages/Settings.svelte";

  const applicationController = createApplicationController(defaultSettings());
  const meetingLiveController = createMeetingLiveController();
  const closeController = createCloseController();
  let applicationState = $state<ApplicationControllerState>(applicationController.read());
  let meetingViewState = $state<MeetingLiveViewState>(meetingLiveController.read());
  let closeState = $state<CloseControllerState>(closeController.read());

  let booting = $state(true);
  let route=$state<AppRoute>("meeting");
  let meetingSection = $state<"live" | "last" | "saved" | "session">("live");
  let selectedSessionId = $state<string | null>(null);
  // The same archive may appear under Recent and a Project. Highlight only
  // the location the user actually opened, not both copies of its ID.
  let selectedSurface = $state<"recent" | "project">("recent");
  let library = $state<MeetingLibrary | null>(null);
  let libraryBusy = $state(false);
  let settingsTab = $state<"meeting" | "translation" | "appearance" | "advanced">("meeting");
  let muteBusy = $state(false);
  let voiceSelectionRevision = $state(0);
  let notice = $state<AppNotice>(appNotice("Getting TranslateIT ready...", "progress"));
  let liveHealthNotice = $state<AppNotice | null>(null);
  let longSessionNotice = $state<AppNotice | null>(null);
  let meetingActionBusy=$state(false);
  let setupActionBusy=$state(false);
  let micTestBusy=$state(false);
  let myVoiceRecording = $state(false);


  const snapshot = $derived(applicationState.snapshot);
  const voiceRecordingLock = $derived(
    myVoiceRecording || snapshot?.resources.owner_kind === "voice_recording",
  );
  const setupRequired = $derived(applicationState.setupRequired);
  const setupSettings = $derived(applicationState.setupSettings);
  const meetingStatus = $derived(snapshot?.meetingSession ?? null);
  const displayedNotice = $derived(visibleAppNotice(notice, snapshot, liveHealthNotice, longSessionNotice));

  const presence = $derived(
    snapshot?.meeting.lifecycle === "unavailable"
      ? "Unavailable"
      : snapshot?.meeting.live
        ? "Live"
        : snapshot?.meeting.hasSession || snapshot?.meeting.canStop
          ? snapshot.meeting.label
      : snapshot?.readiness.level === "unavailable"
        ? "Unavailable"
        : snapshot?.readiness.meetingReady
          ? "Ready"
          : snapshot?.readiness.level === "blocked"
              ? "Setup Needed"
              : "Checking",
  );

  const closePrimaryLabel = $derived(closeState.action === "retry" ? "Try Again" : closeState.busy ? "Stopping..." : "Stop & Close");
  // Stable boolean dependencies avoid restarting listeners on each refreshed session snapshot.
  const meetingMonitorsActive = $derived(
    !booting && !setupRequired &&
    (snapshot?.resources.owner_kind === "meeting" || closeState.closeAfterExistingStop),
  );
  const meetingReliabilityEnabled = $derived(
    Boolean(snapshot?.meeting.applicationOwned && snapshot.meeting.hasSession),
  );

  function setNotice(message: string): void {
    // Existing feature callbacks provide text without verified severity.
    notice = appNotice(message);
  }

  function setKnownNotice(message: string, kind: AppNoticeKind): void {
    notice = appNotice(message, kind);
  }

  function updateHealthNotice(channel: ReliabilityChannel, entry: ReliabilityNotice | null): void {
    const next = entry ? appNotice(entry.message, entry.kind) : null;
    if (channel === "live") liveHealthNotice = next;
    else longSessionNotice = next;
  }

  function navigate(next: AppRoute): void {
    if (voiceRecordingLock && next !== "my-voice") {
      setKnownNotice("Stop the current My Voice recording before leaving My Voice.", "attention");
      return;
    }
    if (next === "meeting" && (snapshot?.meeting.live || snapshot?.meeting.canStop || snapshot?.meeting.hasSession || snapshot?.meeting.lifecycle === "cleanup_incomplete")) {
      // Global Live/Stop Needed return always opens the native active owner.
      meetingSection = "live";
      selectedSessionId = meetingStatus?.session_id ?? null;
      selectedSurface = "recent";
    }
    route = next;
  }

  function openMeetingSession(id: string, source: "recent" | "project" = "recent"): void {
    if (voiceRecordingLock && route !== "meeting") {
      setKnownNotice("Stop the current My Voice recording before leaving My Voice.", "attention");
      return;
    }
    // A running session is always rendered from the native Live owner.
    if (snapshot?.meeting.live && meetingStatus?.session_id === id) {
      selectedSessionId = id;
      selectedSurface = source;
      meetingSection = "live";
    } else if (library?.sessions.some(s => s.session_id === id)) {
      selectedSessionId = id;
      selectedSurface = source;
      meetingSection = "session";
    }
    route = "meeting";
  }

  function openMeetingSection(next: "live" | "last" | "saved" | "session"): void {
    if (voiceRecordingLock && route !== "meeting") {
      setKnownNotice("Stop the current My Voice recording before leaving My Voice.", "attention");
      return;
    }
    if (next === "last" && (library?.sessions.length ?? 0) > 0) {
      openMeetingSession(library.sessions[0].session_id);
      return;
    }
    meetingSection = next;
    if (next !== "session") {
      selectedSessionId = next === "live" ? meetingStatus?.session_id ?? null : null;
      selectedSurface = "recent";
    }
    route = "meeting";
  }

  function reconcileSelectedMeetingLocation(current: MeetingLibrary): void {
    // Project category deletion / moving to Uncategorized must not leave
    // an active highlight in a Project that no longer owns this session.
    if (current.ok && selectedSurface === "project" && selectedSessionId
      && !current.sessions.some(s => s.session_id === selectedSessionId)) {
      selectedSurface = "recent";
    }
  }

  async function refreshMeetingLibrary(): Promise<void> {
    const result = await runtimeApi.listMeetingLibrary();
    library = result;
    reconcileSelectedMeetingLocation(result);
  }

  async function changeMeetingLibrary(change: Omit<MeetingLibraryChange, "expectedRevision">): Promise<boolean> {
    if (libraryBusy || !library?.ok) {
      setKnownNotice("Meeting library is unavailable. Refresh Saved Meeting before editing.", "attention");
      return false;
    }
    libraryBusy = true;
    try {
      const result = await runtimeApi.updateMeetingLibrary({ ...change, expectedRevision: library.revision });
      if (!result.ok) {
        setKnownNotice(result.message || "Could not verify Meeting changes.", "attention");
        await refreshMeetingLibrary();
        return false;
      }
      library = result;
      reconcileSelectedMeetingLocation(result);
      setNotice("Meeting library updated.");
      return true;
    } catch {
      setKnownNotice("Meeting changes could not be verified. Refresh before retrying.", "unverified");
      await refreshMeetingLibrary();
      return false;
    } finally { libraryBusy = false; }
  }

  async function deleteMeetingChannel(id: string): Promise<boolean> {
    if (libraryBusy || !library?.ok || !library.sessions.some(s => s.session_id === id)) return false;
    libraryBusy = true;
    try {
      const result = await runtimeApi.deleteSavedMeetingTranscript(id);
      await refreshMeetingLibrary();
      if (!result.ok) {
        setKnownNotice(result.message, "attention");
        return false;
      }
      if (selectedSessionId === id) {
        selectedSessionId = null;
        selectedSurface = "recent";
        meetingSection = "live";
      }
      setNotice("Saved Meeting deleted.");
      return true;
    } catch {
      setKnownNotice("Deletion was not confirmed. Check Saved Meeting.", "unverified");
      await refreshMeetingLibrary();
      return false;
    } finally { libraryBusy = false; }
  }

  async function toggleMeetingMute(): Promise<void> {
    if (muteBusy || !snapshot?.meeting.live || !meetingStatus?.session_id) return;
    muteBusy = true;
    try {
      const desired = !meetingStatus.muted;
      const result = await runtimeApi.setMeetingOutputMuted(desired);
      if (!result?.ok) {
        setKnownNotice(result?.message ?? "Mute status could not be verified. Check Meeting before retrying.", "unverified");
        return;
      }
      await refreshSnapshot(result.message);
    } catch {
      setKnownNotice("Mute status could not be verified. Check Meeting before retrying.", "unverified");
    } finally {
      muteBusy = false;
    }
  }

  function applyMeetingStatus(status: MeetingSessionStatus, preferredNotice?: string, kind: AppNoticeKind = "info"): void {
    const ownershipChanged = applicationController.applyMeetingSession(status);
    applicationState = applicationController.read();
    if (preferredNotice) setKnownNotice(preferredNotice, kind);
    if (ownershipChanged) void refreshSnapshot();
  }

  function applyRefreshResult(
    result: Awaited<ReturnType<typeof applicationController.refresh>>,
    preferredNotice?: string,
  ): void {
    applicationState = applicationController.read();
    if (!result.applied) return;
    // A canonical application snapshot supersedes an earlier Meeting-only
    // poll. Do not let that poll reapply stale Live/cleanup or caption state.
    meetingViewState = meetingLiveController.invalidate();

    if (result.setupRequired) {
      meetingViewState = meetingLiveController.reset();
      if (preferredNotice) setNotice(preferredNotice);
      else setKnownNotice("Continue Meeting setup to use voice translation.", "attention");
      return;
    }

    const next = result.snapshot;
    if (!next) return;
    if (result.previousSessionId !== result.nextSessionId) {
      // Quick Access and native notifications can Start/Stop outside this view.
      // Reconcile the saved channel list only on an ownership transition.
      void refreshMeetingLibrary();
      if (route === "meeting" && meetingSection === "live" && next.meeting.live) {
        selectedSessionId = next.meetingSession?.session_id ?? null;
        selectedSurface = "recent";
      }
    }
    if (!next.meeting.hasSession || result.previousSessionId !== result.nextSessionId) {
      meetingViewState = meetingLiveController.reset();
    }
    if (preferredNotice) {
      setNotice(preferredNotice);
    } else {
      const kind: AppNoticeKind = next.meeting.lifecycle === "unavailable" || next.readiness.level === "unavailable"
        ? "unverified" : next.readiness.level === "checking" ? "progress"
        : next.meeting.live || next.readiness.level === "ready" ? "success" : "info";
      setKnownNotice(next.meeting.hasSession ? next.meeting.message : next.readiness.summary, kind);
    }
  }

  async function refreshSnapshot(preferredNotice?: string, knownSettings?: RuntimeSettings): Promise<void> {
    try {
      applyRefreshResult(
        await applicationController.refresh(knownSettings),
        preferredNotice,
      );
    } catch {
      setKnownNotice("TranslateIT couldn't confirm its current status. Check Meeting or Help before retrying.", "unverified");
    }
  }

  async function refreshFromApplicationEvent(
    application: ApplicationSnapshot,
    reason: string,
  ): Promise<void> {
    try {
      applyRefreshResult(
        await applicationController.refreshFromApplication(application, reason),
      );
    } catch {
      setKnownNotice("TranslateIT couldn't confirm its latest runtime status. Check Meeting or Help.", "unverified");
    }
  }

  async function applySettings(next: RuntimeSettings): Promise<void> {
    applicationController.applySettings(next);
    applicationState = applicationController.read();
  }

  async function syncMeetingVoice(message?: string): Promise<void> {
    voiceSelectionRevision += 1;
    await refreshSnapshot(message ?? "Meeting voice updated.");
  }

  async function finishFirstSetup(
    next: RuntimeSettings,
  ): Promise<void> {
    booting = true;
    try {
      applicationController.applySettings(next);
      applicationState = applicationController.read();
      route = "meeting";
      await refreshSnapshot(
        "Setup saved.",
        next,
      );
    } finally {
      booting = false;
    }
  }

  async function openMyVoiceFromSetup(next: RuntimeSettings): Promise<void> {
    applicationController.applySettings(next);
    applicationState = applicationController.read();
    await refreshSnapshot("Choose a Meeting voice before starting translation.", next);
    route = "my-voice";
  }

  async function handleMeetingAction(source: "original" | "translateit" = "translateit"): Promise<void> {
    if (meetingActionBusy || !snapshot) return;
    const control = meetingControlState(
      snapshot.meeting,
      snapshot.readiness.approvedVoiceReady,
      snapshot.settings.meeting_setup_state,
      meetingActionBusy,
    );
    const startOriginal = source === "original"
      && !snapshot.meeting.hasSession && snapshot.meeting.lifecycle === "idle"
      && snapshot.readiness.microphoneReady === true
      && snapshot.readiness.meetingRouteReady === true;
    if ((!control.enabled || !control.action) && !startOriginal) {
      setKnownNotice(snapshot.meeting.canStop ? snapshot.meeting.message : snapshot.readiness.nextAction ?? snapshot.meeting.message,
        snapshot.meeting.lifecycle === "unavailable" || snapshot.readiness.level === "unavailable" ? "unverified" : "attention");
      return;
    }
    const action = startOriginal ? "start" : control.action;
    if (!action) return;
    const previousSessionId = meetingStatus?.session_id ?? null;

    // Fence an in-flight Meeting-only poll before this action changes native
    // ownership. Application events may still reconcile a newer state first.
    const actionControllerRevision = applicationController.read().revision;
    meetingViewState = meetingLiveController.invalidate();
    meetingActionBusy = true;
    setKnownNotice(action === "start"
      ? source === "original" ? "Starting original microphone..." : "Starting translation..."
      : "Stopping microphone...", "progress");
    try {
      const result = await runtimeProductFacade.runProductMeetingAction(action, source);
      const resultNotice = result.meeting.lifecycle === "cleanup_incomplete" || result.state === "cleanup_incomplete"
        ? "Translation output stopped, but cleanup needs attention. Retry Stop."
        : result.meeting.lifecycle === "unavailable" || result.state === "frontend_bridge_error"
          ? "Meeting status is unknown. Check its status before trying another action."
          : result.ok && action === "start" && result.meeting.live
            ? source === "original" ? "Original microphone is live." : "Translation is live."
            : result.ok && action === "stop" && !result.meeting.hasSession && result.meeting.lifecycle === "idle"
              ? "Meeting audio stopped."
              : result.meeting.message;
      if (applicationController.read().revision !== actionControllerRevision) {
        // A native application event or newer refresh won the race. The
        // action receipt has no application revision and cannot overwrite it.
        await refreshSnapshot();
        return;
      }
      const kind: AppNoticeKind = result.meeting.lifecycle === "cleanup_incomplete" || result.state === "cleanup_incomplete"
        ? "attention"
        : result.meeting.lifecycle === "unavailable" || result.state === "frontend_bridge_error"
          ? "unverified"
          : result.ok && ((action === "start" && result.meeting.live)
            || (action === "stop" && !result.meeting.hasSession && result.meeting.lifecycle === "idle"))
            ? "success" : result.ok ? "info" : "attention";
      applyMeetingStatus(result.status, resultNotice, kind);
      if (result.ok && action === "start" && result.meeting.live && result.status.session_id) {
        selectedSessionId = result.status.session_id;
        selectedSurface = "recent";
        meetingSection = "live";
        route = "meeting";
      } else if (result.ok && action === "stop" && result.meeting.lifecycle === "idle"
        && !result.meeting.hasSession && previousSessionId) {
        selectedSessionId = previousSessionId;
        selectedSurface = "recent";
        meetingSection = "session";
        route = "meeting";
      }
      void refreshMeetingLibrary();
      if (!result.status.has_session || action === "start") {
        meetingViewState = meetingLiveController.reset();
      }
    } catch {
      setKnownNotice("The Meeting action couldn't be confirmed. Check its status before trying again.", "unverified");
      await refreshSnapshot();
    } finally {
      meetingActionBusy = false;
    }
  }

  async function runSetupAction(action: ProductSetupAction): Promise<void> {
    if (setupActionBusy) return;
    setupActionBusy = true;
    setKnownNotice(action === "verify-models" ? "Checking translation files..." : "Checking setup...", "progress");
    try {
      const message = await runtimeProductFacade.runProductSetupAction(action);
      await refreshSnapshot(message);
    } catch {
      setNotice("The setup check could not finish. Open Help or try again.");
    } finally {
      setupActionBusy = false;
    }
  }

  async function fixSetup(): Promise<void> {
    if (setupActionBusy) return;
    setupActionBusy = true;
    setKnownNotice("Checking setup...", "progress");
    try {
      const message = await runtimeProductFacade.runProductRecoveryAction("fix-setup");
      await refreshSnapshot(message);
    } catch {
      setNotice("Repair Setup could not finish. Check your devices and try again.");
    } finally {
      setupActionBusy = false;
    }
  }

  async function toggleMicTest(): Promise<void> {
    if (micTestBusy || !snapshot) return;
    const ownerKind = snapshot.resources.owner_kind;
    const micTestOwnsRuntime = ownerKind === "mic_test";

    if (snapshot.resources.audio_locked && !micTestOwnsRuntime) {
      const message = ownerKind === "meeting"
        ? "Stop Meeting translation before using Mic Test."
        : ownerKind === "voice_recording"
          ? "Stop My Voice recording before using Mic Test."
          : "Finish the current audio action before using Mic Test.";
      setKnownNotice(message, "attention");
      return;
    }

    if (!snapshot.resources.microphone_available && !micTestOwnsRuntime) {
      setKnownNotice("Microphone setup isn't ready yet.", "attention");
      return;
    }

    micTestBusy = true;
    try {
      const result = await applicationRuntimeApi.dispatchIntent(
        micTestOwnsRuntime ? "stop_mic_test" : "start_mic_test",
      );
      await refreshSnapshot(result.ok
        ? (micTestOwnsRuntime ? "Mic Test stopped." : "Mic Test started.")
        : result.message || "Mic Test couldn't be completed. Try again.");
    } catch {
      setNotice("Mic Test couldn't be completed. Check your microphone and try again.");
    } finally {
      micTestBusy = false;
    }
  }

  async function pollMeeting(forceTurns = false): Promise<void> {
    const pollControllerRevision = applicationController.read().revision;
    try {
      meetingViewState = await meetingLiveController.reconcile(
        {
          ownerKind: snapshot?.resources.owner_kind ?? null,
          booting,
          setupRequired,
          closeAfterExistingStop: closeState.closeAfterExistingStop,
        },
        {
          onStatus: (status) => {
            if (applicationController.read().revision !== pollControllerRevision) {
              // The observed Meeting response is older than a committed
              // application/action update; reject its status and overlay.
              meetingViewState = meetingLiveController.invalidate();
              return;
            }
            applyMeetingStatus(status);
          },
          onNotice: setNotice,
          onStoppedForClose: async () => {
            closeState = await closeController.closeWindow();
          },
        },
        forceTurns,
      );
    } catch {
      setKnownNotice("Couldn't refresh the meeting display. Translation status is unknown; open Help if this continues.", "unverified");
    }
  }

  async function inspectNativeCloseRequest(): Promise<void> {
    closeState = await closeController.inspect();
  }

  async function handleCloseDialogPrimary(): Promise<void> {
    closeState = await closeController.primary();
  }

  function keepApplicationOpen(): void {
    closeState = closeController.keepOpen();
  }

  onMount(() => {
    void appUpdateApi.checkAtStartupOnce().catch(() => null);
    const onQuickAccessShortcut = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === "k") {
        event.preventDefault();
        void toggleQuickAccess().catch(() => setNotice("Quick Access couldn't be opened."));
      }
    };
    window.addEventListener("keydown", onQuickAccessShortcut);

    let disposed = false;
    let unlistenClose: (() => void) | null = null;
    let unlistenApplicationRuntime: (() => void) | null = null;
    let unlistenOverlayReady: (() => void) | null = null;
    let unlistenQuickAccessCaptions: (() => void) | null = null;
    discardLegacyOverlayCaption();
    const stopVoiceBuildRuntimeSync = startVoiceBuildRuntimeSync((message) => refreshSnapshot(message));
    // Cold worker/model imports may take tens of seconds. Render the
    // authoritative saved-setup + application snapshot before starting them;
    // a native event/result reconciles readiness after the worker completes.
    // No second worker, retry loop, deadline change or optimistic ready state.
    const boot = async () => {
      let initialSnapshot: ApplicationSnapshot | null = null;
      try {
        const loadedSettings = await runtimeApi.loadSettings();
        if (!loadedSettings) {
          setKnownNotice("TranslateIT can't reach the desktop runtime yet. Try again when it is available.", "unverified");
          return;
        }
        if (disposed) return;
        applicationController.applySettings(loadedSettings);
        applicationState = applicationController.read();
        if (loadedSettings.meeting_setup_state !== "new") {
          initialSnapshot = await applicationRuntimeApi.getSnapshot();
          if (initialSnapshot.revision <= 0 || initialSnapshot.lifecycle === "unavailable") {
            throw new Error("Initial application state was unavailable.");
          }
          if (disposed) return;
          applyRefreshResult(
            await applicationController.refreshFromApplication(
              initialSnapshot, "initial_shell", loadedSettings,
            ),
          );
        }
      } catch {
        if (!disposed) setKnownNotice("Could not load your settings or desktop status. Select Try Again to check the runtime.", "unverified");
      } finally {
        if (disposed) return;
        booting = false;
        void refreshMeetingLibrary();
        // The initial page is no longer gated by Python worker readiness.
        // Only a truly not-started worker gets one automatic start attempt.
        // A blocked or failed worker requires the existing explicit recovery.
        if (
          initialSnapshot?.helper.state === "not_started"
          && initialSnapshot.settings.translation_engine === "local"
          && applicationController.read().snapshot?.helper?.state === "not_started"
        ) {
          setKnownNotice("Preparing the local AI worker. Meeting and Settings remain accessible while it starts.", "progress");
          void (async () => {
            const outcome = await applicationRuntimeApi.dispatchIntent("ensure_runtime_ready");
            if (disposed) return;
            if (outcome.snapshot.revision > 0 && outcome.snapshot.lifecycle !== "unavailable") {
              await refreshFromApplicationEvent(outcome.snapshot, "startup_worker_result");
            }
            if (disposed) return;
            if (!outcome.ok) {
              setKnownNotice(outcome.message, "attention");
            } else {
              const current = applicationController.read().snapshot;
              if (current?.helper?.state === "ready" && !current.meeting.hasSession) {
                setNotice(current.readiness.summary);
              }
            }
          })().catch(() => {
            if (!disposed) setKnownNotice("Could not confirm local AI startup. Check Help before retrying the worker.", "unverified");
          });
        }
      }
    };

    const installCloseGuard = async () => {
      try {
        const stop = await installNativeCloseGuard(inspectNativeCloseRequest);
        if (disposed) {
          stop();
          return;
        }
        unlistenClose = stop;
      } catch {}
    };

    const installApplicationRuntimeSync = async () => {
      try {
        const stop = await applicationRuntimeApi.subscribe((event) => {
          if (disposed || event.snapshot.revision <= 0) return;
          void refreshFromApplicationEvent(event.snapshot, event.reason);
        });
        if (disposed) {
          stop();
          return;
        }
        unlistenApplicationRuntime = stop;
      } catch {}
    };



    void boot();
    void installCloseGuard();
    void installApplicationRuntimeSync();
    void subscribeOverlayReady().then((stop) => {
      if (disposed) stop();
      else unlistenOverlayReady = stop;
    }).catch(() => { /* Live event delivery remains available. */ });
    void subscribeQuickAccessCaptionRequests().then(stop => {
      if (disposed) stop();
      else unlistenQuickAccessCaptions = stop;
    }).catch(() => { /* Quick Access reports a missing caption owner. */ });

    return () => {
      disposed = true;
      window.removeEventListener("keydown", onQuickAccessShortcut);
      unlistenClose?.();
      unlistenApplicationRuntime?.();
      unlistenOverlayReady?.();
      unlistenQuickAccessCaptions?.();
      stopVoiceBuildRuntimeSync();
    };
  });

  $effect(() => {
    if (!meetingMonitorsActive) {
      liveHealthNotice = null;
      longSessionNotice = null;
      return;
    }
    const stop = startMeetingRuntimeMonitors(pollMeeting, updateHealthNotice, meetingReliabilityEnabled);
    return () => {
      stop();
      liveHealthNotice = null;
      longSessionNotice = null;
    };
  });
</script>

{#if booting}
  <main class="grid min-h-screen place-items-center bg-[var(--ti-bg)] p-8">
    <section class="w-full max-w-[560px] text-center">
      <div class="mx-auto w-fit"><TranslateITMark size={48} /></div>
      <h1 class="mb-0 mt-5 text-2xl font-semibold tracking-[-0.03em]">Getting TranslateIT ready</h1>
      <p class="mb-0 mt-2 text-sm leading-6 text-[var(--ti-text-muted)]">Checking your saved setup.</p>
    </section>
  </main>
{:else if setupRequired}
  <FirstSetup initialSettings={setupSettings} onComplete={finishFirstSetup} onOpenMyVoice={openMyVoiceFromSetup} />
{:else if snapshot}
  <main class="flex h-screen min-h-0 bg-[var(--ti-bg)]">
    <Sidebar
      active={route} {presence} {meetingSection} {settingsTab}
      meetingLive={snapshot.meeting.live} meetingMuted={Boolean(meetingStatus?.muted)}
      {muteBusy} {voiceSelectionRevision} settings={snapshot.settings}
      resourcesLocked={snapshot.resources.audio_locked}
      voiceSelectionLocked={voiceRecordingLock || snapshot.voiceBuildActive || snapshot.resources.audio_locked}
      onNavigate={navigate}
      onMeetingSection={openMeetingSection}
      {library} {selectedSessionId} {selectedSurface}
      liveSessionId={snapshot.meeting.live ? meetingStatus?.session_id ?? null : null}
      onOpenSession={openMeetingSession}
      onLibraryChange={changeMeetingLibrary}
      onDeleteSession={deleteMeetingChannel}
      onSettingsTab={(tab) => { settingsTab = tab; }}
      onToggleMute={toggleMeetingMute}
      onSettingsChange={applySettings}
      onRefresh={refreshSnapshot}
      onVoiceChanged={syncMeetingVoice}
      onNotice={setNotice}
    />

    <section class="flex min-w-0 flex-1 flex-col">
      <AppHeader
        {snapshot}
        {route}
        {meetingSection}
        selectedSessionName={library?.sessions.find(s => s.session_id === selectedSessionId)?.display_name ?? null}
        notice={displayedNotice}
        myVoiceRecording={voiceRecordingLock}
        onNavigate={navigate}
        onNotice={setNotice}
        onRefresh={refreshSnapshot}
      />

      <div class={"min-h-0 flex-1 " + (route === "meeting" ? "overflow-hidden" : "overflow-y-auto")}>
        {#if route === "meeting"}
          <Meeting
            section={meetingSection}
            selectedSession={library?.sessions.find(s => s.session_id === selectedSessionId) ?? null}
            onLibraryRefresh={refreshMeetingLibrary}
            onOpenSession={(id) => openMeetingSession(id, "recent")}
            {snapshot}
            {meetingStatus}
            meetingTurns={meetingViewState.turns}
            actionBusy={meetingActionBusy}
            onMeetingAction={handleMeetingAction}
            onRefresh={refreshSnapshot}
            onNavigate={navigate}
          />
        {:else if route === "my-voice"}
          <MyVoice
            voiceChangesLocked={snapshot.resources.audio_locked || snapshot.voiceBuildActive}
            onNotice={setNotice}
            onMeetingVoiceChanged={syncMeetingVoice}
            onRecordingChange={(recording) => { myVoiceRecording = recording; }}
          />
        {:else}
          <Settings
            tab={settingsTab}
            {snapshot}
            settings={snapshot.settings}
            setupBusy={setupActionBusy}
            {micTestBusy}
            onSettingsChange={applySettings}
            onRefresh={refreshSnapshot}
            onSetupAction={runSetupAction}
            onFixSetup={fixSetup}
            onMicTest={toggleMicTest}
            onNotice={setNotice}
          />
        {/if}
      </div>
    </section>
  </main>
{:else}
  <main class="grid min-h-screen place-items-center bg-[var(--ti-bg)] p-8">
    <section class="ti-panel w-full max-w-[560px] p-7">
      <TranslateITMark size={40} />
      <span class="ti-kicker mt-4 block">TranslateIT</span>
      <h1 class="mb-0 mt-3 text-2xl font-semibold">TranslateIT isn't ready yet</h1>
      <p class="mb-0 mt-3 text-sm leading-6 text-[var(--ti-text-muted)]">{notice.message}</p>
      <button type="button" class="ti-button mt-5" onclick={() => { setKnownNotice("Checking again...", "progress"); void refreshSnapshot(); }}>Try Again</button>
    </section>
  </main>
{/if}

<NativeCloseDialog
  open={closeState.open}
  title={closeState.title}
  message={closeState.message}
  action={closeState.action}
  busy={closeState.busy}
  primaryLabel={closePrimaryLabel}
  onOpenChange={(open) => {
    if (!open) closeState = closeController.keepOpen();
  }}
  onKeepOpen={keepApplicationOpen}
  onPrimary={handleCloseDialogPrimary}
/>
