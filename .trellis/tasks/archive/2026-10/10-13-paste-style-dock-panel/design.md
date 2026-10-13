# 技术设计：Paste 风格底部停靠面板

## 1. 边界

| 层 | 负责 | 不负责 |
| --- | --- | --- |
| Rust `settings/` | `layout` / `dockHeightPercent` 模型、默认值、校验 | UI 文案 |
| Rust `window/` | dock 几何计算、最小尺寸切换、窗口状态存档隔离、预览面板首选朝向 | 卡片渲染 |
| Rust `commands/clipboard.rs` | 列表 payload 新增 `charCount` | 字符串格式化 |
| 前端 `pages/Clipboard` | 按 `layout` 切换横向 / 竖向渲染、卡片变体、键位映射 | 窗口定位、数据副本 |
| 前端 `pages/Preference` | 两个新设置项的 schema 与文案 | 校验逻辑（Rust 拒绝越界值） |

## 2. 设置契约

Rust `settings/model.rs`：

```rust
#[serde(rename_all = "camelCase")]
pub enum ClipboardWindowLayout { Vertical, #[default] Dock }

pub struct Window {
    pub layout: ClipboardWindowLayout,     // 默认 Dock
    pub dock_height_percent: u8,           // 默认 40
    ...现有字段
}
```

- `store.rs:validate_settings` 新增 `dock_height_percent` 范围校验 25..=60，越界返回 `AppError::Other("dock height percent must be between 25 and 60")`。
- TS 镜像 `src/types/settings.ts`：`layout: ClipboardWindowLayout`（`"vertical" | "dock"`）、`dockHeightPercent: number`。
- 常量：`src/constants/clipboardLayout.ts` 导出 `CLIPBOARD_WINDOW_LAYOUT`、`DOCK_HEIGHT_PERCENT_MIN/MAX`；Rust 侧对应 `settings::DOCK_HEIGHT_PERCENT_RANGE`。
- 偏好面板 `preferenceSchema.ts` 的 `window` section：
  - `window.layout`：segmented，选项 `vertical` / `dock`，放在 `window.position` 之前。
  - `window.dockHeightPercent`：number，`min 25`、`max 60`、`suffixKey: "percent"`，`parentId: "window.layout"`，`disabledWhen: layout !== "dock"`。
  - `window.position`：`disabledWhen: layout === "dock"`（dock 下忽略）。
- 文案：`src/locales/{zh-CN,en-US}/preferences.json` 新增 `settings.window.layout.{title,description,options.vertical,options.dock}`、`settings.window.dockHeightPercent.{title,description}`，以及 `numberSuffix.percent`（若尚无）。

旧 `settings.json` 无这两个字段时经 `#[serde(default)]` 得到 `Dock` / `40`，下一次 `SettingsStore::update` 写盘即补齐；不加迁移代码。

## 3. 窗口几何（Rust）

### 3.1 dock 定位

`window/position.rs` 新增：

```rust
pub fn apply_dock(window: &WebviewWindow, height_percent: u8) -> Result<()>
```

- 显示器取光标所在屏（复用 `monitor_from_cursor`），区域用 `monitor.work_area()`（Tauri 2.11 提供，物理像素，已扣除 Dock / 任务栏）。
- 高度（物理）= `max(work_area.height * percent / 100, 280 * scale_factor)`，再取 `min(work_area.height)`。
- `set_size(PhysicalSize { width: work_area.width, height })`，`set_position(work_area.x, work_area.y + work_area.height - height)`。
- Windows 平台宽度减 1，与 `preview.rs:preview_window_size` 同样避免精确全屏触发勿扰。

### 3.2 show 流程

`window/mod.rs:apply_clipboard_window_layout`：

```text
layout == Dock:
    window.set_min_size(None)
    position::apply_dock(window, dock_height_percent)
layout == Vertical:
    window.set_min_size(Some(LogicalSize 360×600))   // 与 tauri.conf.json 一致
    restore_window_state + 现有 position 逻辑（不变）
```

`tauri.conf.json` 保持 `minHeight: 600`；dock 模式在运行时解除最小尺寸，避免 `set_size` 被钳到 600。

