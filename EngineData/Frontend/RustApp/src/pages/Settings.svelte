<script lang="ts">
  import { tick, untrack } from "svelte";
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
  import TranscriptAppearanceSettings from "../components/meeting/TranscriptAppearanceSettings.svelte";
  import TranslationFeedbackReview from "../components/settings/TranslationFeedbackReview.svelte";
  import SettingsDiagnostics from "../components/settings/SettingsDiagnostics.svelte";
  import SavedDataInventory from "../components/settings/SavedDataInventory.svelte";
  import StatusBadge from "../components/ui/StatusBadge.svelte";
  import StatusRow from "../components/ui/StatusRow.svelte";

  type SettingsTab = "meeting" | "translation" | "saved-data" | "appearance" | "advanced";

  let {
    tab,
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
    tab: SettingsTab;
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

  let diagnosticsOpen = $state(false);
  let diagnosticsLoading = $state(false);
  let diagnosticsRefreshRevision = $state(0);
  let devices = $state<AudioDeviceListReport | null>(null);
  let routeStatus = $state<VirtualMicRouteContractStatus | null>(null);
  let routeLoading = $state(true);
  let routeReadFailure = $state(false);
  let routeReadRevision = 0;
  let routeCheckButton = $state<HTMLButtonElement | undefined>(undefined);
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
  const routeCheck = $derived(routeLoading ? "checking" : routeReadFailure ? "unavailable" : classifyVirtualMicRoute(routeStatus));
  const virtualMicrophoneReady = $derived(routeCheck === "ready");
  const meetingMicrophoneDevice = $derived(
    routeCheck === "checking" ? "Checking..."
      : routeCheck === "unavailable" ? "Status unavailable"
      : routeCheck === "setup_needed" ? "Not available in Windows"
      : String(routeStatus?.selected_input_device ?? "").trim() || "Device name unavailable",
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
    const revision = ++routeReadRevision;
    routeLoading = true;
    routeReadFailure = false;
    try {
      const result = await runtimeApi.getVirtualMicRouteStatus();
      if (revision !== routeReadRevision) return;
      routeStatus = result;
    } catch {
      if (revision !== routeReadRevision) return;
      routeStatus = null;
      // A thrown bridge read means Unavailable, not an endless Checking state.
      routeReadFailure = true;
    } finally {
      if (revision === routeReadRevision) routeLoading = false;
    }
  }

  async function recheckRouteFromButton(): Promise<void> {
    const restoreFocus = document.activeElement === routeCheckButton;
    await loadRouteStatus();
    if (!restoreFocus || !document.hasFocus()) return;
    await tick();
    if (document.activeElement === document.body && routeCheckButton && !routeCheckButton.disabled) {
      routeCheckButton.focus();
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

  // Only Voice & Audio needs device discovery. Avoid probing Windows audio
  // when opening Appearance, transcript preferences, or diagnostics summaries.
  // untrack prevents the async loaders' own busy flags from retriggering this effect.
  $effect(() => {
    if (tab !== "meeting") return;
    untrack(() => {
      void loadDevices();
      void loadRouteStatus();
    });
  });
</script>

<section class="min-h-0">
  <div class="ti-page">
    <header class="ti-page-header">
      <div>
        <h2 class="ti-page-title">{tab === "meeting" ? "Voice & Audio" : tab === "translation" ? "Meeting & Transcript" : tab === "saved-data" ? "Saved Data" : tab === "appearance" ? "Appearance" : "Help & Diagnostics"}</h2>
        <p class="ti-page-copy">{tab === "meeting" ? "Choose your microphone, meeting sound and audio options." : tab === "translation" ? "Transcript presentation and floating caption preferences." : tab === "saved-data" ? "Review local storage usage without changing or deleting files." : tab === "appearance" ? "The current visual language of TranslateIT." : "Review setup health and recovery options."}</p>
      </div>
    </header>
      <div id="settings-category-panel" class="min-w-0" role="region"
        aria-label={tab === "meeting" ? "Voice & Audio settings" : tab === "translation" ? "Meeting & Transcript settings" : tab === "saved-data" ? "Saved Data settings" : tab === "appearance" ? "Appearance settings" : "Help and diagnostics"}>
    {#if tab === "meeting"}
      <article class="ti-panel ti-settings-panel overflow-hidden" aria-label="Voice and audio configuration">
        <header class="flex flex-wrap items-start justify-between gap-3 px-5 pb-3 pt-5">
          <div class="min-w-0 flex-1">
            <h3 class="m-0 text-[17px] font-semibold tracking-[-0.015em]">Microphone &amp; Meeting Sound</h3>
            <p class="mb-0 mt-1 text-[13px] leading-5 text-[var(--ti-text-muted)]">Choose your spoken microphone and incoming meeting audio source. These are separate from the virtual microphone used by your calling app. TranslateIT verifies each change before saving.</p>
          </div>
          {#if meetingResourcesLocked}
            <StatusBadge label="In Use" tone="neutral" />
          {:else if !snapshot.readiness.meetingReady}
            <StatusBadge
              label={snapshot.readiness.level === "unavailable" ? "Unavailable" : snapshot.readiness.level === "checking" ? "Checking" : "Setup Needed"}
              tone={snapshot.readiness.level === "unavailable" ? "danger" : snapshot.readiness.level === "checking" ? "neutral" : "warning"}
            />
          {/if}
        </header>

        <div class="flex items-center justify-end px-5 pb-3">
          <button type="button" class="ti-button ti-button-secondary min-h-9 px-3 text-[12px]"
            disabled={devicesLoading || deviceSaving} onclick={() => void loadDevices()}>
            {devicesLoading ? "Checking devices..." : "Refresh devices"}
          </button>
        </div>

        <div class="grid grid-cols-1 gap-3 px-5 pb-5 lg:grid-cols-2" aria-busy={devicesLoading || deviceSaving}
          role="group" aria-label="Microphone input and incoming meeting sound device selection">
          <label class="grid min-w-0 content-start gap-2 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface-raised)] p-4">
            <span class="ti-field-label">Microphone Input</span>
            <select class="ti-field min-h-10 px-3" disabled={meetingResourcesLocked || devicesLoading || deviceSaving || !devices?.ok} value={currentDevice("microphone")} onchange={(event) => void changeDevice("microphone", selectValue(event))}>
              <option value="">Windows Default</option>
              {#each devices?.input_devices ?? [] as device (deviceId(device))}
                <option value={deviceId(device)} disabled={devices?.ambiguous_input_names.includes(device.name) || routeStatus?.selected_input_device === device.name}>{device.name}{devices?.ambiguous_input_names.includes(device.name) ? " · ambiguous" : routeStatus?.selected_input_device === device.name ? " · virtual output (not a spoken mic)" : device.is_default ? " · Windows default" : ""}</option>
              {/each}
              {#if deviceMissing("microphone")}
                <option value={currentDevice("microphone")}>{currentDevice("microphone")} · unavailable</option>
              {/if}
            </select>
            <small class="text-[13px] leading-5 text-[var(--ti-text-muted)]">Choose the microphone you speak into, not the virtual audio device used in your calling app.</small>
          </label>

          <label class="grid min-w-0 content-start gap-2 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface-raised)] p-4">
            <span class="ti-field-label">Incoming Meeting Sound</span>
            <select class="ti-field min-h-10 px-3" disabled={meetingResourcesLocked || devicesLoading || deviceSaving || !devices?.ok} value={currentDevice("meeting-sound")} onchange={(event) => void changeDevice("meeting-sound", selectValue(event))}>
              <option value="">Windows Default</option>
              {#each devices?.output_devices ?? [] as device (deviceId(device))}
                <option value={deviceId(device)} disabled={devices?.ambiguous_output_names.includes(device.name) || routeStatus?.selected_output_device === device.name}>{device.name}{devices?.ambiguous_output_names.includes(device.name) ? " · ambiguous" : routeStatus?.selected_output_device === device.name ? " · virtual TTS output (not meeting sound)" : device.is_default ? " · Windows default" : ""}</option>
              {/each}
              {#if deviceMissing("meeting-sound")}
                <option value={currentDevice("meeting-sound")}>{currentDevice("meeting-sound")} · unavailable</option>
              {/if}
            </select>
            <small class="text-[13px] leading-5 text-[var(--ti-text-muted)]">Optional for incoming captions. Use the headphones or speakers for your call. Your calling app's settings stay unchanged.</small>
          </label>
        </div>

        {#if (devices?.ambiguous_input_names.length ?? 0) > 0 || (devices?.ambiguous_output_names.length ?? 0) > 0}
          <p class="mx-5 mb-4 mt-0 text-[12px] leading-5 text-[var(--ti-text-muted)]" role="status">
            Multiple Windows audio endpoints have identical names. Those selections are disabled to prevent accidental routing to the wrong device.
          </p>
        {/if}

        <div class="mx-5 mb-5 flex flex-wrap items-start justify-between gap-4 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface-raised)] p-4" aria-busy={routeLoading}
          role="group" aria-label="Virtual microphone and calling app routing">
          <div class="min-w-0">
            <span class="ti-field-label">Virtual Microphone / Call Routing</span>
            <strong class="mt-1.5 block break-words text-[13px] font-semibold leading-5">{meetingMicrophoneDevice}</strong>
            <p class="mb-0 mt-1 text-[13px] leading-5 text-[var(--ti-text-muted)]" role={routeCheck === "unavailable" ? "alert" : "status"}>
              {routeCheck === "unavailable" ? "Couldn't check this Windows audio device. Select Check route."
                : routeCheck === "checking" ? "Checking the audio device in Windows..."
                : virtualMicrophoneReady
                  ? "Select the exact device name above as the microphone input in your calling app. Windows availability does not confirm the app's selection."
                  : "The virtual audio route is not ready. Check VB-CABLE or select Repair Setup."}
            </p>
          </div>
          {#if !virtualMicrophoneReady}
            <StatusBadge
              label={routeCheck === "unavailable" ? "Unable to check" : routeCheck === "checking" ? "Checking" : "Setup Needed"}
              tone={routeCheck === "unavailable" ? "danger" : routeCheck === "setup_needed" ? "warning" : "neutral"}
            />
          {/if}
          <div class="ti-action-row shrink-0">
            <button type="button" class="ti-button ti-button-secondary min-h-9 px-3 text-[12px]"
              bind:this={routeCheckButton} aria-busy={routeLoading}
              disabled={routeLoading || deviceSaving || setupBusy} onclick={() => void recheckRouteFromButton()}>
              {routeLoading ? "Checking route..." : "Check route"}
            </button>
            <button type="button"
              class={routeCheck === "setup_needed" ? "ti-button min-h-9 px-3 text-[12px]" : "ti-button ti-button-secondary min-h-9 px-3 text-[12px]"}
              disabled={meetingResourcesLocked || setupBusy || deviceSaving} onclick={() => void runSetupRepair()}>
              {setupBusy ? "Repairing..." : "Repair Setup"}
            </button>
          </div>
        </div>

        <details class="group mx-5 mb-5 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)]" open={settings.translation_engine === "codex" || snapshot.meetingSession?.preflight?.blockers?.includes("meeting_session:settings_unverified") === true}>
          <summary class="flex min-h-12 cursor-pointer items-center gap-2 px-5 py-3 text-[13px] font-semibold text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-hover)]">
            <span aria-hidden="true" class="inline-block text-[11px] transition-transform group-open:rotate-90">▸</span> Advanced audio options
            <span class="ml-auto text-[12px] font-normal">Engine &amp; noise</span>
          </summary>
          <div class="border-t border-[var(--ti-border)]">
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

          </div>
        </details>

        <footer class="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <p class="m-0 min-w-0 flex-1 break-words text-[12px] leading-5 text-[var(--ti-text-muted)]" aria-live="polite" aria-atomic="true">
            {meetingResourcesLocked ? meetingResourceLockMessage : deviceMessage}
          </p>
          <button type="button" class="ti-button ti-button-secondary min-h-9 shrink-0 px-3 text-[12px]"
            disabled={micTestBlockedByMeeting || micTestBusy || setupBusy || deviceSaving} onclick={() => void onMicTest()}>
            {micTestBusy ? "Working..." : snapshot.readiness.recording || micTestOwnsResources ? "Stop Mic Test" : "Test microphone"}
          </button>
        </footer>
      </article>
    {:else if tab === "translation"}
      <div class="grid min-w-0 gap-7">
        <section aria-labelledby="transcript-appearance-heading">
          <h3 id="transcript-appearance-heading" class="m-0 text-[16px] font-semibold">Transcript Appearance</h3>
          <p class="mb-4 mt-1 text-[13px] leading-5 text-[var(--ti-text-muted)]">
            Configure bilingual chat bubbles across Live Meeting, Last Meeting and Saved Meeting.
          </p>
          <TranscriptAppearanceSettings {onNotice} />
        </section>
        <section aria-label="Floating captions settings" class="border-t border-[var(--ti-border)] pt-5">
          <FloatingCaptionPreferences {onNotice} />
        </section>
      </div>
    {:else if tab === "saved-data"}
      <SavedDataInventory />
    {:else if tab === "appearance"}
      <section class="grid gap-5" aria-label="Current appearance">
        <h3 class="m-0 text-[16px] font-semibold">Interface Appearance</h3>
        <dl class="grid grid-cols-1 gap-x-6 gap-y-4 border-y border-[var(--ti-border)] py-4 sm:grid-cols-2">
          <div>
            <dt class="text-[12px] text-[var(--ti-text-muted)]">Color theme</dt>
            <dd class="m-0 mt-1 text-[13px] font-medium">Dark</dd>
          </div>
          <div>
            <dt class="text-[12px] text-[var(--ti-text-muted)]">Interface density</dt>
            <dd class="m-0 mt-1 text-[13px] font-medium">Comfortable</dd>
          </div>
          <div>
            <dt class="text-[12px] text-[var(--ti-text-muted)]">Motion</dt>
            <dd class="m-0 mt-1 text-[13px] font-medium">Follows reduced-motion preference</dd>
          </div>
          <div>
            <dt class="text-[12px] text-[var(--ti-text-muted)]">Selected accent</dt>
            <dd class="m-0 mt-1 text-[13px] font-medium">TranslateIT blue</dd>
          </div>
        </dl>
        <p class="m-0 text-[13px] leading-6 text-[var(--ti-text-muted)]">
          These are the current visual defaults. There are no appearance switches that silently reset or change only this screen.
          Customize message bubbles in Meeting & Transcript. Global themes and font-size controls remain unavailable until they have verified persistence and responsive validation.
        </p>
      </section>
    {:else if !diagnosticsOpen}
      <article class="ti-panel overflow-hidden">
        <header class="border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <h3 class="m-0 text-[16px] font-semibold">Meeting readiness</h3>
          <p class="mb-0 mt-1 text-[13px] leading-5 text-[var(--ti-text-muted)]">Check the current setup. Open Diagnostics only if you need to investigate a problem.</p>
        </header>

        <div class="divide-y divide-[var(--ti-border)]">
          <StatusRow
            label="Meeting translation"
            value="Indonesian voice → English voice"
            detail="Checks the microphone, voice, and connection needed for calls."
            status={snapshot.readiness.meetingReady ? "Ready" : snapshot.readiness.meetingStatus}
            tone={snapshot.readiness.meetingReady ? "good" : snapshot.readiness.level === "unavailable" ? "danger" : snapshot.readiness.level === "checking" ? "neutral" : "warning"}
          />
        </div>

        <footer class="flex justify-end border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
          <button type="button" class="ti-button ti-button-secondary" disabled={diagnosticsLoading} onclick={() => void refreshDiagnostics()}>{diagnosticsLoading ? "Opening..." : "Open Diagnostics"}</button>
        </footer>
      </article>
      <details class="ti-panel mt-4">
        <summary class="min-h-11 cursor-pointer px-5 py-4 text-sm font-medium hover:bg-[var(--ti-surface-soft)]">Past translation issue reports</summary>
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
</section>
