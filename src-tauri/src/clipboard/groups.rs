//! 自定义分组输入的归一化与校验：名称长度、图标形态与基础安全检查。

use crate::core::{AppError, Result};

pub const DEFAULT_CLIPBOARD_GROUP_ICON: &str = "i-lets-icons:folder";

const MAX_GROUP_NAME_CHARS: usize = 32;

pub const MAX_GROUP_ICON_BYTES: usize = 256 * 1024;

/// 归一化分组名称，保证持久化前去除首尾空白并限制长度。
pub fn normalize_group_name(name: &str) -> Result<String> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(AppError::localized(
            crate::i18n::commands::Key::GroupNameEmpty,
        ));
    }

    if normalized.chars().count() > MAX_GROUP_NAME_CHARS {
        return Err(AppError::localized(
            crate::i18n::commands::Key::GroupNameTooLong,
        ));
    }

    Ok(normalized.to_owned())
}

/// 归一化分组图标；空值回退到默认预设图标，自定义 SVG 做基础校验。
pub fn normalize_group_icon(icon: &str) -> Result<String> {
    let normalized = icon.trim();
    if normalized.is_empty() {
        return Ok(DEFAULT_CLIPBOARD_GROUP_ICON.to_owned());
    }

    if normalized.len() > MAX_GROUP_ICON_BYTES {
        return Err(AppError::localized(
            crate::i18n::commands::Key::GroupIconTooLarge,
        ));
    }

    if normalized.starts_with("<svg") {
        normalize_group_svg(normalized)?;
    }

    Ok(normalized.to_owned())
}

/// 校验自定义 SVG 图标的基础形态。
pub fn normalize_group_svg(icon: &str) -> Result<()> {
    let normalized = icon.trim_start();
    if !normalized.starts_with("<svg") {
        return Err(AppError::localized(
            crate::i18n::commands::Key::GroupIconInvalid,
        ));
    }

    let lower = normalized.to_ascii_lowercase();
    if lower.contains("<script") || lower.contains("<foreignobject") {
        return Err(AppError::localized(
            crate::i18n::commands::Key::GroupIconUnsafe,
        ));
    }

    Ok(())
}
