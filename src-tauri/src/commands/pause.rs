//! 剪贴板监听暂停相关命令（薄封装）。

use tauri::{AppHandle, State};

use crate::clipboard::{ClipboardPauseState, WatcherPause};
use crate::core::Result;

/// 读取当前用户暂停状态，供剪贴板窗口初始化指示器。
#[tauri::command]
pub async fn get_clipboard_pause_state(
    pause: State<'_, WatcherPause>,
) -> Result<ClipboardPauseState> {
    Ok(pause.snapshot())
}

/// 暂停 / 恢复监听：`minutes = None` 恢复，`0` 无限期暂停，其余为暂停分钟数。
#[tauri::command]
pub async fn set_clipboard_pause(
    app: AppHandle,
    minutes: Option<u32>,
) -> Result<ClipboardPauseState> {
    Ok(crate::clipboard::apply_user_pause(&app, minutes))
}

/// 设置 / 取消「忽略下一次复制」。
#[tauri::command]
pub async fn set_clipboard_skip_next(app: AppHandle, skip: bool) -> Result<ClipboardPauseState> {
    Ok(crate::clipboard::apply_skip_next(&app, skip))
}
