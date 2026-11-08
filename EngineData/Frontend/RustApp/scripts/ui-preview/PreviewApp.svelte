<script lang="ts">
  import { appNotice } from "../../src/app/runtime/applicationNotice";
  import type { AppRoute } from "../../src/app/shared/types";
  import Sidebar from "../../src/components/layout/Sidebar.svelte";
  import AppHeader from "../../src/components/layout/AppHeader.svelte";
  import Meeting from "../../src/pages/Meeting.svelte";
  import {
    makePreviewFixture,
    PREVIEW_SCENARIOS,
    PREVIEW_VIEWPORTS,
    type PreviewScenario,
    type PreviewViewport,
  } from "./fixtures";

  // Only the controls outside the inert preview are interactive.
  // Production navigation, audio and Tauri handlers receive inert callbacks.
  let scenario = $state<PreviewScenario>("live");
  let viewport = $state<PreviewViewport>("desktop");

  const fixture = $derived(makePreviewFixture(scenario));
  const size = $derived(PREVIEW_VIEWPORTS[viewport]);
  const presence = $derived(fixture.snapshot.meeting.label);
  const notice = $derived(
    scenario === "cleanup"
      ? appNotice("Stop required: native cleanup has not been verified in this synthetic scenario.", "attention")
      : scenario === "translation-failed"
        ? appNotice("The last synthetic translation failed. No native audio was produced.", "attention")
        : appNotice("Synthetic preview only. No Windows or audio runtime is connected.", "info"),
  );

  function noOperation(): void {
    // Deliberately empty: the actual UI is inert and cannot cause mutations.
  }

  function noNavigation(_route: AppRoute): void {
    // Deliberately empty: navigation must not call application runtime.
  }
</script>

<svelte:head>
  <title>TranslateIT UI Preview — synthetic, browser only</title>
</svelte:head>

<div class="flex h-screen min-h-0 flex-col bg-[var(--ti-surface-soft)] text-[var(--ti-text)]">
  <header class="z-10 flex shrink-0 flex-wrap items-center gap-4 border-b border-[var(--ti-border)] bg-[var(--ti-sidebar)] px-5 py-3" aria-label="Preview controls">
    <div class="min-w-0 flex-1">
      <h1 class="m-0 text-sm font-semibold">TranslateIT · UI Preview</h1>
      <p class="m-0 mt-1 text-xs text-[var(--ti-text-muted)]">Real Svelte components · synthetic read-only fixture · no Tauri backend</p>
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
    <!-- The browser renders exact production UI components, but no actions can be dispatched. -->
    <div
      data-preview-canvas
      data-preview-synthetic="true"
      data-preview-scenario={scenario}
      data-preview-viewport={viewport}
      inert
      class="flex shrink-0 overflow-hidden bg-[var(--ti-bg)] shadow-[0_16px_50px_rgba(0,0,0,0.4)]"
      style={`width: ${size.width}px; height: ${size.height}px;`}
    >
      <Sidebar
        active="meeting"
        {presence}
        meetingSection="live"
        settingsTab="meeting"
        meetingLive={fixture.snapshot.meeting.live}
        meetingMuted={fixture.meetingStatus.muted}
        onNavigate={noNavigation}
        onMeetingSection={noOperation}
        onSettingsTab={noOperation}
        onToggleMute={noOperation}
        onNotice={noOperation}
      />
      <main class="flex min-w-0 flex-1 flex-col overflow-hidden">
        <AppHeader
          snapshot={fixture.snapshot}
          route="meeting"
          meetingSection="live"
          {notice}
          myVoiceRecording={false}
          showUpdateAction={false}
          onNavigate={noNavigation}
          onNotice={noOperation}
        />
        <div class="min-h-0 flex-1 overflow-hidden">
          <Meeting
            section="live"
            snapshot={fixture.snapshot}
            meetingStatus={fixture.meetingStatus}
            meetingTurns={fixture.turns}
            onToggleMute={noOperation}
            onMeetingAction={noOperation}
            onRefresh={noOperation}
            onNavigate={noNavigation}
          />
        </div>
      </main>
    </div>
  </div>
  <p class="m-0 shrink-0 px-5 py-3 text-xs leading-5 text-[var(--ti-text-muted)]">
    Visual/UI-only fixture. Native audio, model translation, device selection, updater, saved transcripts and Windows appearance are not verified.
  </p>
</div>
