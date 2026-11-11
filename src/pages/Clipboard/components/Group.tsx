import { useMount } from "ahooks";
import type { FC, MouseEvent } from "react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSnapshot } from "valtio";
import {
  createClipboardGroup,
  deleteClipboardGroup,
  listClipboardGroups,
  openPreferenceWithHighlight,
  updateClipboardGroup,
} from "@/commands";
import ClipboardGroupIcon from "@/components/ClipboardGroupIcon";
import ClipboardGroupModal from "@/components/ClipboardGroupModal";
import Dropdown from "@/components/Dropdown";
import KeyHint from "@/components/KeyHint";
import Tooltip from "@/components/Tooltip";
import { TAURI_EVENT } from "@/constants/events";
import { useKeyboardEvent } from "@/hooks/useKeyboardEvent";
import { useTauriListen } from "@/hooks/useTauriListen";
import { clipboardViewState } from "@/stores/clipboardView";
import type {
  ClipboardCategory,
  ClipboardGroupIcon as ClipboardGroupIconValue,
  ClipboardGroupInput,
  ClipboardGroupRecord,
  ClipboardRange,
} from "@/types/clipboard";
import { cn } from "@/utils/cn";
import { getModalApi } from "@/utils/feedback";
import {
  buildCreateMenuItems,
  buildGroupActionMenuItems,
  buildMoreMenuGroupKey,
  buildMoreMenuItems,
  CATEGORY_GROUP_OPTIONS,
  type CategoryGroupOption,
  commitVisibleCustomGroupCount,
  ensureSelectedGroupStillExists,
  GROUP_MENU_ACTION,
  GroupSeparator,
  getCategoryStep,
  isCategoryGroup,
  isRangeGroup,
  MORE_MENU_ACTION,
  parseGroupMenuAction,
  parseMoreMenuAction,
  parseMoreMenuGroupId,
  RANGE_GROUP_OPTIONS,
  type RangeGroupOption,
  scheduleVisibleCustomGroupCountUpdate,
  selectAdjacentCustomGroup,
  shouldUseNativeHorizontalNavigation,
} from "./groupParts";

type GroupModalMode = "create" | "edit";
const CUSTOM_GROUPS_SETTING_ID = "organizing.customGroups";

const GROUP_BUTTON_BASE_CLASS =
  "flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-1.5 border-0 bg-transparent p-0 transition-colors";
const GROUP_ICON_BUTTON_CLASS = GROUP_BUTTON_BASE_CLASS;
const GROUP_TAB_BUTTON_CLASS =
  "flex h-7 shrink-0 cursor-pointer items-center gap-1.5 whitespace-nowrap rounded-full border-0 bg-transparent px-3 text-sm transition-colors";

export type GroupVariant = "icon" | "tab";

interface GroupProps {
  /**
   * `icon`：竖向窗口下的纯图标栏，宽度不足时把自定义分组收进「更多」菜单；
   * `tab`：停靠面板顶栏的带名称胶囊标签，溢出时横向滚动。
   */
  variant?: GroupVariant;
}
/**
 * 分组筛选栏：内置类型分组 + 自定义分组入口；按 `variant` 渲染为图标栏或名称标签栏。
 */
