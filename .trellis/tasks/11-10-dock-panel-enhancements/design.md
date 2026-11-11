# 技术设计：底部停靠面板增强

## 1. 边界

| 层 | 负责 | 不负责 |
| --- | --- | --- |
| Rust `settings/` | `dock_monitor` 模型、默认值 | UI 文案 |
| Rust `window/position.rs` | 显示器解析（策略 / 当前屏）、dock 几何、按像素高度换算百分比 | 动画 |
| Rust `window/mod.rs` | 可见时重排入口、延迟隐藏与取消、拖拽调高入口 | 指针事件 |
| Rust `commands/` | `resize_clipboard_dock` 薄命令；`update_settings` 触发重排 | 几何计算 |
| 前端 `pages/Clipboard` | 滑入滑出的 CSS 过渡、上边缘拖拽手柄、卡片序号徽标 | 窗口尺寸与位置、百分比换算 |
| 前端 `pages/Preference` | `window.dockMonitor` schema 与文案 | 校验 |

## 2. 设置契约（R4）

```rust
#[serde(rename_all = "camelCase")]
pub enum DockMonitor { #[default] Cursor, ActiveWindow, Primary }

pub struct Window {
    pub dock_monitor: DockMonitor,   // 默认 Cursor，#[serde(default)] 兼容旧文件
    ...
}
```

- TS：`src/types/settings.ts` 增 `dockMonitor: DockMonitor`（`"cursor" | "activeWindow" | "primary"`）；
  `src/constants/clipboardLayout.ts` 增 `DOCK_MONITOR` 常量。
- `preferenceSchema.ts`：`window.dockMonitor`，segmented，`parentId: "window.layout"`，
  `disabledWhen: layout !== dock`，排在 `window.dockHeightPercent` 之后。
- 文案：`settings.window.dockMonitor.{title,description,options.cursor,options.activeWindow,options.primary}`，zh-CN / en-US。
- 无迁移：枚举无需范围校验，旧文件缺字段取默认。

## 3. 显示器解析（R1 / R3 / R4）

`position.rs` 把“取哪块屏”从 `apply_dock` 里拆出来：

```rust
pub enum DockTarget {
    /// 呼出时：按设置策略解析。
    Strategy(DockMonitor),
    /// 可见时重排 / 拖拽：留在窗口当前所在显示器。
    Current,
}

pub fn apply_dock(window, target: DockTarget, height: DockHeight) -> Result<()>

pub enum DockHeight { Percent(u8), LogicalPx(f64) }
```

解析规则：

| 目标 | 来源 | 失败回落 |
| --- | --- | --- |
| `Strategy(Cursor)` | 现有 `monitor_from_cursor` | 不定位（现状） |
| `Strategy(Primary)` | `window.primary_monitor()` | Cursor |
| `Strategy(ActiveWindow)` | 平台函数 `active_window_center()` 返回前台窗口中心点，再 `monitor_from_point` | Cursor |
| `Current` | `window.current_monitor()` | Cursor |

`active_window_center()`：

- macOS：`NSScreen::mainScreen()`（系统定义为“当前键盘焦点窗口所在屏”）的 frame 中心，
  由 Cocoa 左下原点换算为左上原点逻辑坐标。必须在 `show` 之前取值，否则面板自身成为 key 后结果失真；
  `apply_clipboard_window_layout` 本就在 show 前执行，满足。需给 `objc2-app-kit` 加 `NSScreen` feature。
- Windows：`GetForegroundWindow` + `GetWindowRect` 取中心点；前台句柄为空或是自身窗口时返回 `None`。

`dock_geometry` 保持纯函数，新增按 `DockHeight` 取高度的分支；`LogicalPx` 同样钳到
`[max(25% 工作区, 280 逻辑像素), 60% 工作区]`，并提供反向纯函数：

```rust
/// 物理高度 → 整数百分比，四舍五入后钳到 DOCK_HEIGHT_PERCENT_RANGE。
fn height_to_percent(work_area_height: u32, height: u32) -> u8
```

## 4. 可见时重排（R1）

`window/mod.rs` 新增：

```rust
/// 剪贴板窗口可见时按当前设置重排；不可见直接返回。
pub fn relayout_clipboard_window_if_visible(app: &AppHandle) -> Result<()>
```

