<script lang="ts">
  import { onMount } from "svelte";
  import { appUpdateApi, type AppUpdateCheck } from "../../app/update/appUpdateApi";

  let {
    audioLocked,
    audioOwnerKind,
    voiceBuildActive,
    myVoiceRecording,
    onNotice,
  }: {
    audioLocked: boolean;
    audioOwnerKind: string;
    voiceBuildActive: boolean;
    myVoiceRecording: boolean;
    onNotice: (message: string) => void;
  } = $props();

  let update = $state<AppUpdateCheck | null>(null);
  let busy = $state(false);
  // Keep the reason for a disabled update visible; a title tooltip alone
  // cannot be reliably discovered by touch, keyboard or screen readers.
  const blockerHint = $derived(
    audioLocked
      ? audioOwnerKind === "meeting" ? "Stop Translation to update."
        : audioOwnerKind === "mic_test" ? "Stop Mic Test to update."
        : audioOwnerKind === "voice_recording" ? "Stop recording to update."
        : "Finish the current audio action to update."
      : myVoiceRecording ? "Stop recording to update."
      : voiceBuildActive ? "Wait for My Voice creation to finish."
      : "",
  );

  onMount(() => {
    void (async () => {
      try {
        const result = await appUpdateApi.checkAtStartupOnce();
        if (result?.available) {
          update = result;
          onNotice(`TranslateIT ${result.version ?? "update"} is available.`);
        }
      } catch {
        // Update availability is optional and must never block app startup.
      }
    })();
  });

  async function install(): Promise<void> {
    if (busy) return;
    if (audioLocked) {
      const message = audioOwnerKind === "meeting"
        ? "Stop Translation before updating TranslateIT."
        : audioOwnerKind === "mic_test"
          ? "Stop Mic Test before updating TranslateIT."
          : audioOwnerKind === "voice_recording"
            ? "Stop the current My Voice recording before updating TranslateIT."
            : "Finish the current audio action before updating TranslateIT.";
      onNotice(message);
      return;
    }
    if (myVoiceRecording) {
      onNotice("Stop the current My Voice recording before updating TranslateIT.");
      return;
    }
    if (voiceBuildActive) {
      onNotice("Wait for My Voice creation to finish before updating TranslateIT.");
      return;
    }

    busy = true;
    onNotice("Preparing TranslateIT update...");
    try {
      const result = await appUpdateApi.install();
      if (!result) {
        onNotice("The update couldn't be started. Try again later.");
        return;
      }
      onNotice(result.message);
      if (result.installed) update = null;
    } catch {
      onNotice("Couldn't confirm the update result. Check your app version before trying again.");
    } finally {
      busy = false;
    }
  }
</script>

{#if update?.available}
  <div class="flex min-w-0 max-w-full flex-wrap items-center gap-x-2 gap-y-1">
  <button
    type="button"
    class="ti-button ti-button-secondary min-h-10 shrink-0 px-3 text-[13px]"
    disabled={busy || audioLocked || myVoiceRecording || voiceBuildActive}
    aria-describedby={blockerHint ? "translateit-update-blocker" : undefined}
    title={audioLocked
      ? audioOwnerKind === "meeting"
        ? "Stop Translation before updating."
        : audioOwnerKind === "mic_test"
          ? "Stop Mic Test before updating."
          : audioOwnerKind === "voice_recording"
            ? "Stop the current My Voice recording before updating."
            : "Finish the current audio action before updating."
      : myVoiceRecording
        ? "Stop the current My Voice recording before updating."
        : voiceBuildActive
          ? "Wait for My Voice creation to finish before updating."
          : update.notes ?? "A TranslateIT update is ready."}
    onclick={() => void install()}
  >{busy ? "Updating..." : `Update ${update.version ?? ""}`}</button>
    {#if blockerHint}
      <p id="translateit-update-blocker" class="m-0 max-w-[240px] break-words text-[13px] leading-5 text-[var(--ti-text-muted)]" role="status">
        {blockerHint}
      </p>
    {/if}
  </div>
{/if}
