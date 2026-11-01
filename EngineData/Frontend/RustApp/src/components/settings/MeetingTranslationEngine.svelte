<script lang="ts">
  import { runtimeApi } from "../../app/bridge/runtimeApi";
  import type { RuntimeSettings } from "../../app/shared/types";

  let {
    settings,
    locked = false,
    onSettingsChange,
    onNotice,
  }: {
    settings: RuntimeSettings;
    locked?: boolean;
    onSettingsChange: (settings: RuntimeSettings) => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

  let saving = $state(false);
  let savingMessage = $state("");

  async function chooseEngine(engine: "local" | "codex"): Promise<void> {
    if (saving || locked || settings.translation_engine === engine) return;
    saving = true;
    savingMessage = "Saving translation engine...";
    let nativeSaved = false;
    try {
      const candidate: RuntimeSettings = { ...settings, translation_engine: engine };
      const saved = await runtimeApi.saveSettings(candidate);
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
      savingMessage = engine === "local"
        ? "Local translation selected."
        : "Codex selected. Meeting translation remains blocked until Codex Voice is verified.";
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

<div class="flex flex-wrap items-start justify-between gap-5 border-t border-[var(--ti-border)] px-5 py-4">
  <div class="min-w-0 flex-1">
    <span class="ti-field-label">Translation engine</span>
    <strong class="mt-1.5 block text-[13px] font-semibold leading-5">
      {settings.translation_engine === "codex" ? "Codex Voice · not yet available" : "Local · offline"}
    </strong>
    <p class="mb-0 mt-1 text-[11.5px] leading-5 text-[var(--ti-text-soft)]">
      Local uses Whisper, MiLMMT and the selected GPT-SoVITS voice.
      Codex Voice is an optional future route and cannot start translation until its audio access,
      included-usage terms and translated-text output are verified.
      Selecting Codex does not send microphone audio or enable cloud inference.
    </p>
    {#if savingMessage}
      <p class="mb-0 mt-2 text-[11.5px] leading-5 text-[var(--ti-text-muted)]" role="status">
        {savingMessage}
      </p>
    {/if}
  </div>
  <select
    class="ti-field min-h-9 w-[228px] px-3"
    aria-label="Translation engine"
    disabled={locked || saving}
    value={settings.translation_engine === "codex" ? "codex" : "local"}
    onchange={(event) => void chooseEngine((event.currentTarget as HTMLSelectElement).value as "local" | "codex")}
  >
    <option value="local">Local · available</option>
    <option value="codex">Codex Voice · experimental</option>
  </select>
</div>
