import { describe, expect, it } from "vitest";
import type { ClipboardItem } from "@/types/clipboard";
import {
  getNavigationDirection,
  getNextKeyboardIndex,
  getSelectedIdAfterDelete,
  shouldRefreshCurrentGroup,
} from "./listHelpers";

const items = ["a", "b", "c"].map((id) => {
  return { id } as ClipboardItem;
});

const getItem = (index: number) => {
  return items[index] ?? null;
};

const getItemIndexById = (id: string) => {
  const index = items.findIndex((item) => {
    return item.id === id;
  });

  return index === -1 ? null : index;
};

describe("getNavigationDirection", () => {
  it("maps arrows by orientation", () => {
    expect(getNavigationDirection("ArrowDown", false)).toBe(1);
    expect(getNavigationDirection("ArrowLeft", false)).toBe(0);
    expect(getNavigationDirection("ArrowLeft", true)).toBe(-1);
    expect(getNavigationDirection("ArrowUp", true)).toBe(0);
  });
});

describe("getNextKeyboardIndex", () => {
  it("clamps to the list bounds", () => {
    expect(getNextKeyboardIndex(getItemIndexById, 0, "a", 3, -1)).toBe(0);
    expect(getNextKeyboardIndex(getItemIndexById, 0, "c", 3, 1)).toBe(2);
    expect(getNextKeyboardIndex(getItemIndexById, 1, null, 3, 1)).toBe(2);
  });
});

describe("getSelectedIdAfterDelete", () => {
  it("moves to the next item when the active item is deleted", () => {
    expect(
      getSelectedIdAfterDelete(getItem, getItemIndexById, 0, "b", "b"),
    ).toBe("c");
  });

  it("falls back to the previous item at the end", () => {
    expect(
      getSelectedIdAfterDelete(getItem, getItemIndexById, 0, "c", "c"),
    ).toBe("b");
  });

  it("keeps an explicit selection when another item is deleted", () => {
    expect(
      getSelectedIdAfterDelete(getItem, getItemIndexById, 0, "a", "c"),
    ).toBe("a");
  });
});

describe("shouldRefreshCurrentGroup", () => {
  it("skips custom groups and favorites", () => {
    expect(shouldRefreshCurrentGroup("all", null, "g1", "text")).toBe(false);
    expect(shouldRefreshCurrentGroup("favorite", null, null, "text")).toBe(
      false,
    );
  });

  it("matches the category filter", () => {
    expect(shouldRefreshCurrentGroup("all", null, null, "image")).toBe(true);
    expect(shouldRefreshCurrentGroup("all", "text", null, "text")).toBe(true);
    expect(shouldRefreshCurrentGroup("all", "text", null, "image")).toBe(false);
  });
});
