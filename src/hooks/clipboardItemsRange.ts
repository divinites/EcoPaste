import type { ClipboardItem } from "@/types/clipboard";

/**
 * 后端分页大小。保持略大于一屏的原有取值，range cache 以此对齐请求边界。
 */
export const PAGE_SIZE = 30;
export const PRELOAD_ROWS = 30;
const CACHE_MAX_ROWS = 180;
const CACHE_KEEP_RADIUS = 90;

export interface ClipboardItemsRange {
  end: number;
  start: number;
}

/**
 * 把可视范围扩展并对齐到分页边界，超出已知总数时收口；全部在负区间时返回 null。
 */
export function normalizeFetchRange(
  rawStartIndex: number,
  rawEndIndex: number,
  total: number,
): ClipboardItemsRange | null {
  if (rawEndIndex < 0) return null;

  const maxKnownIndex = total > 0 ? total - 1 : Math.max(0, rawEndIndex);
  const clampedStart = Math.max(0, Math.min(rawStartIndex, maxKnownIndex));
  const clampedEnd = Math.max(
    clampedStart,
    Math.min(rawEndIndex, maxKnownIndex),
  );
  const start = Math.floor(clampedStart / PAGE_SIZE) * PAGE_SIZE;
  const end = Math.ceil((clampedEnd + 1) / PAGE_SIZE) * PAGE_SIZE - 1;

  return {
    end: total > 0 ? Math.min(end, total - 1) : end,
    start,
  };
}

/**
 * 判断范围内每一行是否都已缓存。
 */
export function isRangeLoaded(
  items: Map<number, ClipboardItem>,
  range: ClipboardItemsRange,
) {
  for (let index = range.start; index <= range.end; index += 1) {
    if (!items.has(index)) return false;
  }

  return true;
}

/**
 * 判断是否已有进行中的请求完整覆盖目标范围。
 */
export function hasCoveringLoadingRange(
  ranges: ClipboardItemsRange[],
  target: ClipboardItemsRange,
) {
  return ranges.some((range) => {
    return range.start <= target.start && range.end >= target.end;
  });
}

/**
 * 缓存超过上限时只保留视口附近与顶部连续置顶行，原地修改。
 */
export function trimCache(
  items: Map<number, ClipboardItem>,
  viewRange: ClipboardItemsRange,
  total: number,
) {
  if (items.size <= CACHE_MAX_ROWS) return;

  const center = Math.floor((viewRange.start + viewRange.end) / 2);
  const keepStart = Math.max(0, center - CACHE_KEEP_RADIUS);
  const keepEnd = Math.min(total - 1, center + CACHE_KEEP_RADIUS);
  const leadingPinnedEnd = getLeadingPinnedEnd(items);

  for (const [index] of items) {
    if (index <= leadingPinnedEnd) continue;
    if (index < keepStart || index > keepEnd) items.delete(index);
  }
}

function getLeadingPinnedEnd(items: Map<number, ClipboardItem>) {
  let index = 0;
  let lastPinnedIndex = -1;

  while (true) {
    const item = items.get(index);
    if (!item?.isPinned) return lastPinnedIndex;

    lastPinnedIndex = index;
    index += 1;
  }
}
