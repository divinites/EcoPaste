import type { CSSProperties } from "react";
import type {
  ClipboardAction,
  ClipboardGroupRecord,
  ClipboardItem,
  ClipboardKind,
  ClipboardRange,
} from "@/types/clipboard";
import type { ItemAction } from "@/types/settings";
import { cn } from "@/utils/cn";

/**
 * 从当前已加载分组列表中取出选中分组名称；找不到时交给文案层兜底。
 */
export function getCurrentGroupName(
  groups: ClipboardGroupRecord[],
  groupId: string | null,
) {
  if (!groupId) return null;

  const current = groups.find((record) => {
    return record.id === groupId;
  });

  return current?.name ?? null;
}

/**
 * 把方向键映射为条目导航方向：竖向列表响应上下键，横向列表响应左右键；其余键返回 0。
 */
export function getNavigationDirection(
  key: string,
  isHorizontal: boolean,
): -1 | 0 | 1 {
  if (isHorizontal) {
    if (key === "ArrowLeft") return -1;
    if (key === "ArrowRight") return 1;

    return 0;
  }

  if (key === "ArrowUp") return -1;
  if (key === "ArrowDown") return 1;

  return 0;
}

/**
 * 根据导航方向和当前选中 id 计算下一项索引。
 */
export function getNextKeyboardIndex(
  getItemIndexById: (id: string) => number | null,
  firstVisibleIndex: number,
  selectedId: string | null,
  total: number,
  direction: -1 | 1,
) {
  const selectedIndex =
    selectedId === null ? null : getItemIndexById(selectedId);
  const currentIndex = selectedIndex ?? firstVisibleIndex;

  if (direction === -1) {
    return Math.max(0, currentIndex - 1);
  }

  return Math.min(total - 1, currentIndex + 1);
}

/**
 * 条目外层容器类：竖向模式只有左右内边距与行间距；
 * 横向模式撑满高度，首尾条目补齐面板两侧留白。
 */
export function itemWrapperClass(
  isHorizontal: boolean,
  index: number,
  total: number,
) {
  if (!isHorizontal) {
    return cn("px-3", { "pt-3": index !== 0 });
  }

  return cn("box-content py-2 pl-4", { "pr-4": index === total - 1 });
}

/**
 * 横向条目的像素尺寸：卡片为正方形，宽高与测得的边长一致；尺寸未测出前不渲染条目尺寸。
 */
export function itemWrapperStyle(
  isHorizontal: boolean,
  cardSize: number,
): CSSProperties | undefined {
  if (!isHorizontal || cardSize <= 0) return void 0;

  return { height: cardSize, width: cardSize };
}

/**
 * 按条目保护规则过滤右键菜单动作，避免受保护项出现删除入口。
 */
export function getAllowedClipboardActions(
  actions: ClipboardAction[] | undefined,
  item: ClipboardItem,
  canDeleteItem: (item: ClipboardItem) => boolean,
) {
  if (canDeleteItem(item)) return actions;

  return actions?.filter((action) => {
    return action !== "delete";
  });
}

/**
 * 从后端声明的右键动作中取出可由 Cmd/Ctrl+O 触发的“打开”动作。
 */
export function getOpenClipboardAction(actions: ClipboardAction[] | undefined) {
  const openActions: ClipboardAction[] = [
    "openLink",
    "sendEmail",
    "revealInFinder",
    "revealInExplorer",
  ];

  return actions?.find((action) => {
    return openActions.includes(action);
  });
}

/**
 * 按条目保护规则过滤悬停快捷动作，避免受保护项出现删除按钮。
 */
export function getAllowedItemActions(
  actions: readonly ItemAction[],
  item: ClipboardItem,
  canDeleteItem: (item: ClipboardItem) => boolean,
) {
  if (canDeleteItem(item)) return [...actions];

  return actions.filter((action) => {
    return action !== "delete";
  });
}

/**
 * 删除当前 active 项后优先选后一项；删除末尾时回退到前一项。
 * 右键删除非 active 项时保留当前显式选中，避免意外跳选。
 */
export function getSelectedIdAfterDelete(
  getItem: (index: number) => ClipboardItem | null,
  getItemIndexById: (id: string) => number | null,
  activeFallbackIndex: number,
  selectedId: string | null,
  deletedId: string,
) {
  const deletedIndex = getItemIndexById(deletedId);
  if (deletedIndex === null) return selectedId;

  const activeId =
    selectedId ?? getItem(activeFallbackIndex)?.id ?? getItem(0)?.id ?? null;

  if (activeId !== deletedId) return selectedId;

  const nextItem = getItem(deletedIndex + 1) ?? getItem(deletedIndex - 1);

  return nextItem?.id ?? null;
}

/**
 * 判断 Cmd/Ctrl+C 是否应交给浏览器原生复制，避免覆盖输入框或文本选区复制。
 */
export function shouldUseNativeCopy(event: KeyboardEvent) {
  const target = event.target;
  if (target instanceof HTMLElement) {
    const tagName = target.tagName.toLowerCase();
    if (target.isContentEditable) return true;
    if (tagName === "input" || tagName === "textarea") return true;
  }

  const selection = window.getSelection();

  return Boolean(selection && !selection.isCollapsed);
}

/**
 * 统计当前已加载页开头连续置顶条目数，供 Virtuoso sticky top items 使用。
 */
export function countLeadingPinnedItems(
  getItem: (index: number) => ClipboardItem | null,
) {
  let count = 0;

  while (true) {
    const item = getItem(count);
    if (!item?.isPinned) break;

    count += 1;
  }

  return count;
}

/**
 * 判断普通剪贴板更新是否会出现在当前分组列表中。
 */
export function shouldRefreshCurrentGroup(
  range: ClipboardRange,
  category: ClipboardKind | null,
  groupId: string | null,
  kind?: ClipboardKind,
) {
  if (groupId) return false;
  if (range === "favorite") return false;
  if (!category) return true;
  if (kind === void 0) return false;

  return category === kind;
}