- dock：`set_min_size(None)` + `apply_dock(Current, Percent(p))`。
- vertical：复用 `apply_clipboard_window_layout` 的 vertical 分支（恢复最小尺寸、存档或默认尺寸、定位策略）。
- 触发点：`commands/settings.rs:update_settings` 增 `touches_clipboard_layout`
  （patch 的 `clipboard.window` 含 `layout` / `dockHeightPercent` / `dockMonitor` 任一键），
  在 `SettingsStore::update` 成功后调用；`reset_settings` 的副作用函数里同样调用。失败只记日志。
- 只改 `dockMonitor` 且窗口可见时：按 `Current` 重排等于无变化，新策略在下次呼出生效；这是有意为之，避免可见面板跳屏。

## 5. 滑入 / 滑出（R2）

窗口是 `transparent: true`，动画在 WebView 内用 `transform: translateY` 完成，两端共用，不写原生动画。

### 5.1 前端

`pages/Clipboard/index.tsx` 的 dock 根容器增加状态 `panelShown`，由 `window://visibility` 驱动：

```text
visible = true            → panelShown = true   （translate-y-0，过渡 180ms ease-out）
visible = false           → panelShown = false  （translate-y-full，过渡 120ms ease-in）
visible = false, instant  → panelShown = false，本次不带过渡
```

- 隐藏后的终态（移出屏幕）就是下次滑入的起点，窗口再次显示的首帧不会先闪出完整面板。
- `motion-reduce:transition-none`；过渡类用 UnoCSS，不写 inline px。
- 监听放进现有可见性订阅方式（`useTauriListen`），抽成 `hooks/useDockPanelTransition.ts`，返回 className。

### 5.2 Rust 延迟隐藏

`window://visibility` payload 增加可选字段 `instant: bool`（缺省 false，前端类型同步）。

`hide_window` 对“剪贴板窗口 + dock 布局”分两种：

```text
hide_window(app, label)                 // 动画隐藏
  1. save state / suppress preview（现状）
  2. emit visibility { visible: false }
  3. generation = HIDE_GENERATION.fetch_add(1) + 1；PENDING_HIDE = true
  4. spawn: sleep 120ms → 若 HIDE_GENERATION 仍等于 generation → 平台 hide + lifecycle::on_hidden

hide_window_immediate(app, label)       // 粘贴路径
  1. 同上 1
  2. HIDE_GENERATION += 1；PENDING_HIDE = false
  3. 平台 hide → emit visibility { visible: false, instant: true } → on_hidden
```

- `show_window`（剪贴板）开头执行 `HIDE_GENERATION += 1; PENDING_HIDE = false`，使未到期的延迟隐藏失效（AC5）。
- `toggle_window` 判断可见性时把 `PENDING_HIDE == true` 视为不可见，滑出期间再按快捷键走 show。
- 延迟期间的重复 `hide_window`（例如 macOS resign-key 与 Esc 先后到达）：`PENDING_HIDE` 已为 true 时直接返回。
- 粘贴相关调用点（`paste_clipboard_item` 及其它“隐藏后立刻注入按键”的路径）改用 `hide_window_immediate`；
  实现时 grep 所有 `hide_window(.., CLIPBOARD_WINDOW_LABEL)` 调用逐一归类。
- `vertical` 布局与非剪贴板窗口：`hide_window` 行为与现状完全一致（不延迟）。
- 延迟常量 `DOCK_HIDE_ANIMATION` 与前端过渡时长分别集中定义，并在注释里互相指明需同步。
- 系统“减弱动态效果”时前端无过渡，但窗口仍晚 120ms 隐藏、期间显示空白透明窗口；可接受，不为此加跨端探测。

## 6. 拖拽调高（R3）

tao 在 macOS 不支持 `start_resize_dragging`，因此不用原生缩放，改为前端手柄 + Rust 命令。

### 6.1 命令

```rust
/// 把停靠面板调整到给定逻辑高度；`commit` 为 true 时换算成百分比写入设置并广播。
/// 返回钳制后的整数百分比。
#[tauri::command]
pub async fn resize_clipboard_dock(app: AppHandle, height: f64, commit: bool) -> Result<u8>
```

- 命令体只做参数校验（`height.is_finite() && height > 0`）并转发 `window::resize_dock`。
- `window::resize_dock`：非 dock 布局返回错误；`apply_dock(Current, LogicalPx(height))`；
  `commit` 时 `SettingsStore::update({clipboard:{window:{dockHeightPercent}}})` + `emit_settings_updated`，
  随后按 `Percent` 再应用一次，使松手后的高度与持久化值严格一致。
- 命令名常量同步到 Rust 命令注册处与 `src/constants/`（按现有命令常量文件约定）。

