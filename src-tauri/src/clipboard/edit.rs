//! 编辑文本记录：用户修改后的内容统一按纯文本保存，重新计算指纹、摘要、子类型与敏感标记。

use sqlx::SqlitePool;

use super::detect::detect_text_sub_kind;
use super::ingest::make_summary;
use super::secrets::contains_secret;
use crate::core::{AppError, Result};
use crate::db::items::{
    content_hash, content_hash_exists_elsewhere, find_item_by_id, update_item_text_content,
    TextContentUpdate,
};
use crate::db::models::ClipboardKind;
use crate::i18n::commands::Key;
use crate::settings::Capture;

/// 把文本记录的内容替换为 `content`。HTML / RTF 记录编辑后变为纯文本记录；
/// 与其它记录内容重复、为空或超过文本大小上限时拒绝。
pub async fn edit_text_item(
    pool: &SqlitePool,
    capture: &Capture,
    id: &str,
    content: &str,
) -> Result<()> {
    let item = find_item_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::localized(Key::ClipboardItemNotFound))?;
    if item.kind != ClipboardKind::Text {
        return Err(AppError::localized(Key::ItemNotEditable));
    }

    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(AppError::localized(Key::ItemContentEmpty));
    }
    if capture
        .max_text_bytes()
        .is_some_and(|limit| content.len() as u64 > limit)
    {
        return Err(AppError::localized(Key::ItemContentTooLarge));
    }

    let hash = content_hash(ClipboardKind::Text, content);
    if hash == item.content_hash && item.sub_kind == detect_text_sub_kind(trimmed) {
        return Ok(());
    }
    if content_hash_exists_elsewhere(pool, &hash, id).await? {
        return Err(AppError::localized(Key::ItemContentDuplicate));
    }

    let summary = make_summary(trimmed);
    update_item_text_content(
        pool,
        id,
        &TextContentUpdate {
            content,
            content_hash: &hash,
            search_text: content,
            summary: summary.as_deref(),
            sub_kind: detect_text_sub_kind(trimmed),
            size: content.len() as i64,
            is_sensitive: item.is_sensitive || contains_secret(content),
        },
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::items::insert_item;
    use crate::db::models::{ClipboardSubKind, Platform};
    use crate::db::test_support::memory_pool;
    use chrono::Utc;

    fn html_item(id: &str, content: &str) -> crate::db::models::ClipboardItem {
        crate::db::models::ClipboardItem {
            id: id.to_owned(),
            kind: ClipboardKind::Text,
            sub_kind: Some(ClipboardSubKind::Html),
            group_id: None,
            source_app_id: None,
            content_hash: content_hash(ClipboardKind::Text, content),
            content: content.to_owned(),
            search_text: Some("plain".to_owned()),
            summary: Some("plain".to_owned()),
            file_types: None,
            size: None,
            width: None,
            height: None,
            use_count: 1,
            is_favorite: false,
            is_pinned: false,
            is_sensitive: false,
            platform: Platform::Macos,
            note: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            source_app_name: None,
            source_app_icon_file: None,
            source_app_icon_path: None,
            image_thumbnail_path: None,
            file_entries: None,
            files_preview_kind: None,
            available_actions: Vec::new(),
            color_preview: None,
            display_created_at: String::new(),
            char_count: None,
        }
    }

    #[tokio::test]
    async fn edit_turns_rich_text_into_plain_text_and_redetects() {
        let pool = memory_pool().await;
        insert_item(&pool, &html_item("a", "<b>plain</b>"))
            .await
            .unwrap();

        edit_text_item(&pool, &Capture::default(), "a", " https://example.com ")
            .await
            .unwrap();

        let item = find_item_by_id(&pool, "a").await.unwrap().unwrap();
        assert_eq!(item.content, " https://example.com ");
        assert_eq!(item.sub_kind, Some(ClipboardSubKind::Url));
        assert_eq!(item.summary.as_deref(), Some("https://example.com"));
    }

    #[tokio::test]
    async fn edit_rejects_empty_and_duplicate_content() {
        let pool = memory_pool().await;
        insert_item(&pool, &html_item("a", "one")).await.unwrap();
        insert_item(&pool, &html_item("b", "two")).await.unwrap();

        assert!(edit_text_item(&pool, &Capture::default(), "a", "  ")
            .await
            .is_err());
        assert!(edit_text_item(&pool, &Capture::default(), "a", "two")
            .await
            .is_err());
    }
}
