<script lang="ts">
  import { onMount } from "svelte";
  import { classifyVirtualMicRoute } from "../app/bridge/virtualMicRoutePresentation";
  import { runtimeApi, type VirtualMicRouteContractStatus } from "../app/bridge/runtimeApi";
  import {
    runtimeProductFacade,
    type ProductAudioDeviceKind,
    type ProductRuntimeSnapshot,
    type ProductSetupAction,
  } from "../app/bridge/runtimeProductFacade";
  import { deviceId } from "../app/shared/state";
  import type { AudioDeviceListReport, RuntimeSettings } from "../app/shared/types";
  import MeetingNoiseSuppression from "../components/settings/MeetingNoiseSuppression.svelte";
  import MeetingTranslationEngine from "../components/settings/MeetingTranslationEngine.svelte";
  import FloatingCaptionPreferences from "../components/settings/FloatingCaptionPreferences.svelte";
  import TranslationFeedbackReview from "../components/settings/TranslationFeedbackReview.svelte";
  import SettingsDiagnostics from "../components/settings/SettingsDiagnostics.svelte";
  import StatusBadge from "../components/ui/StatusBadge.svelte";
  import StatusRow from "../components/ui/StatusRow.svelte";

  type SettingsTab = "meeting" | "translation" | "advanced";

  let {
    snapshot,
    settings,
    setupBusy = false,
    micTestBusy = false,
    onSettingsChange,
    onRefresh,
    onSetupAction,
    onFixSetup,
    onMicTest,
    onNotice,
  }: {
    snapshot: ProductRuntimeSnapshot;
    settings: RuntimeSettings;
    setupBusy?: boolean;
    micTestBusy?: boolean;
    onSettingsChange: (settings: RuntimeSettings) => void | Promise<void>;
    onRefresh: (message?: string) => void | Promise<void>;
    onSetupAction: (action: ProductSetupAction) => void | Promise<void>;
    onFixSetup: () => void | Promise<void>;
    onMicTest: () => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

  let tab = $state<SettingsTab>("meeting");
  let diagnosticsOpen = $state(false);
  let diagnosticsLoading = $state(false);
  let diagnosticsRefreshRevision = $state(0);
  let devices = $state<AudioDeviceListReport | null>(null);
  let routeStatus = $state<VirtualMicRouteContractStatus | null>(null);
  let devicesLoading = $state(false);
  let deviceSaving = $state(false);
  let deviceMessage = $state("Loading audio devices...");

  const meetingResourcesLocked = $derived(snapshot.resources.audio_locked);
  const micTestOwnsResources = $derived(snapshot.resources.owner_kind === "mic_test");
  const micTestBlockedByMeeting = $derived(snapshot.resources.audio_locked && !micTestOwnsResources);
  const meetingResourceLockMessage = $derived(
    snapshot.resources.owner_kind === "meeting"
      ? "Stop Translation before changing meeting audio or running Repair Setup."
      : snapshot.resources.owner_kind === "mic_test"
        ? "Stop Mic Test before changing meeting audio or running Repair Setup."
        : snapshot.resources.owner_kind === "voice_recording"
          ? "Stop My Voice recording before changing meeting audio or running Repair Setup."
          : "Finish the current audio action before changing meeting audio or running Repair Setup.",
  );
  const routeCheck = $derived(classifyVirtualMicRoute(routeStatus));
  const virtualMicrophoneReady = $derived(routeCheck === "ready");
  const meetingMicrophoneDevice = $derived(
    String(routeStatus?.selected_input_device ?? "").trim()
      || (routeCheck === "unavailable" ? "Status unavailable"
        : routeCheck === "checking" ? "Checking..."
        : routeCheck === "ready" ? "Device name unavailable" : "Not available in Windows"),
  );


  function currentDevice(kind: ProductAudioDeviceKind): string {
    return String(kind === "microphone" ? settings.audio.input_device_id ?? "" : settings.audio.output_device_id ?? "");
  }

  function deviceMissing(kind: ProductAudioDeviceKind): boolean {
    const selected = currentDevice(kind).trim();
    if (!selected || !devices?.ok) return false;
    const list = kind === "microphone" ? devices.input_devices : devices.output_devices;
    return !list.some((device) => deviceId(device) === selected);
  }

  async function loadDevices(): Promise<void> {
    if (devicesLoading) return;
    devicesLoading = true;
    deviceMessage = "Loading audio devices...";
    try {
      devices = await runtimeProductFacade.loadProductAudioDevices();
      deviceMessage = devices.ok ? "Choose a device. TranslateIT checks it before saving." : "Audio devices could not be checked. Select Refresh devices to retry.";
    } catch {
      devices = null;
      deviceMessage = "Audio devices could not be checked. Select Refresh devices to retry.";
    } finally {
      devicesLoading = false;
    }
  }

  async function loadRouteStatus(): Promise<void> {
    try {
      routeStatus = await runtimeApi.getVirtualMicRouteStatus();
    } catch {
      routeStatus = null;
    }
  }

  async function changeDevice(kind: ProductAudioDeviceKind, value: string): Promise<void> {
    if (deviceSaving) return;
    if (meetingResourcesLocked) {
      deviceMessage = meetingResourceLockMessage;
      onNotice(meetingResourceLockMessage);
      return;
    }

    const candidate = value.trim() || null;
    const previous = kind === "microphone" ? settings.audio.input_device_id : settings.audio.output_device_id;
    if ((previous ?? null) === candidate) return;

    deviceSaving = true;
    deviceMessage = "Checking device...";
    // Only a confirmed native success proves persistence. A later display
    // refresh failure must never be described as a failed device change.
    let savedByNative = false;
    try {
      const result = await runtimeProductFacade.selectProductAudioDevice(kind, candidate, settings);
      deviceMessage = result.message;
      onNotice(result.message);
      if (!result.ok) return;
      savedByNative = true;
      await onSettingsChange(result.settings);
      await onRefresh(result.message);
      await loadDevices();
      await loadRouteStatus();
    } catch {
      deviceMessage = savedByNative
        ? "Device preference was saved, but the display could not be refreshed. Check Again to verify it."
        : "Could not verify whether the device preference changed. Check current Settings before trying again.";
      onNotice(deviceMessage);
    } finally {
      deviceSaving = false;
    }
  }

  async function runSetupRepair(): Promise<void> {
    if (meetingResourcesLocked) {
      deviceMessage = meetingResourceLockMessage;
      onNotice(meetingResourceLockMessage);
      return;
    }
    await onFixSetup();
    await loadRouteStatus();
  }

  function selectValue(event: Event): string {
    return (event.currentTarget as HTMLSelectElement).value;
  }

  async function refreshDiagnostics(): Promise<void> {
    if (diagnosticsLoading) return;
    diagnosticsOpen = true;
    diagnosticsLoading = true;
    // The mounted reliability panel observes this revision on every explicit
    // Check Again, rather than keeping only its initial onMount snapshot.
    diagnosticsRefreshRevision += 1;
    onNotice("Refreshing Diagnostics...");
    try {
      await onRefresh("Diagnostics refreshed.");
      await loadRouteStatus();
    } finally {
      diagnosticsLoading = false;
    }
  }

  onMount(() => {
    void loadDevices();
    void loadRouteStatus();
  });
</script>

<section class="min-h-0">
  <div class="ti-page">
    <header class="ti-page-header">
      <div>
        <h2 class="ti-page-title">Settings</h2>
        <p class="ti-page-copy">Manage your microphone, captions, and meeting setup.</p>
      </div>
    </header>
    <div class="grid grid-cols-1 gap-5 lg:grid-cols-[190px_minmax(0,1fr)]">
      <div class="flex flex-col gap-1 self-start rounded-[var(--ti-radius-md)] bg-[var(--ti-surface-soft)] p-2.5" role="group" aria-label="Settings categories">
        <p class="px-3 pb-1 pt-2 text-[12px] font-semibold text-[var(--ti-text-muted)]">Settings</p>
        <button type="button" class={"flex min-h-10 items-center rounded-[var(--ti-radius-sm)] border-l-2 px-3 text-left text-[13px] transition-colors " + (tab === "meeting" ? "border-[var(--ti-brand)] bg-[var(--ti-surface-raised)] font-semibold text-[var(--ti-text)]" : "border-transparent text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-raised)] hover:text-[var(--ti-text)]")}
          aria-pressed={tab === "meeting"} aria-controls="settings-category-panel"
          onclick={() => { tab = "meeting"; diagnosticsOpen = false; }}>Voice &amp; Audio</button>
        <button type="button" class={"flex min-h-10 items-center rounded-[var(--ti-radius-sm)] border-l-2 px-3 text-left text-[13px] transition-colors " + (tab === "translation" ? "border-[var(--ti-brand)] bg-[var(--ti-surface-raised)] font-semibold text-[var(--ti-text)]" : "border-transparent text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-raised)] hover:text-[var(--ti-text)]")}
          aria-pressed={tab === "translation"} aria-controls="settings-category-panel"
          onclick={() => { tab = "translation"; diagnosticsOpen = false; }}>Meeting Captions</button>
        <p class="mt-3 px-3 pb-1 text-[12px] font-semibold text-[var(--ti-text-muted)]">Support</p>
        <button type="button" class={"flex min-h-10 items-center rounded-[var(--ti-radius-sm)] border-l-2 px-3 text-left text-[13px] transition-colors " + (tab === "advanced" ? "border-[var(--ti-brand)] bg-[var(--ti-surface-raised)] font-semibold text-[var(--ti-text)]" : "border-transparent text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-raised)] hover:text-[var(--ti-text)]")}
          aria-pressed={tab === "advanced"} aria-controls="settings-category-panel"
          onclick={() => { tab = "advanced"; diagnosticsOpen = false; }}>Help</button>
      </div>
      <div id="settings-category-panel" class="min-w-0" role="region"
        aria-label={tab === "meeting" ? "Voice & Audio settings" : tab === "translation" ? "Meeting Captions settings" : "Help and diagnostics"}>
    {#if tab === "meeting"}
      <article class="ti-panel overflow-hidden">
        <header class="flex items-start justify-between gap-5 border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <div>
            <h3 class="m-0 text-[16px] font-semibold tracking-[-0.015em]">Voice &amp; Audio</h3>
            <p class="mb-0 mt-1 text-[12px] text-[var(--ti-text-muted)]">Choose your audio devices here. Changes are saved only after Windows confirms them.</p>
          </div>
          {#if meetingResourcesLocked}
            <StatusBadge label="In Use" tone="neutral" />
          {:else if !snapshot.readiness.meetingReady}
            <StatusBadge label={snapshot.readiness.level === "unavailable" ? "Unavailable" : "Setup Needed"} tone={snapshot.readiness.level === "unavailable" ? "danger" : "warning"} />
          {/if}
        </header>

        <div class="grid grid-cols-1 gap-5 p-5 2xl:grid-cols-2" aria-busy={devicesLoading || deviceSaving}>
          <label class="grid min-w-0 gap-2">
            <span class="ti-field-label">Microphone</span>
            <select class="ti-field min-h-10 px-3" disabled={meetingResourcesLocked || devicesLoading || deviceSaving || !devices?.ok} value={currentDevice("microphone")} onchange={(event) => void changeDevice("microphone", selectValue(event))}>
              <option value="">Windows Default</option>
              {#each devices?.input_devices ?? [] as device (deviceId(device))}
                <option value={deviceId(device)} disabled={devices?.ambiguous_input_names.includes(device.name) || routeStatus?.selected_input_device === device.name}>{device.name}{devices?.ambiguous_input_names.includes(device.name) ? " · ambiguous" : routeStatus?.selected_input_device === device.name ? " · virtual output (not a spoken mic)" : device.is_default ? " · Windows default" : ""}</option>
              {/each}
              {#if deviceMissing("microphone")}
                <option value={currentDevice("microphone")}>{currentDevice("microphone")} · unavailable</option>
              {/if}
            </select>
            <small class="text-[12.5px] leading-5 text-[var(--ti-text-muted)]">Choose the microphone you speak into, not the virtual audio device used in your calling app.</small>
          </label>

          <label class="grid min-w-0 gap-2">
            <span class="ti-field-label">Sound from your meeting</span>
            <select class="ti-field min-h-10 px-3" disabled={meetingResourcesLocked || devicesLoading || deviceSaving || !devices?.ok} value={currentDevice("meeting-sound")} onchange={(event) => void changeDevice("meeting-sound", selectValue(event))}>
              <option value="">Windows Default</option>
              {#each devices?.output_devices ?? [] as device (deviceId(device))}
                <option value={deviceId(device)} disabled={devices?.ambiguous_output_names.includes(device.name) || routeStatus?.selected_output_device === device.name}>{device.name}{devices?.ambiguous_output_names.includes(device.name) ? " · ambiguous" : routeStatus?.selected_output_device === device.name ? " · virtual TTS output (not meeting sound)" : device.is_default ? " · Windows default" : ""}</option>
              {/each}
              {#if deviceMissing("meeting-sound")}
                <option value={currentDevice("meeting-sound")}>{currentDevice("meeting-sound")} · unavailable</option>
              {/if}
            </select>
            <small class="text-[12.5px] leading-5 text-[var(--ti-text-muted)]">Optional for incoming captions. Select the headphones or speakers used by your call. This does not change the calling app’s audio settings.</small>
          </label>
        </div>

        {#if (devices?.ambiguous_input_names.length ?? 0) > 0 || (devices?.ambiguous_output_names.length ?? 0) > 0}
          <p class="mx-5 mb-4 mt-0 text-[12px] leading-5 text-[var(--ti-text-muted)]" role="status">
            Multiple Windows audio endpoints have identical names. Those selections are disabled to prevent accidental routing to the wrong device.
          </p>
        {/if}

        <MeetingTranslationEngine
          {settings}
          needsReconfirmation={snapshot.meetingSession?.preflight?.blockers?.includes("meeting_session:settings_unverified") === true}
          locked={meetingResourcesLocked}
          onSettingsChange={onSettingsChange}
          onNotice={onNotice}
        />

        <MeetingNoiseSuppression
          {settings}
          locked={meetingResourcesLocked}
          onSettingsChange={onSettingsChange}
          onNotice={onNotice}
        />

        <div class="flex items-start justify-between gap-5 border-t border-[var(--ti-border)] px-5 py-4">
          <div class="min-w-0">
            <span class="ti-field-label">Microphone to select in your calling app</span>
            <strong class="mt-1.5 block break-words text-[13px] font-semibold leading-5">{meetingMicrophoneDevice}</strong>
            <p class="mb-0 mt-1 text-[12.5px] leading-5 text-[var(--ti-text-muted)]">
              {routeCheck === "unavailable" ? "Couldn't check this Windows audio device. Select Check Again."
                : routeCheck === "checking" ? "Checking the audio device in Windows..."
                : virtualMicrophoneReady
                  ? "Select the exact device name above as the microphone input in your calling app. Windows availability does not confirm the app's selection."
                  : "The virtual audio route is not ready. Check VB-CABLE or select Repair Setup."}
            </p>
          </div>
          {#if !virtualMicrophoneReady}
            <StatusBadge
              label={routeCheck === "unavailable" ? "Unable to check" : routeCheck === "checking" ? "Checking" : "Setup Needed"}
              tone={routeCheck === "setup_needed" ? "warning" : "neutral"}
            />
          {/if}
        </div>

        <footer class="flex flex-wrap items-center justify-between gap-4 border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <p class="m-0 min-w-0 flex-1 break-words text-[13px] leading-5 text-[var(--ti-text-muted)]" aria-live="polite" aria-atomic="true">{meetingResourcesLocked ? meetingResourceLockMessage : deviceMessage}</p>
          <div class="ti-action-row shrink-0">
            <!-- Device list refresh remains mounted: a failed read has an actionable, stable retry. -->
            <button type="button" class="ti-button ti-button-secondary"
              disabled={devicesLoading || deviceSaving} onclick={() => void loadDevices()}>
              {devicesLoading ? "Checking devices..." : "Refresh devices"}
            </button>
            {#if routeCheck === "unavailable"}
              <button type="button" class="ti-button ti-button-secondary" disabled={devicesLoading || deviceSaving} onclick={() => void loadRouteStatus()}>Check Again</button>
            {/if}
            <button type="button" class="ti-button ti-button-secondary" disabled={micTestBlockedByMeeting || micTestBusy || setupBusy || deviceSaving} onclick={() => void onMicTest()}>{micTestBusy ? "Working..." : snapshot.readiness.recording || micTestOwnsResources ? "Stop Mic Test" : "Mic Test"}</button>
            <button type="button" class="ti-button ti-button-secondary" disabled={meetingResourcesLocked || setupBusy || deviceSaving} onclick={() => void runSetupRepair()}>{setupBusy ? "Fixing..." : "Repair Setup"}</button>
          </div>
        </footer>
      </article>
    {:else if tab === "translation"}
      <div >
        <FloatingCaptionPreferences {onNotice} />
      </div>
    {:else if !diagnosticsOpen}
      <article class="ti-panel overflow-hidden">
        <header class="border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <h3 class="m-0 text-[16px] font-semibold">Meeting readiness</h3>
          <p class="mb-0 mt-1 text-[12px] text-[var(--ti-text-muted)]">Check the current setup. Open Diagnostics only if you need to investigate a problem.</p>
        </header>

        <div class="divide-y divide-[var(--ti-border)]">
          <StatusRow
            label="Meeting translation"
            value="Indonesian voice → English voice"
            detail="Checks the microphone, voice, and connection needed for calls."
            status={snapshot.readiness.meetingReady ? "Ready" : snapshot.readiness.meetingStatus}
            tone={snapshot.readiness.meetingReady ? "good" : snapshot.readiness.level === "unavailable" ? "danger" : "warning"}
          />
        </div>

        <footer class="flex justify-end border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <button type="button" class="ti-button ti-button-secondary" disabled={diagnosticsLoading} onclick={() => void refreshDiagnostics()}>{diagnosticsLoading ? "Opening..." : "Open Diagnostics"}</button>
        </footer>
      </article>
      <details class="ti-panel mt-4">
        <summary class="cursor-pointer px-5 py-4 text-sm font-medium">Past translation issue reports</summary>
        <div class="border-t border-[var(--ti-border)] p-4">
          <TranslationFeedbackReview {onNotice} />
        </div>
      </details>
    {:else}
      <div >
      <SettingsDiagnostics
        {snapshot}
        {setupBusy}
        {diagnosticsLoading}
        refreshRevision={diagnosticsRefreshRevision}
        onRefreshDiagnostics={refreshDiagnostics}
        {onSetupAction}
        onBack={() => { diagnosticsOpen = false; }}
      />
      </div>
    {/if}
      </div>
    </div>
  </div>
</section>
