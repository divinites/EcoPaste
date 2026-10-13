import type { ClipboardWindowLayout } from "@/types/settings";

/**
 * 与 Rust `settings::ClipboardWindowLayout` 一一对应。
 */
export const CLIPBOARD_WINDOW_LAYOUT = {
  DOCK: "dock",
  VERTICAL: "vertical",
} as const satisfies Record<string, ClipboardWindowLayout>;

export const CLIPBOARD_WINDOW_LAYOUT_OPTIONS = [
  { value: CLIPBOARD_WINDOW_LAYOUT.DOCK },
  { value: CLIPBOARD_WINDOW_LAYOUT.VERTICAL },
] satisfies Array<{ value: ClipboardWindowLayout }>;

/**
 * 与 Rust `settings::DOCK_HEIGHT_PERCENT_RANGE` 保持一致。
 */
export const DOCK_HEIGHT_PERCENT_MIN = 25;
export const DOCK_HEIGHT_PERCENT_MAX = 60;
