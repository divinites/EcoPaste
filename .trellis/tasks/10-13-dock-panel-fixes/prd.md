# 停靠面板修复：双击粘贴权限、滚轮横向滚动、偏好窗口打开

## Goal

用户在已安装的停靠面板版本上反馈三个问题，全部修复并重新构建替换本机。

## Background（已确认事实）

- 用户设置 `clipboard.content.autoPaste = doubleClickPaste`，双击路径代码未变；本机 TCC 数据库里没有任何 EcoPaste 的辅助功能授权记录，`keystroke::simulate_paste` 的 CGEvent 在未授权时被系统静默丢弃，所以双击只写入剪贴板、不粘贴，且没有任何提示。
- `window/macos.rs:handle_reopen` 调用的是模块内 `show_window`（不重建 DestroyWhenIdle 窗口），而非 `super::show_window`；Dock 图标 reopen 时日志稳定报 `window not found: preference`。该缺陷来自上游 f4dac62，与停靠布局无关，但它是用户打开设置的主要路径。
- 停靠列表的滚轮处理 `List.tsx:handleWheel` 未经实机验证。

## Requirements

- R1 粘贴前检查辅助功能权限：未授权时弹出系统授权请求，并返回用户可读错误（走 Rust `i18n/`），不再静默失败。
- R2 Dock 图标 reopen 走 `window::show_window`，能重建并显示偏好窗口。
- R3 停靠面板顶栏增加显眼的「偏好设置」按钮（齿轮），不只藏在「更多」菜单里。
- R4 停靠列表鼠标滚轮纵向滚动量转为横向滚动，实机验证。
- R5 `general.dockIcon = false` 时 Dock 图标不残留：偏好 / 引导 / 更新窗口隐藏或关闭后重新收回 Dock 图标（macOS 显示普通窗口会把进程提升为前台应用）。

## Acceptance Criteria

- [ ] AC1 未授权时双击卡片：弹出系统辅助功能授权对话框，面板 toast 提示需要权限；授权后双击可粘贴到前台应用。
- [ ] AC2 点击 Dock 图标（无可见窗口时）打开偏好窗口，日志无 `window not found`。
- [ ] AC3 停靠顶栏齿轮按钮点击打开偏好窗口。
- [ ] AC4 在停靠面板上滚动鼠标滚轮，卡片横向滚动。
- [ ] AC5 关闭偏好窗口后进程回到 background-only，Dock 中无 EcoPaste 图标。
- [ ] AC6 lint / tsc / clippy / cargo test 通过；release 构建替换本机并实机验证 AC2–AC5。

## Out of Scope

- 不改竖窗模式。
