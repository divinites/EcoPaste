//! 记录编辑、转换粘贴、批量操作与粘贴队列命令（薄封装）。

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};

use super::clipboard::{
    hide_clipboard_window_after_copy, item_not_found, mark_item_reused_if_enabled,
};
use crate::clipboard::{
    edit_text_item, ensure_paste_permission, merge_plain_text, paste_into_frontmost, plain_text_of,
    set_paste_queue, write_plain_text, ImageStore, PasteQueueState, TextTransform, WritebackGuard,
};
use crate::core::{AppError, Result};
use crate::db::items::{find_item_by_id, find_items_by_ids};
use crate::db::DatabaseState;
use crate::i18n::commands::Key;
use crate::settings::SettingsStore;

/// 与前端 `TAURI_EVENT.CLIPBOARD_UPDATED` 对应。
const CLIPBOARD_UPDATED_EVENT: &str = "clipboard://updated";

/// 单次批量操作的上限，防止异常请求拼出超长 SQL。
const MAX_BATCH_IDS: usize = 500;

fn validate_ids(ids: &[String]) -> Result<()> {
    if ids.len() > MAX_BATCH_IDS {
        return Err(AppError::Clipboard(format!(
            "too many items in one batch: {}",
            ids.len()
        )));
    }
    Ok(())
}

fn emit_items_changed(app: &AppHandle) {
    if let Err(err) = app.emit(
        CLIPBOARD_UPDATED_EVENT,
        serde_json::json!({ "imported": true }),
    ) {
        log::warn!("emit {CLIPBOARD_UPDATED_EVENT} after batch change failed: {err}");
    }
}

/// 取记录的完整可编辑纯文本（不做脱敏）：富文本取纯文本表示，文件取路径列表。
#[tauri::command]
pub async fn get_clipboard_item_text(db: State<'_, DatabaseState>, id: String) -> Result<String> {
    let pool = db.pool().await;
    let item = find_item_by_id(&pool, &id)
        .await?
        .ok_or_else(|| item_not_found(&id))?;

    plain_text_of(&item).ok_or_else(|| AppError::localized(Key::ItemNotEditable))
}

/// 编辑文本记录内容；成功后前端按 id 重新拉取列表视图。
#[tauri::command]
pub async fn update_clipboard_item_content(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    id: String,
    content: String,
) -> Result<()> {
    let pool = db.pool().await;
    let capture = app.state::<SettingsStore>().snapshot().clipboard.capture;
    edit_text_item(&pool, &capture, &id, &content).await
}

/// 把文本记录按 `transform` 转换后粘贴到前台应用；历史记录本身不变。
#[tauri::command]
pub async fn paste_clipboard_item_transformed(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    guard: State<'_, Arc<WritebackGuard>>,
    id: String,
    transform: TextTransform,
) -> Result<()> {
    ensure_paste_permission(&app).await?;

    let pool = db.pool().await;
    let item = find_item_by_id(&pool, &id)
        .await?
        .ok_or_else(|| item_not_found(&id))?;
    let text = plain_text_of(&item).ok_or_else(|| AppError::localized(Key::ItemNotEditable))?;
    let transformed = transform.apply(&text)?;

    write_plain_text(guard.inner().as_ref(), transformed)?;
    paste_into_frontmost(&app).await?;

    if let Err(err) = mark_item_reused_if_enabled(&app, &pool, &id, item.kind).await {
        log::warn!("mark clipboard item reused after transformed paste failed: {err}");
    }
    Ok(())
}

/// 批量删除记录并清理图片文件，返回删除条数。保护规则（收藏 / 置顶）由前端在提交前过滤。
#[tauri::command]
pub async fn delete_clipboard_items(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    ids: Vec<String>,
) -> Result<u64> {
    validate_ids(&ids)?;
    let pool = db.pool().await;
    let outcome = crate::db::items::delete_items(&pool, &ids).await?;

    for file_name in &outcome.image_files {
        if let Err(err) = store.remove(file_name) {
            log::warn!("remove deleted image {file_name} failed: {err}");
        }
    }

    emit_items_changed(&app);
    Ok(outcome.removed)
}

/// 批量设置收藏状态，返回受影响条数。
#[tauri::command]
pub async fn set_clipboard_items_favorite(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    ids: Vec<String>,
    favorite: bool,
) -> Result<u64> {
    validate_ids(&ids)?;
    let pool = db.pool().await;
    let changed = crate::db::items::set_items_favorite(&pool, &ids, favorite).await?;

    emit_items_changed(&app);
    Ok(changed)
}

/// 批量移动到分组；`group_id = None` 表示移出分组。
#[tauri::command]
pub async fn update_clipboard_items_group(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    ids: Vec<String>,
    group_id: Option<String>,
) -> Result<u64> {
    validate_ids(&ids)?;
    let pool = db.pool().await;
    if let Some(group_id) = group_id.as_deref() {
        let exists = crate::db::groups::list_groups(&pool)
            .await?
            .iter()
            .any(|group| group.id == group_id);
        if !exists {
            return Err(AppError::localized(Key::GroupNotFound));
        }
    }

    let changed = crate::db::items::update_items_group(&pool, &ids, group_id.as_deref()).await?;

    emit_items_changed(&app);
    Ok(changed)
}

/// 按给定顺序合并多条记录的纯文本，取不到任何文本时报错。
async fn merged_text(db: &DatabaseState, ids: &[String]) -> Result<String> {
    validate_ids(ids)?;
    let pool = db.pool().await;
    let items = find_items_by_ids(&pool, ids).await?;

    merge_plain_text(&items).ok_or_else(|| AppError::localized(Key::NothingToMerge))
}

/// 合并多条记录为一段纯文本写入剪贴板（图片跳过）。
#[tauri::command]
pub async fn copy_clipboard_items_merged(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    guard: State<'_, Arc<WritebackGuard>>,
    ids: Vec<String>,
) -> Result<()> {
    let text = merged_text(&db, &ids).await?;
    write_plain_text(guard.inner().as_ref(), text)?;

    if app
        .state::<SettingsStore>()
        .snapshot()
        .clipboard
        .content
        .copy_then_hide_window
    {
        hide_clipboard_window_after_copy(&app);
    }
    Ok(())
}

/// 合并多条记录为一段纯文本并粘贴到前台应用。
#[tauri::command]
pub async fn paste_clipboard_items_merged(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    guard: State<'_, Arc<WritebackGuard>>,
    ids: Vec<String>,
) -> Result<()> {
    ensure_paste_permission(&app).await?;

    let text = merged_text(&db, &ids).await?;
    write_plain_text(guard.inner().as_ref(), text)?;
    paste_into_frontmost(&app).await
}

/// 读取粘贴队列状态。
#[tauri::command]
pub async fn get_clipboard_paste_queue(app: AppHandle) -> Result<PasteQueueState> {
    Ok(crate::clipboard::paste_queue_state(&app))
}

/// 用 `ids`（粘贴顺序）替换粘贴队列；空数组清空队列。
#[tauri::command]
pub async fn set_clipboard_paste_queue(
    app: AppHandle,
    ids: Vec<String>,
) -> Result<PasteQueueState> {
    validate_ids(&ids)?;
    Ok(set_paste_queue(&app, ids))
}
