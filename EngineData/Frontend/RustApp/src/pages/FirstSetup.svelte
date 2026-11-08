<script lang="ts">
  import { onMount, tick } from "svelte";
  import { myVoiceBuildApi } from "../app/bridge/myVoiceBuildApi";
  import { classifyVirtualMicRoute } from "../app/bridge/virtualMicRoutePresentation";
  import { runtimeApi, type VirtualMicRouteContractStatus } from "../app/bridge/runtimeApi";
  import {
    runtimeProductFacade,
    type ProductRuntimeSnapshot,
  } from "../app/bridge/runtimeProductFacade";
  import {
    persistedSetupCheckpoint,
    safeSetupResumeStep,
    setupCheckpoint,
    type SetupStep,
  } from "../app/runtime/setupFlow";
  import { cloneSettings, compact, deviceId } from "../app/shared/state";
  import type { AudioDeviceListReport, RuntimeSettings } from "../app/shared/types";
  import SetupNavigation from "../components/setup/SetupNavigation.svelte";
  import MeetingTranslationEngine from "../components/settings/MeetingTranslationEngine.svelte";
  import SetupVoiceChoice from "../components/setup/SetupVoiceChoice.svelte";
  import StatusRow from "../components/ui/StatusRow.svelte";
  import TranslateITMark from "../components/brand/TranslateITMark.svelte";

  type SetupState = "new" | "deferred" | "completed";
  type BuiltinVoiceId = "MaleVoice" | "FemaleVoice";

  let {
    initialSettings,
    onComplete,
    onOpenMyVoice,
  }: {
    initialSettings: RuntimeSettings;
    onComplete: (settings: RuntimeSettings) => void | Promise<void>;
    onOpenMyVoice: (settings: RuntimeSettings) => void | Promise<void>;
  } = $props();

  let settings = $state<RuntimeSettings>((() => cloneSettings(initialSettings))());
  let snapshot = $state<ProductRuntimeSnapshot | null>(null);
  let routeStatus = $state<VirtualMicRouteContractStatus | null>(null);
  // Presentation state is distinct from the native readiness snapshot.
  let setupReadState = $state<"checking" | "verified" | "unavailable">("checking");
  let setupReadRevision = 0;
  let microphonesLoading = $state(false);
  let microphonesChecked = $state(false);
  let devices = $state<AudioDeviceListReport | null>(null);
  let step = $state<SetupStep>((() => setupCheckpoint(settings.meeting_setup_checkpoint))());
  let busy = $state(false);
  let checkingSetup = $state(false);
  let message = $state("");
  let selectedMicrophone = $state((() => String(settings.audio.input_device_id ?? ""))());
  let builtinPendingId = $state<BuiltinVoiceId | null>(null);
  let checkMicrophoneSelectionButton = $state<HTMLButtonElement>();

  const meetingVoiceReady = $derived(snapshot?.readiness.approvedVoiceReady ?? null);
  const engineSettingsUnverified = $derived(
    snapshot?.meetingSession?.preflight?.blockers?.includes("meeting_session:settings_unverified") === true,
  );
  const routeCheck = $derived(setupReadState === "unavailable" ? "unavailable" : classifyVirtualMicRoute(routeStatus));
  // An unsaved device choice is not the verified microphone in the runtime snapshot.
  const microphoneSelectionPending = $derived(
    selectedMicrophone.trim() !== String(settings.audio.input_device_id ?? "").trim(),
  );

  function microphoneDevices() {
    return devices?.input_devices ?? [];
  }

  function currentMicrophoneId(): string {
    return String(settings.audio.input_device_id ?? "").trim();
  }

  function savedMicrophoneMissing(): boolean {
    const current = currentMicrophoneId();
    return Boolean(current && devices?.ok && !microphoneDevices().some((device) => deviceId(device) === current));
  }

  function currentMicrophone(): string {
    return compact(snapshot?.inputStatus?.selected_device_name ?? settings.audio.input_device_id, "Windows Default");
  }

  function currentMeetingMicrophone(): string {
    if (routeCheck === "unavailable") return "Unable to check in Windows";
    if (routeCheck === "checking") return "Checking...";
    return compact(routeStatus?.selected_input_device,
      routeCheck === "ready" ? "Device name unavailable — check Settings" : "Not available in Windows");
  }

  function currentMeetingSound(): string {
    return compact(settings.audio.output_device_id, "Windows Default");
  }

  function builtinLabel(voiceId: BuiltinVoiceId): string {
    return voiceId === "MaleVoice" ? "Built-in Male" : "Built-in Female";
  }

  async function refreshSnapshot(): Promise<void> {
    const revision = ++setupReadRevision;
    setupReadState = "checking";
    // Never display a previous route name as if the current read verified it.
    routeStatus = null;
    try {
      const [nextSnapshot, nextRouteStatus] = await Promise.all([
        runtimeProductFacade.loadProductRuntimeSnapshot(),
        runtimeApi.getVirtualMicRouteStatus(),
      ]);
      if (revision !== setupReadRevision) return;
      snapshot = nextSnapshot;
      routeStatus = nextRouteStatus;
      settings = cloneSettings(nextSnapshot.settings);
      selectedMicrophone = String(settings.audio.input_device_id ?? "");
      setupReadState = "verified";
      if (message === "Couldn't check setup. Try again.") message = "";
    } catch {
      if (revision !== setupReadRevision) return;
      routeStatus = null;
      setupReadState = "unavailable";
      message = "Couldn't check setup. Try again.";
    }
  }

  async function refreshDevices(): Promise<void> {
    if (microphonesLoading) return;
    microphonesLoading = true;
    try {
      devices = await runtimeProductFacade.loadProductAudioDevices();
      if (devices.ok && message === "Microphones could not be checked. Select Refresh microphones.") message = "";
      if (!devices.ok && !message) message = "Microphones could not be checked. Select Refresh microphones.";
    } catch {
      devices = null;
      if (!message) message = "Microphones could not be checked. Select Refresh microphones.";
    } finally {
      microphonesChecked = true;
      microphonesLoading = false;
    }
  }

  async function initialize(): Promise<void> {
    if (checkingSetup) return;
    checkingSetup = true;
    try {
      // Even Step 1 must discover a corrupt saved engine before Continue/Defer.
      await refreshSnapshot();
      if (setupReadState !== "verified") return;
      if (step > 1) {
        step = safeSetupResumeStep(step, {
          microphoneReady: snapshot?.readiness.microphoneReady === true,
          meetingRouteReady: snapshot?.readiness.meetingRouteReady === true,
        });
        if (step === 2) await refreshDevices();
      }
    } finally {
      checkingSetup = false;
    }
  }

  async function confirmEngineSelection(confirmed: RuntimeSettings): Promise<void> {
    settings = cloneSettings(confirmed);
    // Recovery can restore the previous setup checkpoint/state as well as its
    // engine. Refresh native truth before leaving the recovery-only UI.
    await refreshSnapshot();
    if (setupReadState !== "verified" || !snapshot?.meetingSession?.preflight
        || snapshot.meetingSession.preflight.blockers.includes("meeting_session:settings_unverified")) {
      message = "Engine confirmation was saved, but setup status could not be verified. Check setup status.";
      return;
    }
    const restored = cloneSettings(snapshot.settings);
    settings = restored;
    if (restored.meeting_setup_state !== "new") {
      // A previously completed/deferred setup must not be overwritten by a
      // second wizard simply because the engine field was once unreadable.
      await onComplete(restored);
      return;
    }
    step = setupCheckpoint(restored.meeting_setup_checkpoint);
    if (step > 1) {
      step = safeSetupResumeStep(step, {
        microphoneReady: snapshot.readiness.microphoneReady === true,
        meetingRouteReady: snapshot.readiness.meetingRouteReady === true,
      });
      if (step === 2) await refreshDevices();
    }
  }

  async function persistSetupFact(state: SetupState, nextCheckpoint: SetupStep): Promise<boolean> {
    if (snapshot === null || engineSettingsUnverified) {
      message = "Saved engine settings must be verified before setup can be saved. Confirm the translation engine first.";
      return false;
    }
    if (setupReadState !== "verified") {
      message = "Saved setup status is not verified. Select Check setup status before saving or leaving setup.";
      return false;
    }
    const expected = cloneSettings(settings);
    const candidate = cloneSettings(expected);
    let savedByNative = false;
    candidate.meeting_setup_state = state;
    const currentCheckpoint = setupCheckpoint(candidate.meeting_setup_checkpoint);
    const effectiveStep = Math.max(currentCheckpoint, nextCheckpoint) as SetupStep;
    candidate.meeting_setup_checkpoint = persistedSetupCheckpoint(effectiveStep);
    try {
      const result = await runtimeApi.saveSettings(candidate, expected);
      if (!result.ok) {
        message = result.message || "Setup settings changed. Refresh setup before saving.";
        return false;
      }
      savedByNative = true;
      const savedSettings = await runtimeApi.loadSettings();
      if (!savedSettings
        || savedSettings.meeting_setup_state !== state
        || savedSettings.meeting_setup_checkpoint !== candidate.meeting_setup_checkpoint) {
        // A successful native acknowledgement without an exact readback is not
        // authority to leave setup or reuse the candidate as persisted state.
        setupReadState = "unavailable";
        message = "Setup save was acknowledged, but its saved state could not be verified. Select Check setup status before another action.";
        return false;
      }
      settings = cloneSettings(savedSettings);
      return true;
    } catch {
      if (savedByNative) setupReadState = "unavailable";
      message = savedByNative
        ? "Setup save was acknowledged, but its current settings could not be read back. Select Check setup status before another change."
        : "Couldn't verify whether setup progress was saved. Check the current setup status before trying again.";
      return false;
    }
  }

  async function advance(nextStep: SetupStep): Promise<void> {
    if (busy) return;
    busy = true;
    message = "Saving...";
    const saved = await persistSetupFact("new", nextStep);
    if (saved) {
      step = nextStep;
      message = "";
      await refreshSnapshot();
      if (nextStep === 2) await refreshDevices();
    }
    busy = false;
  }

  function goBack(): void {
    if (busy || step <= 1) return;
    step = Math.max(1, step - 1) as SetupStep;
    message = "";
    if (step === 2) void refreshDevices();
  }

  async function deferSetup(): Promise<void> {
    if (busy) return;
    busy = true;
    message = "Saving...";
    try {
      const saved = await persistSetupFact("deferred", step);
      if (saved) await onComplete(settings);
    } finally {
      busy = false;
    }
  }

  async function saveSelectedMicrophone(): Promise<void> {
    if (busy) return;
    const candidate = selectedMicrophone.trim() || null;
    let savedByNative = false;

    busy = true;
    message = "Checking microphone...";
    try {
      const result = await runtimeProductFacade.selectProductAudioDevice("microphone", candidate, settings);
      settings = cloneSettings(result.settings);
      message = result.message;
      if (result.ok) {
        savedByNative = true;
        selectedMicrophone = String(settings.audio.input_device_id ?? "");
        await refreshDevices();
        await refreshSnapshot();
        message = setupReadState === "verified" ? result.message
          : "Windows acknowledged the microphone selection, but setup status could not be verified. Select Check selection.";
      }
    } catch {
      message = savedByNative
        ? "The microphone selection was acknowledged, but the display could not refresh. Select Check selection."
        : "Couldn't verify whether the microphone changed. Select Check selection before trying again.";
    } finally {
      busy = false;
    }
  }

  async function checkSavedMicrophoneSelection(): Promise<void> {
    if (busy || checkingSetup) return;
    const hadFocus = document.activeElement === checkMicrophoneSelectionButton;
    busy = true;
    message = "Checking saved microphone selection...";
    try {
      await refreshSnapshot();
      if (setupReadState === "verified") {
        message = snapshot?.readiness.microphoneReady === true
          ? "The saved microphone is reported ready. This does not test recording quality."
          : "The saved microphone is not verified ready. Check the device connection before saving another choice.";
      }
    } finally {
      busy = false;
      if (hadFocus && document.hasFocus()) {
        await tick();
        if (document.activeElement === document.body && checkMicrophoneSelectionButton) {
          checkMicrophoneSelectionButton.focus();
        }
      }
    }
  }

  async function repairSetup(): Promise<void> {
    if (busy) return;
    busy = true;
    message = "Fixing setup...";
    try {
      const result = await runtimeProductFacade.runProductRecoveryAction("fix-setup");
      await refreshSnapshot();
      message = setupReadState === "verified"
        ? compact(result, "Setup check finished.")
        : "Repair was requested, but setup status could not be verified. Select Check Again.";
    } catch {
      message = "Couldn't fix setup. Try again.";
    } finally {
      busy = false;
    }
  }

  async function checkSetupStatus(): Promise<void> {
    if (busy || checkingSetup) return;
    busy = true;
    message = "Checking saved setup status...";
    try {
      await refreshSnapshot();
      if (setupReadState !== "verified") return;
      if (engineSettingsUnverified) {
        message = "Saved engine settings need confirmation before setup can continue.";
        return;
      }
      // An earlier acknowledged Defer/Finish may have reached native storage.
      // Leave only after a fresh verified read shows that exact saved state.
      if (settings.meeting_setup_state === "deferred" || settings.meeting_setup_state === "completed") {
        await onComplete(settings);
        return;
      }
      message = "Saved setup status verified. Continue setup or choose Set Up Later.";
    } catch {
      message = "Couldn't finish checking setup status. Check the saved state before trying again.";
    } finally {
      busy = false;
    }
  }

  async function refreshReadiness(): Promise<void> {
    if (busy) return;
    busy = true;
    message = "Refreshing status...";
    try {
      const readinessMessage = step === 4 && setupReadState === "verified"
        ? await runtimeProductFacade.runProductSetupAction("check-readiness")
        : null;
      await refreshSnapshot();
      if (setupReadState !== "verified") return;
      message = meetingVoiceReady && snapshot?.readiness.meetingReady
        ? "Everything needed for Meeting translation is ready."
        : readinessMessage && readinessMessage !== "TranslateIT is ready."
          ? readinessMessage
          : "A few setup items still need attention.";
    } catch {
      message = "Couldn't refresh setup status. Try again.";
    } finally {
      busy = false;
    }
  }

  async function selectBuiltin(voiceId: BuiltinVoiceId, confirmed = false): Promise<void> {
    if (busy) return;
    let selectedByNative = false;
    busy = true;
    message = `Selecting ${builtinLabel(voiceId)}...`;
    try {
      const result = await myVoiceBuildApi.selectBuiltin(voiceId, confirmed);
      if (result.state === "approval_required") {
        builtinPendingId = voiceId;
        message = `Confirm before replacing the current Voice others hear with ${builtinLabel(voiceId)}.`;
        return;
      }
      builtinPendingId = null;
      message = result.message;
      if (result.ok) {
        selectedByNative = true;
        await runtimeProductFacade.runProductSetupAction("check-readiness");
        await refreshSnapshot();
        message = setupReadState === "verified"
          ? `${builtinLabel(voiceId)} is selected.`
          : "The voice selection was acknowledged, but setup status could not be verified. Select Check Again.";
      }
    } catch {
      message = selectedByNative
        ? "The voice selection was acknowledged, but its current setup status could not be checked. Select Check Again."
        : "Couldn't verify whether the Meeting voice changed. Select Check Again before selecting another voice.";
    } finally {
      busy = false;
    }
  }

  async function confirmPendingBuiltin(): Promise<void> {
    const voiceId = builtinPendingId;
    if (!voiceId) return;
    await selectBuiltin(voiceId, true);
  }

  async function openMyVoice(): Promise<void> {
    if (busy) return;
    busy = true;
    message = "Saving your choices...";
    const saved = await persistSetupFact("deferred", 4);
    busy = false;
    if (saved) await onOpenMyVoice(settings);
  }

  async function completeSetup(): Promise<void> {
    if (busy || !meetingVoiceReady || !snapshot?.readiness.meetingReady || routeCheck !== "ready") return;
    busy = true;
    message = "Saving...";
    const saved = await persistSetupFact("completed", 4);
    busy = false;
    if (saved) await onComplete(settings);
  }

  function selectValue(event: Event): string {
    return (event.currentTarget as HTMLSelectElement).value;
  }

  onMount(() => {
    void initialize();
  });
