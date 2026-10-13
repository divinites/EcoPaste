use crate::i18n::keys::CommandKey as Key;

/// 返回简体中文 Tauri 命令错误根因文案。
pub fn label(key: Key) -> &'static str {
    match key {
        Key::AccessibilityRequired => {
            "需要在「系统设置 → 隐私与安全性 → 辅助功能」里允许 EcoPaste，才能模拟粘贴"
        }
        Key::DragSourceFilesMissing => "拖拽源文件已不存在",
        Key::DragImageMissing => "图片文件已不存在",
        Key::DragTextEmpty => "文本内容为空",
        Key::ExternalUrlUnsupported => "只能打开 http 或 https 开头的链接",
    }
}
