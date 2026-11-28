<script lang="ts">
  import { runtimeApi } from "../../app/bridge/runtimeApi";
  import type { RuntimeSettings } from "../../app/shared/types";

  let {
    settings,
    locked = false,
    needsReconfirmation = false,
    onSettingsChange,
    onNotice,
  }: {
    settings: RuntimeSettings;
    locked?: boolean;
    needsReconfirmation?: boolean;
    onSettingsChange: (settings: RuntimeSettings) => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

  let saving = $state(false);
  let savingMessage = $state("");

  async function chooseEngine(engine: "local"): Promise<void> {
    // A corrupt persisted settings file is presented through tolerant UI
    // defaults, but only an explicit re-save can restore native admission.
    if (saving || locked || (settings.translation_engine === engine && !needsReconfirmation)) return;
    saving = true;
    savingMessage = "Saving translation engine...";
    let nativeSaved = false;
    try {
      // Both engine-changing native actions carry only the selected engine.
      // For an already verified setting, native compares the previous engine
      // before writing so stale UI state cannot silently overwrite Codex/Local.
      const saved = needsReconfirmation
        ? await runtimeApi.confirmUnverifiedTranslationEngine(engine)
        : await runtimeApi.selectTranslationEngine(engine, settings.translation_engine);
      if (!saved.ok) {
        savingMessage = saved.message || "Engine selection was not saved.";
        onNotice(savingMessage);
        return;
      }
      nativeSaved = true;
      // Re-read canonical Rust persistence instead of assuming frontend state
      // or a successful transport call proves the choice is installed.
      const confirmed = await runtimeApi.loadSettings();
      if (!confirmed || confirmed.translation_engine !== engine) {
        savingMessage = "Engine was saved, but its current selection could not be verified. Refresh Settings.";
        onNotice(savingMessage);
        return;
      }
      await onSettingsChange(confirmed);
      savingMessage = "Local translation selected.";
      onNotice(savingMessage);
    } catch {
      savingMessage = nativeSaved
        ? "Engine preference was saved, but the display could not be refreshed. Check Settings."
        : "Could not verify whether the engine preference changed.";
      onNotice(savingMessage);
    } finally {
      saving = false;
    }
  }
</script>

{#if needsReconfirmation || settings.translation_engine === "codex"}
  <div class="flex flex-wrap items-start justify-between gap-5 border-t border-[var(--ti-border)] px-5 py-4">
    <div class="min-w-0 flex-1">
      <span class="ti-field-label">Translation engine recovery</span>
      <strong class="mt-1.5 block text-[13px] font-semibold leading-5">Local · offline</strong>
      <p class="mb-0 mt-2 text-[13px] leading-5 text-[var(--ti-text-muted)]" role="alert">
        {needsReconfirmation
          ? "Saved engine settings could not be verified. Confirm Local before starting Meeting translation. Preferences that could not be recovered from a damaged file may reset to defaults."
          : "Your saved Codex Voice preference is retired. Switch to Local to use offline translation. Original Microphone remains available."}
      </p>
      <button type="button" class="ti-button ti-button-secondary mt-3"
        disabled={locked || saving} onclick={() => void chooseEngine("local")}>
        {needsReconfirmation ? "Reconfirm Local engine" : "Switch to Local"}
      </button>
      {#if savingMessage}
        <p class="mb-0 mt-2 text-[11.5px] leading-5 text-[var(--ti-text-muted)]" role="status">{savingMessage}</p>
      {/if}
    </div>
  </div>
{/if}
