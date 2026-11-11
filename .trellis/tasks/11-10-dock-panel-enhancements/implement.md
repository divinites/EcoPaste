# 执行计划：底部停靠面板增强

按顺序执行；每个阶段结束跑对应校验，再进入下一阶段。

## 阶段 A：设置与显示器解析（R4，后续阶段的基础）

- [ ] `settings/model.rs`：新增 `DockMonitor` 枚举与 `Window.dock_monitor`，更新 `Default`，从 `settings/mod.rs` 导出。
- [ ] `window/position.rs`：引入 `DockTarget`、`DockHeight`；`apply_dock` 改签名；实现 `Primary` / `ActiveWindow` / `Current` 解析与回落。
- [ ] 平台函数 `active_window_center()`：macOS（`NSScreen::mainScreen`，补 `NSScreen` feature）、Windows（`GetForegroundWindow` + `GetWindowRect`）。
- [ ] `dock_geometry` 支持 `LogicalPx`；新增 `height_to_percent`；补单元测试（像素高度钳制、百分比往返、边界 25 / 60）。
- [ ] `window/mod.rs:apply_clipboard_window_layout` 改用 `DockTarget::Strategy(dock_monitor)`。
- [ ] 前端：`types/settings.ts`、`constants/clipboardLayout.ts`、`preferenceSchema.ts`、zh-CN / en-US `preferences.json`；检查设置默认值镜像处是否需要补字段。
- [ ] 校验：`cargo fmt --check && cargo clippy -- -D warnings && cargo test`，`pnpm lint`。

## 阶段 B：可见时重排（R1）

- [ ] `window/mod.rs`：`relayout_clipboard_window_if_visible`，抽出 vertical 分支供两处复用。
- [ ] `commands/settings.rs`：`touches_clipboard_layout` 判断 + 调用；`apply_reset_side_effects` 同步调用。
- [ ] 校验：同阶段 A 的 Rust 命令。

## 阶段 C：拖拽调高（R3）

- [ ] `window/mod.rs:resize_dock`（非 dock 报错、应用像素高度、commit 时写设置并广播）。
- [ ] `commands/`：`resize_clipboard_dock` 薄命令，注册到 invoke handler；命令名常量 Rust / `src/constants/` / `src/commands` 封装同步。
- [ ] 前端 `DockResizeHandle.tsx`，挂到 `pages/Clipboard/index.tsx` 的 dock 分支；拖拽开始关闭预览。
- [ ] 校验：Rust 三件套 + `pnpm lint`。

## 阶段 D：滑入 / 滑出（R2）

- [ ] grep 全部剪贴板 `hide_window` 调用点，列表归类为“动画隐藏”或“立即隐藏（粘贴路径）”。
- [ ] `window/mod.rs`：`HIDE_GENERATION` / `PENDING_HIDE`、延迟隐藏、`hide_window_immediate`、`show_window` 取消、`toggle_window` 判定；仅 dock 布局启用。
- [ ] visibility payload 增 `instant`，前端 `WindowVisibilityPayload` 类型同步。
- [ ] 前端 `hooks/useDockPanelTransition.ts` + 根容器过渡类（含 `motion-reduce`）。
- [ ] 为延迟隐藏的取消判定补可单测的纯逻辑测试（generation 比较）。
- [ ] 校验：Rust 三件套 + `pnpm lint`。

## 阶段 E：卡片常显序号（R5）

- [ ] `ClipboardCard.tsx` dock 底栏右侧徽标，悬停让位给快捷动作；修饰键文案复用现有工具。
- [ ] 校验：`pnpm lint`。

## 阶段 F：实操验证（macOS）

先按记忆里的开发环境约定：退出已安装的 EcoPaste，再 `npx pnpm tauri dev`。

- [ ] AC1–AC2：固定窗口后改高度、切布局，观察即时重排。
- [ ] AC3–AC6：呼出 / Esc / 点击外部 / 双击粘贴 / 滑出中再呼出 / 减弱动态效果。
- [ ] AC7：拖拽上边缘到上下边界，松手后核对偏好设置数值与下次呼出高度。
- [ ] AC8–AC9：三种显示器策略（单屏时至少验证 `primary` 与回落不报错）；删字段启动。
- [ ] AC10–AC11：序号徽标与快捷粘贴一致；切到 vertical 回归。
- [ ] 处理 design.md 第 11 节列出的阴影、首帧闪烁、底边抖动问题。

## 评审门与回滚点

- 阶段 A、B 完成后是第一个可独立提交点（纯几何与设置，风险最低）。
- 阶段 D 风险最高：若取消竞态或首帧闪烁无法在本轮解决，回滚阶段 D 的 Rust 改动，保留前端滑入，向用户说明后再定。
- 最后一轮跑 `trellis-check` 做全量检查，再更新 `.trellis/spec/backend/settings-window-platform.md` 与前端组件规范。

## 交付时必须说明

- Windows 路径（`active_window_center`、延迟隐藏与鼠标钩子）仅通过 `cfg` 隔离与代码审阅保证，未实测。
- 单显示器环境下无法完整验证 AC8 时，如实标注哪些组合未验证。
