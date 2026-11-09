import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const meeting = readFileSync(new URL("../../src/pages/Meeting.svelte", import.meta.url), "utf8");
const live = readFileSync(new URL("../../src/components/meeting/MeetingActivity.svelte", import.meta.url), "utf8");
const review = readFileSync(new URL("../../src/components/meeting/MeetingSessionReview.svelte", import.meta.url), "utf8");
const header = readFileSync(new URL("../../src/components/layout/AppHeader.svelte", import.meta.url), "utf8");

test("Header owns audio source switching without hiding the native Stop control", () => {
  assert.match(header, /runtimeApi\.selectMeetingAudioSource\(source\)/);
  assert.match(header, /runtimeApi\.selectMeetingPreferredAudioSource\(source\)/);
  assert.match(header, /aria-label="Audio sent to meeting"/);
  assert.match(meeting, /const actionEnabled = \$derived\(!resumeBusy && !meeting\.busy && !actionBusy/);
  assert.match(meeting, /meeting\.lifecycle === "cleanup_incomplete" \? primaryControl\.label : "Stop Meeting"/);
});

test("Meeting main dock is valid markup and has no stray text after the closing footer", () => {
  assert.equal((meeting.match(/<footer\b/g) ?? []).length, 1);
  assert.equal((meeting.match(/<\/footer>/g) ?? []).length, 1);
  assert.match(meeting, /<\/footer>\s*\{\/if\}\s*<\/section>/);
  assert.doesNotMatch(meeting, /<\/footer>ter>/);
});

test("Meeting has a single primary button that can Start, Stop, or guide recovery", () => {
  assert.match(header, /Audio sent/);
  assert.match(meeting, /meetingControlState\(/);
  assert.match(meeting, /disabled=\{!actionEnabled\}/);
  assert.match(meeting, /onclick=\{handlePrimary\}/);
  assert.match(meeting, /needsSetup \? "Finish Setup"/);
  assert.match(meeting, /needsVoice \? "Choose Voice"/);
  assert.equal((meeting.match(/onclick=\{handlePrimary\}/g) ?? []).length, 1);
  for (const retired of ["MeetingPresetPanel", "meeting-presets", "swapListenDirection",
    "detectMeetingApp", "Meeting options", "Mic Test"]) {
    assert.equal(meeting.includes(retired), false, retired);
  }
});
test("Stop or Retry Stop remains in one dock below the transcript", () => {
  assert.match(meeting, /meeting\.lifecycle === "cleanup_incomplete" \? primaryControl\.label : "Stop Meeting"/);
  const action = meeting.indexOf('aria-label="Meeting main action"');
  const transcript = meeting.indexOf("<MeetingActivity status={meetingStatus}");
  assert.ok(transcript > 0 && transcript < action);
  assert.match(meeting, /section === "live" \|\| meeting\.hasSession \|\| meeting\.canStop/);
  assert.doesNotMatch(meeting, /onToggleMute/);
  assert.equal((meeting.match(/onclick=\{handlePrimary\}/g) ?? []).length, 1);
});

test("deferred setup has a real recovery path without creating another Meeting start", () => {
  assert.match(meeting, /async function continueSetup\(\)/);
  assert.match(meeting, /meeting_setup_state: "new"/);
  assert.match(meeting, /onNavigate\("settings"\)/);
  assert.match(meeting, /onNavigate\("my-voice"\)/);
  assert.match(meeting, /meeting\.canStop/);
});
test("live Meeting retains committed conversation and optional session review", () => {
  assert.match(meeting, /<MeetingActivity status=\{meetingStatus\} turns=\{meetingTurns\}/);
  assert.match(meeting, /<MeetingSessionReview/);
  assert.match(meeting, /section === "live" \|\| meeting\.hasSession \|\| meeting\.canStop/);
});
test("Meeting Live reports local output truth, not fabricated participant delivery", () => {
  assert.match(live, /if \(status\.outbound\.output_active\)/);
  assert.match(live, /label: "Speaking"/);
  assert.match(live, /label: "Translating"/);
  assert.match(live, /label: "Listening"/);
  assert.match(live, /case "output_complete": return "Local playback complete"/);
  assert.match(live, /case "output_failed": return "Not played"/);
  assert.doesNotMatch(live, /"Sent"|"Delivered to participants"|"Sending"/);
  assert.match(live, /statusLabel=\{turn\.lane === "you" \? deliveryLabel\(turn\) : ""\}/);
});

test("Meeting exposes bounded speech loss without falsely attributing a speaker", () => {
  const native = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/session_state.rs", import.meta.url), "utf8");
  assert.match(native, /status\.overflow_dropped_utterance_count = overflow_dropped_utterance_count\(\)/);
  assert.match(native, /status\.evicted_pending_utterance_count = evicted_pending_utterance_count\(\)/);
  assert.match(live, /status\.outbound\.overflow_dropped_utterance_count/);
  assert.match(live, /status\.outbound\.evicted_pending_utterance_count/);
  assert.match(live, /\{#if missingSpeech\}/);
  assert.match(live, /counts combine your microphone and incoming meeting audio/);
  assert.match(live, /Important details may need repeating/);
});

test("Live transcript stays minimal while preserving reading position and copy errors", () => {
  assert.doesNotMatch(live, /searchOpen|toggleSearch|togglePin|pinnedSequences|PinOff/);
  assert.match(live, /followingLatest/);
  assert.match(live, /onscroll=\{updateScrollPosition\}/);
  assert.match(live, />Jump to latest<\/button>/);
  assert.match(live, /copyFailedSequence = turn\.sequence/);
  assert.match(live, /Couldn't copy this phrase\. Try again\./);
  const bubble = readFileSync(new URL("../../src/components/meeting/MeetingTurnBubble.svelte", import.meta.url), "utf8");
  assert.match(live, /<MeetingTurnBubble \{turn\}/);
  assert.match(bubble, /languageName\(turn\.source_language\)/);
  assert.match(bubble, /lang=\{block\.kind === "original" \? turn\.source_language/);
  assert.match(bubble, /turn\.translation_state === "translated" \? turn\.target_language : undefined/);
});

test("global notices expose full text without noninteractive keyboard tabindex", () => {
  const notice = header.slice(header.indexOf('<div class={`ti-notice-bar'));
  assert.match(notice, /flex-1 break-words/);
  assert.match(notice, /role="alert" aria-atomic="true"/);
  assert.match(notice, /role="status" aria-live="polite" aria-atomic="true"/);
  assert.match(notice, /\{notice\.message\}/);
  assert.doesNotMatch(notice, /\b(?:tabindex|truncate|line-clamp-\d+)\b/);
});

test("Last-session transcript is private-by-default and export remains explicit", () => {
  assert.match(review, /<details bind:open=\{reviewOpen\}/);
  assert.match(review, /reviewOpen = false/);
  assert.match(review, /getRecentMeetingTranscript\(\)/);
  assert.match(review, /Earlier sessions may be in Saved Meeting/);
  assert.doesNotMatch(review, /<MeetingTranscriptExport/);
  assert.match(meeting, /<MeetingTranscriptExport hasSession=\{meeting\.hasSession\}/);
  assert.doesNotMatch(review, /\$effect\([\s\S]{0,250}exportMeetingTranscript\(/);
});

test("live and last-session transcript reuse one Discord-style bilingual lane-aware bubble", () => {
  const bubble = readFileSync(new URL("../../src/components/meeting/MeetingTurnBubble.svelte", import.meta.url), "utf8");
  assert.match(live, /<MeetingTurnBubble \{turn\}/);
  assert.match(review, /<MeetingTurnBubble \{turn\}/);
  assert.match(bubble, /own \? "justify-end" : "justify-start"/);
  assert.match(bubble, /meetingSpeakerLabel\(turn\.lane\)/);
  assert.match(bubble, /turn\.created_unix_ms/);
  assert.match(bubble, /Original/);
  // Speaker identity remains available in bubble accessibility metadata,
  // without repeating technical explanations in the transcript toolbar.
  assert.doesNotMatch(live, /Meeting audio is combined; individual speakers are not identified/);
  const labels = readFileSync(new URL("../../src/app/runtime/meetingTranscriptPresentation.ts", import.meta.url), "utf8");
  assert.match(labels, /Combined incoming audio; speakers are not identified/);
  assert.doesNotMatch(review, /Meeting audio combines incoming sound; individual speakers are not identified/);
  assert.match(bubble, /Translation/);
  assert.doesNotMatch(bubble, /Person 2|speaker_id|diarizationResult/);
});

test("failed Meeting translation retains source and cannot look completed", () => {
  const bubble = readFileSync(new URL("../../src/components/meeting/MeetingTurnBubble.svelte", import.meta.url), "utf8");
  const display = readFileSync(new URL("../../src/app/runtime/meetingTranscriptPresentation.ts", import.meta.url), "utf8");
  const native = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/committed_turns.rs", import.meta.url), "utf8");
  assert.match(bubble, /meetingTranslationDisplay\(turn\)/);
  assert.match(display, /Translation unavailable/);
  assert.match(native, /fn finish_translation_at\(/);
  assert.match(native, /translation_state = "failed"/);
  assert.match(native, /fn live_turn_owner_matches\([\s\S]*?session\.authority_active\s*&& session\.phase == "live"/);
  assert.match(native, /pub\(super\) fn finish_meeting_turn_translation\([\s\S]*?if !live_turn_owner_matches\(current\.snapshot\.as_ref\(\), session_id, generation\) \{\s*return false;/);
});

test("Discord-inspired shell uses exactly one sidebar, channel navigation, and bottom settings", () => {
  const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
  const sidebar = readFileSync(new URL("../../src/components/layout/Sidebar.svelte", import.meta.url), "utf8");
  const settings = readFileSync(new URL("../../src/pages/Settings.svelte", import.meta.url), "utf8");
  const css = readFileSync(new URL("../../src/styles/tokens.css", import.meta.url), "utf8");
  assert.equal((app.match(/<Sidebar\b/g) ?? []).length, 1);
  assert.match(app, /meetingSection = \$state<"live" \| "last" \| "saved" \| "session">/);
  assert.match(sidebar, /onMeetingSection\("live"\)/);
  assert.match(sidebar, /<p class="ti-channel-heading">HOME<\/p>/);
  assert.match(sidebar, /aria-label="Start Meeting"/);
  assert.match(sidebar, /<span>My Voice<\/span>/);
  assert.doesNotMatch(sidebar, /<p class="ti-channel-heading">VOICE<\/p>/);
  assert.doesNotMatch(sidebar, /label: "Last Meeting"|label: "Saved Meeting"/);
  assert.match(sidebar, /onViewAll=\{\(\) => onMeetingSection\("saved"\)\}/);
  assert.match(sidebar, /<SidebarUserControls/);
  const userControls = readFileSync(new URL("../../src/components/layout/SidebarUserControls.svelte", import.meta.url), "utf8");
  assert.match(userControls, /aria-label="Open Settings"/);
  assert.match(header, /aria-label="Open Quick Access"/);
  assert.doesNotMatch(settings, /aria-label="Settings categories"/);
  assert.match(css, /--ti-sidebar-width: 248px/);
  assert.match(app, /route === "meeting" \? "overflow-hidden" : "overflow-y-auto"/);
});

test("Native Meeting mute and source owner are reused by two-column call dock", () => {
  const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
  const sidebar = readFileSync(new URL("../../src/components/layout/Sidebar.svelte", import.meta.url), "utf8");
  const native = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/audio_source.rs", import.meta.url), "utf8");
  assert.match(app, /runtimeApi\.setMeetingOutputMuted\(desired\)/);
  assert.match(app, /if \(muteBusy \|\| !snapshot\?\.meeting\.live/);
  assert.match(sidebar, /onToggleMute/);
  assert.doesNotMatch(meeting, /onToggleMute/, "Mute must only be in the sidebar footer");
  assert.match(native, /Mute is an output gate, never a third microphone source/);
  assert.match(meeting, /<MeetingSessionReview sessionActive=\{meeting\.hasSession\} openByDefault=\{true\}/);
  assert.match(meeting, /<MeetingTranscriptExport hasSession=\{meeting\.hasSession\}/);
});

test("Discord-like footer groups Voice Actor, Mute, device dropdown and Settings only on the left", () => {
  const sidebar = readFileSync(new URL("../../src/components/layout/Sidebar.svelte", import.meta.url), "utf8");
  const controls = readFileSync(new URL("../../src/components/layout/SidebarUserControls.svelte", import.meta.url), "utf8");
  assert.match(sidebar, /<SidebarUserControls/);
  assert.match(controls, /aria-label="Choose Voice Actor"/);
  assert.match(controls, /aria-label=\{meetingMuted \? "Unmute TranslateIT output" : "Mute TranslateIT output"\}/);
  assert.match(controls, /aria-label="Choose audio input and meeting sound devices"/);
  assert.match(controls, /aria-label="Open Settings"/);
  assert.doesNotMatch(controls, /Deafen|Undeafen|Headphones/);
  assert.match(controls, /myVoiceBuildApi\.getStatus\(\)/);
  assert.match(controls, /myVoiceBuildApi\.selectBuiltin\(id, confirmed\)/);
  assert.match(controls, /result\.state === "approval_required"/);
  assert.match(controls, /resourcesLocked/);
  assert.match(controls, /runtimeProductFacade\.selectProductAudioDevice\(kind, requestedId, settings\)/);
  assert.match(controls, /unsafeDevice\(kind, candidate/);
  assert.match(controls, /onOpenAudioSettings\(\)/);
});

test("One header contains Audio Sent and More, with Display in Transcript itself", () => {
  const overlay = readFileSync(new URL("../../src/app/runtime/translationOverlayRuntime.ts", import.meta.url), "utf8");
  assert.match(header, /aria-label="TranslateIT workspace header"/);
  assert.match(header, /aria-label="Audio sent to meeting"/);
  assert.match(header, /aria-label="More meeting controls"/);
  assert.match(header, /Floating Captions/);
  assert.match(header, /Quick Access/);
  assert.match(header, /toggleFloatingCaptions\(\)/);
  assert.match(header, /aria-haspopup="menu"/);
  assert.match(overlay, /export async function toggleFloatingCaptions\(/);
  assert.match(overlay, /await window\.isVisible\(\)/);
  assert.match(overlay, /updateOverlayPreferences\(\{ meetingEnabled: true \}\)/);
  assert.match(live, /id="ti-transcript-display"/);
  assert.match(live, /updateTranscriptPreferences\(\{ textMode: mode \}\)/);
  assert.match(live, /<option value="both">Both<\/option>/);
  assert.match(live, /<option value="translation-only">Translation<\/option>/);
  assert.doesNotMatch(header, /aria-label="Transcript display mode"/);
  assert.doesNotMatch(meeting, /aria-label="Live Meeting toolbar"|meeting-audio-source|Floating Captions/);
  assert.match(meeting, /ti-call-stop/);
  assert.match(meeting, /onclick=\{handlePrimary\}/);
  const dock = meeting.indexOf('aria-label="Meeting main action"');
  const footer = meeting.slice(dock);
  assert.doesNotMatch(footer, /Audio sent|Captions|onToggleMute/);
});

test("muted state colors both halves of the sidebar split button", () => {
  const userControls = readFileSync(new URL("../../src/components/layout/SidebarUserControls.svelte", import.meta.url), "utf8");
  const css = readFileSync(new URL("../../src/styles/app.css", import.meta.url), "utf8");
  assert.match(userControls, /ti-mute-split flex/);
  assert.match(userControls, /meetingMuted \? "is-muted" : ""/);
  assert.match(css, /\.ti-mute-split\.is-muted \.ti-user-action/);
  assert.doesNotMatch(meeting, /Deafen|Undeafen/);
});

test("bilingual bubble keeps statuses truthful, source and translation together, with accessible copy", () => {
  const bubble = readFileSync(new URL("../../src/components/meeting/MeetingTurnBubble.svelte", import.meta.url), "utf8");
  const css = readFileSync(new URL("../../src/styles/app.css", import.meta.url), "utf8");
  assert.match(bubble, /ti-chat-bubble-own/);
  assert.match(bubble, /meetingTranslationDisplay\(turn\)/);
  assert.match(bubble, /meetingTime\(turn\.created_unix_ms\)/);
  assert.match(bubble, /\{#if onCopy\}/);
  assert.match(bubble, /onCopy\?\.\(\)/);
  assert.match(bubble, /statusLabel/);
  assert.match(live, /onCopy=\{\(\) => void copyTurn\(turn\)\}/);
  assert.match(live, /followingLatest/);
  assert.match(live, /role="log"/);
  assert.match(css, /width: min\(68%, 680px\)/);
  assert.match(css, /overflow-wrap: anywhere/);
});

test("Settings categories separate saved caption controls from read-only appearance defaults", () => {
  const sidebar = readFileSync(new URL("../../src/components/layout/Sidebar.svelte", import.meta.url), "utf8");
  const settings = readFileSync(new URL("../../src/pages/Settings.svelte", import.meta.url), "utf8");
  const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
  assert.match(sidebar, /Meeting & Transcript/);
  assert.match(sidebar, /Appearance/);
  assert.match(sidebar, /Voice & Audio/);
  assert.match(sidebar, /Help & Diagnostics/);
  assert.match(app, /settingsTab = \$state<"meeting" \| "translation" \| "appearance" \| "advanced">/);
  assert.match(settings, /Transcript Appearance/);
  assert.match(settings, /<TranscriptAppearanceSettings \{onNotice\}/);
  assert.match(settings, /\{:else if tab === "appearance"\}/);
  assert.match(settings, /<FloatingCaptionPreferences \{onNotice\}/);
});

test("Settings device discovery is lazy and only runs in Voice & Audio", () => {
  const settings = readFileSync(new URL("../../src/pages/Settings.svelte", import.meta.url), "utf8");
  assert.match(settings, /import \{ tick, untrack \} from "svelte"/);
  assert.match(settings, /if \(tab !== "meeting"\) return;/);
  assert.match(settings, /untrack\(\(\) => \{/);
  assert.match(settings, /void loadDevices\(\)/);
  assert.match(settings, /void loadRouteStatus\(\)/);
  assert.doesNotMatch(settings, /onMount\(\(\) => \{\s*void loadDevices/);
});

test("Sidebar popup ARIA controls only reference menus while mounted", () => {
  const controls = readFileSync(new URL("../../src/components/layout/SidebarUserControls.svelte", import.meta.url), "utf8");
  assert.match(controls, /aria-controls=\{popup === "voice" \? "ti-voice-actor-menu" : undefined\}/);
  assert.match(controls, /aria-controls=\{popup === "devices" \? "ti-audio-device-menu" : undefined\}/);
  assert.match(controls, /if \(popup && event\.key === "Escape"\)/);
  assert.match(controls, /openAudioSettings\(\)/);
});

test("Bilingual transcript appearance has editable persistent controls, without duplicating global theme", () => {
  const settings = readFileSync(new URL("../../src/components/meeting/TranscriptAppearanceSettings.svelte", import.meta.url), "utf8");
  const policy = readFileSync(new URL("../../src/components/meeting/transcriptPreferences.ts", import.meta.url), "utf8");
  assert.match(settings, /Message density/);
  assert.match(settings, /Text order/);
  assert.match(settings, /Timestamps/);
  assert.match(settings, /Show speaker icons/);
  assert.match(settings, /Follow latest message/);
  assert.match(settings, /Show local playback status/);
  assert.match(settings, /Restore defaults/);
  assert.match(settings, /result\.ok/);
  assert.match(policy, /normalizeTranscriptPreferences/);
  assert.match(policy, /localStorage/);
  assert.doesNotMatch(policy, /source_text|transcript_body|recording|mic_audio/);
});

test("Meetings organize by native ID, retaining archive source and storage status", () => {
  const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
  const library = readFileSync(new URL("../../src/components/layout/MeetingLibraryNavigation.svelte", import.meta.url), "utf8");
  const channel = readFileSync(new URL("../../src/components/meeting/MeetingChannel.svelte", import.meta.url), "utf8");
  const native = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/meeting_catalog.rs", import.meta.url), "utf8");
  const registry = readFileSync(new URL("../../src-tauri/src/commands/registry.rs", import.meta.url), "utf8");
  assert.match(app, /listMeetingLibrary\(\)/);
  assert.match(app, /updateMeetingLibrary\(/);
  assert.match(app, /previousSessionId/);
  assert.match(app, /selectedSessionId = result\.status\.session_id/);
  assert.match(library, /Rename Meeting/);
  assert.match(library, /Move to Project/);
  assert.match(library, /Pin Meeting/);
  assert.match(library, /Unpin Meeting/);
  assert.doesNotMatch(library, /Star Meeting|Remove Star|<Star|star_meeting/);
  assert.match(library, /Create Project category/);
  assert.match(library, /Delete Project/);
  assert.match(channel, /getSavedMeetingTranscriptPreview/);
  assert.match(native, /expected_revision/);
  assert.match(native, /saved_session_operation_guard/);
  assert.match(native, /alias = "starred"/);
  assert.match(native, /"pin_meeting" => item\.pinned/);
  assert.match(native, /legacy_starred_catalog_entries_migrate_to_pinned_without_losing_state/);
  assert.match(native, /session_id: id\.to_string\(\)/);
  assert.match(native, /Damaged category metadata must not hide or mutate intact archives/);
  assert.match(registry, /list_meeting_library/);
  assert.match(registry, /update_meeting_library/);
});

test("Ready state uses one Start action and a concise call setup instruction", () => {
  assert.match(meeting, /Ready to Translate/);
  assert.match(meeting, /Select TranslateIT as your microphone in Discord or Zoom, then select Start/);
  assert.match(meeting, /: "Start"/);
  assert.equal((meeting.match(/onclick=\{handlePrimary\}/g) ?? []).length, 1);
  assert.doesNotMatch(live, /Meeting audio capture active · speakers not identified/);
  assert.doesNotMatch(live, /Meeting audio is combined; individual speakers are not identified/);
});

test("HOME only shows Start Meeting and My Voice, with Recent and Projects below", () => {
  const sidebar = readFileSync(new URL("../../src/components/layout/Sidebar.svelte", import.meta.url), "utf8");
  const navigation = readFileSync(new URL("../../src/components/layout/MeetingLibraryNavigation.svelte", import.meta.url), "utf8");
  const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
  assert.match(sidebar, /<p class="ti-channel-heading">HOME<\/p>/);
  assert.match(sidebar, /aria-label="Start Meeting"/);
  assert.match(sidebar, /<span>My Voice<\/span>/);
  assert.doesNotMatch(sidebar, /<p class="ti-channel-heading">VOICE<\/p>/);
  assert.match(navigation, /RECENT MEETINGS/);
  assert.match(navigation, /PROJECTS/);
  assert.match(navigation, /View All/);
  assert.match(navigation, /recentMeetings\(library\?\.sessions \?\? \[\]\)/);
  assert.match(navigation, /pinnedFirst\(projectSessions\)/);
  assert.match(sidebar, /onViewAll=\{\(\) => onMeetingSection\("saved"\)\}/);
  assert.match(app, /liveSessionId=\{snapshot\.meeting\.live \? meetingStatus\?\.session_id \?\? null : null\}/);
});

test("A Meeting only highlights the Recent or Project row that opened it", () => {
  const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
  const navigation = readFileSync(new URL("../../src/components/layout/MeetingLibraryNavigation.svelte", import.meta.url), "utf8");
  assert.match(app, /selectedSurface = \$state<"recent" \| "project">\("recent"\)/);
  assert.match(app, /function openMeetingSession\(id: string, source: "recent" \| "project" = "recent"\)/);
  assert.match(app, /selectedSurface = source/);
  assert.match(navigation, /selectedIn\(session, "recent"\)/);
  assert.match(navigation, /selectedIn\(session, "project"\)/);
  assert.match(navigation, /liveSessionId === session\.session_id/);
  assert.match(navigation, /"recent:" \+ session\.session_id/);
  assert.match(navigation, /"project:" \+ session\.session_id/);
  assert.match(navigation, /"category:" \+ project\.project_id/);
  assert.match(navigation, /onOpenSession\(session\.session_id, "recent"\)/);
  assert.match(navigation, /onOpenSession\(session\.session_id, "project"\)/);
});

test("A meeting has only one open action menu across Recent and Projects", () => {
  const navigation = readFileSync(new URL("../../src/components/layout/MeetingLibraryNavigation.svelte", import.meta.url), "utf8");
  // One shared state, not independent per-row booleans for duplicate sessions.
  assert.equal((navigation.match(/let menu = \$state<string \| null>\(null\)/g) ?? []).length, 1);
  assert.match(navigation, /function toggleMenu\(key: string\)/);
  assert.match(navigation, /const next = menu === key \? null : key;/);
  assert.match(navigation, /clearMenus\(\);\s*menu = next;/);
  assert.match(navigation, /toggleMenu\("recent:" \+ session\.session_id\)/);
  assert.match(navigation, /toggleMenu\("project:" \+ session\.session_id\)/);
  assert.match(navigation, /toggleMenu\("category:" \+ project\.project_id\)/);
  assert.match(navigation, /menu === "recent:" \+ session\.session_id/);
  assert.match(navigation, /menu === "project:" \+ session\.session_id/);
  assert.doesNotMatch(navigation, /onclick=\{\(\) => menu = menu ===/);
});

test("Selecting a channel or starting another action closes the previous inline menu", () => {
  const navigation = readFileSync(new URL("../../src/components/layout/MeetingLibraryNavigation.svelte", import.meta.url), "utf8");
  assert.match(navigation, /function openSession\(sessionId: string, surface: "recent" \| "project"\)/);
  assert.match(navigation, /function openMove\(sessionId: string\)/);
  assert.match(navigation, /function requestDelete\(id: string\)/);
  assert.match(navigation, /function openEdit\(kind: Edit\["kind"\], id: string, previous = ""\)/);
  assert.match(navigation, /function toggleProject\(projectId: string\)/);
  assert.match(navigation, /<svelte:window onkeydown=\{handleEscape\}/);
  assert.match(navigation, /if \(event\.key === "Escape"/);
  for (const fn of ["openSession", "openMove", "requestDelete", "openEdit", "toggleProject"]) {
    const start = navigation.indexOf(`function ${fn}(`);
    assert.ok(start >= 0);
    assert.match(navigation.slice(start, start + 420), /clearMenus\(\)/, fn);
  }
});

test("Transcript display mode only changes presentation; failures retain original text", () => {
  const preferences = readFileSync(new URL("../../src/components/meeting/transcriptPreferences.ts", import.meta.url), "utf8");
  const bubble = readFileSync(new URL("../../src/components/meeting/MeetingTurnBubble.svelte", import.meta.url), "utf8");
  assert.match(preferences, /textMode: "both"/);
  assert.match(preferences, /raw\.textMode === "translation-only"/);
  assert.match(bubble, /preferences\.textMode === "translation-only"/);
  assert.match(bubble, /turn\.translation_state === "translated"/);
  assert.match(bubble, /\[original, translation\]/);
});
