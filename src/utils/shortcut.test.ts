import { describe, expect, it, vi } from "vitest";

vi.mock("@/utils/is", () => {
  return { isMac: false, isWin: true };
});

const {
  buildShortcutFromEvent,
  formatShortcutDisplay,
  isRecordableShortcut,
  normalizeShortcutValue,
} = await import("./shortcut");

const keyEvent = (init: Partial<KeyboardEvent>) => {
  return {
    altKey: false,
    code: "",
    ctrlKey: false,
    key: "",
    metaKey: false,
    shiftKey: false,
    ...init,
  } as KeyboardEvent;
};

describe("normalizeShortcutValue", () => {
  it("ignores case and whitespace", () => {
    expect(normalizeShortcutValue(" Command + Shift+V ")).toBe(
      "command+shift+v",
    );
  });
});

describe("formatShortcutDisplay", () => {
  it("uses Windows labels", () => {
    expect(formatShortcutDisplay("CommandOrControl+Shift+KeyV")).toBe(
      "Ctrl + Shift + V",
    );
  });
});

describe("buildShortcutFromEvent", () => {
  it("orders modifiers consistently", () => {
    const event = keyEvent({
      code: "KeyV",
      ctrlKey: true,
      key: "v",
      shiftKey: true,
    });

    expect(buildShortcutFromEvent(event)).toBe("Control+Shift+V");
  });

  it("rejects plain keys without modifiers", () => {
    expect(buildShortcutFromEvent(keyEvent({ code: "KeyV", key: "v" }))).toBe(
      null,
    );
  });

  it("accepts function keys alone", () => {
    expect(buildShortcutFromEvent(keyEvent({ code: "F5", key: "F5" }))).toBe(
      "F5",
    );
  });

  it("ignores lone modifier presses", () => {
    const event = keyEvent({ code: "ShiftLeft", key: "Shift", shiftKey: true });

    expect(buildShortcutFromEvent(event)).toBe(null);
  });
});

describe("isRecordableShortcut", () => {
  it("requires a modifier unless the key is a function key", () => {
    expect(isRecordableShortcut("")).toBe(false);
    expect(isRecordableShortcut("V")).toBe(false);
    expect(isRecordableShortcut("F12")).toBe(true);
    expect(isRecordableShortcut("Alt+V")).toBe(true);
  });
});
