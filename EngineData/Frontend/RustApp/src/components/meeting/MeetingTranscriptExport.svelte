<script lang="ts">
  import { onMount } from "svelte";
  import { Check, Copy, Download, ExternalLink, FolderOpen, RefreshCw, Trash2, Pin } from "@lucide/svelte";
  import { runtimeApi, type MeetingCommittedTurn, type MeetingCommittedTurnsSnapshot, type MeetingLibrary } from "../../app/bridge/runtimeApi";
  import { languageName } from "../../app/shared/state";
  import { meetingSpeakerLabel, meetingTime } from "../../app/runtime/meetingTranscriptPresentation";
  import MeetingTurnBubble from "./MeetingTurnBubble.svelte";
  import {
    DEFAULT_TRANSCRIPT_PREFERENCES, readTranscriptPreferences, subscribeTranscriptPreferences,
    type TranscriptPreferences,
  } from "./transcriptPreferences";

  let { hasSession = false, onNotice, onDeleted, onOpenSession }: {
    hasSession?: boolean;
    onNotice: (message: string) => void;
    onDeleted?: () => void;
    onOpenSession?: (id: string) => void;
  } = $props();
  let history = $state<MeetingLibrary | null>(null);
  // Pin is a presentation preference. The latest native archive order still
  // determines Last Meeting; this ordering is only for the Saved list.
  const visibleSessions = $derived([...(history?.sessions ?? [])]
    .sort((a, b) => Number(b.pinned) - Number(a.pinned)));
  let appearance = $state<TranscriptPreferences>({ ...DEFAULT_TRANSCRIPT_PREFERENCES });
  onMount(() => {
    appearance = readTranscriptPreferences();
    return subscribeTranscriptPreferences((next) => { appearance = next; });
  });
  let expanded = $state<string | null>(null);
  let preview = $state<MeetingCommittedTurnsSnapshot | null>(null);
  let previewLoading = $state(false);
  let previewError = $state("");
  let previewPanel = $state<HTMLDivElement>();
  // Ignore older reads after the reader closes or changes sessions.
  let previewRevision = 0;
  let language = $state<"bilingual" | "id" | "en">("bilingual");
  let confirming = $state<string | null>(null);
  let busy = $state(false);
  let loading = $state(false);
  let historyChecked = $state(false);
  let notice = $state("");
  let htmlSample = $state<{ sessionId: string; html: string; entryCount: number } | null>(null);
  let previewBusy = $state(false);
  let htmlPreviewRevision = 0;
  let receipt = $state<{ sessionId: string; path: string; count: number; incomplete: boolean } | null>(null);
  let previousActive = $state<boolean | null>(null);

  function sessionLabel(id: string): string {
    const savedName = history?.sessions.find(session => session.session_id === id)?.display_name;
    if (savedName) return savedName;
    const ms = Number(id.split("_")[1]);
    const date = new Date(ms);
    return Number.isSafeInteger(ms) && ms > 0 && !Number.isNaN(date.getTime())
      ? date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" })
      : "Saved Meeting";
  }
  function textFor(turn: MeetingCommittedTurn, code: "id" | "en") {
    if (turn.source_language === code) return { text: turn.source_text, label: "Original", fallback: false };
    if (turn.translation_state === "translated" && turn.translated_text.trim()) {
      return { text: turn.translated_text, label: "Translation", fallback: false };
    }
    return { text: turn.source_text, label: "Translation unavailable · Original fallback", fallback: true };
  }
  async function refresh(): Promise<void> {
    if (loading) return;
    loading = true;
    try {
      history = await runtimeApi.listMeetingLibrary();
      notice = history.ok ? "" : history.message;
      if (!history.ok) {
        expanded = null;
        preview = null;
        previewLoading = false;
        previewError = "";
        previewRevision += 1;
      }
    } catch {
      history = null;
      expanded = null;
      preview = null;
      previewLoading = false;
      previewError = "";
      previewRevision += 1;
      notice = "Saved transcripts are unavailable.";
    } finally {
      historyChecked = true;
      loading = false;
    }
  }
  async function togglePreview(sessionId: string): Promise<void> {
    confirming = null;
    notice = "";
    if (expanded === sessionId) {
      expanded = null;
      preview = null;
      previewLoading = false;
      previewError = "";
      previewRevision += 1;
      return;
    }
    expanded = sessionId;
    preview = null;
    previewError = "";
    previewLoading = true;
    const revision = ++previewRevision;
    try {
      const result = await runtimeApi.getSavedMeetingTranscriptPreview(sessionId);
      if (revision !== previewRevision || expanded !== sessionId) return;
      if (!result.ok) {
        previewError = result.note || "Saved transcript could not be loaded.";
        return;
      }
      preview = result;
    } catch {
      if (revision === previewRevision && expanded === sessionId) {
        previewError = "Saved transcript could not be loaded.";
      }
    } finally {
      if (revision === previewRevision && expanded === sessionId) previewLoading = false;
    }
  }

  async function retryPreview(sessionId: string): Promise<void> {
    if (expanded !== sessionId || previewLoading) return;
    // The Retry button will be removed by Loading. Keep keyboard focus on
    // its persistent preview region instead of dropping it to the document.
    previewPanel?.focus();
    expanded = null;
    await togglePreview(sessionId);
  }
  async function toggleHtmlPreview(sessionId: string): Promise<void> {
    if (previewBusy) return;
    if (htmlSample?.sessionId === sessionId) {
      htmlSample = null;
      return;
    }
    previewBusy = true;
    const revision = ++htmlPreviewRevision;
    notice = "";
    try {
      const result = await runtimeApi.previewSavedMeetingTranscriptHtml(sessionId);
      if (revision !== htmlPreviewRevision) return;
      if (!result.ok) {
        htmlSample = null;
        notice = result.message || "HTML preview is unavailable.";
        return;
      }
      htmlSample = { sessionId, html: result.html, entryCount: result.entry_count };
    } catch {
      if (revision === htmlPreviewRevision) {
        htmlSample = null;
        notice = "HTML preview could not be rendered.";
      }
    } finally { previewBusy = false; }
  }

  async function exportHtml(sessionId: string): Promise<void> {
    if (busy) return;
    busy = true; notice = ""; receipt = null; confirming = null;
    try {
      const result = await runtimeApi.exportSavedMeetingTranscriptHtml(sessionId);
      if (!result.ok || !result.file_path) { notice = result.message; return; }
      receipt = { sessionId, path: result.file_path, count: result.turn_count, incomplete: result.incomplete };
      onNotice("HTML transcript exported.");
      await refresh();
    } catch { notice = "HTML export could not be completed."; }
    finally { busy = false; }
  }
  async function deleteArchive(sessionId: string): Promise<void> {
    if (busy || confirming !== sessionId) return;
    busy = true; notice = "";
    try {
      const result = await runtimeApi.deleteSavedMeetingTranscript(sessionId);
      if (!result.ok) { notice = result.message; return; }
      if (expanded === sessionId) {
        expanded = null;
        preview = null;
        previewLoading = false;
        previewError = "";
        previewRevision += 1;
      }
      if (htmlSample?.sessionId === sessionId) htmlSample = null;
      htmlPreviewRevision += 1;
      if (receipt?.sessionId === sessionId) receipt = null;
      confirming = null;
      onDeleted?.();
      onNotice("Saved Meeting deleted.");
      await refresh();
    } catch { notice = "Saved transcript could not be deleted."; }
    finally { busy = false; }
  }
  async function openExport(folder: boolean): Promise<void> {
    if (!receipt) return;
    const fileName = receipt.path.split("/").pop();
    if (!fileName) return;
    const result = await runtimeApi.openSavedMeetingHtml(receipt.sessionId, fileName, folder);
    if (!result.ok) notice = result.message;
  }
  async function copyPath(): Promise<void> {
    if (!receipt) return;
    try {
      await navigator.clipboard.writeText(receipt.path);
      onNotice("Transcript file location copied.");
    } catch { notice = "Could not copy the file location."; }
  }
  $effect(() => {
    if (previousActive !== hasSession) {
      previousActive = hasSession;
      void refresh();
    }
  });
