//! 模拟系统级粘贴。
//!
//! 写回剪贴板由 `clipboard::write` 负责；本模块只负责「按键模拟」这一步——
//! 配合 watcher 的 `WritebackGuard` 抑制自身写回带来的回环。
//!
//! - macOS：⌘V（CGEvent）
//! - Windows：Ctrl+V（SendInput），原因见 `windows.rs`。

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::simulate_paste;
#[cfg(target_os = "windows")]
pub use windows::simulate_paste;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn simulate_paste() -> crate::core::error::Result<()> {
    Err(anyhow::anyhow!("simulate_paste not implemented on this platform").into())
}

/// 等待用户松开修饰键再注入粘贴：全局快捷键触发时修饰键仍按着，
/// Windows 上会和注入的 Ctrl+V 叠成 Ctrl+Alt+V 等组合。超时后直接返回。
/// macOS 注入的 CGEvent 显式设置了 flags，不受物理修饰键影响，无需等待。
pub async fn wait_for_modifiers_released(timeout: std::time::Duration) {
    #[cfg(target_os = "windows")]
    {
        let started = std::time::Instant::now();
        while windows::modifiers_pressed() && started.elapsed() < timeout {
            tokio::time::sleep(std::time::Duration::from_millis(15)).await;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = timeout;
    }
}