### 3.3 状态存档隔离

`window/mod.rs:save_all_window_states`：当 label 为剪贴板窗口且当前 `layout == Dock` 时跳过保存，
避免整宽几何写入 `window-state.json` 污染 `vertical` 的 remember 尺寸。

### 3.4 预览面板朝向

`window/preview.rs:resolve_panel_rect` 增加参数 `prefer_top: bool`：

- `prefer_top == true` 时候选顺序为 Top → Right → Left → Bottom，否则保持现有顺序。
- `show_clipboard_preview` 从 `SettingsStore` 快照读取 `layout == Dock` 决定 `prefer_top`。
- 放不下时仍走现有 `clamp_rect` 兜底；不改预览面板尺寸。
- 单元测试补一条：dock 场景（source 在底部、上方空间充足）返回 `Top`。

## 4. 列表 payload

`commands/clipboard.rs` 列表路径（`attach_display_created_at` 旁）新增 `attach_char_count`：
text 类条目在清空 `content` 之前写入 `char_count = content.chars().count()`；
`db::models::ClipboardItem` 加 `#[serde(skip_serializing_if = "Option::is_none")] pub char_count: Option<usize>`，
`sqlx::FromRow` 用 `#[sqlx(skip)]`（与 `display_created_at` 等已有派生字段做法一致，实现时对齐现有写法）。
TS：`charCount?: number`。

## 5. 前端结构

### 5.1 布局分支

`pages/Clipboard/index.tsx` 读取 `settingsState.clipboard.window.layout`：

```text
vertical: Header / Group / List / Footer          （现状，不动）
dock:     DockHeader / List orientation=horizontal / Footer
```

`DockHeader`（新文件 `components/DockHeader.tsx`）：一行 flex，左 logo + `SearchInput`（`w-56`），
中间 `Group variant="tab"`（`flex-1 min-w-0`），右侧固定按钮 + 更多菜单。
搜索防抖、显隐钩子等逻辑从 `Header.tsx` 抽到 `hooks/useClipboardHeaderActions.ts` 供两种 Header 共用。

### 5.2 Group 变体

`Group.tsx` 新增 `variant: "icon" | "tab"`（默认 icon）：

- `tab`：按钮渲染图标 + 名称，`rounded-full px-3 h-7`，选中态 `bg-ant-primary text-ant-light-solid`，
  容器 `overflow-x-auto` 横向滚动，不做溢出“更多”菜单；新增按钮固定在最右。
- 溢出测量（`commitVisibleCustomGroupCount` 一族）只在 `icon` 变体执行。
- 键位：`tab` 变体下 `ArrowUp/ArrowDown` 切分类，`ArrowLeft/ArrowRight` 不处理（交给列表）；`icon` 变体保持现状。

### 5.3 List 横向模式

`List.tsx` 新增 prop `orientation: "vertical" | "horizontal"`：

| 关注点 | vertical（现状） | horizontal |
| --- | --- | --- |
| Virtuoso | 现状 | `horizontalDirection`，`topItemCount = 0` |
| 条目外层 | `px-3 pt-3` | `h-full pl-4 py-3`，最后一项 `pr-4` |
| 导航键 | ArrowUp / ArrowDown | ArrowLeft / ArrowRight |
| 卡片 | `ClipboardCard variant="compact"` | `ClipboardCard variant="dock"` |
| 滚轮 | 原生 | `onWheel`：`deltaY` 非零且 `deltaX` 为零时 `scrollLeft += deltaY` |
| 占位骨架 | 现状 | 正方形骨架 |

`VirtuosoScroller` 增加 `horizontal` prop，初始化时把 overlayscrollbars viewport 的 `overflowX/overflowY` 变量按方向设置。
其余键盘、菜单、预览、拖出、快捷动作逻辑不复制，不分叉。

### 5.4 卡片变体

`ClipboardCard.tsx` 新增 `variant: "compact" | "dock"`，根节点事件绑定共用；`dock` 时渲染：

