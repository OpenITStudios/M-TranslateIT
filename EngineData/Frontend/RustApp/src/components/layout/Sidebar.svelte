<script lang="ts">
  import { AudioLines, Captions, Clock3, FolderClosed, Hash, Mic, Settings, ArrowLeft } from "@lucide/svelte";
  import type { AppRoute } from "../../app/shared/types";
  import type { RuntimeSettings } from "../../app/shared/types";
  import SidebarUserControls from "./SidebarUserControls.svelte";
  import TranslateITMark from "../brand/TranslateITMark.svelte";

  type MeetingSection = "live" | "last" | "saved";
  type SettingsTab = "meeting" | "translation" | "advanced";

  let {
    active, presence, meetingSection, settingsTab, meetingLive = false,
    meetingMuted = false, muteBusy = false, voiceSelectionRevision = 0, settings, resourcesLocked = false,
    voiceSelectionLocked = false, previewReadOnly = false, onNavigate,
    onMeetingSection, onSettingsTab, onToggleMute, onSettingsChange,
    onRefresh, onVoiceChanged, onNotice,
  }: {
    active: AppRoute;
    presence: string;
    meetingSection: MeetingSection;
    settingsTab: SettingsTab;
    meetingLive?: boolean;
    meetingMuted?: boolean;
    muteBusy?: boolean;
    voiceSelectionRevision?: number;
    settings: RuntimeSettings;
    resourcesLocked?: boolean;
    voiceSelectionLocked?: boolean;
    previewReadOnly?: boolean;
    onNavigate: (route: AppRoute) => void;
    onMeetingSection: (section: MeetingSection) => void;
    onSettingsTab: (tab: SettingsTab) => void;
    onToggleMute: () => void | Promise<void>;
    onSettingsChange: (next: RuntimeSettings) => void | Promise<void>;
    onRefresh: (message?: string) => void | Promise<void>;
    onVoiceChanged: (message?: string) => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

  const meetingItems = [
    { id: "live" as const, label: "Live Meeting", icon: Hash },
    { id: "last" as const, label: "Last Meeting", icon: Clock3 },
    { id: "saved" as const, label: "Saved Transcripts", icon: FolderClosed },
  ];
  const settingItems = [
    { id: "meeting" as const, label: "Voice & Audio", icon: Mic },
    { id: "translation" as const, label: "Meeting Captions", icon: Captions },
    { id: "advanced" as const, label: "Help & Diagnostics", icon: Settings },
  ];

</script>

<aside class="ti-sidebar flex h-screen min-h-0 w-[var(--ti-sidebar-width)] shrink-0 flex-col bg-[var(--ti-sidebar)]" aria-label="TranslateIT navigation">
  <button
    type="button"
    class="ti-sidebar-brand flex min-h-[66px] w-full items-center gap-3 border-b border-[var(--ti-border)] px-4 text-left hover:bg-[var(--ti-surface-hover)]"
    title="Back to Live Meeting"
    onclick={() => onMeetingSection("live")}
  >
    <TranslateITMark size={38} />
    <span class="min-w-0 truncate text-[15px] font-bold tracking-[-0.02em]">TranslateIT</span>
  </button>

  <nav class="ti-primary-navigation min-h-0 flex-1 overflow-y-auto px-2.5 py-4" aria-label="Workspace navigation">
    {#if active === "settings"}
      <button type="button" class="ti-channel-item mb-4 w-full" onclick={() => onMeetingSection("live")}>
        <ArrowLeft size={17} aria-hidden="true" />
        <span>Back to Meeting</span>
      </button>
      <p class="ti-channel-heading">USER SETTINGS</p>
      {#each settingItems as item}
        {@const Icon = item.icon}
        <button type="button" class={"ti-channel-item w-full " + (settingsTab === item.id ? "is-active" : "")}
          aria-current={settingsTab === item.id ? "page" : undefined}
          aria-label={item.label} onclick={() => onSettingsTab(item.id)}>
          <Icon size={18} strokeWidth={1.8} aria-hidden="true" />
          <span class="truncate">{item.label}</span>
        </button>
      {/each}
    {:else}
      <p class="ti-channel-heading">MEETING</p>
      {#each meetingItems as item}
        {@const Icon = item.icon}
        <button type="button" class={"ti-channel-item w-full " + (active === "meeting" && meetingSection === item.id ? "is-active" : "")}
          aria-current={active === "meeting" && meetingSection === item.id ? "page" : undefined}
          aria-label={item.label} onclick={() => onMeetingSection(item.id)}>
          <Icon size={18} strokeWidth={1.8} aria-hidden="true"/>
          <span class="truncate">{item.label}</span>
          {#if item.id === "live" && meetingLive}
            <span class="ml-auto size-2 rounded-full bg-[var(--ti-success)]" title="Meeting live"></span>
          {/if}
        </button>
      {/each}

      <p class="ti-channel-heading mt-6">VOICE</p>
      <button type="button" class={"ti-channel-item w-full " + (active === "my-voice" ? "is-active" : "")}
        aria-current={active === "my-voice" ? "page" : undefined}
        onclick={() => onNavigate("my-voice")}>
        <AudioLines size={18} strokeWidth={1.8} aria-hidden="true"/>
        <span>My Voice</span>
      </button>
    {/if}
  </nav>

  <SidebarUserControls {active} {presence} {meetingLive} {meetingMuted}
    {muteBusy} {voiceSelectionRevision} {settings} {resourcesLocked} {voiceSelectionLocked}
    {previewReadOnly} {onNavigate} {onToggleMute} {onSettingsChange}
    {onRefresh} {onVoiceChanged}
    onOpenAudioSettings={() => { onSettingsTab("meeting"); onNavigate("settings"); }}
    {onNotice} />
</aside>
