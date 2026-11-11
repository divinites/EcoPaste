import { describe, expect, it } from "vitest";
import type { ClipboardItem } from "@/types/clipboard";
import {
  EMPTY_SELECTION,
  selectionIds,
  selectRange,
  toggleSelection,
} from "./clipboardSelection";

const items = ["a", "b", "c", "d", "e"].map((id) => {
  return { id } as ClipboardItem;
});

const getItem = (index: number) => {
  return items[index] ?? null;
};

describe("toggleSelection", () => {
  it("adds in click order and removes on second toggle", () => {
    let selection = toggleSelection(EMPTY_SELECTION, items[2]);
    selection = toggleSelection(selection, items[0]);

    expect(selectionIds(selection)).toEqual(["c", "a"]);
    expect(selection.anchorId).toBe("a");

    selection = toggleSelection(selection, items[2]);
    expect(selectionIds(selection)).toEqual(["a"]);
  });
});

describe("selectRange", () => {
  it("selects forward and backward ranges without duplicates", () => {
    const start = toggleSelection(EMPTY_SELECTION, items[1]);

    expect(selectionIds(selectRange(start, items[3], 1, 3, getItem))).toEqual([
      "b",
      "c",
      "d",
    ]);
    expect(selectionIds(selectRange(start, items[0], 1, 0, getItem))).toEqual([
      "b",
      "a",
    ]);
  });

  it("falls back to a toggle without an anchor", () => {
    expect(
      selectionIds(selectRange(EMPTY_SELECTION, items[4], null, 4, getItem)),
    ).toEqual(["e"]);
  });

  it("skips rows that are not loaded", () => {
    const sparse = (index: number) => {
      return index === 2 ? null : getItem(index);
    };
    const start = toggleSelection(EMPTY_SELECTION, items[1]);

    expect(selectionIds(selectRange(start, items[3], 1, 3, sparse))).toEqual([
      "b",
      "d",
    ]);
  });
});
