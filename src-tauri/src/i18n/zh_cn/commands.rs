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
        Key::ClipboardItemNotFound => "找不到这条剪贴板记录",
        Key::StorageDirNotCurrentEnv => "目标目录不是当前环境的 EcoPaste 数据目录",
        Key::StorageDirInvalid => "目标 EcoPaste 数据目录已存在且不是有效数据目录",
        Key::StorageDirsNested => "新旧数据目录不能互相包含",
        Key::LegacyImportTypeRequired => "请至少选择一种导入类型",
        Key::LegacyDatabaseMissing => "未找到可导入的旧版数据库",
        Key::GroupNameEmpty => "分组名称不能为空",
        Key::GroupNameTooLong => "分组名称不能超过 32 个字符",
        Key::GroupIconTooLarge => "SVG 图标不能超过 256 KB",
        Key::GroupIconInvalid => "请选择有效的 SVG 图标",
        Key::GroupIconUnsafe => "SVG 图标不能包含脚本或 foreignObject",
        Key::SvgFileRequired => "请选择 SVG 文件",
        Key::SvgFileUnreadable => "无法读取 SVG 文件",
        Key::SvgFileNotUtf8 => "SVG 文件必须是 UTF-8 文本",
        Key::GroupNotFound => "分组不存在",
        Key::BackupPasswordRequired => "请输入备份密码",
        Key::BackupPasswordTooShort => "备份密码至少需要 8 个字符",
        Key::BackupPlainWithPassword => "明文备份不应包含密码",
        Key::BackupTargetRequired => "请选择备份保存位置",
        Key::BackupExtensionInvalid => "备份文件后缀必须是 .ecopastebak",
        Key::BackupFileRequired => "请选择 .ecopastebak 备份文件",
        Key::BackupInvalidFile => "不是有效的 EcoPaste 备份文件",
        Key::BackupHeaderInvalid => "备份文件头无效",
        Key::BackupEncryptionUnsupported => "暂不支持该备份加密格式",
        Key::BackupPasswordWrong => "备份密码不正确或文件已损坏",
        Key::BackupManifestMissing => "备份文件缺少 manifest.json",
        Key::BackupDatabaseMissing => "备份文件缺少历史数据库",
        Key::BackupSettingsMissing => "备份文件缺少设置文件",
        Key::BackupVersionUnsupported => "暂不支持该备份格式版本",
        Key::FilterPatternInvalid => "排除规则不是有效的正则表达式",
    }
}