</script>

<main class="flex h-screen min-h-0 items-start justify-center overflow-y-auto bg-[var(--ti-bg)] px-4 py-6 sm:px-8 sm:py-10" aria-label="TranslateIT Setup">
  <section class="ti-panel w-full max-w-[900px] shrink-0 overflow-hidden">
    <header class="flex flex-wrap items-center gap-3 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4 sm:px-7">
      <TranslateITMark size={40} />
      <div>
        <strong class="block text-sm font-semibold">TranslateIT</strong>
        <span class="mt-0.5 block text-xs text-[var(--ti-text-muted)]">{step === 1 ? "Get started" : "Meeting setup"}</span>
      </div>
      {#if snapshot !== null && !engineSettingsUnverified}
        <div class="ml-auto min-w-44" role="progressbar" aria-label="Setup step" aria-valuemin="1" aria-valuemax="4" aria-valuenow={step}>
          <div class="text-[12px] font-semibold text-[var(--ti-text-muted)]">Step {step} of 4</div>
          <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-[var(--ti-border)]">
            <div class="h-full rounded-full bg-[var(--ti-accent)]" style={`width:${step * 25}%`}></div>
          </div>
        </div>
      {/if}
    </header>

    <section class="mx-auto grid max-w-[720px] gap-5 px-5 py-7 sm:px-8 sm:py-9">
      {#if snapshot === null}
        <p class="m-0 text-sm leading-6 text-[var(--ti-text-muted)]"
          role={setupReadState === "unavailable" ? "alert" : "status"}>
          {setupReadState === "unavailable"
            ? "Saved setup status could not be verified. Select Check setup status to retry. No settings were replaced."
            : "Checking saved engine settings before setup can continue."}
        </p>
        <button type="button" class="ti-button ti-button-secondary" aria-busy={checkingSetup}
          disabled={busy || checkingSetup} onclick={() => void initialize()}>
          {checkingSetup ? "Checking..." : "Check setup status"}
        </button>
      {:else if engineSettingsUnverified}
        <div>
          <span class="ti-kicker">Settings recovery</span>
          <h1 class="ti-page-title">Confirm the Meeting translation engine</h1>
          <p class="ti-page-copy">
            The saved engine settings could not be verified. Setup will not overwrite
            them with a default choice. Confirm an engine explicitly before continuing.
          </p>
        </div>
        <MeetingTranslationEngine
          {settings}
          needsReconfirmation={true}
          locked={busy}
          onSettingsChange={confirmEngineSelection}
          onNotice={(notice) => { message = notice; }}
        />
        {#if message}
          <p class="m-0 text-sm leading-6 text-[var(--ti-text-muted)]" role="status">{message}</p>
        {/if}
      {:else}
        {#if step === 1}
        <div>
          <span class="ti-kicker">Meeting setup</span>
          <h1 class="ti-page-title">Set up Meeting translation</h1>
          <p class="ti-page-copy">Follow four steps to select your microphone, configure your calling app, and choose an English voice. You can finish later.</p>
        </div>
      {:else if step === 2}
        <div>
          <span class="ti-kicker">Microphone</span>
          <h1 class="ti-page-title">Choose your microphone</h1>
          <p class="ti-page-copy">Select the microphone you speak into. If unsure, keep Windows Default. Save your choice before continuing.</p>
        </div>
        <div class="grid grid-cols-1 items-end gap-3 sm:grid-cols-[minmax(0,1fr)_auto]" aria-busy={microphonesLoading}>
          <label class="grid min-w-0 gap-2">
            <span class="ti-field-label">Microphone you speak into</span>
            <select class="ti-field min-h-11 px-3" disabled={busy || microphonesLoading || !devices?.ok || setupReadState !== "verified"} value={selectedMicrophone} onchange={(event) => { selectedMicrophone = selectValue(event); }}>
              <option value="">Windows Default</option>
              {#each microphoneDevices() as device (deviceId(device))}
                <option value={deviceId(device)}>{device.name}{device.is_default ? " · Windows default" : ""}</option>
              {/each}
              {#if savedMicrophoneMissing()}
                <option value={currentMicrophoneId()}>{currentMicrophoneId()} · unavailable</option>
              {/if}
            </select>
          </label>
          <button type="button" class="ti-button ti-button-secondary" disabled={busy || microphonesLoading || !devices?.ok || setupReadState !== "verified"} onclick={() => void saveSelectedMicrophone()}>Use this microphone</button>
        </div>
        <div class="flex flex-wrap items-center justify-between gap-3">
          {#if microphonesLoading}
            <p class="m-0 text-[13px] leading-5 text-[var(--ti-text-muted)]" role="status">Checking available microphones...</p>
          {:else if microphonesChecked && !devices?.ok}
            <p class="m-0 text-[13px] leading-5 text-[var(--ti-warning)]" role="alert">Microphone list could not be verified. The saved selection was not replaced.</p>
          {/if}
          <div class="flex flex-wrap gap-2">
            <button type="button" class="ti-button ti-button-secondary" disabled={busy || microphonesLoading}
              onclick={() => void refreshDevices()}>{microphonesLoading ? "Checking..." : "Refresh microphones"}</button>
            <button type="button" class="ti-button ti-button-secondary" bind:this={checkMicrophoneSelectionButton}
              aria-busy={busy || checkingSetup} disabled={busy || checkingSetup}
              onclick={() => void checkSavedMicrophoneSelection()}>Check selection</button>
          </div>
        </div>
        <div class="ti-subtle-card overflow-hidden">
          <StatusRow
            label="Selected microphone"
            value={setupReadState === "verified" ? currentMicrophone() : "Current microphone not verified"}
            detail={setupReadState !== "verified"
              ? "The previous selection is not current proof. Select Check selection to verify the saved microphone."
              : microphoneSelectionPending ? "The saved microphone is shown here. Save your new choice before continuing."
              : "The microphone currently saved for your calls."}
            status={setupReadState !== "verified" ? setupReadState === "checking" ? "Checking" : "Not verified"
              : microphoneSelectionPending ? "Not saved" : snapshot?.readiness.microphoneReady ? "Ready"
                : snapshot?.readiness.level === "checking" ? "Checking" : "Setup Needed"}
            tone={setupReadState !== "verified" ? "neutral"
              : microphoneSelectionPending ? "warning" : snapshot?.readiness.microphoneReady ? "good"
                : snapshot?.readiness.level === "checking" ? "neutral" : "warning"}
          />
        </div>
        {#if microphoneSelectionPending}
          <p class="m-0 text-sm leading-6 text-[var(--ti-warning)]" role="status">This microphone choice has not been saved or checked. Select “Use this microphone” first.</p>
        {:else if snapshot && !snapshot.readiness.microphoneReady}
          <p class="m-0 text-sm leading-6 text-[var(--ti-text-muted)]" role="status">This microphone is not ready. Check its connection, then choose “Use this microphone” again.</p>
        {/if}
      {:else if step === 3}
        <div>
          <span class="ti-kicker">Calling app</span>
          <h1 class="ti-page-title">Select the audio device in your calling app</h1>
          <p class="ti-page-copy">In Discord, Zoom, Google Meet, or another calling app, choose the exact device name shown below as your microphone input.</p>
        </div>
        <div class="ti-subtle-card p-5">
          <span class="ti-field-label">Device name to select</span>
          <strong class="mt-2 block break-words text-base font-semibold">{currentMeetingMicrophone()}</strong>
          <p class="mb-0 mt-2 text-xs leading-5 text-[var(--ti-text-muted)]">Check Again verifies that Windows sees this device. It cannot confirm your calling app's microphone selection.</p>
        </div>
        <div class="ti-subtle-card overflow-hidden">
          <StatusRow
            label="Audio route in Windows"
            value={currentMeetingMicrophone()}
            detail={routeCheck === "ready" ? "Available in Windows. Select the same device in your calling app."
              : routeCheck === "unavailable" ? "Couldn't check the Windows device status. Select Check Again."
              : routeCheck === "checking" ? "Checking the audio route in Windows..."
              : "The virtual audio route needs setup before you can continue."}
            status={routeCheck === "ready" ? "Available" : routeCheck === "unavailable" ? "Unable to check" : routeCheck === "checking" ? "Checking" : "Setup Needed"}
            tone={routeCheck === "ready" ? "good" : routeCheck === "setup_needed" ? "warning" : routeCheck === "unavailable" ? "danger" : "neutral"}
          />
        </div>
      {:else}
        <SetupVoiceChoice
          snapshot={setupReadState === "verified" ? snapshot : null}
          meetingVoiceReady={setupReadState === "verified" ? meetingVoiceReady : null}
          {routeCheck}
          busy={busy || setupReadState !== "verified"}
          {builtinPendingId}
          microphone={setupReadState === "verified" ? currentMicrophone() : "Current microphone not verified"}
          meetingMicrophone={currentMeetingMicrophone()}
          meetingSound={currentMeetingSound()}
          onSelect={(voiceId) => selectBuiltin(voiceId)}
          onConfirm={confirmPendingBuiltin}
          onCancel={() => { builtinPendingId = null; message = ""; }}
          onOpenMyVoice={openMyVoice}
        />
      {/if}

      {#if setupReadState !== "verified"}
        <div class="flex flex-wrap items-center justify-between gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] p-4">
          <p class="m-0 min-w-0 flex-1 text-[13px] leading-5 text-[var(--ti-text-muted)]" role="status">
            {setupReadState === "checking" ? "Checking saved setup status..."
              : "The current saved setup status could not be verified. Recheck it before saving or leaving setup."}
          </p>
          <button type="button" class="ti-button ti-button-secondary" aria-busy={busy || checkingSetup}
            disabled={busy || checkingSetup} onclick={() => void checkSetupStatus()}>
            {busy || checkingSetup ? "Checking..." : "Check setup status"}
          </button>
        </div>
      {/if}

      {#if message}
        <p class="m-0 rounded-[var(--ti-radius-sm)] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-4 py-3 text-sm leading-6 text-[var(--ti-text-muted)]" aria-live="polite">{message}</p>
      {/if}

      <SetupNavigation
        {step}
        {busy}
        setupVerified={setupReadState === "verified"}
        microphoneReady={setupReadState === "verified" && snapshot?.readiness.microphoneReady === true}
        {microphoneSelectionPending}
        meetingRouteReady={setupReadState === "verified" && snapshot?.readiness.meetingRouteReady === true && routeCheck === "ready"}
        meetingVoiceReady={setupReadState === "verified" ? meetingVoiceReady : null}
        meetingReady={setupReadState === "verified" && snapshot?.readiness.meetingReady === true && routeCheck === "ready"}
        onBack={goBack}
        onDefer={deferSetup}
        onAdvance={advance}
        onRepair={repairSetup}
        onRefresh={refreshReadiness}
        onComplete={completeSetup}
      />
      {/if}
    </section>
  </section>
</main>
