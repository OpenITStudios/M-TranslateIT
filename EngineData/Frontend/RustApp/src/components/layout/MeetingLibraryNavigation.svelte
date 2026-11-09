<script lang="ts">
  import { Folder, FolderPlus, Hash, MoreHorizontal, Pencil, Trash2, FolderInput, ChevronDown, Pin, PinOff } from "@lucide/svelte";
  import type { MeetingLibrary, MeetingLibraryChange, MeetingLibrarySession } from "../../app/bridge/runtimeApi";

  let {
    library, selectedSessionId, meetingLive = false, onOpenSession, onChange, onDeleteSession,
  }: {
    library: MeetingLibrary | null;
    selectedSessionId: string | null;
    meetingLive?: boolean;
    onOpenSession: (sessionId: string) => void;
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
  let collapsed = $state<Record<string, boolean>>({});

  // Keep the latest archive order authoritative; Pin only affects presentation.
  const recent = $derived([...(library?.sessions ?? [])]
    .sort((a, b) => Number(b.pinned) - Number(a.pinned)).slice(0, 6));
  function clearMenus(): void { menu = null; edit = null; moving = null; confirmingDelete = null; }
  function openEdit(kind: Edit["kind"], id: string, previous = ""): void {
    menu = null; edit = { kind, id }; value = previous;
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
      if (await onChange(action)) { edit = null; value = ""; }
    } finally { busy = false; }
  }
  async function modify(action: Omit<MeetingLibraryChange, "expectedRevision">): Promise<void> {
    if (busy) return;
    busy = true;
    try { if (await onChange(action)) clearMenus(); }
    finally { busy = false; }
  }
  async function deleteConfirmed(id: string): Promise<void> {
    if (busy || confirmingDelete !== id) return;
    busy = true;
    try {
      const ok = id.startsWith("project_")
        ? await onChange({ action: "delete_project", projectId: id })
        : await onDeleteSession(id);
      if (ok) clearMenus();
    } finally { busy = false; }
  }
  function itemLabel(session: MeetingLibrarySession): string {
    return session.display_name;
  }
</script>

