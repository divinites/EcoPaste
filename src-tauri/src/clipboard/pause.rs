//! 剪贴板监听的暂停状态。
//!
//! 三种来源互不覆盖：
//! - 用户无限期暂停（托盘「停止监听」）；
//! - 用户定时暂停（到期自动恢复，[`spawn_resume_timer`] 负责刷新托盘并通知前端）；
//! - 内部挂起（数据目录迁移、备份导入期间），用计数 guard，结束后不影响用户状态。
//!
//! 另有一次性的「忽略下一次复制」标记，监听回调读到内容后消费。

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// 暂停状态变更事件，与前端 `TAURI_EVENT.CLIPBOARD_PAUSE_CHANGED` 对应。
pub const CLIPBOARD_PAUSE_CHANGED_EVENT: &str = "clipboard://pause-changed";

/// 定时暂停的最长时长，防止前端传入异常值。
const MAX_PAUSE_MINUTES: u32 = 24 * 60;

#[derive(Debug, Default)]
struct Inner {
    user_paused: AtomicBool,
    until: Mutex<Option<DateTime<Utc>>>,
    internal: AtomicUsize,
    skip_next: AtomicBool,
    timer_generation: AtomicU64,
}

/// 监听暂停开关，`Arc` 共享给监听线程、托盘与命令。
#[derive(Debug, Default, Clone)]
pub struct WatcherPause(Arc<Inner>);

/// 推给前端的暂停快照。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardPauseState {
    pub paused: bool,
    /// 定时暂停的恢复时间；无限期暂停或未暂停时为 `None`。
    pub until: Option<DateTime<Utc>>,
    pub skip_next: bool,
}

impl WatcherPause {
    /// 监听回调是否应跳过本次变更（任一来源处于暂停）。
    pub fn is_paused(&self) -> bool {
        self.0.internal.load(Ordering::Relaxed) > 0 || self.is_user_paused()
    }

    /// 用户层面是否暂停（不含内部挂起），托盘与前端据此展示。
    pub fn is_user_paused(&self) -> bool {
        self.0.user_paused.load(Ordering::Relaxed) || self.paused_until().is_some()
    }

    /// 仍在生效的定时暂停截止时间。
    pub fn paused_until(&self) -> Option<DateTime<Utc>> {
        let until = *self.lock_until();
        until.filter(|until| *until > Utc::now())
    }

