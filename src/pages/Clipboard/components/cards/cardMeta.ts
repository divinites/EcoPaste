import type { TFunction } from "i18next";
import type { ClipboardItem } from "@/types/clipboard";

/**
 * 生成停靠卡片底部的一行元信息：链接显示域名，文本显示字符数，
 * 图片显示像素尺寸，文件显示条目数；信息不足时返回空串不渲染。
 */
export function formatCardMeta(
  item: ClipboardItem,
  t: TFunction<"clipboard">,
): string {
  if (item.kind === "image") {
    if (item.width === null || item.height === null) return "";

    return t("meta.dimensions", { height: item.height, width: item.width });
  }

  if (item.kind === "files") {
    const count = countFileEntries(item.content);
    if (count === 0) return "";

    return t("meta.files", { count });
  }

  if (item.subKind === "url") {
    const host = parseHostname(item.summary);
    if (host) return host;
  }

  if (item.charCount === void 0) return "";

  return t("meta.characters", { count: item.charCount });
}

/**
 * files 条目的 `content` 是换行分隔的路径列表，列表视图不裁剪，可直接计数。
 */
function countFileEntries(content: string) {
  return content.split("\n").filter((line) => {
    return line.trim().length > 0;
  }).length;
}

/**
 * 从 URL 摘要里取主机名；摘要不是合法 URL 时返回 null，由调用方回落到字符数。
 */
function parseHostname(summary: string | null) {
  if (!summary) return null;

  try {
    return new URL(summary.trim()).hostname || null;
  } catch {
    return null;
  }
}
