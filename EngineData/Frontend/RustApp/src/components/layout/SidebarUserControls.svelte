<script lang="ts">
  import { onMount, tick } from "svelte";
  import { AudioLines, Check, ChevronDown, Mic, MicOff, Settings, CircleAlert, RotateCw } from "@lucide/svelte";
  import { myVoiceBuildApi, type MyVoiceBuildStatus } from "../../app/bridge/myVoiceBuildApi";
  import { runtimeApi, type VirtualMicRouteContractStatus } from "../../app/bridge/runtimeApi";
  import { runtimeProductFacade } from "../../app/bridge/runtimeProductFacade";
  import { deviceId } from "../../app/shared/state";
  import type { AppRoute, AudioDeviceListReport, AudioDeviceSummary, RuntimeSettings } from "../../app/shared/types";

  type DeviceKind = "microphone" | "meeting-sound";
  type Popup = "voice" | "devices" | null;
  type BuiltinVoice = "MaleVoice" | "FemaleVoice";
  const builtins: ReadonlyArray<{ id: BuiltinVoice; label: string }> = [
    { id: "MaleVoice", label: "Built-in Male" },
    { id: "FemaleVoice", label: "Built-in Female" },
  ];

  let {
    presence, active, meetingLive, meetingMuted, muteBusy, voiceSelectionRevision = 0, settings, resourcesLocked,
    voiceSelectionLocked, previewReadOnly = false, onNavigate, onToggleMute,
    onSettingsChange, onRefresh, onVoiceChanged, onOpenAudioSettings, onNotice,
  }: {
    presence: string;
    active: AppRoute;
    meetingLive: boolean;
    meetingMuted: boolean;
    muteBusy: boolean;
    voiceSelectionRevision?: number;
    settings: RuntimeSettings;
    resourcesLocked: boolean;
    voiceSelectionLocked: boolean;
    previewReadOnly?: boolean;
    onNavigate: (route: AppRoute) => void;
    onToggleMute: () => void | Promise<void>;
    onSettingsChange: (next: RuntimeSettings) => void | Promise<void>;
    onRefresh: (message?: string) => void | Promise<void>;
    onVoiceChanged: (message?: string) => void | Promise<void>;
    onOpenAudioSettings: () => void;
    onNotice: (message: string) => void;
  } = $props();

  let host = $state<HTMLDivElement>();
  let voicePopup = $state<HTMLElement>();
  let devicePopup = $state<HTMLElement>();
  let voiceTrigger = $state<HTMLButtonElement>();
  let devicesTrigger = $state<HTMLButtonElement>();
  let popup = $state<Popup>(null);
  let voiceStatus = $state<MyVoiceBuildStatus | null>(null);
  let voiceLoading = $state(false);
  let voiceBusy = $state(false);
  let pendingVoice = $state<BuiltinVoice | null>(null);
  let deviceReport = $state<AudioDeviceListReport | null>(null);
  let routeReport = $state<VirtualMicRouteContractStatus | null>(null);
  let devicesLoading = $state(false);
  let deviceBusy = $state(false);
  let popupMessage = $state("");
  let disposed = false;
  let voiceReadRevision = 0;
  let deviceReadRevision = 0;

  const voiceVerified = $derived(Boolean(voiceStatus && voiceStatus.phase !== "unavailable"
    && voiceStatus.selected_voice_kind !== "unknown"));
  const voiceLabel = $derived(
    !voiceVerified ? "Voice Actor"
      : voiceStatus?.selected_voice_kind === "personal" ? "My Voice"
      : voiceStatus?.selected_builtin_voice_id === "MaleVoice" ? "Built-in Male"
      : voiceStatus?.selected_builtin_voice_id === "FemaleVoice" ? "Built-in Female"
      : "Choose a voice",
  );
  const canChangeVoice = $derived(!previewReadOnly && voiceVerified && !voiceBusy && !voiceLoading
    && !voiceSelectionLocked && !resourcesLocked
    && !voiceStatus?.active && !voiceStatus?.preview_active);
  const canChangeDevice = $derived(!previewReadOnly && !deviceBusy && !devicesLoading
    && !resourcesLocked && deviceReport?.ok === true && routeReport?.ok === true);
  const statusHint = $derived(
    presence === "Live" ? meetingMuted ? "Muted" : "Meeting is live"
      : presence === "Ready" ? "Ready" : presence === "Stop Needed" ? "Stop needed" : presence,
  );
  const presenceColor = $derived(
    presence === "Ready" || presence === "Live" ? "bg-[var(--ti-success)]"
      : presence === "Unavailable" ? "bg-[var(--ti-danger)]"
      : presence === "Stop Needed" || presence === "Setup Needed" ? "bg-[var(--ti-warning)]"
      : "bg-[var(--ti-text-soft)]",
  );
  const deviceGroups = $derived([
    {
      kind: "microphone" as const, label: "Input Device",
      selected: settings.audio.input_device_id,
      items: deviceReport?.input_devices ?? [],
      ambiguous: deviceReport?.ambiguous_input_names ?? [],
    },
    {
      kind: "meeting-sound" as const, label: "Sound from Meeting",
      selected: settings.audio.output_device_id,
      items: deviceReport?.output_devices ?? [],
      ambiguous: deviceReport?.ambiguous_output_names ?? [],
    },
  ]);

  onMount(() => {
    if (!previewReadOnly) void readVoice();
    return () => { disposed = true; voiceReadRevision++; deviceReadRevision++; };
  });
  $effect(() => {
    if (voiceSelectionRevision > 0 && !previewReadOnly) void readVoice();
  });

  async function readVoice(): Promise<boolean> {
    if (previewReadOnly) return false;
    const revision = ++voiceReadRevision;
    voiceLoading = true;
    try {
      const observed = await myVoiceBuildApi.getStatus();
      if (disposed || revision !== voiceReadRevision) return false;
      voiceStatus = observed;
      return observed.phase !== "unavailable" && observed.selected_voice_kind !== "unknown";
    } catch {
      if (!disposed && revision === voiceReadRevision) voiceStatus = null;
      return false;
    } finally {
      if (!disposed && revision === voiceReadRevision) voiceLoading = false;
    }
  }

  async function readDevices(): Promise<void> {
    if (previewReadOnly) return;
    const revision = ++deviceReadRevision;
    devicesLoading = true;
    popupMessage = "";
    try {
      const [list, route] = await Promise.all([
        runtimeProductFacade.loadProductAudioDevices(),
        runtimeApi.getVirtualMicRouteStatus(),
      ]);
      if (disposed || revision !== deviceReadRevision) return;
      deviceReport = list;
      routeReport = route;
      if (!list.ok || !route?.ok) popupMessage = "Device routing could not be verified. Open Voice & Audio Settings.";
    } catch {
      if (disposed || revision !== deviceReadRevision) return;
      deviceReport = null;
      routeReport = null;
      popupMessage = "Audio devices are unavailable. Open Voice & Audio Settings.";
    } finally {
      if (!disposed && revision === deviceReadRevision) devicesLoading = false;
    }
  }

  async function focusPopup(target: Exclude<Popup, null>): Promise<void> {
    await tick();
    if (popup !== target) return;
    const element = target === "voice" ? voicePopup : devicePopup;
    element?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }

  function togglePopup(target: Exclude<Popup, null>): void {
    if (popup === target) { popup = null; pendingVoice = null; return; }
    popup = target;
    pendingVoice = null;
    popupMessage = "";
    if (target === "voice") void readVoice();
    if (target === "devices") void readDevices();
    void focusPopup(target);
  }

  function closePopup(): void { popup = null; pendingVoice = null; popupMessage = ""; }
  async function dismissPopup(): Promise<void> {
    const opened = popup;
    closePopup();
    await tick();
    if (opened === "voice") voiceTrigger?.focus();
    if (opened === "devices") devicesTrigger?.focus();
  }
  function handleOutsideClick(event: MouseEvent): void {
    if (popup && event.target instanceof Node && !host?.contains(event.target)) closePopup();
  }
  function handleEscape(event: KeyboardEvent): void {
    if (popup && event.key === "Escape") { event.preventDefault(); void dismissPopup(); }
  }
  function navigateTo(route: AppRoute): void { closePopup(); onNavigate(route); }
  function openAudioSettings(): void { closePopup(); onOpenAudioSettings(); }

  function voiceChosen(id: BuiltinVoice): boolean {
    return voiceVerified && voiceStatus?.selected_voice_kind === "built_in"
      && voiceStatus.selected_builtin_voice_id === id;
  }

  async function selectBuiltin(id: BuiltinVoice, confirmed = false): Promise<void> {
    if (!canChangeVoice || voiceChosen(id)) return;
    voiceBusy = true;
    let accepted = false;
    try {
      const result = await myVoiceBuildApi.selectBuiltin(id, confirmed);
      if (disposed) return;
      if (result.state === "approval_required") {
        pendingVoice = id;
        return;
      }
      pendingVoice = null;
      if (result.state === "frontend_bridge_error" || result.build.phase === "unavailable") {
        voiceStatus = null;
        onNotice("Voice selection is unverified. Check My Voice before trying again.");
        return;
      }
      if (!result.ok) { onNotice(result.message); return; }
      accepted = true;
      const verified = await readVoice();
      if (!verified || !voiceChosen(id)) {
        onNotice("Voice selection was accepted, but the selected voice could not be verified.");
        return;
      }
      await onVoiceChanged(result.message);
      closePopup();
    } catch {
      if (!disposed) onNotice(accepted
        ? "The voice changed, but the display could not be refreshed."
        : "The voice change could not be verified. Check My Voice.");
    } finally {
      voiceBusy = false;
    }
  }

  async function selectPersonal(): Promise<void> {
    if (!canChangeVoice || !voiceStatus?.saved_personal_voice_available
      || voiceStatus.selected_voice_kind === "personal") return;
    voiceBusy = true;
    let accepted = false;
    try {
      const result = await myVoiceBuildApi.selectSavedPersonalVoice();
      if (disposed) return;
      if (result.state === "frontend_bridge_error" || result.build.phase === "unavailable") {
        voiceStatus = null;
        onNotice("My Voice selection could not be verified. Check My Voice.");
        return;
      }
      if (!result.ok) { onNotice(result.message); return; }
      accepted = true;
      const verified = await readVoice();
      if (!verified || voiceStatus?.selected_voice_kind !== "personal") {
        onNotice("My Voice was accepted, but its selection could not be verified.");
        return;
      }
      await onVoiceChanged(result.message);
      closePopup();
    } catch {
      if (!disposed) onNotice(accepted
        ? "My Voice changed, but the display could not be refreshed."
        : "My Voice selection could not be verified.");
    } finally {
      voiceBusy = false;
    }
  }

  function unsafeDevice(kind: DeviceKind, device: AudioDeviceSummary, ambiguous: string[]): boolean {
    return ambiguous.includes(device.name)
      || (kind === "microphone" && device.name === routeReport?.selected_input_device)
      || (kind === "meeting-sound" && device.name === routeReport?.selected_output_device);
  }

  async function selectDevice(kind: DeviceKind, requestedId: string | null): Promise<void> {
    if (!canChangeDevice) return;
    const previous = kind === "microphone" ? settings.audio.input_device_id : settings.audio.output_device_id;
    if (previous === requestedId) return;
    if (requestedId !== null) {
      const list = kind === "microphone" ? deviceReport?.input_devices : deviceReport?.output_devices;
      const ambiguous = kind === "microphone" ? deviceReport?.ambiguous_input_names : deviceReport?.ambiguous_output_names;
      const candidate = list?.find((device) => deviceId(device) === requestedId);
      if (!candidate || unsafeDevice(kind, candidate, ambiguous ?? [])) return;
    }
    deviceBusy = true;
    popupMessage = "Checking device...";
    let accepted = false;
    try {
      const result = await runtimeProductFacade.selectProductAudioDevice(kind, requestedId, settings);
      if (disposed) return;
      if (!result.ok) { popupMessage = result.message; onNotice(result.message); return; }
      accepted = true;
      await onSettingsChange(result.settings);
      await onRefresh(result.message);
      closePopup();
    } catch {
      if (disposed) return;
      popupMessage = accepted
        ? "Device was accepted, but its current setting could not be refreshed."
        : "Device change could not be verified. Open Voice & Audio Settings.";
      onNotice(popupMessage);
    } finally {
      deviceBusy = false;
    }
  }