</script>

<section class="ti-panel mt-4 overflow-hidden" aria-label="Saved Meeting transcripts">
  <header class="flex flex-wrap items-start justify-between gap-3 px-5 py-4">
    <div class="grid gap-1">
      <h3 class="m-0 text-[15px] font-semibold">Saved Meeting</h3>
      <p class="m-0 max-w-xl text-[13px] leading-5 text-[var(--ti-text-muted)]">
        Conversations stay on this device until you delete them. Exporting HTML does not delete saved entries.
      </p>
    </div>
    <button type="button" class="ti-button ti-button-secondary min-h-10 text-[13px]" disabled={loading || busy}
      onclick={() => void refresh()}><RefreshCw size={14}/>{loading ? "Checking..." : "Refresh"}</button>
  </header>
  {#if history && !history.ok}
    <p class="mx-5 mb-3 text-[13px] leading-5 text-[var(--ti-warning)]" role="alert">{history.message}</p>
  {/if}
  {#if notice && history?.ok}
    <p class="mx-5 mb-3 break-words text-[13px] leading-5 text-[var(--ti-warning)]" role="alert">{notice}</p>
  {/if}
  {#if history && history.active_write_errors > 0}
    <p class="mx-5 mb-3 text-[13px] text-[var(--ti-warning)]" role="alert">
      Some transcript entries could not be saved; storage completeness is unverified.
    </p>
  {/if}
  {#if !history && (!historyChecked || loading)}
    <p class="m-0 border-t border-[var(--ti-border)] px-5 py-5 text-[13px] text-[var(--ti-text-muted)]"
      role="status" aria-live="polite">Checking saved transcripts...</p>
  {:else if !history || (!history.ok && history.sessions.length === 0)}
    <p class="m-0 border-t border-[var(--ti-border)] px-5 py-5 text-[13px] leading-5 text-[var(--ti-warning)]"
      role="status">Saved Meeting library could not be verified. Select Refresh; existing files were not changed.</p>
  {:else if history.sessions.length === 0}
    <p class="m-0 border-t border-[var(--ti-border)] px-5 py-6 text-sm text-[var(--ti-text-muted)]">
      No Meeting archives have been saved yet.
    </p>
  {:else}
    <div class="grid gap-2 border-t border-[var(--ti-border)] px-3 py-3">
      {#each visibleSessions as session (session.session_id)}
        <div class="grid gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface-raised)] px-4 py-4">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div class="min-w-0">
              <strong class="flex items-center gap-2 text-[13px] font-semibold">
                {#if session.pinned}<Pin size={14} class="shrink-0 text-[var(--ti-text-muted)]" aria-label="Pinned Meeting"/>{/if}
                <span class="min-w-0 truncate">{sessionLabel(session.session_id)}</span>
              </strong>
              <p class="m-0 mt-1 text-[13px] text-[var(--ti-text-muted)]">
                {session.entry_count === 0 ? "No recognized entries" : `${session.entry_count} saved entries`} · {session.complete ? "Finalized (entry count checked)" : "Storage finalization unverified"}
              </p>
              {#if session.storage_warning || !session.complete}
                <p class="m-0 mt-1 text-[13px] text-[var(--ti-warning)]">Check this transcript for missing entries.</p>
              {/if}
            </div>
            <div class="flex min-w-0 flex-wrap items-start gap-2">
              <button type="button" class="ti-button ti-button-secondary min-h-10 text-[13px] font-semibold"
                aria-label={`Open saved Meeting channel ${sessionLabel(session.session_id)}`}
                disabled={busy || !onOpenSession}
                onclick={() => onOpenSession?.(session.session_id)}>
                Open channel
              </button>
              <details class="min-w-[125px] max-w-full rounded-[var(--ti-radius-sm)] border border-[var(--ti-border)] bg-[var(--ti-bg)]">
                <summary class="min-h-10 cursor-pointer px-3 py-2.5 text-[13px] font-medium text-[var(--ti-text)] hover:bg-[var(--ti-surface-hover)]"
                  aria-label={`More actions for saved transcript ${sessionLabel(session.session_id)}`}>More options</summary>
                <div class="grid gap-2 border-t border-[var(--ti-border)] p-2" role="group"
                  aria-label={`Transcript file actions for ${sessionLabel(session.session_id)}`}>
                  <button type="button" class="ti-button ti-button-secondary min-h-10 w-full justify-start px-3 text-[13px]"
                    disabled={busy || previewBusy || session.entry_count === 0} aria-expanded={htmlSample?.sessionId === session.session_id}
                    aria-label={`${htmlSample?.sessionId === session.session_id ? "Hide HTML preview" : "Preview HTML"} for saved transcript ${sessionLabel(session.session_id)}`}
                    onclick={() => void toggleHtmlPreview(session.session_id)}>
                    {htmlSample?.sessionId === session.session_id ? "Hide HTML Preview" : "Preview HTML"}
                  </button>
                  <button type="button" class="ti-button ti-button-secondary min-h-10 w-full justify-start px-3 text-[13px]" disabled={busy || session.entry_count === 0}
                    aria-label={`Export HTML for saved transcript ${sessionLabel(session.session_id)}`}
                    onclick={() => void exportHtml(session.session_id)}><Download size={14} aria-hidden="true"/>Export HTML</button>
                  <button type="button" class="ti-button ti-button-secondary min-h-10 w-full justify-start px-3 text-[13px]" disabled={busy}
                    aria-label={`Delete saved transcript ${sessionLabel(session.session_id)}`}
                    onclick={() => { confirming = session.session_id; notice = ""; }}><Trash2 size={14} aria-hidden="true"/>Delete</button>
                </div>
              </details>
            </div>
          </div>
          {#if confirming === session.session_id}
            <div class="rounded-[var(--ti-radius-md)] border border-[var(--ti-danger-border)] bg-[var(--ti-danger-surface)] p-3">
              <p class="m-0 text-sm text-[var(--ti-danger)]">
                Delete this saved conversation permanently? Existing exported HTML files will remain.
              </p>
              <div class="mt-3 flex flex-wrap gap-2">
                <button type="button" class="ti-button ti-button-danger min-h-10"
                  onclick={() => void deleteArchive(session.session_id)} disabled={busy}>Delete Meeting</button>
                <button type="button" class="ti-button ti-button-secondary min-h-10"
                  onclick={() => confirming = null}>Cancel</button>
              </div>
            </div>
          {/if}
          {#if htmlSample?.sessionId === session.session_id}
            <div class="grid gap-2 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-3">
              <strong class="text-sm">HTML document preview</strong>
              <p class="m-0 text-[13px] text-[var(--ti-text-muted)]">
                Sample of the latest {htmlSample.entryCount} saved entries, rendered with the HTML export template.
                The final file includes all archived entries. Switch reading language inside the preview.
              </p>
              <iframe title="Offline HTML transcript document preview" sandbox=""
                srcdoc={htmlSample.html} class="h-[480px] w-full rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-bg)]"></iframe>
            </div>
          {/if}
          {#if expanded === session.session_id}
            <div bind:this={previewPanel} class="rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] p-3"
              role="region" aria-label={`Preview saved transcript ${sessionLabel(session.session_id)}`} tabindex="-1">
              <div class="mb-3 flex flex-wrap items-center justify-between gap-3">
                <div>
                  <strong class="text-sm">Conversation</strong>
                  <p class="m-0 text-[13px] text-[var(--ti-text-muted)]">
                    Latest {preview?.turns.length ?? 0} entries · HTML includes all archived entries.
                  </p>
                </div>
                <div class="flex flex-wrap gap-1 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-1"
                  role="group" aria-label="Read transcript in">
                  {#each [{ code: "bilingual", name: "Bilingual" }, { code: "id", name: "Bahasa Indonesia" }, { code: "en", name: "English" }] as option (option.code)}
                    <button type="button" class={"inline-flex min-h-10 items-center rounded-[var(--ti-radius-sm)] px-3 text-[13px] " +
                      (language === option.code ? "bg-[var(--ti-surface-raised)]" : "text-[var(--ti-text-muted)]")}
                      aria-pressed={language === option.code}
                      onclick={() => language = option.code as "bilingual" | "id" | "en"}>{option.name}</button>
                  {/each}
                </div>
              </div>
              {#if previewLoading}
                <p class="m-0 text-[13px] text-[var(--ti-text-muted)]" role="status" aria-live="polite">Loading saved conversation...</p>
              {:else if previewError}
                <div class="grid gap-3">
                  <p class="m-0 break-words text-[13px] leading-5 text-[var(--ti-warning)]" role="alert">{previewError} The saved session was not modified.</p>
                  <button type="button" class="ti-button ti-button-secondary justify-self-start"
                    onclick={() => void retryPreview(session.session_id)}>Retry preview</button>
                </div>
              {:else if !preview}
                <p class="m-0 text-[13px] text-[var(--ti-text-muted)]">Preview is unavailable. Select Read to close and try again.</p>
              {:else}
                <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need focus to scroll this named region.) -->
                <div class="max-h-[360px] divide-y divide-[var(--ti-border)] overflow-y-auto" role="region" aria-label="Saved conversation" tabindex="0">
                  {#each preview.turns as turn (turn.sequence)}
                    <div class="py-3 first:pt-0 last:pb-0">
                      {#if language === "bilingual"}
                        <MeetingTurnBubble {turn} preferences={appearance} />
                      {:else}
                        {@const selected = textFor(turn, language)}
                        <div class={"flex w-full " + (turn.lane === "you" ? "justify-end" : "justify-start")}>
                          <article
                            class={"w-[min(88%,690px)] min-w-0 rounded-[var(--ti-radius-lg)] border px-4 py-3 " +
                              (turn.lane === "you" ? "border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)]" :
                                "border-[var(--ti-border)] bg-[var(--ti-surface-soft)]")}
                            aria-label={meetingSpeakerLabel(turn.lane)}
                          >
                            <header class="mb-2 flex flex-wrap items-center justify-between gap-2">
                              <strong class="text-[13px] font-semibold">{meetingSpeakerLabel(turn.lane)}</strong>
                              <span class="text-[13px] text-[var(--ti-text-muted)]">{meetingTime(turn.created_unix_ms)}</span>
                            </header>
                            <span class="text-[13px] text-[var(--ti-text-muted)]">{selected.label} · {languageName(selected.fallback ? turn.source_language : language)}</span>
                            <p class={"m-0 mt-1 whitespace-pre-wrap break-words text-[14px] leading-6 " +
                              (selected.fallback ? "text-[var(--ti-warning)]" : "")}
                              lang={selected.fallback ? turn.source_language : language}>{selected.text}</p>
                          </article>
                        </div>
                      {/if}
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  {#if receipt}
    <div class="grid gap-2 border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4" role="status">
      <div class="flex items-center gap-2">
        <Check size={16} class="text-[var(--ti-success)]"/>
        <strong class="text-sm">HTML transcript saved</strong>
      </div>
      <p class="m-0 break-words text-[13px] text-[var(--ti-text-muted)]">{receipt.count} entries · {receipt.path}</p>
      {#if receipt.incomplete}
        <p class="m-0 text-[13px] text-[var(--ti-warning)]">Storage completeness could not be verified. The HTML includes a warning.</p>
      {/if}
      <div class="flex flex-wrap gap-2">
        <button type="button" class="ti-button ti-button-secondary min-h-10 text-[13px]"
          onclick={() => void openExport(false)}><ExternalLink size={14}/>Open File</button>
        <button type="button" class="ti-button ti-button-secondary min-h-10 text-[13px]"
          onclick={() => void openExport(true)}><FolderOpen size={14}/>Open Folder</button>
        <button type="button" class="ti-button ti-button-secondary min-h-10 text-[13px]"
          onclick={() => void copyPath()}><Copy size={14}/>Copy Location</button>
      </div>
      <p class="m-0 text-[13px] text-[var(--ti-text-muted)]">
        The source transcript remains saved until you choose Delete.
      </p>
    </div>
  {/if}
</section>
