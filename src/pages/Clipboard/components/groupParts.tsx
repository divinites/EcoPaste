import type { TFunction } from "i18next";
import type { Dispatch, FC, RefObject, SetStateAction } from "react";
import ClipboardGroupIcon from "@/components/ClipboardGroupIcon";
import Dropdown, { type DropdownMenuItems } from "@/components/Dropdown";
import { clipboardViewState } from "@/stores/clipboardView";
import type {
  ClipboardCategory,
  ClipboardGroupIcon as ClipboardGroupIconValue,
  ClipboardGroupRecord,
  ClipboardRange,
} from "@/types/clipboard";

export type MoreMenuAction = "manageGroups" | "newGroup";
export type GroupMenuAction = "delete" | "edit" | "hide";
export type MoreMenuGroupKey = `group:${string}`;

export interface RangeGroupOption {
  labelKey: string;
  value: ClipboardRange;
  icon: ClipboardGroupIconValue;
}

export interface CategoryGroupOption {
  labelKey: string;
  value: ClipboardCategory;
  icon: ClipboardGroupIconValue;
}

export interface OverflowGroupMenuLabelProps {
  menuItems: DropdownMenuItems;
  onContext: (record: ClipboardGroupRecord) => void;
  onMenuClick: (info: { key: string }) => void;
  record: ClipboardGroupRecord;
}

export interface GroupSeparatorProps {
  separatorRef?: RefObject<HTMLSpanElement | null>;
}

export const RANGE_GROUP_OPTIONS: RangeGroupOption[] = [
  { icon: "i-lets-icons:widget", labelKey: "groups.all", value: "all" },
  {
    icon: "i-lets-icons:star",
    labelKey: "groups.favorite",
    value: "favorite",
  },
];

export const CATEGORY_GROUP_OPTIONS: CategoryGroupOption[] = [
  { icon: "i-lets-icons:file-dock", labelKey: "groups.text", value: "text" },
  { icon: "i-lets-icons:img-box", labelKey: "groups.image", value: "image" },
  {
    icon: "i-lets-icons:folder-file-alt",
    labelKey: "groups.files",
    value: "files",
  },
];

export const GROUP_MENU_ACTION = {
  DELETE: "delete",
  EDIT: "edit",
  HIDE: "hide",
} as const satisfies Record<string, GroupMenuAction>;

export const MORE_MENU_ACTION = {
  MANAGE_GROUPS: "manageGroups",
  NEW_GROUP: "newGroup",
} as const satisfies Record<string, MoreMenuAction>;

export const GROUP_BUTTON_WIDTH = 24;
export const GROUP_BUTTON_GAP = 4;
export const GROUP_SEPARATOR_MARGIN = 4;

/**
 * 分隔范围、分类、自定义分组三段。
 */
export const GroupSeparator: FC<GroupSeparatorProps> = (props) => {
  const { separatorRef } = props;

  return (
    <span
      aria-hidden
      className="mx-1 h-4 w-px shrink-0 bg-ant-split"
      ref={separatorRef}
    />
  );
};

/**
 * 溢出菜单里的分组行：左键选择分组，右键打开同一套分组管理菜单。
 */
export const OverflowGroupMenuLabel: FC<OverflowGroupMenuLabelProps> = (
  props,
) => {
  const { menuItems, onContext, onMenuClick, record } = props;

  const handleContextMenu = () => {
    onContext(record);
  };

  return (
    <Dropdown
      menu={{
        items: menuItems,
        onClick: onMenuClick,
      }}
      trigger={["contextMenu"]}
    >
      <span
        className="flex min-w-28 items-center gap-2"
        onContextMenu={handleContextMenu}
        role="menuitem"
        tabIndex={-1}
      >
        <ClipboardGroupIcon icon={record.icon} inheritColor />
        <span>{record.name}</span>
      </span>
    </Dropdown>
  );
};

/**
 * 下一帧提交可见自定义分组数量；分组数据更新后等待 DOM 渲染完成再测量。
 */
