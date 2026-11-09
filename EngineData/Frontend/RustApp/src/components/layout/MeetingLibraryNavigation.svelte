<script lang="ts">
  import { onMount, tick } from "svelte";
  import { Folder, FolderPlus, Hash, MoreHorizontal, Pencil, Trash2, FolderInput, ChevronDown, Pin, PinOff } from "@lucide/svelte";
  import type { MeetingLibrary, MeetingLibraryChange, MeetingLibrarySession } from "../../app/bridge/runtimeApi";
  import { pinnedFirst, recentMeetings } from "./meetingNavigationOrder";

  let {
    library, selectedSessionId, liveSessionId = null, selectedSurface = "recent",
    onOpenSession, onChange, onDeleteSession,
  }: {
    library: MeetingLibrary | null;
    selectedSessionId: string | null;
    liveSessionId?: string | null;
    selectedSurface?: "recent" | "project";
    onOpenSession: (sessionId: string, surface: "recent" | "project") => void;
    onChange: (change: Omit<MeetingLibraryChange, "expectedRevision">) => Promise<boolean>;
    onDeleteSession: (sessionId: string) => Promise<boolean>;
  } = $props();

  type Edit = { kind: "create_project" | "rename_project" | "rename_meeting"; id: string };
  let menu = $state<string | null>(null);
  let menuStyle = $state("visibility:hidden");
  let edit = $state<Edit | null>(null);
  let value = $state("");
  let moving = $state<string | null>(null);
  let confirmingDelete = $state<string | null>(null);
  let busy = $state(false);
  let returnFocus = $state<HTMLButtonElement | null>(null);
  let fallbackFocus = $state<HTMLButtonElement | null>(null);
  let editInput = $state<HTMLInputElement | null>(null);
  let collapsed = $state<Record<string, boolean>>({});
  let showAllRecent = $state(false);
  let uncategorizedCollapsed = $state(false);
  let actionSurface = $state<"recent" | "project">("recent");
  const deletingProject = $derived(Boolean(confirmingDelete && library?.projects.some(project => project.project_id === confirmingDelete)));

  // The native archive list is newest-first. Show ALL pins plus five ordinary
  // recent sessions; never let the pin quota hide an ordinary recent meeting.
  const recent = $derived(showAllRecent ? pinnedFirst(library?.sessions ?? []) : recentMeetings(library?.sessions ?? []));
  const uncategorized = $derived(pinnedFirst((library?.sessions ?? []).filter(session => session.project_id === null)));
  function selectedIn(session: MeetingLibrarySession, surface: "recent" | "project"): boolean {
    // Recent is a shortcut to the same channel, not a second Meeting identity.
    // Keep one visible active location: the origin selected by the user.
    return selectedSessionId === session.session_id && selectedSurface === surface;
  }
  async function restoreFocus(): Promise<void> {
    await tick();
    if (returnFocus?.isConnected) returnFocus.focus();
    else if (fallbackFocus?.isConnected) fallbackFocus.focus();
    returnFocus = null;
  }
  function dismissActions(): void {
    clearMenus();
    void restoreFocus();
  }
  function clearMenus(): void {
    menu = null;
    menuStyle = "visibility:hidden";
    edit = null;
    moving = null;
    confirmingDelete = null;
  }
  // One exclusive action surface across Recent, Project and category rows.
  // No independent menus may open for duplicate appearances of one session.
  async function toggleMenu(key: string, trigger: HTMLButtonElement): Promise<void> {
    if (busy) return;
    const next = menu === key ? null : key;
    if (next === null) { dismissActions(); return; }
    clearMenus();
    returnFocus = trigger;
    menuStyle = "visibility:hidden";
    menu = next;
    await tick();
    if (menu !== next || !trigger.isConnected) return;
    const popup = trigger.closest(".ti-library-popup-anchor")?.querySelector<HTMLElement>(".ti-library-menu");
    if (!popup) return;
    const rect = trigger.getBoundingClientRect();
    const width = Math.min(216, Math.max(160, window.innerWidth - 16));
    const height = popup.getBoundingClientRect().height;
    const usableHeight = Math.max(0, window.innerHeight - 16);
    const shownHeight = Math.min(height, usableHeight);
    const below = window.innerHeight - rect.bottom - 8;
    const above = rect.top - 8;
    const preferAbove = below < shownHeight && above > below;
    const preferredTop = preferAbove ? rect.top - shownHeight - 5 : rect.bottom + 5;
    const top = Math.max(8, Math.min(window.innerHeight - shownHeight - 8, preferredTop));
    const left = Math.max(8, Math.min(window.innerWidth - width - 8, rect.right - width));
    menuStyle = `top:${Math.round(top)}px;left:${Math.round(left)}px;width:${width}px;max-height:${Math.round(usableHeight)}px;overflow-y:auto`;
    await tick();
    if (menu === next && trigger.isConnected) popup.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }
  function closeMenuOnViewportChange(): void {
    if (menu !== null && !busy) { menu = null; menuStyle = "visibility:hidden"; }
  }
  function handleOutsidePointer(event: PointerEvent): void {
    if (menu === null || busy) return;
    const target = event.target;
    if (!(target instanceof Node)) return;
    const trigger = returnFocus;
    const popup = trigger?.closest(".ti-library-popup-anchor")?.querySelector(".ti-library-menu");
    if (trigger?.contains(target) || popup?.contains(target)) return;
    menu = null;
    menuStyle = "visibility:hidden";
  }
  onMount(() => {
    // Sidebar scroll is a nested scroll container; window's ordinary scroll
    // event does not bubble from it. Capture scroll at the document level.
    const onScroll = () => closeMenuOnViewportChange();
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("pointerdown", handleOutsidePointer, true);
    return () => {
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("pointerdown", handleOutsidePointer, true);
    };
  });
  function openSession(sessionId: string, surface: "recent" | "project"): void {
    if (busy) return;
    clearMenus();
    onOpenSession(sessionId, surface);
  }
  function toggleProject(projectId: string): void {
    if (busy) return;
    clearMenus();
    collapsed = { ...collapsed, [projectId]: !collapsed[projectId] };
  }
  function openMove(sessionId: string, surface: "recent" | "project"): void {
    clearMenus();
    moving = sessionId;
    actionSurface = surface;
  }
  function requestDelete(id: string, surface: "recent" | "project" = "recent"): void {
    clearMenus();
    confirmingDelete = id;
    actionSurface = surface;
  }
  async function openEdit(kind: Edit["kind"], id: string, previous = "", surface: "recent" | "project" = "recent", trigger?: HTMLButtonElement): Promise<void> {
    clearMenus();
    if (trigger) returnFocus = trigger;
    edit = { kind, id };
    actionSurface = surface;
    value = previous;
    await tick();
    editInput?.focus();
  }
  function handleEscape(event: KeyboardEvent): void {
    if (busy) return;
    if (event.key === "Escape" && (menu !== null || edit !== null ||
      moving !== null || confirmingDelete !== null)) {
      event.preventDefault();
      dismissActions();
    }
  }
  async function commitEdit(): Promise<void> {
    if (!edit || busy || !value.trim()) return;
    busy = true;
    try {
      const action: Omit<MeetingLibraryChange, "expectedRevision"> = edit.kind === "create_project"
        ? { action: "create_project", name: value }
        : edit.kind === "rename_project"
          ? { action: "rename_project", projectId: edit.id, name: value }
          : { action: "rename_meeting", sessionId: edit.id, name: value };
      if (await onChange(action)) { edit = null; value = ""; void restoreFocus(); }
    } finally { busy = false; }
  }
  async function modify(action: Omit<MeetingLibraryChange, "expectedRevision">): Promise<void> {
    if (busy) return;
    busy = true;
    try { if (await onChange(action)) dismissActions(); }
    finally { busy = false; }
  }
  async function deleteConfirmed(id: string): Promise<void> {
    if (busy || confirmingDelete !== id) return;
    busy = true;
    try {
      const ok = library?.projects.some(project => project.project_id === id)
        ? await onChange({ action: "delete_project", projectId: id })
        : await onDeleteSession(id);
      if (ok) dismissActions();
    } finally { busy = false; }
  }
  function itemLabel(session: MeetingLibrarySession): string {
    return session.display_name;
  }