### 6.2 前端

新组件 `pages/Clipboard/components/DockResizeHandle.tsx`，绝对定位在根容器顶边（`h-1.5 cursor-ns-resize`）：

- `pointerdown`：`setPointerCapture`，记录 `startScreenY` 与 `startHeight = window.innerHeight`，关闭悬浮预览。
- `pointermove`：目标高度 `startHeight + (startScreenY - screenY)`；用 `requestAnimationFrame` 合并，
  每帧至多一次 `resizeClipboardDock(height, false)`，上一帧调用未返回时跳过。
- `pointerup` / `pointercancel`：`resizeClipboardDock(height, true)`，释放捕获。
- 手柄不带 `data-tauri-drag-region`，避免触发窗口拖动。
- 拖拽期间滑入过渡类不参与（transform 不变），窗口尺寸变化由 Rust 驱动，卡片尺寸由现有 `ResizeObserver` 跟随。

失焦隐藏：macOS 面板在拖拽中保持 key；Windows 外部点击钩子只在窗口外按下时触发，指针捕获期间不触发。两端都不需要额外抑制，实现后实测确认。

## 7. 卡片常显序号（R5）

- `ClipboardCard` dock 变体底栏右侧：`hintKey` 存在且未悬停时渲染徽标（`⌘1` / `Ctrl+1` 形式，
  `rounded-1 bg-ant-fill-secondary px-1 font-mono text-xs text-ant-secondary`）；悬停时由 `ClipboardQuickActions` 取代。
- 修饰键文案复用现有快捷键显示工具（`src/utils/shortcut.ts` 一族），不新写平台判断。
- `KeyHint` 与 `hintKey` 的计算（`List.tsx` 中 `index - firstVisibleIndex`）不改；compact 变体不改。

## 8. 数据流

```text
偏好面板 ─updateSettings→ SettingsStore.update ─→ touches_clipboard_layout? ─→ relayout_clipboard_window_if_visible
                                               └→ emit settings://updated ─→ settingsState 镜像

快捷键 ─→ show_window ─→ 取消 pending hide ─→ apply_dock(Strategy(dockMonitor), Percent) ─→ 平台 show
                                             └→ emit visibility{true} ─→ 前端滑入

Esc / 失焦 ─→ hide_window ─→ emit visibility{false} ─→ 前端滑出 ─(120ms)→ 平台 hide
粘贴 ─→ hide_window_immediate ─→ 平台 hide ─→ emit visibility{false, instant}

手柄拖动 ─resize_clipboard_dock(h,false)→ apply_dock(Current, LogicalPx)
手柄松开 ─resize_clipboard_dock(h,true)→ 写 dockHeightPercent ─→ emit settings://updated
```

## 9. 兼容与回滚

- 新字段 `dockMonitor` 缺省即现状；降级到旧版本时 serde 忽略未知字段。
- `window://visibility` 新增字段为可选，旧监听方不受影响。
- 每项增强互相独立：R2 出问题可把 `DOCK_HIDE_ANIMATION` 路径整体退回同步隐藏；R3 移除手柄组件即回退。
- `vertical` 布局代码路径不变。

## 10. 权衡

- **WebView 内动画而非原生窗口动画**：一套实现覆盖两端，代价是隐藏要靠 Rust 延迟 120ms 并处理取消竞态。
- **粘贴不滑出**：保证粘贴延迟不变，代价是两种隐藏观感不一致。
- **拖拽中按像素、松手才落百分比**：拖动跟手，持久化仍是既定的整数百分比契约。
- **可见时重排留在当前屏**：避免在偏好设置里改值时面板跳到偏好窗口所在屏。

## 11. 实现时核验

- macOS 透明 NSPanel 的窗口阴影是否随 WebView 内容位移正确刷新；若残留，dock 布局下关闭面板阴影。
- 立即隐藏后再次呼出的首帧是否闪出旧画面（WebView 隐藏后可能未绘制“移出”状态）；若有，在 show 前多等一帧或由前端在收到 visible 前保持不透明度 0。
- `objc2-app-kit` 的 `NSScreen` feature 与 `mainScreen` 在面板为 nonactivating 时的返回值。
- Windows 侧 `active_window_center`、延迟隐藏与外部点击钩子的交互无法在本机验证，只能保证编译期 `cfg` 隔离与逻辑对称，交付时标注为未实测。
- `resize_clipboard_dock` 高频调用下 `set_size` + `set_position` 是否出现底边抖动；若有，合并为一次 frame 设置。
