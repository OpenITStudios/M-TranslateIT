<script lang="ts">
  import { runtimeApi } from "../../app/bridge/runtimeApi";
  import { cloneSettings } from "../../app/shared/state";
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

  async function changeMode(mode: "auto" | "off"): Promise<void> {
    if (saving || locked || settings.audio.noise_suppression === mode) return;
    saving = true;
    try {
      const expected = cloneSettings(settings);
      const candidate: RuntimeSettings = {
        ...expected,
        audio: { ...expected.audio, noise_suppression: mode },
      };
      const result = await runtimeApi.saveSettings(candidate, expected);
      if (!result.ok) {
        onNotice(result.message || "Noise suppression setting couldn't be saved.");
        return;
      }
      const saved = (await runtimeApi.loadSettings()) ?? candidate;
      await onSettingsChange(saved);
      onNotice(mode === "auto" ? "Noise suppression set to Auto." : "Noise suppression turned off.");
    } catch {
      onNotice("Noise suppression setting couldn't be saved.");
    } finally {
      saving = false;
    }
  }
</script>

<div class="flex flex-wrap items-start justify-between gap-5 border-t border-[var(--ti-border)] px-5 py-4">
  <div class="min-w-0 flex-1">
    <span class="ti-field-label">Noise suppression</span>
    <strong class="mt-1.5 block text-[13px] font-semibold leading-5">
      {settings.audio.noise_suppression === "off" ? "Off" : "Auto · recommended"}
    </strong>
    <p class="mb-0 mt-1 text-[12.5px] leading-5 text-[var(--ti-text-muted)]">
      Auto reduces faint background noise after a phrase is detected, before recognizing its words. It does not remove all noise or change the sound of your call directly.
    </p>
  </div>
  <select
    class="ti-field w-full shrink-0 px-3 sm:w-[180px]"
    disabled={locked || saving}
    value={settings.audio.noise_suppression === "off" ? "off" : "auto"}
    aria-label="Noise suppression"
    onchange={(event) => void changeMode((event.currentTarget as HTMLSelectElement).value as "auto" | "off")}
  >
    <option value="auto">Auto · recommended</option>
    <option value="off">Off</option>
  </select>
</div>
