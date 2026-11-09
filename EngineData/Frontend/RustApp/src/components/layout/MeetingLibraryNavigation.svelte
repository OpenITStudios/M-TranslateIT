<script lang="ts">
  import { tick } from "svelte";
  import { Folder, FolderPlus, Hash, MoreHorizontal, Pencil, Trash2, FolderInput, ChevronDown, Pin, PinOff } from "@lucide/svelte";
  import type { MeetingLibrary, MeetingLibraryChange, MeetingLibrarySession } from "../../app/bridge/runtimeApi";
  import { pinnedFirst, recentMeetings } from "./meetingNavigationOrder";

  let {
    library, selectedSessionId, liveSessionId = null, selectedSurface = "recent",
    viewAllSelected = false, onOpenSession, onViewAll, onChange, onDeleteSession,
  }: {
    library: MeetingLibrary | null;
    selectedSessionId: string | null;
    liveSessionId?: string | null;
    selectedSurface?: "recent" | "project";
    viewAllSelected?: boolean;
    onOpenSession: (sessionId: string, surface: "recent" | "project") => void;
    onViewAll: () => void;
    onChange: (change: Omit<MeetingLibraryChange, "expectedRevision">) => Promise<boolean>;
    onDeleteSession: (sessionId: string) => Promise<boolean>;
  } = $props();

  type Edit = { kind: "create_project" | "rename_project" | "rename_meeting"; id: string };
  let menu = $state<string | null>(null);
  let edit = $state<Edit | null>(null);
  let value = $state("");
  let moving = $state<string | null>(null);
  let confirmingDelete = $state<string | null>(null);
  let busy = $state(false);
  let returnFocus = $state<HTMLButtonElement | null>(null);
  let fallbackFocus = $state<HTMLButtonElement | null>(null);
  let editInput = $state<HTMLInputElement | null>(null);
  let collapsed = $state<Record<string, boolean>>({});
  let actionSurface = $state<"recent" | "project">("recent");
  const deletingProject = $derived(Boolean(confirmingDelete && library?.projects.some(project => project.project_id === confirmingDelete)));

  // The native archive list is newest-first. Show ALL pins plus five ordinary
  // recent sessions; never let the pin quota hide an ordinary recent meeting.
  const recent = $derived(recentMeetings(library?.sessions ?? []));
  function selectedIn(session: MeetingLibrarySession, surface: "recent" | "project"): boolean {
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
    edit = null;
    moving = null;
    confirmingDelete = null;
  }
  // One exclusive action surface across Recent, Project and category rows.
  // No independent menus may open for duplicate appearances of one session.
  function toggleMenu(key: string, trigger: HTMLButtonElement): void {
    if (busy) return;
    const next = menu === key ? null : key;
    if (next === null) { dismissActions(); return; }
    clearMenus();
    returnFocus = trigger;
    menu = next;
  }
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
        <button type="submit" class="ti-library-small ti-library-primary" disabled={busy || !library.ok || !value.trim()}>Save</button>
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

<svelte:window onkeydown={handleEscape} />

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
        aria-label="View all saved Meetings" aria-current={viewAllSelected ? "page" : undefined}
        onclick={() => { if (!busy) { clearMenus(); onViewAll(); } }}>View All</button>
    </div>
    {#if recent.length === 0}
      <p class="mx-3 my-2 text-[12px] text-[var(--ti-text-soft)]">No recent meetings</p>
    {:else}
      {#each recent as session (session.session_id)}
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
          <div class="ti-library-menu">
            <button onclick={() => openEdit("rename_meeting", session.session_id, session.display_name)}><Pencil size={14}/> Rename</button>
            <button onclick={() => void modify({ action: "pin_meeting", sessionId: session.session_id, pinned: !session.pinned })}>
              {#if session.pinned}<PinOff size={14}/>{:else}<Pin size={14}/>{/if} {session.pinned ? "Unpin Meeting" : "Pin Meeting"}
            </button>
            <button onclick={() => openMove(session.session_id, "recent")}><FolderInput size={14}/> Move to Project</button>
            <button onclick={() => requestDelete(session.session_id, "recent")}><Trash2 size={14}/> Delete Meeting</button>
          </div>
        {/if}
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
  {#each library.projects as project (project.project_id)}
    {@const projectSessions = library.sessions.filter(session => session.project_id === project.project_id)}
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
      <div class="ti-library-menu">
        <button onclick={() => openEdit("rename_project", project.project_id, project.name)}><Pencil size={14}/> Rename Project</button>
        <button onclick={() => requestDelete(project.project_id)}><Trash2 size={14}/> Delete Project</button>
      </div>
    {/if}
    {@render actionEditor("project:" + project.project_id)}
    {#if !collapsed[project.project_id]}
      {#each pinnedFirst(projectSessions) as session (session.session_id)}
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
          <div class="ti-library-menu ml-5">
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
        {@render actionEditor("project:" + session.session_id)}
      {/each}
    {/if}
  {/each}


{/if}
