<script lang="ts">
  import { AudioLines, Captions, Clock3, FolderClosed, Hash, Mic, MicOff, Settings, Zap, ArrowLeft } from "@lucide/svelte";
  import type { AppRoute } from "../../app/shared/types";
  import { toggleQuickAccess } from "../../app/runtime/quickAccessRuntime";
  import TranslateITMark from "../brand/TranslateITMark.svelte";

  type MeetingSection = "live" | "last" | "saved";
  type SettingsTab = "meeting" | "translation" | "advanced";

  let {
    active, presence, meetingSection, settingsTab, meetingLive = false,
    meetingMuted = false, muteBusy = false, onNavigate,
    onMeetingSection, onSettingsTab, onToggleMute, onNotice,
  }: {
    active: AppRoute;
    presence: string;
    meetingSection: MeetingSection;
    settingsTab: SettingsTab;
    meetingLive?: boolean;
    meetingMuted?: boolean;
    muteBusy?: boolean;
    onNavigate: (route: AppRoute) => void;
    onMeetingSection: (section: MeetingSection) => void;
    onSettingsTab: (tab: SettingsTab) => void;
    onToggleMute: () => void | Promise<void>;
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

  function presenceDot(): string {
    if (presence === "Live" || presence === "Ready") return "bg-[var(--ti-success)]";
    if (presence === "Unavailable") return "bg-[var(--ti-danger)]";
    if (presence === "Setup Needed" || presence === "Stop Needed") return "bg-[var(--ti-warning)]";
    return "bg-[var(--ti-text-soft)]";
  }

  function presenceLabel(): string {
    if (presence === "Live") return "Meeting is live";
    if (presence === "Ready") return "Ready to translate";
    if (presence === "Setup Needed") return "Setup needed";
    if (presence === "Unavailable") return "Unavailable";
    if (presence === "Stop Needed") return "Meeting cleanup needed";
    if (presence === "Starting") return "Starting translation";
    if (presence === "Stopping") return "Stopping translation";
    if (presence === "Active") return "Meeting needs attention";
    if (presence === "Check Status") return "Meeting status unknown";
    if (presence === "In Use") return "Audio in use";
    return "Checking";
  }

  async function openQuickAccess(): Promise<void> {
    try {
      await toggleQuickAccess();
    } catch {
      onNotice("Quick Access couldn't be opened.");
    }
  }
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

  <footer class="ti-sidebar-footer shrink-0 border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-2.5 py-3">
    <div class="mb-2 flex items-center gap-2 px-2" role="status" aria-live="polite" aria-atomic="true">
      <span class={"size-2 shrink-0 rounded-full " + presenceDot()} aria-hidden="true"></span>
      <span class="min-w-0 flex-1 truncate text-[12px] font-medium text-[var(--ti-text-muted)]">{presenceLabel()}</span>
    </div>
    <div class="flex items-center justify-between gap-1.5">
      <button type="button" class="ti-sidebar-icon-button"
        disabled={!meetingLive || muteBusy}
        aria-label={meetingMuted ? "Unmute TranslateIT output" : "Mute TranslateIT output"}
        title={meetingLive ? meetingMuted ? "Unmute TranslateIT output" : "Mute TranslateIT output" : "Mute is available during Meeting"}
        aria-pressed={meetingLive && meetingMuted}
        onclick={() => void onToggleMute()}>
        {#if meetingMuted}<MicOff size={19}/>{:else}<Mic size={19}/>{/if}
      </button>
      <button type="button" class="ti-sidebar-icon-button" aria-label="Open Quick Access" title="Quick Access (Ctrl+K)"
        onclick={() => void openQuickAccess()}><Zap size={19}/></button>
      <button type="button" class={"ti-sidebar-icon-button " + (active === "settings" ? "is-active" : "")}
        aria-label="Open Settings" aria-current={active === "settings" ? "page" : undefined}
        title="User Settings" onclick={() => onNavigate("settings")}><Settings size={19}/></button>
    </div>
  </footer>
</aside>
