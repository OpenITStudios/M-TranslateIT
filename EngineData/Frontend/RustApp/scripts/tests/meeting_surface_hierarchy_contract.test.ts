import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const meeting = readFileSync(new URL("../../src/pages/Meeting.svelte", import.meta.url), "utf8");
const live = readFileSync(new URL("../../src/components/meeting/MeetingActivity.svelte", import.meta.url), "utf8");
const review = readFileSync(new URL("../../src/components/meeting/MeetingSessionReview.svelte", import.meta.url), "utf8");
const header = readFileSync(new URL("../../src/components/layout/AppHeader.svelte", import.meta.url), "utf8");

test("Meeting Stop stays actionable during pending audio source changes", () => {
  // In-flight source selection is not permission to hide the native Stop path.
  assert.match(meeting, /const actionEnabled = \$derived\(!resumeBusy && !meeting\.busy && !actionBusy/);
  assert.match(meeting, /primaryControl\.action === "stop"\s*\? primaryControl\.enabled/);
  assert.match(meeting, /: !sourceChanging && \(primaryControl\.enabled/);
  assert.doesNotMatch(meeting, /!resumeBusy && !sourceChanging && !meeting\.busy/);
  assert.match(meeting, /meeting\.lifecycle === "cleanup_incomplete" \? primaryControl\.label : "Stop Meeting"/);
});

test("Meeting main dock is valid markup and has no stray text after the closing footer", () => {
  assert.equal((meeting.match(/<footer\b/g) ?? []).length, 1);
  assert.equal((meeting.match(/<\/footer>/g) ?? []).length, 1);
  assert.match(meeting, /<\/footer>\s*\{\/if\}\s*<\/section>/);
  assert.doesNotMatch(meeting, /<\/footer>ter>/);
});

test("Meeting has a single primary button that can Start, Stop, or guide recovery", () => {
  assert.match(meeting, /Indonesian speech → English voice/);
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
test("Meeting keeps a single accessible Stop or Retry Stop in a persistent dock below the transcript", () => {
  assert.match(meeting, /meeting\.lifecycle === "cleanup_incomplete" \? primaryControl\.label : "Stop Meeting"/);
  assert.match(meeting, /meeting\.lifecycle === "cleanup_incomplete" \? "warning"/);
  const action = meeting.indexOf('aria-label="Meeting main action"');
  const sourceSelector = meeting.indexOf('id="meeting-audio-source"');
  const transcript = meeting.indexOf("<MeetingActivity status={meetingStatus}");
  assert.ok(sourceSelector > 0 && sourceSelector < transcript && transcript < action,
    "Audio Sent belongs to the Live toolbar, transcript stays central, and the dock comes last");
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
  assert.match(bubble, /lang=\{turn\.translation_state === "translated" \? turn\.target_language : undefined\}/);
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
  assert.match(review, /Saved Transcripts below keeps recorded sessions until you delete them/);
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
  assert.match(live, /Meeting audio is combined; individual speakers are not identified/);
  assert.match(review, /Meeting audio combines incoming sound; individual speakers are not identified/);
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
  assert.match(app, /meetingSection = \$state<"live" \| "last" \| "saved">/);
  assert.match(sidebar, /onMeetingSection\("live"\)/);
  assert.match(sidebar, /Live Meeting/);
  assert.match(sidebar, /Last Meeting/);
  assert.match(sidebar, /Saved Transcripts/);
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

test("Meeting puts Audio Sent and Captions in the header while only Stop remains below", () => {
  assert.match(meeting, /aria-label="Live Meeting toolbar"/);
  assert.match(meeting, /id="meeting-audio-source"/);
  assert.match(meeting, /aria-label="Open captions controls in Quick Access"/);
  assert.match(meeting, /ti-call-stop/);
  assert.match(meeting, /onclick=\{handlePrimary\}/);
  assert.doesNotMatch(meeting, /onToggleMute|MicOff|Deafen|Undeafen/);
  const source = meeting.indexOf('id="meeting-audio-source"');
  const captions = meeting.indexOf('aria-label="Open captions controls in Quick Access"');
  const transcript = meeting.indexOf("<MeetingActivity status={meetingStatus}");
  const dock = meeting.indexOf('aria-label="Meeting main action"');
  assert.ok(source > 0 && captions > source && transcript > captions && dock > transcript);
  assert.equal((meeting.match(/onclick=\{handlePrimary\}/g) ?? []).length, 1);
  const footer = meeting.slice(dock);
  assert.doesNotMatch(footer, /meeting-audio-source|Captions|onToggleMute/);
  assert.match(meeting, /does not leave the Discord or Zoom call/);
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
  assert.match(settings, /not editable settings/);
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
