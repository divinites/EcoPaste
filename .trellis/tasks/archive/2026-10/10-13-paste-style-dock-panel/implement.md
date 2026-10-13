# 执行计划：Paste 风格底部停靠面板

前置：当前仓库无 `node_modules`，先 `pnpm install`。分支策略：当前在 `master`，提交前新建 `feat/paste-dock-panel`。

## 阶段 0 核验（design.md §9）

- [x] `cargo doc -p tauri --no-deps` 或 registry 源码确认 `Monitor::work_area()`；不可用则改用 macOS `NSScreen.visibleFrame` + Windows `SystemParametersInfoW(SPI_GETWORKAREA)`，并回到 design.md 更新 §3.1。
- [x] `node_modules/react-virtuoso` 类型确认 `horizontalDirection`、`scrollIntoView`。
- [x] 读 `src-tauri/src/db/models.rs` 中 `display_created_at` 等派生字段写法，对齐 `char_count`。

## 阶段 1 设置契约（Rust → TS → 偏好面板）

- [x] `settings/model.rs`：`ClipboardWindowLayout` 枚举、`Window.layout`、`Window.dock_height_percent` 与默认值；导出 `DOCK_HEIGHT_PERCENT_RANGE`。
- [x] `settings/store.rs:validate_settings`：范围校验 + 单元测试（24 / 25 / 60 / 61）。
- [x] `src/types/settings.ts`、`src/constants/clipboardLayout.ts`。
- [x] `preferenceSchema.ts` 三项改动（layout segmented、dockHeightPercent number、position disabledWhen）。
- [x] zh-CN / en-US `preferences.json` 文案；`numberSuffix.percent`。
- 验证：`cargo test settings`、`pnpm tsc`、打开偏好面板切换两项并确认 `settings.json` 落盘。

## 阶段 2 窗口几何（Rust）

- [x] `window/position.rs:apply_dock`。
- [x] `window/mod.rs:apply_clipboard_window_layout` 分支 + `set_min_size` 切换。
- [x] `window/mod.rs:save_all_window_states` 在 dock 下跳过剪贴板窗口。
- [x] `window/preview.rs`：`prefer_top` 参数、候选顺序、调用点读取布局、新增测试。
- 验证：`cargo clippy -- -D warnings`、`cargo test window`；`pnpm tauri dev` 在 dock / vertical 间切换各呼出一次，确认位置、尺寸、Dock 避让、`window-state.json` 未被 dock 几何覆盖。

## 阶段 3 列表 payload

- [x] `db/models.rs` `char_count` 字段；`commands/clipboard.rs` 列表路径 `attach_char_count`；测试断言 text 条目有值、image 无值。
- [x] `src/types/clipboard.ts` `charCount?: number`。

## 阶段 4 前端布局

- [x] `hooks/useClipboardHeaderActions.ts` 抽取 Header 逻辑；`Header.tsx` 改为消费该 hook（行为不变）。
- [x] `components/DockHeader.tsx`。
- [x] `Group.tsx` `variant` 与 tab 渲染、键位分支。
- [x] `VirtuosoScroller` `horizontal` prop。
- [x] `List.tsx` `orientation`：Virtuoso 配置、键位映射、外层类、滚轮转横向、占位骨架、`topItemCount`。
- [x] `pages/Clipboard/index.tsx` 布局分支。
- 验证：`pnpm lint`、`pnpm tsc`；dock 下左右键 / 滚轮 / 数字快捷键 / 右键菜单 / 拖出 / Space 预览逐项实操；vertical 下回归上下键与原有布局。

## 阶段 5 卡片

- [x] `cards/cardTheme.ts`、`cards/cardMeta.ts`；`clipboard.json` zh-CN / en-US `meta.*` 文案。
- [x] `ClipboardCard.tsx` `variant="dock"` 渲染：标题带、主体、元信息、选中 ring、置顶 / 敏感水印。
- [x] `TextCard` / `ImageCard` / `FilesCard` 在 dock 变体下的填充模式（object-cover、动态 line-clamp、颜色整块）。
- [x] `ShortcutList.tsx` 按布局切换导航提示；`shortcuts.navigateHorizontal` 文案。
- 验证：准备文本 / 链接 / 邮箱 / 颜色 / HTML / 图片 / 文件 / 敏感 / 置顶 / 带备注各一条，浅色与深色各截图一次核对 R4。

## 阶段 6 收尾

- [x] 全量：`pnpm lint && pnpm tsc && (cd src-tauri && cargo fmt --check && cargo clippy -- -D warnings && cargo test)`。
- [x] 用一份不含新字段的旧 `settings.json` 启动，确认直接进入 dock（AC8）。
- [x] `trellis-check`；`.trellis/spec/backend/settings-window-platform.md` 补 layout 契约小节；`frontend/component-guidelines.md` 补卡片变体约定（如适用）。
- [x] 提交：`feat: add Paste-style bottom dock layout for clipboard window`。

## 风险与回滚点

| 风险 | 处置 |
| --- | --- |
| `work_area` 不可用或 macOS 多屏返回异常 | 阶段 0 兜底方案；阶段 2 单独提交，可独立 revert |
| Virtuoso 横向 `scrollIntoView` 不准 | 退回 `scrollToIndex({ align: "center" })` |
| Windows 整宽窗口触发勿扰 / 全屏检测 | 宽度减 1（已在设计中） |
| 老用户对默认 dock 不适应 | 设置项一键切回，文档 / release note 说明 |

每个阶段独立可编译、可运行；阶段 1 到 3 完成后即使阶段 4 / 5 未做，vertical 模式也完全可用。
