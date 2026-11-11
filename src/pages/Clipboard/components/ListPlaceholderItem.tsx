import type { FC } from "react";
import { cn } from "@/utils/cn";
import { itemWrapperClass, itemWrapperStyle } from "./listHelpers";

interface ListPlaceholderItemProps {
  cardSize: number;
  index: number;
  isHorizontal: boolean;
  total: number;
}

/**
 * 尚未加载到的列表行骨架，尺寸与真实卡片一致，避免滚动时跳动。
 */
const ListPlaceholderItem: FC<ListPlaceholderItemProps> = (props) => {
  const { cardSize, index, isHorizontal, total } = props;

  if (isHorizontal) {
    return (
      <div
        aria-hidden="true"
        className={itemWrapperClass(isHorizontal, index, total)}
        style={itemWrapperStyle(isHorizontal, cardSize)}
      >
        <div className="size-full overflow-hidden rounded-4 bg-ant-fill-quaternary ring-1 ring-ant-border-secondary">
          <div className="h-13 bg-ant-fill-secondary" />
          <div className="flex flex-col gap-2 p-3">
            <span className="h-3 w-9/12 rounded-1 bg-ant-fill-secondary" />
            <span className="h-3 w-6/12 rounded-1 bg-ant-fill-secondary" />
          </div>
        </div>
      </div>
    );
  }

  return (
    <div aria-hidden="true" className={cn("px-3", { "pt-3": index !== 0 })}>
      <div className="min-h-24 rounded-2 border border-ant-border-secondary bg-ant-fill-quaternary p-2">
        <div className="flex items-center gap-1 text-ant-secondary text-xs">
          <span className="size-4 rounded-1 bg-ant-fill-secondary" />
          <span className="h-3 w-16 rounded-1 bg-ant-fill-secondary" />
        </div>
        <div className="mt-3 flex flex-col gap-2">
          <span className="h-3 w-9/12 rounded-1 bg-ant-fill-secondary" />
          <span className="h-3 w-6/12 rounded-1 bg-ant-fill-secondary" />
        </div>
      </div>
    </div>
  );
};

export default ListPlaceholderItem;
