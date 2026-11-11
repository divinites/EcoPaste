import { useMount } from "ahooks";
import type { MenuProps } from "antd";
import type { FC } from "react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  type ClipboardPauseState,
  getClipboardPauseState,
  setClipboardPause,
  setClipboardSkipNext,
} from "@/commands";
import CustomIconButton from "@/components/CustomIconButton";
import Dropdown, {
  type AppDropdownProps,
  type DropdownMenuItems,
} from "@/components/Dropdown";
import Tooltip from "@/components/Tooltip";
import { TAURI_EVENT } from "@/constants/events";
import { useTauriListen } from "@/hooks/useTauriListen";

const PAUSE_TRIGGER: AppDropdownProps["trigger"] = ["click"];

const PAUSE_MINUTES: Record<string, number> = {
  pause5: 5,
  pause30: 30,
  pause60: 60,
  pauseForever: 0,
};

/**
 * 顶栏的监听开关：未暂停时提供定时暂停与「忽略下一次复制」，暂停时一键恢复。
 * 状态以 Rust 为准，托盘操作通过 `clipboard://pause-changed` 同步过来。
 */
const ListeningToggle: FC = () => {
  const { t } = useTranslation("clipboard");
  const [state, setState] = useState<ClipboardPauseState>({
    paused: false,
    skipNext: false,
    until: null,
  });

  useMount(async () => {
    try {
      setState(await getClipboardPauseState());
    } catch {
      // 命令包装层已统一提示；保持默认未暂停展示。
    }
  });

  const handlePauseChanged = (event: { payload: ClipboardPauseState }) => {
    setState(event.payload);
  };

  useTauriListen<ClipboardPauseState>(
    TAURI_EVENT.CLIPBOARD_PAUSE_CHANGED,
    handlePauseChanged,
  );

  const resume = async () => {
    setState(await setClipboardPause(null));
  };

  const handleMenuClick: MenuProps["onClick"] = async (info) => {
    if (info.key === "skipNext") {
      setState(await setClipboardSkipNext(!state.skipNext));
      return;
    }

    const minutes = PAUSE_MINUTES[info.key];
    if (minutes === void 0) return;

    setState(await setClipboardPause(minutes));
  };

  if (state.paused) {
    const title = state.until
      ? t("header.listening.pausedUntil", {
          time: new Date(state.until).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
          }),
        })
      : t("header.listening.pausedForever");

    return (
      <Tooltip title={title}>
        <CustomIconButton
          danger
          icon={
            <i aria-hidden="true" className="i-lucide:circle-play text-base" />
          }
          onClick={resume}
          size="small"
          type="text"
        />
      </Tooltip>
    );
  }

  const menuItems: DropdownMenuItems = [
    {
      icon: "i-lucide:timer",
      key: "pause5",
      label: t("header.listening.pause5"),
    },
    {
      icon: "i-lucide:timer",
      key: "pause30",
      label: t("header.listening.pause30"),
    },
    {
      icon: "i-lucide:timer",
      key: "pause60",
      label: t("header.listening.pause60"),
    },
    {
      icon: "i-lucide:circle-pause",
      key: "pauseForever",
      label: t("header.listening.pauseForever"),
    },
    { type: "divider" },
    {
      icon: state.skipNext ? "i-lucide:x" : "i-lucide:skip-forward",
      key: "skipNext",
      label: t(
        state.skipNext
          ? "header.listening.cancelSkipNext"
          : "header.listening.skipNext",
      ),
    },
  ];

  return (
    <Dropdown
      menu={{ items: menuItems, onClick: handleMenuClick }}
      tooltip={t(
        state.skipNext
          ? "header.listening.skipNextActive"
          : "header.listening.pause",
      )}
      trigger={PAUSE_TRIGGER}
    >
      <CustomIconButton
        icon={
          <i
            aria-hidden="true"
            className={
              state.skipNext
                ? "i-lucide:skip-forward text-base"
                : "i-lucide:pause text-base"
            }
          />
        }
        size="small"
        type={state.skipNext ? "primary" : "text"}
      />
    </Dropdown>
  );
};

export default ListeningToggle;
