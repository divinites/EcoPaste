use crate::i18n::keys::TrayKey as Key;

/// 返回美式英文系统托盘菜单文案。
pub fn label(key: Key) -> &'static str {
    match key {
        Key::Preference => "Preference",
        Key::StartListening => "Start Listening",
        Key::PauseListening => "Pause Listening",
        Key::PauseFiveMinutes => "Pause for 5 Minutes",
        Key::PauseThirtyMinutes => "Pause for 30 Minutes",
        Key::PauseOneHour => "Pause for 1 Hour",
        Key::PauseUntilResumed => "Until Resumed",
        Key::PausedUntil => "Paused until",
        Key::SkipNextCopy => "Ignore Next Copy",
        Key::CancelSkipNextCopy => "Cancel Ignore Next Copy",
        Key::OpenSourceAddress => "Open Source Address",
        Key::CheckForUpdates => "Check for Updates",
        Key::Version => "Version",
        Key::Relaunch => "Relaunch",
        Key::Exit => "Exit",
    }
}
