import type { FC, PointerEvent } from "react";
import { useRef } from "react";
import { closeClipboardPreview, resizeClipboardDock } from "@/commands";

interface DragState {
  height: number;
  pending: boolean;
  pointerId: number;
  queued: boolean;
  startHeight: number;
  startScreenY: number;
}

/**
 * 停靠面板上边缘的拖拽手柄：拖动时按逻辑像素实时调高（每帧至多一次 IPC），
 * 松手后由 Rust 换算为整数百分比写入 `dockHeightPercent`。几何计算与钳制都在 Rust。
 */
const DockResizeHandle: FC = () => {
  const dragRef = useRef<DragState | null>(null);

  /**
   * 合并同一帧内的多次移动；上一帧的调整尚未返回时只记下最新高度。
   */
  const flush = async () => {
    const drag = dragRef.current;
    if (!drag || drag.pending) return;

    drag.pending = true;
    drag.queued = false;

    try {
      await resizeClipboardDock(drag.height, false);
    } finally {
      drag.pending = false;
    }

    if (dragRef.current?.queued) scheduleFlush();
  };

  const scheduleFlush = () => {
    const drag = dragRef.current;
    if (!drag || drag.queued) return;

    drag.queued = true;
    requestAnimationFrame(() => {
      void flush();
    });
  };

  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;

    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = {
      height: window.innerHeight,
      pending: false,
      pointerId: event.pointerId,
      queued: false,
      startHeight: window.innerHeight,
      startScreenY: event.screenY,
    };
    void closeClipboardPreview();
  };

  const handlePointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;

    drag.height = drag.startHeight + (drag.startScreenY - event.screenY);
    scheduleFlush();
  };

  const finishDrag = async (event: PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;

    dragRef.current = null;
    event.currentTarget.releasePointerCapture(event.pointerId);

    if (drag.height === drag.startHeight) return;

    await resizeClipboardDock(drag.height, true);
  };

  return (
    <div
      aria-hidden="true"
      className="absolute inset-x-0 top-0 z-30 h-1.5 cursor-ns-resize"
      onPointerCancel={finishDrag}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={finishDrag}
    />
  );
};

export default DockResizeHandle;
