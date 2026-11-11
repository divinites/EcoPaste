//! 模拟粘贴的公共流程：权限检查、让出焦点、注入按键、恢复固定窗口焦点。
//! 剪贴板内容由调用方先写好；这里只负责「把它粘到前台应用」。

use std::time::Duration;

use tauri::AppHandle;

use crate::core::Result;
use crate::window::{self, CLIPBOARD_WINDOW_LABEL};

/// hide / resign 都是主线程异步派发，注入按键前等一拍。
const FOCUS_SETTLE_DELAY: Duration = Duration::from_millis(50);

/// macOS 模拟粘贴依赖辅助功能权限；未授权时 CGEvent 会被系统静默丢弃，用户只看到"没反应"。
/// 这里先弹系统授权请求，再返回可读错误让前端 toast 提示。Windows 无此权限模型，直接放行。
pub async fn ensure_paste_permission(app: &AppHandle) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        if tauri_plugin_macos_permissions::check_accessibility_permission().await {
            return Ok(());
        }

        tauri_plugin_macos_permissions::request_accessibility_permission().await;

        let _ = app;
        Err(crate::core::AppError::localized(
            crate::i18n::commands::Key::AccessibilityRequired,
        ))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(())
    }
}

/// 剪贴板已写好后执行粘贴：未固定时先隐藏剪贴板窗口，固定时让出 key window，
/// 注入系统粘贴按键，固定窗口再取回焦点。
///
/// macOS 上 panel 是 key window，CGEvent ⌘V 若不先 hide / resign 会被 panel 自己吞掉。
pub async fn paste_into_frontmost(app: &AppHandle) -> Result<()> {
    let pinned = window::is_clipboard_window_pinned();
    if pinned {
        // Windows 剪贴板窗口 focusable=false，无需处理。
        #[cfg(target_os = "macos")]
        if let Err(err) = window::macos::resign_clipboard_panel_key(app) {
            log::warn!("resign clipboard panel key before paste failed: {err:?}");
        }
    } else if let Err(err) = window::hide_window_immediate(app, CLIPBOARD_WINDOW_LABEL) {
        log::warn!("hide clipboard window before paste failed: {err:?}");
    }

    tokio::time::sleep(FOCUS_SETTLE_DELAY).await;
    crate::keystroke::simulate_paste()?;

    // 再等一拍让粘贴按键被目标 App 消费完，避免 make_key 抢回焦点把按键吞回 panel。
    #[cfg(target_os = "macos")]
    if pinned {
        tokio::time::sleep(FOCUS_SETTLE_DELAY).await;
        if let Err(err) = window::macos::make_clipboard_panel_key(app) {
            log::warn!("restore clipboard panel key after paste failed: {err:?}");
        }
    }

    Ok(())
}
