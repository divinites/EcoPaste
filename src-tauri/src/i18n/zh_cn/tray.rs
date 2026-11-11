use crate::i18n::keys::TrayKey as Key;

/// 返回简体中文系统托盘菜单文案。
pub fn label(key: Key) -> &'static str {
    match key {
        Key::Preference => "偏好设置",
        Key::StartListening => "开启监听",
        Key::PauseListening => "暂停监听",
        Key::PauseFiveMinutes => "暂停 5 分钟",
        Key::PauseThirtyMinutes => "暂停 30 分钟",
        Key::PauseOneHour => "暂停 1 小时",
        Key::PauseUntilResumed => "直到手动恢复",
        Key::PausedUntil => "已暂停至",
        Key::SkipNextCopy => "忽略下一次复制",
        Key::CancelSkipNextCopy => "取消忽略下一次复制",
        Key::OpenSourceAddress => "开源地址",
        Key::CheckForUpdates => "检查更新",
        Key::Version => "版本",
        Key::Relaunch => "重启应用",
        Key::Exit => "退出应用",
    }
}
