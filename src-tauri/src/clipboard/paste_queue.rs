//! 粘贴队列：用户在列表中选好若干条记录后，每按一次「粘贴下一条」全局快捷键就按顺序粘贴一条。
//! 队列非空时才注册该快捷键，避免平时占用按键。

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{write_to_clipboard, ImageStore, WritebackGuard};
use crate::core::{AppError, Result};
use crate::db::items::find_item_by_id;
use crate::i18n::commands::Key;
use crate::settings::SettingsStore;

/// 队列状态变更事件，与前端 `TAURI_EVENT.CLIPBOARD_PASTE_QUEUE` 对应。
pub const CLIPBOARD_PASTE_QUEUE_EVENT: &str = "clipboard://paste-queue";

/// 等待用户松开快捷键修饰键的上限，超时后仍继续粘贴。
const MODIFIER_RELEASE_TIMEOUT: Duration = Duration::from_millis(800);

#[derive(Debug, Default)]
struct QueueInner {
    ids: VecDeque<String>,
    total: usize,
}

/// 粘贴队列状态，作为 Tauri managed state。
#[derive(Debug, Default)]
pub struct PasteQueue(Mutex<QueueInner>);

/// 推给前端的队列快照。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PasteQueueState {
    pub remaining: usize,
    pub total: usize,
    /// 实际生效的「粘贴下一条」快捷键；为空表示未配置。
    pub shortcut: String,
}

impl PasteQueue {
    fn lock(&self) -> std::sync::MutexGuard<'_, QueueInner> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn is_active(&self) -> bool {
        !self.lock().ids.is_empty()
    }

    fn replace(&self, ids: Vec<String>) {
        let mut inner = self.lock();
        inner.total = ids.len();
        inner.ids = ids.into();
    }

    fn pop(&self) -> Option<String> {
        let mut inner = self.lock();
        let next = inner.ids.pop_front();
        if inner.ids.is_empty() {
            inner.total = 0;
        }
        next
    }

    fn counts(&self) -> (usize, usize) {
        let inner = self.lock();
        (inner.ids.len(), inner.total)
    }
}

/// 当前队列快照。
pub fn paste_queue_state(app: &AppHandle) -> PasteQueueState {
    let (remaining, total) = app
        .try_state::<PasteQueue>()
        .map(|queue| queue.counts())
        .unwrap_or_default();
    let shortcut = app
        .try_state::<SettingsStore>()
        .map(|store| store.snapshot().shortcuts.paste_next)
        .unwrap_or_default();

    PasteQueueState {
        remaining,
        total,
        shortcut,
    }
}

/// 用 `ids`（按粘贴顺序）替换队列；空列表即清空。会按队列状态重新注册全局快捷键。
pub fn set_paste_queue(app: &AppHandle, ids: Vec<String>) -> PasteQueueState {
    if let Some(queue) = app.try_state::<PasteQueue>() {
        queue.replace(ids);
    }

    notify_queue_changed(app)
}

/// 「粘贴下一条」快捷键回调：弹出队首记录写回剪贴板并粘贴到前台应用。
pub async fn paste_next(app: &AppHandle) -> Result<()> {
    super::paste::ensure_paste_permission(app).await?;

    let Some(id) = app.try_state::<PasteQueue>().and_then(|queue| queue.pop()) else {
        notify_queue_changed(app);
        return Err(AppError::localized(Key::PasteQueueEmpty));
    };

    let pool = app.state::<crate::db::DatabaseState>().pool().await;
    let item = match find_item_by_id(&pool, &id).await? {
        Some(item) => item,
        None => {
            // 记录已被删除时跳过，不中断队列。
            notify_queue_changed(app);
            return Ok(());
        }
    };

    let settings = app.state::<SettingsStore>().snapshot();
    let plain = settings.clipboard.content.paste_plain
        && item.kind == crate::db::models::ClipboardKind::Text;
    {
        let store = app.state::<ImageStore>();
        let guard = app.state::<std::sync::Arc<WritebackGuard>>();
        write_to_clipboard(&store, guard.inner().as_ref(), &item, plain)?;
    }

    crate::keystroke::wait_for_modifiers_released(MODIFIER_RELEASE_TIMEOUT).await;
    super::paste::paste_into_frontmost(app).await?;
    notify_queue_changed(app);
    Ok(())
}

/// 重新注册全局快捷键并广播队列状态。
fn notify_queue_changed(app: &AppHandle) -> PasteQueueState {
    if let Some(store) = app.try_state::<SettingsStore>() {
        if let Err(err) = crate::shortcut::apply(app, &store.snapshot().shortcuts) {
            log::warn!("re-register shortcuts after paste queue change failed: {err}");
        }
    }

    let state = paste_queue_state(app);
    if let Err(err) = app.emit(CLIPBOARD_PASTE_QUEUE_EVENT, &state) {
        log::warn!("emit {CLIPBOARD_PASTE_QUEUE_EVENT} failed: {err}");
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_pops_in_order_and_resets_total() {
        let queue = PasteQueue::default();
        queue.replace(vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(queue.counts(), (2, 2));
        assert_eq!(queue.pop().as_deref(), Some("a"));
        assert_eq!(queue.counts(), (1, 2));
        assert_eq!(queue.pop().as_deref(), Some("b"));
        assert_eq!(queue.counts(), (0, 0));
        assert!(!queue.is_active());
    }
}