</script>

<svelte:window onclick={handleOutsideClick} onkeydown={handleEscape} />

<div class="ti-user-controls relative w-full" bind:this={host}>
  {#if popup === "voice"}
    <section bind:this={voicePopup} id="ti-voice-actor-menu" class="ti-user-popover absolute bottom-full left-0 z-30 mb-2 w-[min(332px,calc(100vw-18px))]"
      role="dialog" aria-label="Choose Voice Actor">
      <header class="ti-user-popover-heading">
        <strong class="text-sm">Voice Actor</strong>
        <button type="button" class="ti-user-popup-close" aria-label="Close Voice Actor menu" onclick={() => void dismissPopup()}>×</button>
      </header>
      <div class="max-h-[min(420px,calc(100vh-135px))] overflow-y-auto p-2">
        {#if voiceLoading}
          <p class="px-2 py-3 text-xs text-[var(--ti-text-muted)]" role="status">Checking selected voice...</p>
        {:else if !voiceVerified}
          <p class="px-2 py-3 text-xs leading-5 text-[var(--ti-warning)]" role="status">Voice selection could not be verified.</p>
          <button type="button" class="ti-user-menu-option" onclick={() => void readVoice()}><RotateCw size={15}/> Retry status</button>
        {:else}
          {#each builtins as actor (actor.id)}
            <button type="button" class="ti-user-menu-option" aria-pressed={voiceChosen(actor.id)}
              disabled={!canChangeVoice || voiceChosen(actor.id)}
              onclick={() => void selectBuiltin(actor.id)}>
              <AudioLines size={17} aria-hidden="true"/>
              <span class="min-w-0 flex-1 truncate text-left">{actor.label}</span>
              {#if voiceChosen(actor.id)}<Check size={16} class="text-[var(--ti-success)]" aria-hidden="true"/>{/if}
            </button>
          {/each}
          {#if voiceStatus?.saved_personal_voice_available}
            <button type="button" class="ti-user-menu-option"
              aria-pressed={voiceStatus.selected_voice_kind === "personal"}
              disabled={!canChangeVoice || voiceStatus.selected_voice_kind === "personal"}
              onclick={() => void selectPersonal()}>
              <AudioLines size={17} aria-hidden="true"/>
              <span class="min-w-0 flex-1 truncate text-left">My Voice</span>
              {#if voiceStatus.selected_voice_kind === "personal"}<Check size={16} class="text-[var(--ti-success)]" aria-hidden="true"/>{/if}
            </button>
          {/if}
        {/if}
        {#if voiceSelectionLocked || resourcesLocked || voiceStatus?.active || voiceStatus?.preview_active}
          <p class="mx-2 my-2 flex gap-2 text-xs leading-5 text-[var(--ti-warning)]"><CircleAlert size={15} class="shrink-0"/> Finish Meeting, recording or voice creation before changing actor.</p>
        {/if}
        {#if pendingVoice}
          <div class="mt-2 rounded-md border border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] p-3" role="group" aria-label="Confirm voice replacement">
            <strong class="text-[13px]">Replace the current voice?</strong>
            <p class="my-2 text-xs leading-5">The new actor will be used for future translated speech.</p>
            <div class="flex justify-end gap-2">
              <button type="button" class="ti-user-menu-utility" onclick={() => pendingVoice = null}>Cancel</button>
              <button type="button" class="ti-button min-h-9 px-3 text-xs" disabled={!canChangeVoice}
                onclick={() => { if (pendingVoice) void selectBuiltin(pendingVoice, true); }}>Confirm</button>
            </div>
          </div>
        {/if}
      </div>
      <button type="button" class="ti-user-menu-footer" onclick={() => navigateTo("my-voice")}>Open My Voice settings</button>
    </section>
  {:else if popup === "devices"}
    <section bind:this={devicePopup} id="ti-audio-device-menu" class="ti-user-popover absolute bottom-full left-0 z-30 mb-2 w-[min(352px,calc(100vw-18px))]"
      role="dialog" aria-label="Choose microphone and Meeting sound devices">
      <header class="ti-user-popover-heading">
        <strong class="text-sm">Audio Devices</strong>
        <button type="button" class="ti-user-popup-close" aria-label="Close Audio Devices menu" onclick={() => void dismissPopup()}>×</button>
      </header>
      <div class="max-h-[min(490px,calc(100vh-135px))] overflow-y-auto p-2">
        {#if devicesLoading}
          <p class="px-2 py-3 text-xs text-[var(--ti-text-muted)]" role="status">Checking Windows audio devices...</p>
        {:else}
          {#each deviceGroups as group (group.kind)}
            <div class="mb-3" role="group" aria-label={group.label}>
              <p class="ti-user-menu-heading">{group.label}</p>
              <p class="mx-2 mb-2 mt-0 text-[11px] leading-4 text-[var(--ti-text-soft)]">{group.kind === "microphone" ? "Your physical microphone" : "Audio received from the meeting"}</p>
              <button type="button" class="ti-user-menu-option" aria-pressed={group.selected === null}
                disabled={!canChangeDevice || group.selected === null}
                onclick={() => void selectDevice(group.kind, null)}>
                <span class="min-w-0 flex-1 text-left">Windows Default</span>
                {#if group.selected === null}<Check size={16} class="text-[var(--ti-success)]" aria-hidden="true"/>{/if}
              </button>
              {#each group.items as device (deviceId(device))}
                {@const selected = group.selected === deviceId(device)}
                <button type="button" class="ti-user-menu-option" aria-pressed={selected}
                  disabled={!canChangeDevice || selected || unsafeDevice(group.kind, device, group.ambiguous)}
                  title={unsafeDevice(group.kind, device, group.ambiguous) ? "Virtual or ambiguous device cannot be selected here" : selected ? "Selected device" : device.name}
                  onclick={() => void selectDevice(group.kind, deviceId(device))}>
                  <span class="min-w-0 flex-1 truncate text-left" title={device.name}>{device.name}{device.is_default ? " · Windows default" : ""}</span>
                  {#if selected}<Check size={16} class="text-[var(--ti-success)]" aria-hidden="true"/>{/if}
                </button>
              {/each}
              {#if group.items.length === 0}
                <p class="px-2 py-2 text-xs text-[var(--ti-text-muted)]">{deviceReport?.ok && routeReport?.ok ? "No devices reported for this input." : "Device list could not be verified. Refresh or open Voice & Audio Settings."}</p>
              {/if}
            </div>
          {/each}
          {#if resourcesLocked}
            <p class="mx-2 my-2 flex gap-2 text-xs leading-5 text-[var(--ti-warning)]"><CircleAlert size={15} class="shrink-0"/> Stop the current audio operation before changing devices.</p>
          {/if}
        {/if}
        {#if popupMessage}<p class="mx-2 my-2 text-xs leading-5 text-[var(--ti-warning)]" role="status">{popupMessage}</p>{/if}
      </div>
      <div class="flex justify-between border-t border-[var(--ti-border)] px-2 py-2">
        <button type="button" class="ti-user-menu-utility" disabled={devicesLoading || deviceBusy}
          onclick={() => void readDevices()}><RotateCw size={14}/> Refresh</button>
        <button type="button" class="ti-user-menu-utility" onclick={openAudioSettings}>Voice &amp; Audio Settings</button>
      </div>
    </section>
  {/if}

  <div class="flex min-h-[62px] items-center gap-1.5 border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-2 py-2.5">
    <button bind:this={voiceTrigger} type="button" class="ti-actor-trigger group flex min-w-0 flex-1 items-center gap-2 rounded-md px-1.5 py-1.5 text-left"
      aria-label="Choose Voice Actor" aria-controls={popup === "voice" ? "ti-voice-actor-menu" : undefined} aria-expanded={popup === "voice"}
      title={previewReadOnly ? "Synthetic preview: voice selection not connected" : "Choose the voice used for translated Meeting speech"}
      disabled={previewReadOnly} onclick={() => togglePopup("voice")}>
      <span class="relative grid size-8 shrink-0 place-items-center rounded-full bg-[var(--ti-brand)] text-white">
        <AudioLines size={17} aria-hidden="true"/>
        <span class={"absolute -bottom-0.5 -right-0.5 size-2.5 rounded-full border-2 border-[var(--ti-surface-soft)] " + presenceColor} aria-hidden="true"></span>
      </span>
      <span class="grid min-w-0 flex-1 gap-0">
        <strong class="truncate text-[12px] font-semibold leading-4">{voiceLabel}</strong>
        <span class="truncate text-[10px] leading-4 text-[var(--ti-text-muted)]">{statusHint}</span>
      </span>
    </button>

    <div class={"ti-mute-split flex shrink-0 items-center rounded-md bg-[var(--ti-sidebar)] " + (meetingMuted ? "is-muted" : "")}>
      <button type="button" class={"ti-user-action " + (meetingMuted ? "is-muted" : "")}
        aria-label={meetingMuted ? "Unmute TranslateIT output" : "Mute TranslateIT output"}
        aria-pressed={meetingLive && meetingMuted}
        disabled={!meetingLive || muteBusy || previewReadOnly}
        title={meetingLive ? (meetingMuted ? "Unmute audio sent to the meeting" : "Mute audio sent to the meeting") : "Mute is available only during a live Meeting"}
        onclick={() => void onToggleMute()}>
        {#if meetingMuted}<MicOff size={19}/>{:else}<Mic size={19}/>{/if}
      </button>
      <button bind:this={devicesTrigger} type="button" class="ti-user-action ti-user-chevron"
        aria-label="Choose audio input and meeting sound devices"
        aria-controls={popup === "devices" ? "ti-audio-device-menu" : undefined} aria-expanded={popup === "devices"}
        title="Choose physical microphone and incoming Meeting sound devices" disabled={previewReadOnly}
        onclick={() => togglePopup("devices")}><ChevronDown size={14}/></button>
    </div>
    <button type="button" class={"ti-user-action shrink-0 " + (active === "settings" ? "is-active" : "")}
      aria-label="Open Settings" aria-current={active === "settings" ? "page" : undefined}
      title="Open application Settings" disabled={previewReadOnly} onclick={() => navigateTo("settings")}><Settings size={19}/></button>
  </div>
</div>