export function scheduleVisibleCustomGroupCountUpdate(
  toolbarRef: RefObject<HTMLDivElement | null>,
  customAnchorRef: RefObject<HTMLSpanElement | null>,
  setVisibleCustomGroupCount: Dispatch<SetStateAction<number>>,
  groupCount: number,
) {
  requestAnimationFrame(() => {
    commitVisibleCustomGroupCount(
      toolbarRef.current,
      customAnchorRef.current,
      setVisibleCustomGroupCount,
      groupCount,
    );
  });
}

/**
 * 根据自定义分组栏可用宽度写入可见分组数量。
 */
export function commitVisibleCustomGroupCount(
  toolbar: HTMLDivElement | null,
  customAnchor: HTMLSpanElement | null,
  setVisibleCustomGroupCount: Dispatch<SetStateAction<number>>,
  groupCount: number,
) {
  const rawCapacity =
    toolbar && customAnchor
      ? computeCustomGroupCapacity(toolbar, customAnchor)
      : groupCount;
  const visibleCount = Math.min(groupCount, rawCapacity);

  setVisibleCustomGroupCount((current) => {
    if (current === visibleCount) return current;

    return visibleCount;
  });
}

/**
 * 按整条分组栏剩余宽度计算自定义分组可显示数量。
 */
export function computeCustomGroupCapacity(
  toolbar: HTMLDivElement,
  customAnchor: HTMLSpanElement,
) {
  const toolbarRect = toolbar.getBoundingClientRect();
  const customRect = customAnchor.getBoundingClientRect();
  const customStart =
    customRect.right -
    toolbarRect.left +
    GROUP_SEPARATOR_MARGIN +
    GROUP_BUTTON_GAP;
  const actionSlotWidth = GROUP_BUTTON_GAP + GROUP_BUTTON_WIDTH;
  const availableWidth = Math.max(
    0,
    toolbar.clientWidth - customStart - actionSlotWidth,
  );

  return Math.max(
    0,
    Math.floor(
      (availableWidth + GROUP_BUTTON_GAP) /
        (GROUP_BUTTON_WIDTH + GROUP_BUTTON_GAP),
    ),
  );
}

/**
 * 构建自定义分组右键菜单；内联分组和溢出菜单分组共用这一份定义。
 */
export function buildGroupActionMenuItems(
  t: TFunction<["clipboard", "common"]>,
): DropdownMenuItems {
  return [
    {
      icon: "i-lucide:pencil",
      key: GROUP_MENU_ACTION.EDIT,
      label: t("clipboard:groups.edit"),
    },
    {
      icon: "i-lucide:eye-off",
      key: GROUP_MENU_ACTION.HIDE,
      label: t("clipboard:groups.hide"),
    },
    { type: "divider" },
    {
      danger: true,
      icon: "i-lucide:trash-2",
      key: GROUP_MENU_ACTION.DELETE,
      label: t("clipboard:groups.delete"),
    },
  ];
}

/**
 * 构建新增按钮右键菜单；左键继续新增，右键提供管理入口。
 */
export function buildCreateMenuItems(
  t: TFunction<["clipboard", "common"]>,
): DropdownMenuItems {
  return [
    {
      icon: "i-lucide:settings-2",
      key: MORE_MENU_ACTION.MANAGE_GROUPS,
      label: t("clipboard:groups.manage"),
    },
  ];
}

/**
 * 构建更多菜单项：新增 / 管理入口 + 溢出分组快速入口。
 */
export function buildMoreMenuItems(
  groups: ClipboardGroupRecord[],
  groupMenuItems: DropdownMenuItems,
  onGroupMenuClick: (info: { key: string }) => void,
  onGroupContext: (record: ClipboardGroupRecord) => void,
  t: TFunction<["clipboard", "common"]>,
): DropdownMenuItems {
  const groupItems = groups.map((record) => {
    return {
      key: buildMoreMenuGroupKey(record.id),
      label: (
        <OverflowGroupMenuLabel
          menuItems={groupMenuItems}
          onContext={onGroupContext}
          onMenuClick={onGroupMenuClick}
          record={record}
        />
      ),
    };
  });

  if (groupItems.length === 0) {
    return [
      {
        icon: "i-lucide:plus",
        key: MORE_MENU_ACTION.NEW_GROUP,
        label: t("clipboard:groups.add"),
      },
      {
        icon: "i-lucide:settings-2",
        key: MORE_MENU_ACTION.MANAGE_GROUPS,
        label: t("clipboard:groups.manage"),
      },
    ];
  }

  return [
    ...groupItems,
    { type: "divider" },
    {
      icon: "i-lucide:plus",
      key: MORE_MENU_ACTION.NEW_GROUP,
      label: t("clipboard:groups.add"),
    },
    {
      icon: "i-lucide:settings-2",
      key: MORE_MENU_ACTION.MANAGE_GROUPS,
      label: t("clipboard:groups.manage"),
    },
  ];
}

