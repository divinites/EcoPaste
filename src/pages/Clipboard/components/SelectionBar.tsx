import type { MenuProps } from "antd";
import type { FC } from "react";
import { useTranslation } from "react-i18next";
import CustomIconButton from "@/components/CustomIconButton";
import Dropdown, {
  type AppDropdownProps,
  type DropdownMenuItems,
} from "@/components/Dropdown";
import Tooltip from "@/components/Tooltip";
import type { ClipboardGroupRecord } from "@/types/clipboard";
import { cn } from "@/utils/cn";

const MOVE_TRIGGER: AppDropdownProps["trigger"] = ["click"];

export type SelectionBarAction =
  | "clear"
  | "copyMerged"
  | "delete"
  | "favorite"
  | "pasteMerged"
  | "pasteQueue"
  | "unfavorite";

interface SelectionBarProps {
  allFavorite: boolean;
  canDelete: boolean;
  count: number;
  groups: ClipboardGroupRecord[];
  onAction: (action: SelectionBarAction) => void;
  onMoveToGroup: (groupId: string) => void;
}

interface BarButtonProps {
  action: SelectionBarAction;
  danger?: boolean;
  disabled?: boolean;
  icon: string;
  label: string;
  onAction: (action: SelectionBarAction) => void;
}

/**
 * 多选浮动操作条：合并粘贴 / 复制、加入粘贴队列、收藏、移动分组、删除。
 */
const SelectionBar: FC<SelectionBarProps> = (props) => {
  const { allFavorite, canDelete, count, groups, onAction, onMoveToGroup } =
    props;
  const { t } = useTranslation("clipboard");
  const visibleGroups = groups.filter((group) => {
    return !group.isHidden;
  });

  const groupItems: DropdownMenuItems = visibleGroups.map((group) => {
    return { key: group.id, label: group.name };
  });

  const handleMoveClick: MenuProps["onClick"] = (info) => {
    onMoveToGroup(info.key);
  };

  return (
    <div
      className="absolute inset-x-3 bottom-3 z-20 flex items-center gap-1 rounded-2 bg-ant-container px-3 py-1 shadow-lg ring-1 ring-ant-border-secondary"
      role="toolbar"
    >
      <span className="min-w-0 flex-1 truncate text-ant-secondary text-sm">
        {t("selection.count", { count })}
      </span>

      <BarButton
        action="pasteMerged"
        icon="i-lucide:clipboard-paste"
        label={t("selection.pasteMerged")}
        onAction={onAction}
      />
      <BarButton
        action="copyMerged"
        icon="i-lucide:copy"
        label={t("selection.copyMerged")}
        onAction={onAction}
      />
      <BarButton
        action="pasteQueue"
        icon="i-lucide:list-ordered"
        label={t("selection.pasteQueue")}
        onAction={onAction}
      />
      <BarButton
        action={allFavorite ? "unfavorite" : "favorite"}
        icon={allFavorite ? "i-lucide:star-off" : "i-lucide:star"}
        label={t(allFavorite ? "selection.unfavorite" : "selection.favorite")}
        onAction={onAction}
      />

      {groupItems.length > 0 ? (
        <Dropdown
          menu={{ items: groupItems, onClick: handleMoveClick }}
          tooltip={t("selection.moveToGroup")}
          trigger={MOVE_TRIGGER}
        >
          <CustomIconButton
            icon={
              <i
                aria-hidden="true"
                className="i-lucide:folder-input text-base"
              />
            }
            size="small"
            type="text"
          />
        </Dropdown>
      ) : null}

      <BarButton
        action="delete"
        danger
        disabled={!canDelete}
        icon="i-lucide:trash-2"
        label={t("selection.delete")}
        onAction={onAction}
      />
      <BarButton
        action="clear"
        icon="i-lucide:x"
        label={t("selection.clear")}
        onAction={onAction}
      />
    </div>
  );
};

/**
 * 操作条上的单个图标按钮。
 */
const BarButton: FC<BarButtonProps> = (props) => {
  const { action, danger, disabled, icon, label, onAction } = props;

  const handleClick = () => {
    onAction(action);
  };

  return (
    <Tooltip title={label}>
      <CustomIconButton
        aria-label={label}
        danger={danger}
        disabled={disabled}
        icon={<i aria-hidden="true" className={cn(icon, "text-base")} />}
        onClick={handleClick}
        size="small"
        type="text"
      />
    </Tooltip>
  );
};

export default SelectionBar;
