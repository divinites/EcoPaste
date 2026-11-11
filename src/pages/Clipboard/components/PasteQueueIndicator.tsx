import { useMount } from "ahooks";
import { Tag } from "antd";
import type { FC } from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  getClipboardPasteQueue,
  type PasteQueueState,
  setClipboardPasteQueue,
} from "@/commands";
import Tooltip from "@/components/Tooltip";
import { TAURI_EVENT } from "@/constants/events";
import { useTauriListen } from "@/hooks/useTauriListen";
import { formatShortcutDisplay } from "@/utils/shortcut";

/**
 * 粘贴队列进度徽标：队列非空时显示「剩余 / 总数」，点击关闭清空队列。
 */
const PasteQueueIndicator: FC = () => {
  const { t } = useTranslation("clipboard");
  const [state, setState] = useState<PasteQueueState | null>(null);

  useMount(async () => {
    try {
      setState(await getClipboardPasteQueue());
    } catch {
      // 命令包装层已统一提示。
    }
  });

  const handleQueueChanged = (event: { payload: PasteQueueState }) => {
    setState(event.payload);
  };

  useTauriListen<PasteQueueState>(
    TAURI_EVENT.CLIPBOARD_PASTE_QUEUE,
    handleQueueChanged,
  );

  const clearQueue = async () => {
    setState(await setClipboardPasteQueue([]));
  };

  if (!state || state.remaining === 0) return null;

  return (
    <Tooltip
      title={t("pasteQueue.tooltip", {
        shortcut: formatShortcutDisplay(state.shortcut),
      })}
    >
      <Tag className="m-0" closable color="processing" onClose={clearQueue}>
        {t("pasteQueue.label", {
          remaining: state.remaining,
          total: state.total,
        })}
      </Tag>
    </Tooltip>
  );
};

export default PasteQueueIndicator;
