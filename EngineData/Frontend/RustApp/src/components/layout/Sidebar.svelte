<script lang="ts">
  import { AudioLines, Mic, Settings } from "@lucide/svelte";
  import type { AppRoute } from "../../app/shared/types";
  import TranslateITMark from "../brand/TranslateITMark.svelte";

  let {
    active,
    presence,
    onNavigate,
  }: {
    active: AppRoute;
    presence: string;
    onNavigate: (route: AppRoute) => void;
  } = $props();

  const items = [
    { route: "meeting" as const, label: "Meeting", description: "Translate your voice in calls", icon: Mic },
    { route: "my-voice" as const, label: "My Voice", description: "Choose or create the voice others hear", icon: AudioLines },
    { route: "settings" as const, label: "Settings", description: "Microphone, captions, and help", icon: Settings },
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

  function shortPresence(): string {
    if (presence === "Setup Needed") return "Setup";
    if (presence === "Stop Needed") return "Stop needed";
    if (presence === "Unavailable") return "Unavailable";
    if (presence === "Check Status") return "Check status";
    return presence;
  }
</script>

<aside class="ti-sidebar flex h-screen min-h-0 w-[var(--ti-sidebar-width)] shrink-0 flex-col border-r border-[var(--ti-border)] bg-[var(--ti-sidebar)] px-2.5 py-4">
  <header class="ti-sidebar-brand flex items-center justify-center py-1" title="TranslateIT">
    <TranslateITMark size={48} />
    <span class="sr-only">TranslateIT</span>
  </header>

  <nav class="ti-primary-navigation mt-6 grid gap-1.5" aria-label="Primary navigation">
    {#each items as item}
      {@const Icon = item.icon}
      <button
        type="button"
        class={"ti-nav-item group flex min-h-[70px] min-w-0 flex-col items-center justify-center gap-1 rounded-[var(--ti-radius-md)] border px-1 py-2 text-center transition-colors " + (active === item.route
          ? "is-active border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] text-[var(--ti-text)]"
          : "border-transparent text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-hover)] hover:text-[var(--ti-text)]")}
        aria-current={active === item.route ? "page" : undefined}
        aria-label={item.label}
        title={item.description}
        onclick={() => onNavigate(item.route)}
      >
        <Icon size={22} strokeWidth={1.8} aria-hidden="true" />
        <strong class="ti-nav-label text-[11px] font-semibold leading-4">{item.label}</strong>
      </button>
    {/each}
  </nav>

  <div class="ti-sidebar-presence mt-auto border-t border-[var(--ti-border)] px-1 pt-3">
    <div class="flex min-h-11 flex-col items-center justify-center gap-1 text-center" role="status" aria-live="polite" aria-atomic="true" aria-label={presenceLabel()} title={presenceLabel()}>
      <span class={"size-2 rounded-full " + presenceDot()} aria-hidden="true"></span>
      <span class="ti-presence-label block max-w-full truncate text-[10px] font-semibold text-[var(--ti-text-muted)]" aria-hidden="true">{shortPresence()}</span>
    </div>
  </div>
</aside>