```text
┌──────────────────────────────┐
│ ■ 标题带：类型名 / 时间   [App] │  h-14，bg 按类型着色，文字 text-ant-light-solid
├──────────────────────────────┤
│ 主体（flex-1 overflow-hidden） │  TextCard line-clamp 动态（按高度）；ImageCard object-cover 填充；
│                              │  FilesCard 列表或单图；颜色子类型整块填充 colorPreview
├──────────────────────────────┤
│ 底部元信息（h-8，text-ant-tertiary）│  charCount / 宽×高 / 文件数 / 域名
└──────────────────────────────┘
外框：rounded-4 shadow-md bg-ant-container；选中 ring-2 ring-ant-primary；置顶 / 敏感水印沿用
```

类型配色集中在新文件 `components/cards/cardTheme.ts`：

| typeKey | 标题带类 |
| --- | --- |
| text | `bg-ant-gold-6` |
| url | `bg-ant-green-6` |
| email | `bg-ant-cyan-6` |
| html / rtf | `bg-ant-orange-6` |
| path | `bg-ant-geekblue-6` |
| image | `bg-ant-red-5` |
| files | `bg-ant-blue-6` |
| color | `bg-ant-purple-6`（颜色值在主体整块展示，标题带不用它做背景以保证白字对比度） |

这些调色板 token 已由 `presetAntdColors` 注册（`^([a-z]+)-(\d{1,2})$`），不需扩 preset。
hover 快捷动作栏放在标题带右侧（替换时间，与现状同一组件）。

元信息格式化在 `components/cards/cardMeta.ts`：`formatCardMeta(item, t)`，文案键
`clipboard:meta.{characters,dimensions,files,domain}` 补 zh-CN / en-US。URL 域名用 `new URL(summary).hostname`，解析失败不显示。

### 5.5 快捷键提示

`ShortcutList.tsx` 读取 `layout`：dock 下导航行显示 `← / →`，切分类显示 `↑ / ↓`；文案键 `shortcuts.navigate` 改为中性的“导航”，另加 `shortcuts.navigateHorizontal`。

## 6. 数据流

```text
偏好面板 segmented/number ──updateSettings──▶ Rust SettingsStore.update（校验）──emit settings://updated──▶ settingsState 镜像
                                                     │
下一次快捷键呼出 ─▶ show_window ─▶ apply_clipboard_window_layout（读快照）─▶ apply_dock / 现有定位 ─▶ window://visibility
                                                                                                        │
前端 Clipboard/index.tsx 读 settingsState.layout ─▶ 选择 DockHeader + List(horizontal) ◀────────────────┘
```

切换布局时若剪贴板窗口正可见，前端立即换渲染，窗口几何在下一次 show 才应用；这是可接受的瞬态，不做实时重定位。

## 7. 兼容与回滚

- 旧配置文件无新字段：默认 Dock / 40，不报错（AC8）。
- 回滚到旧版本：旧版本 `#[serde(default)]` 忽略未知字段？—— serde 默认不拒绝未知字段（未加 `deny_unknown_fields`），因此降级安全。
- `window-state.json` 不写 dock 几何，降级后 vertical 尺寸不受影响。
- 功能开关即设置项本身：出问题可切回 `vertical`，路径与改动前完全一致。

## 8. 权衡

- **不复制 List / Card 逻辑**而是加 `orientation` / `variant`，文件变大但交互单源；拆分为独立组件会复制约 900 行事件逻辑。
- **预览只改首选朝向**，不改尺寸：小屏幕 40% dock 下上方可能放不下 480 高面板，走现有 clamp 覆盖到面板上方区域，接受。
- **tab 变体横向滚动替代溢出菜单**：避免为变宽标签重写测量逻辑。
- **仅 dock 模式新卡片**：vertical 保持原样，符合 PRD out-of-scope。

## 9. 待实现时核验

- `tauri::Monitor::work_area()` 在锁定的 tauri 2.11.5 可用（`cargo doc` 或 registry 源码确认）。
- `react-virtuoso` 4.18 `horizontalDirection` 与 `scrollIntoView` 在横向模式的行为（需先 `pnpm install`，当前无 `node_modules`）。
- `db::models::ClipboardItem` 派生字段的既有写法（`sqlx(skip)` 或 `default`）。