</script>

{@snippet actionEditor(target: string)}
  {#if edit && (edit.kind === "create_project" ? target === "create_project" : edit.kind === "rename_project" ? target === "project:" + edit.id : target === actionSurface + ":" + edit.id)}
    <form class="ti-library-editor" onsubmit={(event) => { event.preventDefault(); void commitEdit(); }}>
      <label for="ti-library-edit-name" class="text-[12px] font-semibold">
        {edit.kind === "create_project" ? "New Project" : edit.kind === "rename_project" ? "Rename Project" : "Rename Meeting"}
      </label>
      <input bind:this={editInput} id="ti-library-edit-name" class="ti-field min-h-9 px-2 text-[13px]" maxlength="80" required
        bind:value={value} placeholder="Enter name"/>
      <div class="mt-2 flex justify-end gap-2">
        <button type="button" class="ti-library-small" disabled={busy} onclick={dismissActions}>Cancel</button>
        <button type="submit" class="ti-library-small ti-library-primary" disabled={busy || !library?.ok || !value.trim()}>Save</button>
      </div>
    </form>
  {/if}
  {#if moving && target === actionSurface + ":" + moving}
    <div class="ti-library-editor" role="group" aria-label="Move Meeting to Project">
      <p class="m-0 text-[12px] font-semibold">Move Meeting to Project</p>
      <button class="ti-library-small w-full text-left" disabled={busy}
        onclick={() => void modify({ action: "move_meeting", sessionId: moving, projectId: null })}>Uncategorized</button>
      {#each (library?.projects ?? []) as project (project.project_id)}
        <button class="ti-library-small w-full text-left" disabled={busy}
          onclick={() => void modify({ action: "move_meeting", sessionId: moving, projectId: project.project_id })}>{project.name}</button>
      {/each}
      <button class="ti-library-small w-full text-left" disabled={busy} onclick={dismissActions}>Cancel</button>
    </div>
  {/if}
  {#if confirmingDelete && target === (deletingProject ? "project:" : actionSurface + ":") + confirmingDelete}
    <div class="ti-library-editor border-[var(--ti-danger-border)]" role="group" aria-label="Confirm deletion">
      <p class="m-0 text-[12px] font-semibold text-[var(--ti-danger)]">Confirm deletion</p>
      <p class="m-0 text-[12px] leading-5 text-[var(--ti-danger)]">
        {deletingProject
          ? "Delete this category? Meetings will become Uncategorized."
          : "Delete this Meeting's local transcript? Exported HTML files remain."}
      </p>
      <div class="mt-2 flex justify-end gap-2">
        <button class="ti-library-small" disabled={busy} onclick={dismissActions}>Cancel</button>
        <button class="ti-library-small ti-library-danger" disabled={busy}
          onclick={() => { if (confirmingDelete) void deleteConfirmed(confirmingDelete); }}>Confirm Delete</button>
      </div>
    </div>
  {/if}
{/snippet}

<svelte:window onkeydown={handleEscape} onresize={closeMenuOnViewportChange} onscroll={closeMenuOnViewportChange} />

{#if library === null}
  <p class="mx-3 my-3 text-[12px] text-[var(--ti-text-soft)]">Checking saved Meetings...</p>
{:else}
  {#if !library.ok}
    <p class="mx-3 my-3 text-[12px] leading-5 text-[var(--ti-warning)]" role="status">
      {library.message || "Meeting metadata unavailable. Archived sessions remain accessible."}
    </p>
  {/if}
  <div class="mt-5">
    <div class="flex items-center justify-between gap-2 px-2">
      <p class="ti-channel-heading m-0">RECENT MEETINGS</p>
      <button bind:this={fallbackFocus} type="button" class="ti-library-view-all"
        aria-label={showAllRecent ? "Show fewer recent Meetings" : "Show all recent Meetings"}
        aria-expanded={showAllRecent}
        onclick={() => { if (!busy) { clearMenus(); showAllRecent = !showAllRecent; } }}>{showAllRecent ? "Show Less" : "View All"}</button>
    </div>
    {#if recent.length === 0}
      <p class="mx-3 my-2 text-[12px] text-[var(--ti-text-soft)]">No recent meetings</p>
    {:else}
      {#each recent as session (session.session_id)}
        <div class="ti-library-popup-anchor">
        <div class="ti-library-channel">
          <button type="button" class={"ti-channel-item min-w-0 flex-1 " + (selectedIn(session, "recent") ? "is-active" : "")}
            aria-label={itemLabel(session) + (liveSessionId === session.session_id ? ", live Meeting" : "")} aria-current={selectedIn(session, "recent") ? "page" : undefined}
            title={itemLabel(session)} onclick={() => openSession(session.session_id, "recent")}>
            {#if session.pinned}<Pin size={16} class="shrink-0 text-[var(--ti-text-muted)]"/>{:else}<Hash size={16} class="shrink-0"/>{/if}
            <span class="min-w-0 flex-1 truncate">{itemLabel(session)}</span>
            {#if liveSessionId === session.session_id}
              <span class="shrink-0 text-[10px] font-semibold text-[var(--ti-success)]" title="Meeting currently live">LIVE</span>
            {:else if session.storage_warning || !session.complete}
              <span class="size-1.5 shrink-0 rounded-full bg-[var(--ti-warning)]" title="Saved Meeting needs storage review" aria-label="Storage needs review"></span>
            {/if}
          </button>
          <button type="button" class="ti-library-more" aria-label={"Manage " + itemLabel(session)}
            aria-expanded={menu === "recent:" + session.session_id}
            disabled={!library.ok || busy}
            onclick={(event) => toggleMenu("recent:" + session.session_id, event.currentTarget)}>
            <MoreHorizontal size={16}/>
          </button>
        </div>
        {#if menu === "recent:" + session.session_id && library.ok}
          <div class="ti-library-menu" style={menuStyle}>
            <button onclick={() => openEdit("rename_meeting", session.session_id, session.display_name)}><Pencil size={14}/> Rename</button>
            <button onclick={() => void modify({ action: "pin_meeting", sessionId: session.session_id, pinned: !session.pinned })}>
              {#if session.pinned}<PinOff size={14}/>{:else}<Pin size={14}/>{/if} {session.pinned ? "Unpin Meeting" : "Pin Meeting"}
            </button>
            <button onclick={() => openMove(session.session_id, "recent")}><FolderInput size={14}/> Move to Project</button>
            <button onclick={() => requestDelete(session.session_id, "recent")}><Trash2 size={14}/> Delete Meeting</button>
          </div>
        {/if}
        </div>
        {@render actionEditor("recent:" + session.session_id)}
      {/each}
    {/if}
  </div>
  <div class="mt-5 flex items-center justify-between gap-2 px-2">
    <p class="ti-channel-heading m-0">PROJECTS</p>
    <button type="button" class="ti-library-more" aria-label="Create Project category"
      title="Create Project" disabled={!library.ok || busy} onclick={(event) => void openEdit("create_project", "", "recent", event.currentTarget)}><FolderPlus size={17}/></button>
  </div>
  {@render actionEditor("create_project")}
  {#if library.projects.length === 0}
    <p class="mx-3 my-2 text-[12px] text-[var(--ti-text-soft)]">No projects yet. Use + to create one.</p>
  {/if}
  <div class="ti-library-channel mt-2">
    <button type="button" class="ti-channel-item min-w-0 flex-1" aria-expanded={!uncategorizedCollapsed}
      onclick={() => { clearMenus(); uncategorizedCollapsed = !uncategorizedCollapsed; }}>
      <ChevronDown size={14} class={uncategorizedCollapsed ? "-rotate-90" : ""} aria-hidden="true"/>
      <Folder size={16} aria-hidden="true"/>
      <span class="min-w-0 flex-1 truncate">Uncategorized</span>
      <span class="shrink-0 text-[11px] text-[var(--ti-text-soft)]" aria-label={uncategorized.length + " Meetings"}>{uncategorized.length}</span>
    </button>
  </div>
  {#if !uncategorizedCollapsed}
    {#each uncategorized as session (session.session_id)}
      <div class="ti-library-popup-anchor">
      <div class="ti-library-channel ml-4">
        <button type="button" class={"ti-channel-item min-w-0 flex-1 " + (selectedIn(session, "project") ? "is-active" : "")}
          aria-current={selectedIn(session, "project") ? "page" : undefined}
          title={session.display_name} onclick={() => openSession(session.session_id, "project")}>
          {#if session.pinned}<Pin size={15} aria-hidden="true"/>{:else}<Hash size={15} aria-hidden="true"/>{/if}
          <span class="min-w-0 flex-1 truncate">{session.display_name}</span>
          {#if liveSessionId === session.session_id}<span class="text-[10px] font-semibold text-[var(--ti-success)]">LIVE</span>{/if}
        </button>
        <button type="button" class="ti-library-more" aria-label={"Manage " + session.display_name}
          aria-expanded={menu === "project:" + session.session_id} disabled={!library.ok || busy}
          onclick={(event) => toggleMenu("project:" + session.session_id, event.currentTarget)}><MoreHorizontal size={16}/></button>
      </div>
      {#if menu === "project:" + session.session_id && library.ok}
        <div class="ti-library-menu" style={menuStyle}>
          <button type="button" onclick={() => openEdit("rename_meeting", session.session_id, session.display_name, "project")}><Pencil size={14}/> Rename</button>
          <button type="button" onclick={() => void modify({ action: "pin_meeting", sessionId: session.session_id, pinned: !session.pinned })}>
            {#if session.pinned}<PinOff size={14}/>{:else}<Pin size={14}/>{/if} {session.pinned ? "Unpin Meeting" : "Pin Meeting"}
          </button>
          <button type="button" onclick={() => openMove(session.session_id, "project")}><FolderInput size={14}/> Move to Project</button>
          <button type="button" onclick={() => requestDelete(session.session_id, "project")}><Trash2 size={14}/> Delete Meeting</button>
        </div>
      {/if}
      </div>
      {@render actionEditor("project:" + session.session_id)}
    {/each}
  {/if}
  {#each library.projects as project (project.project_id)}
    {@const projectSessions = library.sessions.filter(session => session.project_id === project.project_id)}
    <div class="ti-library-popup-anchor">
    <div class="ti-library-channel">
      <button type="button" class="ti-channel-item min-w-0 flex-1"
        aria-expanded={!collapsed[project.project_id]}
        onclick={() => toggleProject(project.project_id)}>
        <ChevronDown size={14} class={collapsed[project.project_id] ? "-rotate-90" : ""}/>
        <Folder size={16}/>
        <span class="min-w-0 flex-1 truncate" title={project.name}>{project.name}</span>
        <span class="shrink-0 text-[11px] text-[var(--ti-text-soft)]"
          aria-label={projectSessions.length + " Meetings"}>{projectSessions.length}</span>
      </button>
      <button type="button" class="ti-library-more" aria-label={"Manage Project " + project.name}
        aria-expanded={menu === "category:" + project.project_id}
        disabled={!library.ok || busy}
        onclick={(event) => toggleMenu("category:" + project.project_id, event.currentTarget)}>
        <MoreHorizontal size={16}/>
      </button>
    </div>
    {#if menu === "category:" + project.project_id && library.ok}
      <div class="ti-library-menu" style={menuStyle}>
        <button onclick={() => openEdit("rename_project", project.project_id, project.name)}><Pencil size={14}/> Rename Project</button>
        <button onclick={() => requestDelete(project.project_id)}><Trash2 size={14}/> Delete Project</button>
      </div>
    {/if}
    </div>
    {@render actionEditor("project:" + project.project_id)}
    {#if !collapsed[project.project_id]}
      {#each pinnedFirst(projectSessions) as session (session.session_id)}
        <div class="ti-library-popup-anchor">
        <div class="ti-library-channel ml-4">
          <button type="button"
            class={"ti-channel-item min-w-0 flex-1 " + (selectedIn(session, "project") ? "is-active" : "")}
            aria-label={session.display_name + (liveSessionId === session.session_id ? ", live Meeting" : "")}
            aria-current={selectedIn(session, "project") ? "page" : undefined}
            title={session.display_name}
            onclick={() => openSession(session.session_id, "project")}>
            {#if session.pinned}<Pin size={15} class="shrink-0"/>{:else}<Hash size={15} class="shrink-0"/>{/if}
            <span class="min-w-0 flex-1 truncate">{session.display_name}</span>
            {#if liveSessionId === session.session_id}
              <span class="shrink-0 text-[10px] font-semibold text-[var(--ti-success)]" title="Meeting currently live">LIVE</span>
            {:else if session.storage_warning || !session.complete}
              <span class="size-1.5 shrink-0 rounded-full bg-[var(--ti-warning)]" title="Saved Meeting needs storage review" aria-label="Storage needs review"></span>
            {/if}
          </button>
          <button type="button" class="ti-library-more"
            aria-label={"Manage " + session.display_name + " in Project"}
            aria-expanded={menu === "project:" + session.session_id}
            disabled={!library.ok || busy}
            onclick={(event) => toggleMenu("project:" + session.session_id, event.currentTarget)}>
            <MoreHorizontal size={16}/>
          </button>
        </div>
        {#if menu === "project:" + session.session_id && library.ok}
          <div class="ti-library-menu" style={menuStyle}>
            <button type="button" onclick={() => openEdit("rename_meeting", session.session_id, session.display_name, "project")}>
              <Pencil size={14}/> Rename
            </button>
            <button type="button" onclick={() => void modify({ action: "pin_meeting", sessionId: session.session_id, pinned: !session.pinned })}>
              {#if session.pinned}<PinOff size={14}/>{:else}<Pin size={14}/>{/if}
              {session.pinned ? "Unpin Meeting" : "Pin Meeting"}
            </button>
            <button type="button" onclick={() => openMove(session.session_id, "project")}>
              <FolderInput size={14}/> Move to Project
            </button>
            <button type="button" onclick={() => requestDelete(session.session_id, "project")}>
              <Trash2 size={14}/> Delete Meeting
            </button>
          </div>
        {/if}
        </div>
        {@render actionEditor("project:" + session.session_id)}
      {/each}
    {/if}
  {/each}


{/if}