/**
 * 解析自定义分组右键菜单动作。
 */
export function parseGroupMenuAction(key: string): GroupMenuAction | null {
  const actions = Object.values(GROUP_MENU_ACTION);
  if (!actions.includes(key as GroupMenuAction)) return null;

  return key as GroupMenuAction;
}

/**
 * 解析更多菜单动作。
 */
export function parseMoreMenuAction(key: string): MoreMenuAction | null {
  const actions = Object.values(MORE_MENU_ACTION);
  if (!actions.includes(key as MoreMenuAction)) return null;

  return key as MoreMenuAction;
}

/**
 * 生成更多菜单中的分组 key。
 */
export function buildMoreMenuGroupKey(id: string): MoreMenuGroupKey {
  return `group:${id}`;
}

/**
 * 从更多菜单 key 中解析自定义分组 id。
 */
export function parseMoreMenuGroupId(key: string) {
  if (!key.startsWith("group:")) return null;

  return key.slice("group:".length);
}

/**
 * 判断字符串是否为范围分组值。
 */
export function isRangeGroup(value: unknown): value is ClipboardRange {
  return RANGE_GROUP_OPTIONS.some((option) => {
    return option.value === value;
  });
}

/**
 * 判断字符串是否为分类分组值。
 */
export function isCategoryGroup(value: unknown): value is ClipboardCategory {
  return CATEGORY_GROUP_OPTIONS.some((option) => {
    return option.value === value;
  });
}

/**
 * 在可见自定义分组间前后循环；当前未选中分组时，正向取第一个，反向取最后一个。
 */
export function selectAdjacentCustomGroup(
  groups: ClipboardGroupRecord[],
  groupId: string | null,
  reverse: boolean,
) {
  if (groups.length === 0) return null;

  const currentIndex = groupId
    ? groups.findIndex((record) => {
        return record.id === groupId;
      })
    : -1;

  if (reverse) {
    if (currentIndex === -1) return groups[groups.length - 1]?.id ?? null;

    return (
      groups[(currentIndex - 1 + groups.length) % groups.length]?.id ?? null
    );
  }

  if (currentIndex === -1) return groups[0]?.id ?? null;

  return groups[(currentIndex + 1) % groups.length]?.id ?? null;
}

/**
 * 把方向键映射为分类切换步进：图标栏响应左右键，标签栏响应上下键；其余键返回 0。
 */
export function getCategoryStep(key: string, isTab: boolean): -1 | 0 | 1 {
  if (isTab) {
    if (key === "ArrowUp") return -1;
    if (key === "ArrowDown") return 1;

    return 0;
  }

  if (key === "ArrowLeft") return -1;
  if (key === "ArrowRight") return 1;

  return 0;
}

/**
 * 判断方向键是否应交给输入控件原生光标导航。
 */
export function shouldUseNativeHorizontalNavigation(event: KeyboardEvent) {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return false;

  const tagName = target.tagName.toLowerCase();
  if (target.isContentEditable) return true;

  return tagName === "input" || tagName === "textarea";
}

/**
 * 当前选中分组被删除或不再存在时，回到全部分组。
 */
export function ensureSelectedGroupStillExists(groups: ClipboardGroupRecord[]) {
  const selectedGroupId = clipboardViewState.groupId;
  if (!selectedGroupId) return;

  const exists = groups.some((record) => {
    return record.id === selectedGroupId;
  });
  if (exists) return;

  clipboardViewState.groupId = null;
}
