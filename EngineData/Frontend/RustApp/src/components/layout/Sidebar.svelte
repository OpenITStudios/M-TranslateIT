<script lang="ts">
  import { AudioLines, Captions, Clock3, FolderClosed, Mic, Settings, Palette, ArrowLeft, Play } from "@lucide/svelte";
  import type { AppRoute } from "../../app/shared/types";
  import type { RuntimeSettings } from "../../app/shared/types";
  import SidebarUserControls from "./SidebarUserControls.svelte";
  import MeetingLibraryNavigation from "./MeetingLibraryNavigation.svelte";
  import type { MeetingLibrary, MeetingLibraryChange } from "../../app/bridge/runtimeApi";
  import TranslateITMark from "../brand/TranslateITMark.svelte";

  type MeetingSection = "live" | "last" | "saved" | "session";
  type SettingsTab = "meeting" | "translation" | "appearance" | "advanced";

  let {
    active, presence, meetingSection, settingsTab, meetingLive = false,
    meetingMuted = false, muteBusy = false, voiceSelectionRevision = 0, settings, resourcesLocked = false,
    voiceSelectionLocked = false, previewReadOnly = false, onNavigate,
    onMeetingSection, onSettingsTab, onToggleMute, onSettingsChange,
    onRefresh, onVoiceChanged, onNotice,
    library = null, selectedSessionId = null, onOpenSession, onLibraryChange, onDeleteSession,
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
    library?: MeetingLibrary | null;
    selectedSessionId?: string | null;
    onOpenSession?: (id: string) => void;
    onLibraryChange?: (change: Omit<MeetingLibraryChange, "expectedRevision">) => Promise<boolean>;
    onDeleteSession?: (id: string) => Promise<boolean>;
  } = $props();

  const meetingItems = [
    { id: "live" as const, label: "Start", icon: Play },
    { id: "last" as const, label: "Last Meeting", icon: Clock3 },
    { id: "saved" as const, label: "Saved Meeting", icon: FolderClosed },
  ];
  const settingItems = [
    { id: "meeting" as const, label: "Voice & Audio", icon: Mic },
    { id: "translation" as const, label: "Meeting & Transcript", icon: Captions },
    { id: "appearance" as const, label: "Appearance", icon: Palette },
    { id: "advanced" as const, label: "Help & Diagnostics", icon: Settings },
  ];

</script>

<aside class="ti-sidebar flex h-screen min-h-0 w-[var(--ti-sidebar-width)] shrink-0 flex-col bg-[var(--ti-sidebar)]" aria-label="TranslateIT navigation">
  <button
    type="button"
    class="ti-sidebar-brand flex min-h-[66px] w-full items-center gap-3 border-b border-[var(--ti-border)] px-4 text-left hover:bg-[var(--ti-surface-hover)]"
    title="Back to Translate"
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
      <p class="ti-channel-heading">TRANSLATE</p>
      {#each meetingItems as item}
        {@const Icon = item.icon}
        <button type="button" class={"ti-channel-item w-full " + (active === "meeting" && meetingSection === item.id && !(item.id === "live" && meetingLive && selectedSessionId) ? "is-active" : "")}
          aria-current={active === "meeting" && meetingSection === item.id ? "page" : undefined}
          aria-label={item.label} onclick={() => onMeetingSection(item.id)}>
          <Icon size={18} strokeWidth={1.8} aria-hidden="true"/>
          <span class="truncate">{item.label}</span>
        </button>
      {/each}

      {#if onOpenSession && onLibraryChange && onDeleteSession}
        <MeetingLibraryNavigation {library} {selectedSessionId} {meetingLive}
          {onOpenSession} onChange={onLibraryChange} {onDeleteSession}/>
      {/if}

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
