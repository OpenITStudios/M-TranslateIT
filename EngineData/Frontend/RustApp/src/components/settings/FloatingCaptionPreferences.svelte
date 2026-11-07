<script lang="ts">
  import { Eye } from "@lucide/svelte";
  import {
    notifyOverlayPreferencesChanged,
    showTranslationOverlay,
  } from "../../app/runtime/translationOverlayRuntime";
  import {
    readOverlayDiagnostic,
    readOverlayPreferences,
    updateOverlayPreferences,
  } from "../../app/runtime/translationOverlayState";
  import { overlayFontSize, overlayWidth } from "../../app/runtime/translationOverlayPolicy";
  import type {
    OverlayContrast,
    OverlayTextSize,
    OverlayWidth,
  } from "../../app/runtime/translationOverlayPolicy";

  let {
    onNotice,
  }: {
    onNotice: (message: string) => void;
  } = $props();

  let preferences = $state(readOverlayPreferences());
  let diagnostic = $state(readOverlayDiagnostic());

  async function update(
    patch: {
      meetingEnabled?: boolean;
      textSize?: OverlayTextSize;
      width?: OverlayWidth;
      contrast?: OverlayContrast;
      clickThrough?: boolean;
    },
  ): Promise<void> {
    preferences = updateOverlayPreferences(patch);
    await notifyOverlayPreferencesChanged(preferences);
    diagnostic = readOverlayDiagnostic();
    onNotice("Floating caption preference saved.");
  }

  async function show(): Promise<void> {
    const shown = await showTranslationOverlay();
    preferences = readOverlayPreferences();
    diagnostic = readOverlayDiagnostic();
    onNotice(shown ? "Floating caption shown." : "Floating caption is unavailable right now.");
  }
</script>

<article class="ti-panel overflow-hidden">
  <header class="border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
    <h3 class="m-0 text-[15px] font-semibold">Floating captions</h3>
    <p class="mb-0 mt-1 text-[13px] leading-5 text-[var(--ti-text-muted)]">
      Adjust the text you read during meetings. The preview below uses sample text, not your conversation.
    </p>
  </header>

  <div class="grid min-w-0 gap-5 p-5">
    <label class="flex min-w-0 items-start justify-between gap-4">
      <span class="min-w-0 flex-1">
        <strong class="block text-[13px] font-semibold">Show during meetings</strong>
        <small class="mt-1 block text-[13px] leading-5 text-[var(--ti-text-muted)]">
          New committed meeting translations appear automatically. Hiding the caption is always respected.
        </small>
      </span>
      <input
        type="checkbox"
        class="mt-1 size-5 shrink-0 accent-[var(--ti-accent)]"
        checked={preferences.meetingEnabled}
        onchange={(event) => void update({ meetingEnabled: (event.currentTarget as HTMLInputElement).checked })}
      />
    </label>

    <label class="grid min-w-0 max-w-sm gap-2">
      <span class="ti-field-label">Text size</span>
      <select class="ti-field min-h-11 px-3"
        value={preferences.textSize}
        onchange={(event) => void update({ textSize: (event.currentTarget as HTMLSelectElement).value as OverlayTextSize })}
      >
        <option value="small">Small</option>
        <option value="medium">Medium · recommended</option>
        <option value="large">Large</option>
        <option value="extra-large">Extra large</option>
      </select>
    </label>

    <details class="min-w-0 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)]">
      <summary class="min-h-11 cursor-pointer px-4 py-3 text-sm font-medium hover:bg-[var(--ti-surface-soft)]">More caption options</summary>
      <div class="grid min-w-0 gap-5 border-t border-[var(--ti-border)] p-4">
        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <label class="grid gap-2">
            <span class="ti-field-label">Reading width</span>
            <select class="ti-field min-h-11 px-3"
              value={preferences.width}
              onchange={(event) => void update({ width: (event.currentTarget as HTMLSelectElement).value as OverlayWidth })}
            >
              <option value="compact">Compact</option>
              <option value="standard">Standard · recommended</option>
              <option value="wide">Wide</option>
            </select>
          </label>
          <label class="grid gap-2">
            <span class="ti-field-label">Contrast</span>
            <select class="ti-field min-h-11 px-3"
              value={preferences.contrast}
              onchange={(event) => void update({ contrast: (event.currentTarget as HTMLSelectElement).value as OverlayContrast })}
            >
              <option value="standard">Standard</option>
              <option value="high">High contrast</option>
            </select>
          </label>
        </div>
        <div class="rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] p-4">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <strong class="text-[13px] font-semibold">Reading preview</strong>
            <span class="text-[13px] text-[var(--ti-text-muted)]">Sample only · not live translation</span>
          </div>
          <div class="mt-3 overflow-hidden rounded-[var(--ti-radius-md)] bg-[#282c32] p-3">
            <div role="img"
              aria-label="Example floating caption: Good morning, the meeting will begin shortly"
              class="mx-auto break-words rounded-[12px] px-4 py-4 text-center leading-[1.45] text-[#f7f8fa]"
              class:border-2={preferences.contrast === "high"}
              class:border-white={preferences.contrast === "high"}
              style={`max-width: ${overlayWidth(preferences.width)}px; font-size: ${overlayFontSize(preferences.textSize)}px; background-color: ${preferences.contrast === "high" ? "#000" : "#101316"};`}
            >Good morning. The meeting will begin shortly.</div>
          </div>
          <p class="mb-0 mt-2 text-[13px] leading-5 text-[var(--ti-text-muted)]">Actual size and placement depend on screen size and Windows scaling.</p>
        </div>
        <label class="flex min-w-0 items-start justify-between gap-4">
          <span class="min-w-0 flex-1">
            <strong class="block text-[13px] font-semibold">Allow clicks through caption</strong>
            <small class="mt-1 block text-[13px] leading-5 text-[var(--ti-text-muted)]">
              Lets you click windows behind the caption. Return here to disable it before using caption buttons again.
            </small>
          </span>
          <input type="checkbox" class="mt-1 size-5 shrink-0 accent-[var(--ti-accent)]" checked={preferences.clickThrough}
            onchange={(event) => void update({ clickThrough: (event.currentTarget as HTMLInputElement).checked })} />
        </label>
      </div>
    </details>

    <div>
      <button type="button" class="ti-button ti-button-secondary" onclick={() => void show()}>
        <Eye size={15} /> Show caption window
      </button>
    </div>

    <p class="m-0 text-[13px] leading-5 text-[var(--ti-text-muted)]">Showing the caption window does not start Meeting translation. Previous private caption text is not restored.</p>

    {#if diagnostic}
      <p class="m-0 break-words text-[13px] leading-5 text-[var(--ti-warning)]" role="status">
        Floating caption last reported: {diagnostic.message}
      </p>
    {/if}
  </div>
</article>
