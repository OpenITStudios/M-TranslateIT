<script lang="ts">
  import { PREVIEW_SCENARIOS, PREVIEW_VIEWPORTS, type PreviewScenario, type PreviewViewport } from "./fixtures";

  // Controls live outside the frame. This is never a second application runtime.
  let scenario = $state<PreviewScenario>("live");
  let viewport = $state<PreviewViewport>("desktop");
  const size = $derived(PREVIEW_VIEWPORTS[viewport]);
  const frameUrl = $derived(`/scripts/ui-preview/frame.html?scenario=${encodeURIComponent(scenario)}`);
</script>

<svelte:head>
  <title>TranslateIT UI Preview — synthetic, browser only</title>
</svelte:head>

<div class="flex h-screen min-h-0 flex-col bg-[var(--ti-surface-soft)] text-[var(--ti-text)]">
  <header class="z-10 flex shrink-0 flex-wrap items-center gap-4 border-b border-[var(--ti-border)] bg-[var(--ti-sidebar)] px-5 py-3" aria-label="Preview controls">
    <div class="min-w-0 flex-1">
      <h1 class="m-0 text-sm font-semibold">TranslateIT · UI Preview</h1>
      <p class="m-0 mt-1 text-xs text-[var(--ti-text-muted)]">Source Svelte components · synthetic read-only fixture · no Tauri runtime</p>
    </div>
    <label class="flex items-center gap-2 text-xs text-[var(--ti-text-muted)]">
      Scenario
      <select class="ti-field min-h-9 min-w-[185px] px-2" bind:value={scenario} aria-label="Preview scenario">
        {#each PREVIEW_SCENARIOS as item (item.id)}
          <option value={item.id}>{item.label}</option>
        {/each}
      </select>
    </label>
    <label class="flex items-center gap-2 text-xs text-[var(--ti-text-muted)]">
      Viewport
      <select class="ti-field min-h-9 min-w-[160px] px-2" bind:value={viewport} aria-label="Preview viewport">
        {#each Object.entries(PREVIEW_VIEWPORTS) as [key, dimensions] (key)}
          <option value={key}>{dimensions.width} × {dimensions.height}</option>
        {/each}
      </select>
    </label>
    <span class="rounded border border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] px-3 py-2 text-xs font-semibold text-[var(--ti-warning)]">
      SYNTHETIC — NOT NATIVE
    </span>
  </header>

  <div class="min-h-0 flex-1 overflow-auto p-4" aria-label="Preview scroll area">
    <!-- Frame viewport, not the outer toolbar viewport, governs 100vh and CSS media queries. -->
    <div
      data-preview-canvas
      data-preview-synthetic="true"
      data-preview-scenario={scenario}
      data-preview-viewport={viewport}
      class="shrink-0 overflow-hidden bg-[var(--ti-bg)] shadow-[0_16px_50px_rgba(0,0,0,0.4)]"
      style={`width: ${size.width}px; height: ${size.height}px;`}
    >
      <iframe
        data-preview-frame
        title="Synthetic TranslateIT workspace — production Svelte components"
        src={frameUrl}
        sandbox="allow-scripts allow-same-origin"
        class="block h-full w-full border-0"
      ></iframe>
    </div>
  </div>
  <p class="m-0 shrink-0 px-5 py-3 text-xs leading-5 text-[var(--ti-text-muted)]">
    Read-only visual fixture. Native audio, device controls, update checks, saved archives and Windows appearance are not verified.
  </p>
</div>
