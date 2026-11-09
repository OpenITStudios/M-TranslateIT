<script lang="ts">
  import { appNotice } from "../../src/app/runtime/applicationNotice";
  import type { AppRoute } from "../../src/app/shared/types";
  import Sidebar from "../../src/components/layout/Sidebar.svelte";
  import AppHeader from "../../src/components/layout/AppHeader.svelte";
  import Meeting from "../../src/pages/Meeting.svelte";
  import MyVoice from "../../src/pages/MyVoice.svelte";
  import { makePreviewFixture, makeMyVoicePreviewFixture, isMyVoicePreviewScenario,
    PREVIEW_SCENARIOS, type MeetingPreviewScenario, type PreviewScenario } from "./fixtures";

  const requested = new URLSearchParams(window.location.search).get("scenario");
  const scenario: PreviewScenario = PREVIEW_SCENARIOS.some((item) => item.id === requested)
    ? (requested as PreviewScenario)
    : "ready";
  const voiceScenario = isMyVoicePreviewScenario(scenario) ? scenario : null;
  const meetingScenario: MeetingPreviewScenario = isMyVoicePreviewScenario(scenario) ? "ready" : scenario;
  const fixture = makePreviewFixture(meetingScenario);
  const voiceFixture = voiceScenario ? makeMyVoicePreviewFixture(voiceScenario) : null;
  const presence = fixture.snapshot.meeting.label;
  const notice = scenario === "cleanup"
    ? appNotice("Stop required: native cleanup is not verified in this synthetic scenario.", "attention")
    : scenario === "translation-failed"
      ? appNotice("The last synthetic translation failed. No native audio was produced.", "attention")
      : appNotice("Synthetic preview only. No Windows or audio runtime connected.", "info");

  // All production controls below are inert. These callbacks never simulate success.
  function noOperation(): void {}
  function noNavigation(_route: AppRoute): void {}
</script>

<!-- All app UI is inert. Only outer selector controls can alter which fixture is rendered. -->
<div data-preview-frame-root data-preview-scenario={scenario} data-preview-synthetic="true"
  inert class="flex h-screen min-h-0 w-screen overflow-hidden bg-[var(--ti-bg)] text-[var(--ti-text)]">
  <Sidebar active={voiceFixture ? "my-voice" : "meeting"} {presence} meetingSection="live" settingsTab="meeting"
    meetingLive={fixture.snapshot.meeting.live}
    meetingMuted={fixture.meetingStatus.muted}
    settings={fixture.snapshot.settings}
    resourcesLocked={fixture.snapshot.resources.audio_locked}
    voiceSelectionLocked={true} previewReadOnly={true}
    onNavigate={noNavigation} onMeetingSection={noOperation}
    onSettingsTab={noOperation} onToggleMute={noOperation}
    onSettingsChange={noOperation} onRefresh={noOperation}
    onVoiceChanged={noOperation} onNotice={noOperation}/>
  <main class="flex min-w-0 flex-1 flex-col overflow-hidden">
    <AppHeader snapshot={fixture.snapshot} route={voiceFixture ? "my-voice" : "meeting"} meetingSection="live"
      {notice} myVoiceRecording={false} showUpdateAction={false}
      onNavigate={noNavigation} onNotice={noOperation} onRefresh={noOperation}
      previewReadOnly={true}/>
    <div class={voiceFixture ? "min-h-0 flex-1 overflow-y-auto" : "min-h-0 flex-1 overflow-hidden"}>
      {#if voiceFixture}
        <MyVoice previewReadOnly={true} previewFixture={voiceFixture}
          voiceChangesLocked={false}
          onNotice={noOperation} onRecordingChange={noOperation}
          onMeetingVoiceChanged={noOperation}/>
      {:else}
        <Meeting section="live" snapshot={fixture.snapshot} previewReadOnly={true}
          meetingStatus={fixture.meetingStatus} meetingTurns={fixture.turns}
          onMeetingAction={noOperation}
          onRefresh={noOperation} onNavigate={noNavigation}/>
      {/if}
    </div>
  </main>
</div>
