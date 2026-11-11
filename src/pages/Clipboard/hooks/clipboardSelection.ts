import type { ClipboardItem } from "@/types/clipboard";

/**
 * 多选状态：按选择顺序保存记录快照，批量操作与粘贴队列按这个顺序执行。
 * 保存快照而不是只存 id，是因为列表 range cache 会回收远离视口的行。
 */
export interface ClipboardSelection {
  anchorId: string | null;
  items: ClipboardItem[];
}

export const EMPTY_SELECTION: ClipboardSelection = {
  anchorId: null,
  items: [],
};

/**
 * 切换单条记录的选中态；新选中的记录追加到末尾并成为范围选择锚点。
 */
export function toggleSelection(
  selection: ClipboardSelection,
  item: ClipboardItem,
): ClipboardSelection {
  const exists = selection.items.some((current) => {
    return current.id === item.id;
  });

  if (exists) {
    return {
      anchorId: selection.anchorId === item.id ? null : selection.anchorId,
      items: selection.items.filter((current) => {
        return current.id !== item.id;
      }),
    };
  }

  return { anchorId: item.id, items: [...selection.items, item] };
}

/**
 * 从锚点到目标的连续范围（含两端，仅限已加载行）并入选择；无锚点时等同单选切换。
 */
export function selectRange(
  selection: ClipboardSelection,
  target: ClipboardItem,
  anchorIndex: number | null,
  targetIndex: number,
  getItem: (index: number) => ClipboardItem | null,
): ClipboardSelection {
  if (anchorIndex === null) return toggleSelection(selection, target);

  const step = targetIndex >= anchorIndex ? 1 : -1;
  const selectedIds = new Set(
    selection.items.map((item) => {
      return item.id;
    }),
  );
  const next = [...selection.items];

  for (
    let index = anchorIndex;
    step > 0 ? index <= targetIndex : index >= targetIndex;
    index += step
  ) {
    const item = getItem(index);
    if (!item || selectedIds.has(item.id)) continue;

    selectedIds.add(item.id);
    next.push(item);
  }

  return { anchorId: selection.anchorId, items: next };
}

/**
 * 已选记录 id，保持选择顺序。
 */
export function selectionIds(selection: ClipboardSelection) {
  return selection.items.map((item) => {
    return item.id;
  });
}
