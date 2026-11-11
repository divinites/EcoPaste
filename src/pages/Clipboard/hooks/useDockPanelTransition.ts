import { useState } from "react";
import { TAURI_EVENT } from "@/constants/events";
import { WINDOW_LABEL } from "@/constants/windows";
import { useTauriListen } from "@/hooks/useTauriListen";
import { cn } from "@/utils/cn";
import type { WindowVisibilityPayload } from "./previewController";

/**
 * 停靠面板滑入 / 滑出：Rust 隐藏停靠面板时先发 `visible: false`，等滑出动画
 * （`DOCK_HIDE_ANIMATION`，与这里的 120ms 保持一致）结束后才真正隐藏窗口。
 *
 * 粘贴路径的立即隐藏（`instant`）不滑出：窗口已隐藏时 WebView 可能不重绘，
 * 改变位移会让下次呼出先闪一帧旧画面，所以保持原位，下次呼出直接显示。
 */
export const useDockPanelTransition = (enabled: boolean) => {
  const [shown, setShown] = useState(true);

  const handleVisibility = (event: { payload: WindowVisibilityPayload }) => {
    const { instant, label, visible } = event.payload;
    if (label !== WINDOW_LABEL.CLIPBOARD) return;

    if (visible) {
      setShown(true);
      return;
    }

    if (instant) return;

    setShown(false);
  };

  useTauriListen<WindowVisibilityPayload>(
    TAURI_EVENT.WINDOW_VISIBILITY,
    handleVisibility,
  );

  if (!enabled) return "";

  return cn("transition-transform motion-reduce:transition-none", {
    "translate-y-0 duration-180 ease-out": shown,
    "translate-y-full duration-120 ease-in": !shown,
  });
};
