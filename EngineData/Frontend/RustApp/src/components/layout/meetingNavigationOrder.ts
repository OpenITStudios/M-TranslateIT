/**
 * Meeting archives arrive in canonical newest-first order from Rust.
 * Pin changes only presentation order, never the archive identity or timestamp.
 * Both functions preserve the relative archive order within each group.
 */
export function pinnedFirst<T extends { pinned: boolean }>(sessions: readonly T[]): T[] {
  const pinned: T[] = [];
  const unpinned: T[] = [];
  for (const session of sessions) {
    if (session.pinned) pinned.push(session);
    else unpinned.push(session);
  }
  return [...pinned, ...unpinned];
}

/**
 * Every pinned Meeting stays visible, including old Project members.
 * The limit applies only to unpinned recent sessions.
 * No mutation of the authoritative array.
 */
export function recentMeetings<T extends { pinned: boolean }>(
  sessions: readonly T[],
  regularLimit = 5,
): T[] {
  if (!Number.isSafeInteger(regularLimit) || regularLimit < 0) {
    throw new RangeError("Recent Meeting limit must be a nonnegative integer.");
  }
  const pinned: T[] = [];
  const regular: T[] = [];
  for (const session of sessions) {
    if (session.pinned) pinned.push(session);
    else if (regular.length < regularLimit) regular.push(session);
  }
  return [...pinned, ...regular];
}
