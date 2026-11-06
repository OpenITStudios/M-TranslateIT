<script lang="ts">
  import StatusRow from "../ui/StatusRow.svelte";
  import type { ProductRuntimeSnapshot } from "../../app/bridge/runtimeProductFacade";
  import type { VirtualMicRouteCheck } from "../../app/bridge/virtualMicRoutePresentation";
  type BuiltinVoiceId = "MaleVoice" | "FemaleVoice";
  let {
    snapshot, meetingVoiceReady, routeCheck, busy, builtinPendingId,
    microphone, meetingMicrophone, meetingSound,
    onSelect, onConfirm, onCancel, onOpenMyVoice,
  }: {
    snapshot: ProductRuntimeSnapshot | null;
    meetingVoiceReady: boolean | null;
    routeCheck: VirtualMicRouteCheck;
    busy: boolean;
    builtinPendingId: BuiltinVoiceId | null;
    microphone: string;
    meetingMicrophone: string;
    meetingSound: string;
    onSelect: (voiceId: BuiltinVoiceId) => void | Promise<void>;
    onConfirm: () => void | Promise<void>;
    onCancel: () => void;
    onOpenMyVoice: () => void | Promise<void>;
  } = $props();
</script>

<div class="grid gap-5">
          <span class="ti-kicker">Meeting voice</span>
          <h1 class="ti-page-title">{meetingVoiceReady && snapshot?.readiness.meetingReady && routeCheck === "ready" ? "TranslateIT is ready for a call." : "Choose the English voice others will hear."}</h1>
          <p class="ti-page-copy">Choose a built-in voice now or create your own later. Use the exact Windows audio device shown below in your calling app.</p>
        </div>

        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <button type="button" class="ti-subtle-card min-h-[108px] p-4 text-left transition-colors hover:border-[var(--ti-border-strong)] hover:bg-[var(--ti-surface-hover)]" disabled={busy} onclick={() => void onSelect("MaleVoice")}>
            <span class="ti-field-label">Built-in voice</span>
            <strong class="mt-2 block text-sm font-semibold">Built-in Male</strong>
            <span class="mt-1 block text-xs leading-5 text-[var(--ti-text-soft)]">Use this voice in meetings.</span>
          </button>
          <button type="button" class="ti-subtle-card min-h-[108px] p-4 text-left transition-colors hover:border-[var(--ti-border-strong)] hover:bg-[var(--ti-surface-hover)]" disabled={busy} onclick={() => void onSelect("FemaleVoice")}>
            <span class="ti-field-label">Built-in voice</span>
            <strong class="mt-2 block text-sm font-semibold">Built-in Female</strong>
            <span class="mt-1 block text-xs leading-5 text-[var(--ti-text-soft)]">Use this voice in meetings.</span>
          </button>
        </div>

        {#if builtinPendingId}
          <div class="rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] p-4">
            <strong class="text-sm font-semibold">Replace the selected Meeting voice?</strong>
            <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">{(builtinPendingId === "MaleVoice" ? "Built-in Male" : "Built-in Female")} will replace the voice currently selected for Meeting.</p>
            <div class="ti-action-row mt-4">
              <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={onCancel}>Cancel</button>
              <button type="button" class="ti-button" disabled={busy} onclick={() => void onConfirm()}>Replace Voice</button>
            </div>
          </div>
        {/if}

        <div class="ti-panel divide-y divide-[var(--ti-border)] overflow-hidden">
          <StatusRow label="Microphone" value={microphone} status={snapshot?.readiness.microphoneReady ? "Ready" : "Setup Needed"} tone={snapshot?.readiness.microphoneReady ? "good" : "warning"} />
          <StatusRow label="Meeting voice" value={meetingVoiceReady ? "A Meeting voice is ready" : meetingVoiceReady === null ? "Checking the Meeting voice" : "Choose a built-in voice or create My Voice"} status={meetingVoiceReady ? "Ready" : meetingVoiceReady === null ? "Checking" : "Setup Needed"} tone={meetingVoiceReady ? "good" : meetingVoiceReady === null ? "neutral" : "warning"} />
          <StatusRow label="Windows audio route" value={meetingMicrophone}
            detail={routeCheck === "unavailable" ? "Couldn't check the Windows audio device. Select Check Again." : "Windows availability does not confirm selection inside your calling app."}
            status={routeCheck === "ready" ? "Available" : routeCheck === "unavailable" ? "Unable to check" : routeCheck === "checking" ? "Checking" : "Setup Needed"}
            tone={routeCheck === "ready" ? "good" : routeCheck === "setup_needed" ? "warning" : "neutral"} />
          <StatusRow label="Incoming captions" value={`English → Indonesian text · ${meetingSound}`} detail="Optional. You can turn this on or change it later." status="Optional" tone="neutral" />
        </div>

        <button type="button" class="ti-button ti-button-secondary justify-self-start" disabled={busy} onclick={() => void onOpenMyVoice()}>Create My Voice instead</button>
        {#if snapshot && (!snapshot.readiness.meetingReady || routeCheck !== "ready")}
          <div class="rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] p-4" role="status">
            <strong class="block text-sm font-semibold">What still needs attention</strong>
            <p class="mb-0 mt-1 text-sm leading-6 text-[var(--ti-text-muted)]">
              {routeCheck === "unavailable"
                ? "Couldn't check the virtual audio device in Windows. Go back to the Calling app step and select Check Again."
                : snapshot.readiness.level === "checking" || routeCheck === "checking"
                  ? "The local translation check is still running. Select Check Again."
                : !snapshot.readiness.microphoneReady
                  ? "Your input microphone is not ready. Go back to the Microphone step to select or check it."
                  : !snapshot.readiness.meetingRouteReady
                    ? "The virtual audio device is not available in Windows. Go back to the Calling app step to check it."
                    : meetingVoiceReady === false
                      ? "Choose a built-in Meeting voice above or create My Voice."
                      : "The local translation check is not ready. Select Check Again, then Fix Setup if needed."}
            </p>
          </div>
        {/if}
