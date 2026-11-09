# TranslateIT Discord-Inspired Channel Layout Contract

**Status:** ADOPTED design/engineering acceptance contract, Local branch. **Browser, build and native tests NOT_EXECUTED.**
**Pinned audit:** `d1ecbaa12fb63b6b20559eda0979fc621e21dcde`.
**Product boundary:** Discord-inspired organization, not a clone of Discord branding, network behavior, server architecture or voice features. Existing TranslateIT runtime and archive safety remain authoritative.

## Navigation decisions

1. Sidebar: Home (Live Meeting, My Voice), Recent (shortcut list), Projects (folders with Meeting channels), Uncategorized (all sessions with `project_id === null`).
2. **No separate Saved Meetings page in primary navigation.** A session opens in the main channel reader. Do not remove export, HTML document preview, deletion confirmation or storage-integrity warnings.
3. A Meeting has one ID. Recent and Project are two views of that same ID, not duplicate channels. Highlight the clicked location only; maintain channel title and transcript identity across switches.
4. Recent must not be the sole path to older unprojected Meetings. Uncategorized must list every unprojected Meeting; don't silently cap that list at five.
5. Projects must collapse without hiding active session truth. Create/rename/delete/move must preserve keyboard focus and update sidebar location after success.

## Layout and geometry

- App shell owns viewport height; sidebar navigation scrolls independently; footer is pinned. Meeting content scrolls independently of Start/Stop dock.
- Use semantic tokens from `src/styles/tokens.css`; shared footer/dock height `--ti-bottom-dock-height`, shared border width/color, `box-sizing: border-box`.
- All adjacent cross-panel divider Y coordinates must match within **1 CSS pixel** where the elements coexist. Avoid nested duplicate separators.
- Header icon/title/status/action rows share a common vertical centerline. SVG icons render inside fixed-size boxes, not as inline glyphs that shift baselines.
- Sidebar row text/icon/menu must align on the same centerline. Expanded menus must not push channel rows down; popovers should anchor to the trigger and flip or scroll within the viewport.
- Desktop widths to measure: 1920, 1600, 1440, 1280, 1024, 800; short-height windows and scaling 100/125/150/200%. At narrower widths, preserve action reachability and readable labels rather than forcing fixed widths.

## Channel and conversation

- One `MeetingTurnBubble` owns Live, Saved and Last presentation. You is right; incoming Meeting Sound is left. Meeting Sound is an audio lane, **not** a verified individual speaker.
- Bubble max-width and responsive wrapping must be consistent; short phrases should not produce unnecessarily wide empty rectangles. Preserve original/translation hierarchy and fallback/error states.
- Channel header is concise. Do not repeat large titles in every toolbar and content section. A larger beginning-of-channel title is acceptable only when it adds useful context.
- Toolbar owners: Live has audio source and session state; channel reading has Search/Display; archive-specific Copy/Export/Refresh/Delete belong to one contextual action owner. Do not duplicate destructive actions across list and channel.
- Stop/recovery controls must remain accessible during live or cleanup-incomplete states, even when navigating to a saved channel. Never fabricate a Stop button on a static archived channel.
- Search is scoped to a single channel and resets on channel switch. Archive read responses from an old session must not replace newer channel content.
- 240-entry preview limit is not full archive completeness; export and warning semantics must be truthful.

## Context menu contract

- One menu open at a time across Recent and Project appearances of the same Meeting.
- Anchor to the More trigger using measured coordinates and constrain to the sidebar viewport. Flip upward when necessary; do not cause layout reflow or horizontal clipping.
- Minimum 36px menu action height; icons 14–16px and text aligned in a single grid. Separate destructive Delete from ordinary actions, with a confirmation step.
- Escape, outside click and action completion close menus. Restore focus to the trigger, or a stable fallback if it was removed. Focus remains visible.
- Menu placement must be remeasured after scrolling, resizing or content changes; never assume the popup remains visible based on stale coordinates.

## QA and proof

Check: no projects, no recent sessions, many sessions, all unprojected sessions, renamed/long names, identical Meeting in Recent/Project, active live + archived navigation, deleted selected session, project deletion, keyboard-only menu, popup near bottom edge, overflow and scaling, translation failure, empty transcript, truncated preview, stale async response, and setup-needed audio state.

Source commits are `SOURCE_AUTHORED`, not `VISUAL_VERIFIED`. HTML previews are synthetic and do not demonstrate Windows/Tauri behavior. Record browser screenshots/measurements and local build results before declaring alignment PASS. Preserve CI pause and Local-only development policy.

## Current source gaps (not accepted as complete)

- Saved Meetings route and View All still exist in source; require explicit navigation migration to Uncategorized/Projects.
- Inline sidebar context menus currently participate in document flow; need anchored overlay implementation with keyboard/viewport QA.
- Channel toolbar action ownership and archive actions are not yet fully consolidated.
- Native/browser rendered alignment, build and end-to-end interactions remain unverified.
