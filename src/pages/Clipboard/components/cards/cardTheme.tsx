import type { ClipboardItem } from "@/types/clipboard";

/**
 * 停靠卡片标题带的类型配色；键为 `subKind ?? kind`，值为 antd 调色板 token 类。
 * 颜色子类型也走固定 token：颜色值本身已在卡片主体整块展示，标题带若用它做背景，
 * 浅色会让白字不可读。
 * 文件保持 `.tsx` 后缀：UnoCSS 只扫描 tsx 等模板文件，放在 `.ts` 里这些类名不会生成样式。
 */
const CARD_BAND_CLASS: Record<string, string> = {
  color: "bg-ant-purple-6",
  email: "bg-ant-cyan-6",
  files: "bg-ant-blue-6",
  html: "bg-ant-orange-6",
  image: "bg-ant-red-5",
  path: "bg-ant-geekblue-6",
  rtf: "bg-ant-orange-6",
  text: "bg-ant-gold-6",
  url: "bg-ant-green-6",
};

/**
 * 解析停靠卡片标题带的背景类；未知类型回落到文本色。
 */
export function resolveCardBandClass(item: ClipboardItem): string {
  const typeKey = item.subKind ?? item.kind;

  return CARD_BAND_CLASS[typeKey] ?? CARD_BAND_CLASS.text;
}