{#if library === null}
  <p class="mx-3 my-3 text-[12px] text-[var(--ti-text-soft)]">Checking saved Meetings...</p>
{:else}
  {#if !library.ok}
    <p class="mx-3 my-3 text-[12px] leading-5 text-[var(--ti-warning)]" role="status">
      {library.message || "Meeting metadata unavailable. Archived sessions remain accessible."}
    </p>
  {/if}
  <div class="mt-5">
    <p class="ti-channel-heading">RECENT MEETINGS</p>
    {#if recent.length === 0}
      <p class="mx-3 my-2 text-[12px] text-[var(--ti-text-soft)]">No Meeting yet</p>
    {:else}
      {#each recent as session (session.session_id)}
        <div class="ti-library-channel">
          <button type="button" class={"ti-channel-item min-w-0 flex-1 " + (selectedSessionId === session.session_id ? "is-active" : "")}
            aria-label={itemLabel(session)} aria-current={selectedSessionId === session.session_id ? "page" : undefined}
            title={itemLabel(session)} onclick={() => onOpenSession(session.session_id)}>
            {#if session.pinned}<Pin size={16} class="shrink-0 text-[var(--ti-text-muted)]"/>{:else}<Hash size={16} class="shrink-0"/>{/if}
            <span class="min-w-0 flex-1 truncate">{itemLabel(session)}</span>
            {#if meetingLive && selectedSessionId === session.session_id}
              <span class="size-1.5 rounded-full bg-[var(--ti-success)]" title="Live"></span>
            {:else if session.storage_warning || !session.complete}
              <span class="size-1.5 rounded-full bg-[var(--ti-warning)]" title="Storage needs review"></span>
            {/if}
          </button>
          <button type="button" class="ti-library-more" aria-label={"Manage " + itemLabel(session)}
            aria-expanded={menu === session.session_id}
            onclick={() => menu = menu === session.session_id ? null : session.session_id}>
            <MoreHorizontal size={16}/>
          </button>
        </div>
        {#if menu === session.session_id && library.ok}
          <div class="ti-library-menu">
            <button onclick={() => openEdit("rename_meeting", session.session_id, session.display_name)}><Pencil size={14}/> Rename</button>
            <button onclick={() => void modify({ action: "pin_meeting", sessionId: session.session_id, pinned: !session.pinned })}>
              <PinOff size={14}/> {session.pinned ? "Unpin Meeting" : "Pin Meeting"}
            </button>
            <button onclick={() => { moving = session.session_id; menu = null; }}><FolderInput size={14}/> Move to Project</button>
            <button onclick={() => { confirmingDelete = session.session_id; menu = null; }}><Trash2 size={14}/> Delete</button>
          </div>
        {/if}
      {/each}
    {/if}
  </div>
  <div class="mt-5 flex items-center justify-between gap-2 px-2">
    <p class="ti-channel-heading m-0">PROJECTS</p>
    <button type="button" class="ti-library-more" aria-label="Create Project category"
      title="Create Project" onclick={() => openEdit("create_project", "")}><FolderPlus size={17}/></button>
  </div>
  {#if library.projects.length === 0}
    <p class="mx-3 my-2 text-[12px] text-[var(--ti-text-soft)]">Create a Project to group Meetings</p>
  {/if}
  {#each library.projects as project (project.project_id)}
    <div class="ti-library-channel">
      <button type="button" class="ti-channel-item min-w-0 flex-1"
        aria-expanded={!collapsed[project.project_id]}
        onclick={() => collapsed = { ...collapsed, [project.project_id]: !collapsed[project.project_id] }}>
        <ChevronDown size={14} class={collapsed[project.project_id] ? "-rotate-90" : ""}/>
        <Folder size={16}/>
        <span class="min-w-0 flex-1 truncate" title={project.name}>{project.name}</span>
      </button>
      <button type="button" class="ti-library-more" aria-label={"Manage Project " + project.name}
        aria-expanded={menu === project.project_id}
        onclick={() => menu = menu === project.project_id ? null : project.project_id}>
        <MoreHorizontal size={16}/>
      </button>
    </div>
    {#if menu === project.project_id && library.ok}
      <div class="ti-library-menu">
        <button onclick={() => openEdit("rename_project", project.project_id, project.name)}><Pencil size={14}/> Rename Project</button>
        <button onclick={() => { confirmingDelete = project.project_id; menu = null; }}><Trash2 size={14}/> Delete Project</button>
      </div>
    {/if}
    {#if !collapsed[project.project_id]}
      {#each library.sessions.filter(session => session.project_id === project.project_id).sort((a, b) => Number(b.pinned) - Number(a.pinned)) as session (session.session_id)}
        <button type="button" class={"ti-channel-item ml-4 w-[calc(100%-1rem)] " + (selectedSessionId === session.session_id ? "is-active" : "")}
          title={session.display_name} onclick={() => onOpenSession(session.session_id)}>
          {#if session.pinned}<Pin size={15} class="shrink-0"/>{:else}<Hash size={15} class="shrink-0"/>{/if}
          <span class="min-w-0 truncate">{session.display_name}</span>
        </button>
      {/each}
    {/if}
  {/each}

  {#if edit}
    <form class="ti-library-editor" onsubmit={(event) => { event.preventDefault(); void commitEdit(); }}>
      <label for="ti-library-edit-name" class="text-[12px] font-semibold">
        {edit.kind === "create_project" ? "New Project" : edit.kind === "rename_project" ? "Rename Project" : "Rename Meeting"}
      </label>
      <input id="ti-library-edit-name" class="ti-field min-h-9 px-2 text-[13px]" maxlength="80" required
        bind:value={value} placeholder="Enter name"/>
      <div class="mt-2 flex justify-end gap-2">
        <button type="button" class="ti-library-small" onclick={() => edit = null}>Cancel</button>
        <button type="submit" class="ti-library-small ti-library-primary" disabled={busy || !library.ok || !value.trim()}>Save</button>
      </div>
    </form>
  {/if}

  {#if moving}
    <div class="ti-library-editor" role="group" aria-label="Move Meeting to Project">
      <p class="m-0 text-[12px] font-semibold">Move Meeting to</p>
      <button class="ti-library-small w-full text-left" disabled={busy}
        onclick={() => void modify({ action: "move_meeting", sessionId: moving, projectId: null })}>Uncategorized</button>
      {#each library.projects as project (project.project_id)}
        <button class="ti-library-small w-full text-left" disabled={busy}
          onclick={() => void modify({ action: "move_meeting", sessionId: moving, projectId: project.project_id })}>{project.name}</button>
      {/each}
      <button class="ti-library-small w-full text-left" onclick={() => moving = null}>Cancel</button>
    </div>
  {/if}
  {#if confirmingDelete}
    <div class="ti-library-editor border-[var(--ti-danger-border)]" role="group" aria-label="Confirm deletion">
      <p class="m-0 text-[12px] leading-5 text-[var(--ti-danger)]">
        {confirmingDelete.startsWith("project_")
          ? "Delete this category? Meetings will become Uncategorized."
          : "Delete this Meeting's local transcript? Exported HTML files remain."}
      </p>
      <div class="mt-2 flex justify-end gap-2">
        <button class="ti-library-small" onclick={() => confirmingDelete = null}>Cancel</button>
        <button class="ti-library-small ti-library-danger" disabled={busy}
          onclick={() => { if (confirmingDelete) void deleteConfirmed(confirmingDelete); }}>Delete</button>
      </div>
    </div>
  {/if}
{/if}
