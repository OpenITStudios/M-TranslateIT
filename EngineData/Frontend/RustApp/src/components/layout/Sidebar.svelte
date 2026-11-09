<script lang="ts">
  import { AudioLines, Captions, Mic, Settings, Palette, ArrowLeft, Hash } from "@lucide/svelte";
  import type { AppRoute } from "../../app/shared/types";
  import type { RuntimeSettings } from "../../app/shared/types";
  import SidebarUserControls from "./SidebarUserControls.svelte";
  import MeetingLibraryNavigation from "./MeetingLibraryNavigation.svelte";
  import type { MeetingLibrary, MeetingLibraryChange } from "../../app/bridge/runtimeApi";
  import TranslateITMark from "../brand/TranslateITMark.svelte";

  type MeetingSection = "live" | "last" | "session";
  type SettingsTab = "meeting" | "translation" | "appearance" | "advanced";

  let {
    active, presence, meetingSection, settingsTab, meetingLive = false,
    meetingMuted = false, muteBusy = false, voiceSelectionRevision = 0, settings, resourcesLocked = false,
    voiceSelectionLocked = false, previewReadOnly = false, onNavigate,
    onMeetingSection, onSettingsTab, onToggleMute, onSettingsChange,
    onRefresh, onVoiceChanged, onNotice,
    library = null, selectedSessionId = null, liveSessionId = null, selectedSurface = "recent",
    onOpenSession, onLibraryChange, onDeleteSession,
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
    liveSessionId?: string | null;
    selectedSurface?: "recent" | "project";
    onOpenSession?: (id: string, surface: "recent" | "project") => void;
    onLibraryChange?: (change: Omit<MeetingLibraryChange, "expectedRevision">) => Promise<boolean>;
    onDeleteSession?: (id: string) => Promise<boolean>;
  } = $props();

  const settingItems = [
    { id: "meeting" as const, label: "Voice & Audio", icon: Mic },
    { id: "translation" as const, label: "Meeting & Transcript", icon: Captions },
    { id: "appearance" as const, label: "Appearance", icon: Palette },
    { id: "advanced" as const, label: "Help & Diagnostics", icon: Settings },
  ];

  const startSelected = $derived(active === "meeting" && meetingSection === "live"
    && !(meetingLive && selectedSessionId
      && library?.sessions.some(session => session.session_id === selectedSessionId)));
  const navigationSessionId = $derived(active === "meeting" &&
    (meetingSection === "session" || (meetingSection === "live" && meetingLive))
      ? selectedSessionId : null);

</script>

<aside class="ti-sidebar flex h-screen min-h-0 w-[var(--ti-sidebar-width)] shrink-0 flex-col bg-[var(--ti-sidebar)]" aria-label="TranslateIT navigation">
  <button
    type="button"
    class="ti-sidebar-brand flex min-h-[66px] w-full items-center gap-3 border-b border-[var(--ti-border)] px-4 text-left hover:bg-[var(--ti-surface-hover)]"
    title="Open Live Meeting"
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
      <p class="ti-channel-heading">HOME</p>
      <button type="button" class={"ti-channel-item w-full " + (startSelected ? "is-active" : "")}
        aria-current={startSelected ? "page" : undefined}
        aria-label="Live Meeting workspace" onclick={() => onMeetingSection("live")}>
        <Hash size={18} strokeWidth={1.8} aria-hidden="true"/>
        <span>Live Meeting</span>
      </button>
      <button type="button" class={"ti-channel-item w-full " + (active === "my-voice" ? "is-active" : "")}
        aria-current={active === "my-voice" ? "page" : undefined}
        onclick={() => onNavigate("my-voice")}>
        <AudioLines size={18} strokeWidth={1.8} aria-hidden="true"/>
        <span>My Voice</span>
      </button>

      {#if onOpenSession && onLibraryChange && onDeleteSession}
        <MeetingLibraryNavigation {library} selectedSessionId={navigationSessionId}
          {liveSessionId} {selectedSurface}
          {onOpenSession}
          onChange={onLibraryChange} {onDeleteSession}/>
      {/if}

    {/if}
  </nav>

  <SidebarUserControls {active} {presence} {meetingLive} {meetingMuted}
    {muteBusy} {voiceSelectionRevision} {settings} {resourcesLocked} {voiceSelectionLocked}
    {previewReadOnly} {onNavigate} {onToggleMute} {onSettingsChange}
    {onRefresh} {onVoiceChanged}
    onOpenAudioSettings={() => { onSettingsTab("meeting"); onNavigate("settings"); }}
    {onNotice} />
</aside>