    /// 用户无限期暂停 / 恢复；恢复时一并清除定时暂停。
    pub fn set_paused(&self, paused: bool) {
        self.0.user_paused.store(paused, Ordering::Relaxed);
        if !paused {
            *self.lock_until() = None;
            self.0.timer_generation.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// 定时暂停 `minutes` 分钟，返回本次计时的代号供到期回调比对。
    fn pause_for(&self, minutes: u32) -> (DateTime<Utc>, u64) {
        let minutes = minutes.clamp(1, MAX_PAUSE_MINUTES);
        let until = Utc::now() + chrono::Duration::minutes(i64::from(minutes));
        self.0.user_paused.store(false, Ordering::Relaxed);
        *self.lock_until() = Some(until);
        let generation = self.0.timer_generation.fetch_add(1, Ordering::Relaxed) + 1;
        (until, generation)
    }

    /// 内部流程临时挂起监听，guard drop 时恢复，不改动用户暂停状态。
    pub fn suspend(&self) -> WatcherSuspendGuard {
        self.0.internal.fetch_add(1, Ordering::Relaxed);
        WatcherSuspendGuard(self.clone())
    }

    /// 标记忽略下一次剪贴板变更。
    pub fn set_skip_next(&self, skip: bool) {
        self.0.skip_next.store(skip, Ordering::Relaxed);
    }

    /// 消费「忽略下一次复制」标记；返回是否需要忽略本次。
    pub fn take_skip_next(&self) -> bool {
        self.0.skip_next.swap(false, Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> ClipboardPauseState {
        let until = self.paused_until();
        ClipboardPauseState {
            paused: self.0.user_paused.load(Ordering::Relaxed) || until.is_some(),
            until,
            skip_next: self.0.skip_next.load(Ordering::Relaxed),
        }
    }

    fn lock_until(&self) -> std::sync::MutexGuard<'_, Option<DateTime<Utc>>> {
        self.0
            .until
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// [`WatcherPause::suspend`] 的恢复 guard。
pub struct WatcherSuspendGuard(WatcherPause);

impl Drop for WatcherSuspendGuard {
    fn drop(&mut self) {
        self.0 .0.internal.fetch_sub(1, Ordering::Relaxed);
    }
}

/// 用户暂停操作：`None` 恢复监听，`Some(0)` 无限期暂停，`Some(n)` 暂停 n 分钟。
/// 完成后刷新托盘并广播最新状态。
pub fn apply_user_pause(app: &AppHandle, minutes: Option<u32>) -> ClipboardPauseState {
    let Some(pause) = app.try_state::<WatcherPause>() else {
        return ClipboardPauseState {
            paused: false,
            until: None,
            skip_next: false,
        };
    };

    match minutes {
        None => pause.set_paused(false),
        Some(0) => pause.set_paused(true),
        Some(minutes) => {
            let (until, generation) = pause.pause_for(minutes);
            spawn_resume_timer(app.clone(), until, generation);
        }
    }

    log::info!("clipboard watcher pause changed: {minutes:?}");
    notify_pause_changed(app)
}

/// 设置 / 取消「忽略下一次复制」，并广播最新状态。
pub fn apply_skip_next(app: &AppHandle, skip: bool) -> ClipboardPauseState {
    if let Some(pause) = app.try_state::<WatcherPause>() {
        pause.set_skip_next(skip);
    }

    notify_pause_changed(app)
}

/// 刷新托盘文案并广播暂停状态；返回广播的快照。
pub fn notify_pause_changed(app: &AppHandle) -> ClipboardPauseState {
    let state = app
        .try_state::<WatcherPause>()
        .map(|pause| pause.snapshot())
        .unwrap_or(ClipboardPauseState {
            paused: false,
            until: None,
            skip_next: false,
        });

    if let Err(err) = crate::tray::refresh_menu(app) {
        log::warn!("refresh tray menu after pause change failed: {err}");
    }
    if let Err(err) = app.emit(CLIPBOARD_PAUSE_CHANGED_EVENT, &state) {
        log::warn!("emit {CLIPBOARD_PAUSE_CHANGED_EVENT} failed: {err}");
    }

    state
}

/// 定时暂停到期后通知；期间若用户改过暂停状态（代号变化）则不处理。
fn spawn_resume_timer(app: AppHandle, until: DateTime<Utc>, generation: u64) {
    tauri::async_runtime::spawn(async move {
        let wait = (until - Utc::now())
            .to_std()
            .unwrap_or(Duration::ZERO)
            .saturating_add(Duration::from_millis(200));
        tokio::time::sleep(wait).await;

        let Some(pause) = app.try_state::<WatcherPause>() else {
            return;
        };
        if pause.0.timer_generation.load(Ordering::Relaxed) != generation {
            return;
        }

        log::info!("clipboard watcher timed pause expired");
        notify_pause_changed(&app);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_suspend_does_not_touch_user_state() {
        let pause = WatcherPause::default();
        pause.set_paused(true);
        {
            let _guard = pause.suspend();
            assert!(pause.is_paused());
        }
        assert!(pause.is_user_paused());

        pause.set_paused(false);
        {
            let _guard = pause.suspend();
            assert!(pause.is_paused());
            assert!(!pause.is_user_paused());
        }
        assert!(!pause.is_paused());
    }

    #[test]
    fn timed_pause_expires_and_resume_clears_it() {
        let pause = WatcherPause::default();
        pause.pause_for(5);
        assert!(pause.is_paused());
        assert!(pause.paused_until().is_some());

        *pause.lock_until() = Some(Utc::now() - chrono::Duration::seconds(1));
        assert!(!pause.is_paused());

        pause.pause_for(5);
        pause.set_paused(false);
        assert!(!pause.is_paused());
    }

    #[test]
    fn skip_next_is_consumed_once() {
        let pause = WatcherPause::default();
        pause.set_skip_next(true);
        assert!(pause.take_skip_next());
        assert!(!pause.take_skip_next());
    }
}
