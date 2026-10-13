# Paste 风格底部停靠剪贴板面板

## Goal

让呼出剪贴板面板时的观感接近 Paste（<https://pasteapp.io/use-cases#everyone>）：
面板从屏幕底部整宽弹出，内容是一排横向滚动的大卡片，每张卡片带按类型着色的标题带、
来源应用图标和底部元信息。保留现有竖窗模式，用户可在设置里切换。

## Background（仓库已确认的事实）

- 剪贴板窗口由 `src-tauri/tauri.conf.json` 定义：360×600、无边框、透明、`focusable: false`、`alwaysOnTop`。
- 窗口位置策略 `clipboard.window.position`（remember / followCursor / center）由
  `src-tauri/src/window/position.rs` 实现，`window/mod.rs:apply_clipboard_window_layout` 在每次 show 前调用；
  `window/state.rs` 负责 remember 模式的位置与尺寸持久化。
- macOS 侧剪贴板窗口被包装为 NSPanel（`window/macos.rs:setup_clipboard_panel`），层级 `PanelLevel::Dock`。
- 预览窗口 `window/preview.rs` 用 `preview_overlay_bounds(monitor)` 与 `clipboard_window_rect(app)`
  计算预览面板摆放，预览面板 480×480、间距 40。
- 设置模型 `src-tauri/src/settings/model.rs` 所有结构体 `#[serde(default)]`，新增字段缺失时回落默认值；
  TS 镜像在 `src/types/settings.ts`，偏好面板 schema 在 `src/pages/Preference/config/preferenceSchema.ts`。
- 前端列表 `src/pages/Clipboard/components/List.tsx` 用 `react-virtuoso` 竖向单列，
  `useClipboardItems` 做 range cache（PAGE_SIZE 30）；方向键上下导航，`Virtuoso.topItemCount` 实现置顶 sticky。
- 卡片 `components/cards/ClipboardCard.tsx`：紧凑灰边框卡片，头部一行是来源应用图标 + 类型文案 + 时间 / hover 快捷动作。
- 列表 payload 已含 `displayCreatedAt`、`width`/`height`（图片）、`size`、`fileEntries`、`colorPreview`、`sourceAppIconPath`。
- 分组栏 `components/Group.tsx` 在搜索框下方，内置范围（全部 / 收藏）+ 分类（文本 / 图片 / 文件）+ 自定义分组，仅图标。
- 颜色只能用 antd token 映射类（`bg-ant-*`、`text-ant-*`），新颜色需先扩 `src/unocss/presetAntdColors.ts`。

## Requirements

### R1 布局模式设置

- 新增设置 `clipboard.window.layout`，取值 `vertical`（现有竖窗）与 `dock`（底部停靠）。
- 同时新增 `clipboard.window.dockHeightPercent`（见 R2），仅在 `layout = dock` 时可编辑。
- Rust 模型、TS 镜像、偏好面板 schema、zh-CN / en-US 文案同步补齐。
- 默认值为 `dock`，新装与已发布老用户统一生效：老 `settings.json` 缺少该字段时由 `serde(default)` 回落到 `dock`，不加版本标记与迁移逻辑；不习惯的用户在偏好设置里切回 `vertical`。

### R2 底部停靠窗口几何（Rust）

- `dock` 模式下窗口贴当前光标所在显示器的工作区底边，宽度等于工作区宽度。
- 高度按比例取值：新增设置 `clipboard.window.dockHeightPercent`（整数百分比，默认 40，允许 25 到 60），
  高度 = 工作区高度 × 百分比，并保证不低于 280 逻辑像素。
- 卡片尺寸随面板高度等比变化：卡片高度 = 列表可用高度，卡片宽度 = 卡片高度（正方形）。
- 必须使用工作区（work area）而不是显示器全尺寸，避免遮挡 macOS Dock / Windows 任务栏。
- `dock` 模式忽略 `position` 设置；`vertical` 模式行为与现状完全一致。
- 切换布局后下一次呼出即生效，不需要重启。

### R3 横向卡片列表（前端）

- `dock` 模式下列表为横向单行虚拟滚动，卡片等宽等高，支持鼠标滚轮横向滚动。
- 方向键左右切换选中项并滚动到可见；上下键不再用于条目导航。
- 置顶条目仍排在最前，视觉上有置顶标识。
- 分页加载、range cache、hover 预览、Space 预览、快捷键粘贴（修饰键 + 数字）、右键原生菜单、拖出等现有行为全部保留。

### R4 Paste 风格卡片

- 卡片圆角大、带阴影、白底（深色模式下深底），顶部一条按类型着色的标题带：
  标题带左侧为类型名 + 相对时间，右上角为来源应用图标。
- 类型配色（antd 调色板 token）：
  文本 gold、链接 green、邮箱 cyan、颜色 purple（颜色值本身在主体整块展示，避免浅色标题带白字不可读）、HTML / RTF orange、图片 red、文件 blue、路径 geekblue。
- 卡片主体：文本显示多行摘要；图片填充展示缩略图；文件显示图标列表或单图预览；颜色显示大色块。
- 卡片底部元信息：文本显示字符数；图片显示 `宽 × 高`；文件显示文件数；链接显示域名。
- 选中态为主色描边；hover 显示现有快捷动作栏。

### R5 顶栏

- `dock` 模式顶栏一行：左侧搜索框，中间分组标签（带名字与彩色圆点 / 图标），右侧固定、更多菜单。
- 搜索、Cmd/Ctrl+F 聚焦、Cmd/Ctrl+Q 切换范围、Tab 切换自定义分组等快捷键保持不变。

### R6 预览窗口适配

- `dock` 模式下 hover / Space 预览面板显示在卡片正上方，不被底部面板遮挡且不超出工作区。

### R7 平台

- macOS 与 Windows 同步实现；窗口层级、失焦隐藏、键盘钩子在 `dock` 模式下行为与现状一致。

## Acceptance Criteria

- [x] AC1 设置里能在「竖向窗口」与「底部停靠」之间切换，切换后下次呼出生效，无需重启。
- [x] AC2 `dock` 模式下窗口贴工作区底边、整宽，不遮挡 Dock / 任务栏；多显示器时出现在光标所在屏。
- [x] AC2b 修改高度百分比后下次呼出面板高度随之变化，卡片等比缩放；超出 25 到 60 的值被 Rust 拒绝。
- [x] AC3 `vertical` 模式下窗口位置、尺寸、记忆行为与改动前一致。
- [x] AC4 `dock` 模式下卡片横向排列，滚轮与左右方向键均可浏览，选中卡片始终滚入可见区域。
- [x] AC5 文本 / 链接 / 邮箱 / 颜色 / HTML / 图片 / 文件各类条目在 `dock` 模式下标题带颜色与元信息符合 R4。
- [x] AC6 深色模式下卡片、标题带、面板背景对比度正常。
- [x] AC7 hover 预览、Space 预览、数字快捷粘贴、右键菜单、拖出、删除、收藏、置顶、备注在 `dock` 模式下均可用。
- [x] AC8 旧版本 `settings.json`（无 `layout` 字段）启动后不报错，直接进入 `dock` 布局；写回后文件包含 `layout: "dock"`。
- [x] AC9 `pnpm lint`、`pnpm tsc`、`cargo clippy -- -D warnings`、`cargo test` 全部通过；两种布局均实际运行验证。

## Out of Scope

- 不改 `vertical` 模式的卡片样式。
- 不做 Paste 的 pinboard 拖拽排序、多选顺序粘贴、Siri Shortcuts 等功能。
- 不做窗口高度鼠标拖拽调整（高度只通过百分比设置控制）。
- 不做 Linux。
