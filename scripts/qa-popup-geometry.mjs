// Source-mirrored deterministic geometry QA for the viewport-fixed Meeting menus.
// Run manually: node scripts/qa-popup-geometry.mjs
// This is NOT a DOM/browser, Svelte build, or native Windows test.
// Keep this formula in sync with MeetingChannel.svelte and MeetingLibraryNavigation.svelte.
import assert from "node:assert/strict";

function place(rect, widthRequested, height, viewportW, viewportH) {
  const width = Math.min(widthRequested, Math.max(160, viewportW - 16));
  const usableHeight = Math.max(0, viewportH - 16);
  const shownHeight = Math.min(height, usableHeight);
  const above = rect.top - 8;
  const below = viewportH - rect.bottom - 8;
  const preferAbove = below < shownHeight && above > below;
  const proposedTop = preferAbove ? rect.top - shownHeight - 6 : rect.bottom + 6;
  const top = Math.max(8, Math.min(viewportH - shownHeight - 8, proposedTop));
  const left = Math.max(8, Math.min(viewportW - width - 8, rect.right - width));
  return { top, left, width, shownHeight };
}

let cases = 0;
for (const width of [320, 480, 800, 1024, 1280, 1920]) {
  for (const height of [240, 320, 480, 768, 1080]) {
    for (const y of [8, 30, 100, height / 2, height - 45]) {
      for (const x of [0, 8, width / 2, width - 40]) {
        for (const menuHeight of [80, 180, 420]) {
          const result = place({ top: y, bottom: y + 32, right: x + 32 }, 220, menuHeight, width, height);
          assert.ok(result.left >= 8, "left viewport boundary");
          assert.ok(result.left + result.width <= width - 8, "right viewport boundary");
          assert.ok(result.top >= 8, "top viewport boundary");
          assert.ok(result.top + result.shownHeight <= height - 8, "bottom viewport boundary");
          cases++;
        }
      }
    }
  }
}
console.log(`PASS ${cases} deterministic popup geometry cases (not browser QA)`);
