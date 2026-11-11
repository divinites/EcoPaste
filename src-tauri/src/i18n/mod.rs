//! Rust 侧用户可见文案。
//!
//! 仅放会直接展示给用户的短文案；日志与内部错误上下文不走这里。

pub mod clipboard_menu;
pub mod commands;
mod en_us;
mod keys;
pub mod tray;
mod zh_cn;

use std::sync::atomic::{AtomicU8, Ordering};

use tauri::{AppHandle, Manager};

use crate::settings::{Language, SettingsStore};

/// 设置中当前界面语言的进程级镜像：错误序列化时拿不到 `AppHandle`，由 `SettingsStore` 每次落盘后同步。
static ACTIVE_LANGUAGE: AtomicU8 = AtomicU8::new(0);

/// 同步当前界面语言镜像，供 [`active_language`] 读取。
pub fn set_active_language(lang: Language) {
    let value = match lang {
        Language::ZhCN => 0,
        Language::EnUS => 1,
    };
    ACTIVE_LANGUAGE.store(value, Ordering::Relaxed);
}

/// 读取最近一次同步的界面语言；设置尚未加载时为默认语言。
pub fn active_language() -> Language {
    match ACTIVE_LANGUAGE.load(Ordering::Relaxed) {
        1 => Language::EnUS,
        _ => Language::ZhCN,
    }
}

/// 读取当前设置语言；设置尚未初始化时回落到默认语言。
pub fn current_language(app: &AppHandle) -> Language {
    app.try_state::<SettingsStore>()
        .map(|s| s.snapshot().appearance.language)
        .unwrap_or_default()
}
