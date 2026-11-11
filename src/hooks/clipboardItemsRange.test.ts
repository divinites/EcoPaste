import { describe, expect, it } from "vitest";
import type { ClipboardItem } from "@/types/clipboard";
import {
  hasCoveringLoadingRange,
  isRangeLoaded,
  normalizeFetchRange,
  PAGE_SIZE,
  trimCache,
} from "./clipboardItemsRange";

const makeItem = (id: string, isPinned = false) => {
  return { id, isPinned } as ClipboardItem;
};

const makeItems = (count: number, pinned = 0) => {
  const items = new Map<number, ClipboardItem>();

  for (let index = 0; index < count; index += 1) {
    items.set(index, makeItem(`item-${index}`, index < pinned));
  }

  return items;
};

describe("normalizeFetchRange", () => {
  it("returns null when the whole range is negative", () => {
    expect(normalizeFetchRange(-10, -1, 100)).toBeNull();
  });

  it("aligns to page boundaries", () => {
    expect(normalizeFetchRange(5, 40, 1000)).toEqual({
      end: PAGE_SIZE * 2 - 1,
      start: 0,
    });
  });

  it("clamps to the known total", () => {
    expect(normalizeFetchRange(0, 500, 45)).toEqual({ end: 44, start: 0 });
  });

  it("clamps negative starts to zero", () => {
    expect(normalizeFetchRange(-30, 10, 100)).toEqual({
      end: PAGE_SIZE - 1,
      start: 0,
    });
  });

  it("keeps the requested window when the total is still unknown", () => {
    expect(normalizeFetchRange(0, 10, 0)).toEqual({
      end: PAGE_SIZE - 1,
      start: 0,
    });
  });
});

describe("isRangeLoaded", () => {
  it("detects gaps", () => {
    const items = makeItems(10);
    items.delete(4);

    expect(isRangeLoaded(items, { end: 3, start: 0 })).toBe(true);
    expect(isRangeLoaded(items, { end: 5, start: 0 })).toBe(false);
  });
});

describe("hasCoveringLoadingRange", () => {
  it("requires full coverage", () => {
    const ranges = [{ end: 59, start: 0 }];

    expect(hasCoveringLoadingRange(ranges, { end: 29, start: 0 })).toBe(true);
    expect(hasCoveringLoadingRange(ranges, { end: 89, start: 30 })).toBe(false);
  });
});

describe("trimCache", () => {
  it("is a no-op below the cache limit", () => {
    const items = makeItems(50);
    trimCache(items, { end: 10, start: 0 }, 50);

    expect(items.size).toBe(50);
  });

  it("keeps rows around the viewport and leading pinned rows", () => {
    const items = makeItems(400, 3);
    trimCache(items, { end: 310, start: 290 }, 400);

    expect(items.has(0)).toBe(true);
    expect(items.has(2)).toBe(true);
    expect(items.has(3)).toBe(false);
    expect(items.has(300)).toBe(true);
    expect(items.has(150)).toBe(false);
    expect(items.size).toBeLessThanOrEqual(3 + 181);
  });
});