const Group: FC<GroupProps> = (props) => {
  const { variant = "icon" } = props;
  const isTab = variant === "tab";
  const customGroupButtonClass = isTab
    ? GROUP_TAB_BUTTON_CLASS
    : GROUP_ICON_BUTTON_CLASS;
  const { t } = useTranslation(["clipboard", "common"]);
  const { category, groupId, range } = useSnapshot(clipboardViewState);

  const [customGroups, setCustomGroups] = useState<ClipboardGroupRecord[]>([]);
  const [modalOpen, setModalOpen] = useState(false);
  const [modalMode, setModalMode] = useState<GroupModalMode>("create");
  const [visibleCustomGroupCount, setVisibleCustomGroupCount] = useState(
    Number.POSITIVE_INFINITY,
  );
  const [editingGroup, setEditingGroup] = useState<ClipboardGroupRecord | null>(
    null,
  );
  const toolbarRef = useRef<HTMLDivElement>(null);
  const customGroupAnchorRef = useRef<HTMLSpanElement>(null);
  const contextGroupRef = useRef<ClipboardGroupRecord | null>(null);
  const deleteGroupRef = useRef<ClipboardGroupRecord | null>(null);

  const visibleCustomGroups = customGroups.filter((record) => {
    return !record.isHidden;
  });
  const inlineCustomGroups = isTab
    ? visibleCustomGroups
    : visibleCustomGroups.slice(0, visibleCustomGroupCount);
  const overflowCustomGroups = isTab
    ? []
    : visibleCustomGroups.slice(visibleCustomGroupCount);

  /**
   * 从 Rust 拉取自定义分组。
   */
  const loadGroups = async () => {
    const groups = await listClipboardGroups();

    setCustomGroups(groups);
    if (!isTab) {
      scheduleVisibleCustomGroupCountUpdate(
        toolbarRef,
        customGroupAnchorRef,
        setVisibleCustomGroupCount,
        groups.filter((record) => {
          return !record.isHidden;
        }).length,
      );
    }
    ensureSelectedGroupStillExists(groups);
  };

  /**
   * 首次挂载时拉取分组。
   */
  useMount(() => {
    void loadGroups();
  });

  /**
   * 其他窗口或命令修改分组后刷新本地列表。
   */
  const handleGroupsUpdated = () => {
    void loadGroups();
  };

  useTauriListen(TAURI_EVENT.CLIPBOARD_GROUPS_UPDATED, handleGroupsUpdated);

  /**
   * 容器尺寸变化时重新测量溢出状态；标签变体横向滚动，不做溢出折叠。
   */
  useEffect(() => {
    if (isTab) return;

    const toolbar = toolbarRef.current;
    const customAnchor = customGroupAnchorRef.current;
    if (!toolbar || !customAnchor) return;

    const updateVisibleCustomGroupCount = () => {
      commitVisibleCustomGroupCount(
        toolbar,
        customAnchor,
        setVisibleCustomGroupCount,
        visibleCustomGroups.length,
      );
    };

    const observer = new ResizeObserver(updateVisibleCustomGroupCount);
    observer.observe(toolbar);
    updateVisibleCustomGroupCount();

    return () => {
      observer.disconnect();
    };
  }, [isTab, visibleCustomGroups.length]);

  /**
   * 切换范围；范围必须始终保留一个选中项。
   */
  const selectRange = (value: ClipboardRange) => {
    clipboardViewState.range = value;
  };

  /**
   * 切换分类；再次点击当前分类时取消。
   */
  const toggleCategory = (value: ClipboardCategory) => {
    clipboardViewState.category =
      clipboardViewState.category === value ? null : value;
  };

  /**
   * 切换到自定义分组；再次点击当前分组时取消。
   */
  const toggleCustomGroup = (id: string) => {
    clipboardViewState.groupId = clipboardViewState.groupId === id ? null : id;
  };

  /**
   * 点击分组按钮时根据 data 属性切换筛选。
   */
  const handleGroupClick = (event: MouseEvent<HTMLButtonElement>) => {
    const type = event.currentTarget.dataset.type;
    const value = event.currentTarget.dataset.value;
    const nextGroupId = event.currentTarget.dataset.groupId;

    if (nextGroupId) {
      toggleCustomGroup(nextGroupId);
      return;
    }

    if (type === "range" && isRangeGroup(value)) {
      selectRange(value);
      return;
    }

    if (type === "category" && isCategoryGroup(value)) {
      toggleCategory(value);
    }
  };

  /**
   * 记录右键菜单所属分组。
   */
  const handleCustomGroupContextMenu = (
    event: MouseEvent<HTMLButtonElement>,
  ) => {
    const nextGroupId = event.currentTarget.dataset.groupId;
    if (!nextGroupId) return;

    contextGroupRef.current =
      customGroups.find((record) => {
        return record.id === nextGroupId;
      }) ?? null;
  };

  /**
   * 处理分组栏快捷键：Cmd/Ctrl+Q 切换范围，Tab / Shift+Tab 仅在可见自定义分组间循环。
   * 切分类的方向键与列表导航键互补：图标栏（竖向列表）用左右键，标签栏（横向列表）用上下键。
   */
  const handleKeyDown = (event: KeyboardEvent) => {
    const eventModifierPressed = event.metaKey || event.ctrlKey;

    if (eventModifierPressed && event.key.toLowerCase() === "q") {
      event.preventDefault();
      toggleRange();

      return;
    }

    const categoryStep = getCategoryStep(event.key, isTab);

    if (categoryStep !== 0 && !shouldUseNativeHorizontalNavigation(event)) {
      event.preventDefault();
      selectAdjacentCategory(categoryStep);

      return;
    }

    if (event.key !== "Tab") return;

    event.preventDefault();

    const nextGroupId = selectAdjacentCustomGroup(
      visibleCustomGroups,
      groupId,
      event.shiftKey,
    );

    if (!nextGroupId) return;

    toggleCustomGroup(nextGroupId);
  };

  useKeyboardEvent("keydown", handleKeyDown);

  /**
   * 在全部 / 收藏范围之间循环切换，不影响分类与自定义分组筛选。
   */
  const toggleRange = () => {
    clipboardViewState.range =
      clipboardViewState.range === "all" ? "favorite" : "all";
  };

  /**
   * 按方向键在固定分类序列内循环；未选分类时从方向对应的端点进入。
   */
  const selectAdjacentCategory = (direction: -1 | 1) => {
    const options = CATEGORY_GROUP_OPTIONS.map((option) => {
      return option.value;
    });
    const currentCategory = clipboardViewState.category;
    const current = currentCategory ? options.indexOf(currentCategory) : -1;
    const startIndex = direction === 1 ? -1 : options.length;
    const nextIndex =
      (current === -1 ? startIndex + direction : current + direction) %
      options.length;
    const normalizedIndex = (nextIndex + options.length) % options.length;

    clipboardViewState.category = options[normalizedIndex];
  };

  /**
   * 打开新增分组弹框。
   */
  const openCreateModal = () => {
    setModalMode("create");
    setEditingGroup(null);
    setModalOpen(true);
  };

  /**
   * 打开编辑分组弹框。
   */
  const openEditModal = (record: ClipboardGroupRecord) => {
    setModalMode("edit");
    setEditingGroup(record);
    setModalOpen(true);
  };

  /**
   * 关闭新增 / 编辑分组弹框。
   */
  const closeModal = () => {
    setModalOpen(false);
    setEditingGroup(null);
  };

  /**
   * 保存分组弹框内容。
   */
  const handleModalSubmit = async (input: ClipboardGroupInput) => {
    if (modalMode === "create") {
      await createClipboardGroup(input);
      closeModal();
      return;
    }

    if (!editingGroup) return;

    await updateClipboardGroup(editingGroup.id, input);
    closeModal();
  };

  /**
   * 执行自定义分组右键菜单动作。
   */
  const handleGroupMenuClick = (info: { key: string }) => {
    const record = contextGroupRef.current;
    if (!record) return;

    const action = parseGroupMenuAction(info.key);
    if (!action) return;

    if (action === GROUP_MENU_ACTION.EDIT) {
      openEditModal(record);
      return;
    }

    if (action === GROUP_MENU_ACTION.HIDE) {
      void updateClipboardGroup(record.id, {
        icon: record.icon,
        isHidden: true,
        name: record.name,
      });
      return;
    }

    if (action === GROUP_MENU_ACTION.DELETE) {
      requestDeleteGroup(record);
    }
  };

  /**
   * 执行新增分组动作。
   */
  const handleCreateGroupAction = () => {
    openCreateModal();
  };

  /**
   * 打开偏好设置并定位到自定义分组管理项。
   */
  const openGroupPreference = async () => {
    await openPreferenceWithHighlight(CUSTOM_GROUPS_SETTING_ID);
  };

  /**
   * 执行更多菜单动作：新增 / 管理分组，或切换到溢出的自定义分组。
   */
  const handleMoreMenuClick = async (info: { key: string }) => {
    const action = parseMoreMenuAction(info.key);
    if (action === MORE_MENU_ACTION.NEW_GROUP) {
      handleCreateGroupAction();
      return;
    }

    if (action === MORE_MENU_ACTION.MANAGE_GROUPS) {
      await openGroupPreference();
      return;
    }

    const id = parseMoreMenuGroupId(info.key);
    if (!id) return;

    toggleCustomGroup(id);
  };

  /**
   * 弹出删除确认框。
   */
  const requestDeleteGroup = (record: ClipboardGroupRecord) => {
    deleteGroupRef.current = record;

    getModalApi().confirm({
      centered: true,
      content: (
        <span className="text-ant-secondary text-sm">
          {t("clipboard:groups.deleteConfirmDescription", {
            group: record.name,
          })}
        </span>
      ),
      okButtonProps: { danger: true },
      okText: t("common:actions.delete"),
      onOk: confirmDeleteGroup,
      title: t("clipboard:groups.delete"),
    });
  };

  /**
   * 确认删除当前待删除分组。
   */
  const confirmDeleteGroup = async () => {
    const record = deleteGroupRef.current;
    if (!record) return;

    await deleteClipboardGroup(record.id);

    if (clipboardViewState.groupId === record.id) {
      clipboardViewState.groupId = null;
    }

    deleteGroupRef.current = null;
  };

  const groupMenuItems = buildGroupActionMenuItems(t);
  const createMenuItems = buildCreateMenuItems(t);

  /**
   * 记录溢出菜单中右键菜单所属分组。
   */
  const handleOverflowGroupContext = (record: ClipboardGroupRecord) => {
    contextGroupRef.current = record;
  };

  const moreMenuItems = buildMoreMenuItems(
    overflowCustomGroups,
    groupMenuItems,
    handleGroupMenuClick,
    handleOverflowGroupContext,
    t,
  );
  const moreMenuSelectedKeys = groupId ? [buildMoreMenuGroupKey(groupId)] : [];
  const moreButtonSelected = overflowCustomGroups.some((record) => {
    return record.id === groupId;
  });

  /**
   * 渲染溢出分组菜单按钮。
   */
  const renderMoreButton = () => {
    if (overflowCustomGroups.length === 0) return null;

    return (
      <Dropdown
        menu={{
          items: moreMenuItems,
          onClick: handleMoreMenuClick,
          selectedKeys: moreMenuSelectedKeys,
        }}
        tooltip={t("clipboard:groups.more")}
        trigger={["click"]}
      >
        <button
          className={cn(GROUP_BUTTON_BASE_CLASS, {
            "bg-ant-primary text-ant-light-solid": moreButtonSelected,
            "text-ant-secondary hover:bg-ant-fill-tertiary":
              !moreButtonSelected,
          })}
          type="button"
        >
          <KeyHint hintKey="N" onKeyPress={handleCreateGroupAction}>
            <i aria-hidden className="i-lucide:more-horizontal text-sm!" />
          </KeyHint>
        </button>
      </Dropdown>
    );
  };

  /**
   * 渲染独立新增按钮；存在溢出菜单时由菜单内新增入口承接。
   */
  const renderCreateButton = () => {
    if (overflowCustomGroups.length > 0) return null;

    return (
      <Dropdown
        menu={{
          items: createMenuItems,
          onClick: handleMoreMenuClick,
        }}
        tooltip={t("clipboard:groups.add")}
        trigger={["contextMenu"]}
      >
        <button
          className={cn(
            GROUP_BUTTON_BASE_CLASS,
            "text-ant-secondary hover:bg-ant-fill-tertiary",
          )}
          onClick={handleCreateGroupAction}
          type="button"
        >
          <KeyHint hintKey="N" onKeyPress={handleCreateGroupAction}>
            <i aria-hidden className="i-lucide:plus text-sm!" />
          </KeyHint>
        </button>
      </Dropdown>
    );
  };

  /**
   * 渲染范围按钮。
   */
  const renderRangeButton = ({ labelKey, value, icon }: RangeGroupOption) => {
    const selected = range === value;
    const nextRange =
      range === "all" ? "favorite" : range === "favorite" ? "all" : void 0;
    const showShortcutHint = nextRange === value;

    return renderFilterButton({
      icon,
      label: t(`clipboard:${labelKey}`),
      selected,
      showShortcutHint,
      type: "range",
      value,
    });
  };

  /**
   * 渲染分类按钮。
   */
  const renderCategoryButton = ({
    labelKey,
    value,
    icon,
  }: CategoryGroupOption) => {
    const selected = category === value;

    return renderFilterButton({
      icon,
      label: t(`clipboard:${labelKey}`),
      selected,
      type: "category",
      value,
    });
  };

  /**
   * 渲染单个筛选按钮。
   */
  const renderFilterButton = (options: {
    icon: ClipboardGroupIconValue;
    label: string;
    selected: boolean;
    showShortcutHint?: boolean;
    type: "category" | "range";
    value: ClipboardCategory | ClipboardRange;
  }) => {
    const { icon, label, selected, showShortcutHint, type, value } = options;
    const iconNode = showShortcutHint ? (
      <KeyHint hintKey="Q">
        <ClipboardGroupIcon icon={icon} selected={selected} />
      </KeyHint>
    ) : (
      <ClipboardGroupIcon icon={icon} selected={selected} />
    );

    if (isTab) {
      return (
        <button
          className={cn(GROUP_TAB_BUTTON_CLASS, {
            "bg-ant-primary text-ant-light-solid": selected,
            "text-ant-secondary hover:bg-ant-fill-tertiary": !selected,
          })}
          data-type={type}
          data-value={value}
          key={`${type}:${value}`}
          onClick={handleGroupClick}
          type="button"
        >
          {iconNode}
          <span>{label}</span>
        </button>
      );
    }

    return (
      <Tooltip key={`${type}:${value}`} title={label}>
        <button
          className={cn(GROUP_ICON_BUTTON_CLASS, {
            "bg-ant-primary text-ant-light-solid": selected,
            "text-ant-secondary hover:bg-ant-fill-tertiary": !selected,
          })}
          data-type={type}
          data-value={value}
          onClick={handleGroupClick}
          type="button"
        >
          {iconNode}
        </button>
      </Tooltip>
    );
  };

  return (
    <>
      <div
        className={cn("flex items-center gap-1", {
          "min-w-0 flex-1 overflow-x-auto [&::-webkit-scrollbar]:hidden": isTab,
          "overflow-hidden px-3 pb-2": !isTab,
        })}
        data-tauri-drag-region
        ref={toolbarRef}
      >
        {RANGE_GROUP_OPTIONS.map(renderRangeButton)}
        <GroupSeparator />
        {CATEGORY_GROUP_OPTIONS.map(renderCategoryButton)}
        <GroupSeparator separatorRef={customGroupAnchorRef} />

        {inlineCustomGroups.length > 0 && (
          <div className="flex min-w-0 shrink-0 items-center gap-1 overflow-hidden">
            {inlineCustomGroups.map((record) => {
              const selected = groupId === record.id;

              return (
                <Dropdown
                  key={record.id}
                  menu={{
                    items: groupMenuItems,
                    onClick: handleGroupMenuClick,
                  }}
                  tooltip={record.name}
                  trigger={["contextMenu"]}
                >
                  <button
                    className={cn(customGroupButtonClass, {
                      "bg-ant-primary text-ant-light-solid": selected,
                      "text-ant-secondary hover:bg-ant-fill-tertiary":
                        !selected,
                    })}
                    data-group-id={record.id}
                    onClick={handleGroupClick}
                    onContextMenu={handleCustomGroupContextMenu}
                    type="button"
                  >
                    <ClipboardGroupIcon
                      icon={record.icon}
                      selected={selected}
                    />
                    {isTab ? <span>{record.name}</span> : null}
                  </button>
                </Dropdown>
              );
            })}
          </div>
        )}

        {renderMoreButton()}
        {renderCreateButton()}
      </div>

      <ClipboardGroupModal
        group={editingGroup}
        mode={modalMode}
        onCancel={closeModal}
        onSubmit={handleModalSubmit}
        open={modalOpen}
      />
    </>
  );
};

export default Group;
