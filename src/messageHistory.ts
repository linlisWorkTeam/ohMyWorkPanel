/** Chat history windowing: hot page from server + progressive reveal on scroll-up. */

export const HOT_MESSAGE_LIMIT = 100;
export const INITIAL_VISIBLE_MESSAGES = 10;
export const VISIBLE_EXPAND_STEP = 20;
export const OLDER_PAGE_SIZE = 50;

export type MessageLike = { id: string; createdAt: number };

/** Merge a fresh hot page with already-loaded older messages (by id / createdAt). */
export function mergeHotWithOlder<T extends MessageLike>(
  olderLoaded: T[],
  hotPage: T[],
): T[] {
  const hotIds = new Set(hotPage.map((m) => m.id));
  const hotOldest = hotPage[0]?.createdAt;
  const kept = olderLoaded.filter(
    (m) => !hotIds.has(m.id) && (hotOldest === undefined || m.createdAt < hotOldest),
  );
  return [...kept, ...hotPage];
}

/** Prepend an older page; drop duplicates already present. */
export function prependOlderMessages<T extends MessageLike>(
  current: T[],
  olderPage: T[],
): T[] {
  const existing = new Set(current.map((m) => m.id));
  const unique = olderPage.filter((m) => !existing.has(m.id));
  return [...unique, ...current];
}

export function sliceVisibleMessages<T>(all: T[], visibleCount: number): T[] {
  if (visibleCount >= all.length) return all;
  return all.slice(-Math.max(0, visibleCount));
}

/** Near top of scroll container → should try expand / fetch older. */
export function shouldLoadOlderOnScroll(scrollTop: number, thresholdPx = 80): boolean {
  return scrollTop <= thresholdPx;
}

export function nextVisibleCount(current: number, loaded: number, step = VISIBLE_EXPAND_STEP): number {
  return Math.min(loaded, current + step);
}
