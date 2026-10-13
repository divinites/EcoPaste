import type { CSSProperties, FC } from "react";
import { useSnapshot } from "valtio";
import AssetImage from "@/components/AssetImage";
import { settingsState } from "@/stores/settings";
import type { ClipboardItem } from "@/types/clipboard";

interface ImageCardProps extends ClipboardItem {
  /**
   * 停靠卡片填充模式：缩略图铺满容器并裁切，忽略 `imageMaxHeight` 设置。
   */
  fill?: boolean;
}

/**
 * 图片类卡片：按 `content`（文件名）向 Rust 取缩略图路径并 `convertFileSrc` 加载。
 * 紧凑模式高度按设置限制；填充模式铺满停靠卡片主体。
 */
const ImageCard: FC<ImageCardProps> = (props) => {
  const { imageThumbnailPath, fill } = props;
  const { clipboard } = useSnapshot(settingsState);

  if (fill) {
    return (
      <AssetImage
        className="size-full rounded-3 object-cover"
        src={imageThumbnailPath}
      />
    );
  }

  const style: CSSProperties = {
    maxHeight: clipboard.display.imageMaxHeight,
  };

  return (
    <AssetImage className="self-start" src={imageThumbnailPath} style={style} />
  );
};

export default ImageCard;
