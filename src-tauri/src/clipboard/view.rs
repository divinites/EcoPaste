//! 剪贴板记录的「视图」组装：把 DB 模型补齐为前端可直接渲染的字段
//! （缩略图 / 图标绝对路径、文件条目、色值预览、脱敏摘要、可用动作、预览 payload）。
//! 命令层只负责取 State 与转发，业务规则集中在这里。

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Datelike, Local, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

use super::{sanitize_css_color, AppIconStore, FileIconStore, ImageStore};
use crate::core::{AppError, Result};
use crate::db::models::{
    ClipboardAction, ClipboardApp, ClipboardItem, ClipboardKind, ClipboardSubKind, FileEntry,
    Platform,
};

/// 列表 / 单条刷新共用的增强参数，调用方在批处理前取一次设置与时间。
pub struct ListItemContext<'a> {
    pub pool: &'a SqlitePool,
    pub image_store: &'a ImageStore,
    pub app_icon_store: &'a AppIconStore,
    pub file_icon_store: &'a FileIconStore,
    pub file_entry_limit: usize,
    pub redact_sensitive: bool,
    pub now: DateTime<Local>,
}

/// 把列表视图记录补齐为前端可直接渲染的形态。
pub async fn enrich_list_item(ctx: &ListItemContext<'_>, item: &mut ClipboardItem) -> Result<()> {
    attach_image_thumbnail_path(ctx.image_store, item).await?;
    attach_source_app_icon_path(ctx.app_icon_store, item);
    attach_file_entries(ctx.pool, ctx.file_icon_store, item, ctx.file_entry_limit).await?;
    attach_color_preview(item);
    attach_display_created_at(item, &ctx.now);
    redact_sensitive_list_item(item, ctx.redact_sensitive);
    item.available_actions = compute_available_actions(item);
    Ok(())
}

/// 偏好页来源应用列表项：在 DB 模型基础上补齐前端可直接渲染的 icon 绝对路径。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardAppView {
    pub id: String,
    pub name: String,
    pub icon_file: Option<String>,
    pub icon_path: Option<String>,
    pub platform: Platform,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub const PREVIEW_FILE_ENTRY_LIMIT: usize = 64;

/// 预览窗口专用 payload：只暴露 Content Viewer 渲染所需的归一化字段。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardPreviewPayload {
    pub id: String,
    pub kind: ClipboardKind,
    pub sub_kind: Option<ClipboardSubKind>,
    pub updated_at: DateTime<Utc>,
    /// 预览窗口展示用纯文本。HTML / RTF 条目返回 `search_text`，不返回富文本源。
    pub text: Option<String>,
    pub image_path: Option<String>,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
    pub size: Option<i64>,
    pub is_sensitive: bool,
    pub image_exists: bool,
    pub files: Vec<ClipboardPreviewFileEntry>,
    pub total_files: usize,
}

/// 预览窗口里的单个文件条目，比列表卡片保留更多文件并带上 size。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardPreviewFileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub is_image: bool,
    pub exists: bool,
    pub size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<String>,
}

/// 为 image 条目补齐缩略图绝对路径，前端可直接渲染。
/// 历史脏数据（非 `<hash>.png`）或缩略图生成失败时降级为 `None`，不影响列表返回。
async fn attach_image_thumbnail_path(store: &ImageStore, item: &mut ClipboardItem) -> Result<()> {
    if item.kind != ClipboardKind::Image {
        return Ok(());
    }

    if validate_image_file_name(&item.content).is_err() {
        item.image_thumbnail_path = None;
        return Ok(());
    }

    let file_name = item.content.clone();
    let thumb_path = store.thumbnail_path(&file_name);
    let thumb_exists = thumb_path.exists();

    let immediate_path = if thumb_exists {
        thumb_path
    } else {
        store.origin_path(&file_name)
    };

    item.image_thumbnail_path = immediate_path.to_str().map(str::to_owned);

    // 缩略图不存在时后台异步预热，避免把大图缩放阻塞在列表/单条查询的返回路径上。
    if !thumb_exists {
        let store = store.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(err) = store.ensure_thumbnail(&file_name) {
                log::warn!("ensure image thumbnail failed for {:?}: {err}", file_name);
            }
        });
    }

    Ok(())
}

/// 把 `clipboard_apps.icon_file` 解析为磁盘绝对路径写回 [`ClipboardItem::source_app_icon_path`]，
/// utf-8 转换失败时直接置 `None`（前端可降级到首字母占位）。
fn attach_source_app_icon_path(store: &AppIconStore, item: &mut ClipboardItem) {
    item.source_app_icon_path = item
        .source_app_icon_file
        .as_deref()
        .and_then(|name| store.icon_path(name).to_str().map(str::to_owned));
}

/// 把来源应用 DB 模型转换为偏好页列表可直接渲染的视图模型。
pub fn build_clipboard_app_view(store: &AppIconStore, app: ClipboardApp) -> ClipboardAppView {
    let icon_path = app
        .icon_file
        .as_deref()
        .and_then(|name| store.icon_path(name).to_str().map(str::to_owned));

    ClipboardAppView {
        id: app.id,
        name: app.name,
        icon_file: app.icon_file,
        icon_path,
        platform: app.platform,
        created_at: app.created_at,
        updated_at: app.updated_at,
    }
}

/// 仅当 `sub_kind = Color` 时，把 `summary`（或 `content` 兜底）规范化为可信 CSS 颜色串
/// 写入 `color_preview`，前端无需再自行校验即可塞 `style.background`。
fn attach_color_preview(item: &mut ClipboardItem) {
    if item.sub_kind != Some(ClipboardSubKind::Color) {
        return;
    }

    let source = item.summary.as_deref().unwrap_or(&item.content);
    item.color_preview = sanitize_css_color(source);
}

/// 按当前设置对敏感文本列表项返回脱敏摘要，避免前端列表暴露完整凭据。
fn redact_sensitive_list_item(item: &mut ClipboardItem, redact_sensitive: bool) {
    if !redact_sensitive || !item.is_sensitive || item.kind != ClipboardKind::Text {
        return;
    }

    if let Some(summary) = item.summary.as_mut() {
        *summary = mask_sensitive_text(summary);
    }
    item.color_preview = None;
}

/// 生成敏感文本遮罩：每行保留头尾少量字符，中间以星号替换。
fn mask_sensitive_text(text: &str) -> String {
    text.split('\n')
        .map(mask_sensitive_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// 遮罩单行敏感文本；短文本全量替换，长文本保留前后 4 个字符。
fn mask_sensitive_line(line: &str) -> String {
    const VISIBLE_EDGE_CHARS: usize = 4;
    const MAX_MASK_CHARS: usize = 8;

    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    if len == 0 {
        return String::new();
    }
    if len <= VISIBLE_EDGE_CHARS * 2 {
        return "*".repeat(len);
    }

    let head = chars[..VISIBLE_EDGE_CHARS].iter().collect::<String>();
    let tail = chars[len - VISIBLE_EDGE_CHARS..].iter().collect::<String>();
    let mask_len = (len - VISIBLE_EDGE_CHARS * 2).min(MAX_MASK_CHARS);

    [head, "*".repeat(mask_len), tail].concat()
}

/// 返回预览 payload 的文本子类型；脱敏展示时敏感内容强制按纯文本展示。
fn preview_sub_kind(item: &ClipboardItem, redact_sensitive: bool) -> Option<ClipboardSubKind> {
    if redact_sensitive && item.is_sensitive && item.kind == ClipboardKind::Text {
        return None;
    }

    item.sub_kind
}

/// 返回预览窗口展示用文本；HTML / RTF 使用 OS 提供的纯文本表示，避免前端渲染富文本源。
fn preview_text(item: &ClipboardItem, redact_sensitive: bool) -> String {
    let source = match item.sub_kind {
        Some(ClipboardSubKind::Html | ClipboardSubKind::Rtf) => {
            item.search_text.as_deref().unwrap_or(&item.content)
        }
        _ => &item.content,
    };

    if redact_sensitive && item.is_sensitive {
        return mask_sensitive_text(source);
    }

    source.to_owned()
}

/// 把 `created_at`（UTC）按本地时区做三档展示格式化：
/// 今天 → `HH:mm`，今年内 → `MM-DD HH:mm`，跨年 → `YYYY-MM-DD HH:mm`。
/// `now` 由调用方在批处理外取一次，避免列表内逐条 syscall。
fn attach_display_created_at(item: &mut ClipboardItem, now: &chrono::DateTime<Local>) {
    let local = item.created_at.with_timezone(&Local);
    let today = now.date_naive() == local.date_naive();
    let same_year = now.year() == local.year();

    item.display_created_at = if today {
        local.format("%H:%M").to_string()
    } else if same_year {
        local.format("%m-%d %H:%M").to_string()
    } else {
        local.format("%Y-%m-%d %H:%M").to_string()
    };
}

/// 按 `kind` / `sub_kind` 计算右键菜单可用动作，按建议展示顺序返回。
/// 前端只负责把每个动作映射成 `MenuItemOptions`（文案 + 快捷键），
/// 不再自行判定「能否打开链接 / 是否文件可揭示」等业务规则。
fn compute_available_actions(item: &ClipboardItem) -> Vec<ClipboardAction> {
    let mut actions = Vec::with_capacity(10);

    actions.push(ClipboardAction::Paste);

    match item.kind {
        ClipboardKind::Text => actions.push(ClipboardAction::PasteAsPlainText),
        ClipboardKind::Files => actions.push(ClipboardAction::PasteAsPath),
        ClipboardKind::Image => {}
    }

    actions.push(ClipboardAction::Copy);
    if item.kind == ClipboardKind::Image {
        actions.push(ClipboardAction::SaveImage);
    }

    match item.sub_kind {
        Some(ClipboardSubKind::Url) => actions.push(ClipboardAction::OpenLink),
        Some(ClipboardSubKind::Email) => actions.push(ClipboardAction::SendEmail),
        _ => {}
    }

    let can_reveal =
        item.kind == ClipboardKind::Files || item.sub_kind == Some(ClipboardSubKind::Path);
    if can_reveal {
        #[cfg(target_os = "macos")]
        actions.push(ClipboardAction::RevealInFinder);
        #[cfg(target_os = "windows")]
        actions.push(ClipboardAction::RevealInExplorer);
    }

    actions.push(ClipboardAction::ToggleFavorite);
    actions.push(ClipboardAction::TogglePinned);
    actions.push(ClipboardAction::EditNote);
    actions.push(ClipboardAction::Delete);

    actions
}

/// 生成图片另存为对话框默认文件名。
pub fn default_saved_image_file_name(item: &ClipboardItem) -> String {
    let local = item.created_at.with_timezone(&Local);

    format!("EcoPaste-image-{}.png", local.format("%Y%m%d-%H%M%S"))
}

/// 为 files 条目按设置组装前若干项 [`FileEntry`]：
/// 路径 / 文件名 / 目录标记 / 图片标记 / icon 绝对路径，前端直接渲染。
/// 非 files 条目或无路径时保持 `file_entries = None`。
async fn attach_file_entries(
    pool: &SqlitePool,
    store: &FileIconStore,
    item: &mut ClipboardItem,
    limit: usize,
) -> Result<()> {
    if item.kind != ClipboardKind::Files {
        return Ok(());
    }

    let paths: Vec<&str> = item
        .content
        .split('\n')
        .filter(|p| !p.is_empty())
        .take(limit)
        .collect();
    if paths.is_empty() {
        return Ok(());
    }

    let types: Vec<&str> = item
        .file_types
        .as_deref()
        .unwrap_or("")
        .split(',')
        .collect();

    let mut entries = Vec::with_capacity(paths.len());
    for (index, path) in paths.iter().enumerate() {
        let (icon_path, exists) =
            resolve_file_icon_path(pool, store, path, item.file_types.as_deref(), index).await?;
        let is_dir = types.get(index).copied() == Some("d");
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| (*path).to_owned());
        let is_image = !is_dir && is_image_path(path);

        entries.push(FileEntry {
            path: (*path).to_owned(),
            name,
            is_dir,
            is_image,
            exists,
            icon_path,
        });
    }

    item.file_entries = Some(entries);
    item.files_preview_kind = Some(match item.file_entries.as_deref() {
        Some([only]) if only.is_image && only.exists => {
            crate::db::models::FilesPreviewKind::ImagePreview
        }
        _ => crate::db::models::FilesPreviewKind::List,
    });
    Ok(())
}

/// 将完整 `ClipboardItem` 转为预览窗口的轻量数据模型。
pub async fn build_clipboard_preview_payload(
    pool: &SqlitePool,
    image_store: &ImageStore,
    file_icon_store: &FileIconStore,
    item: ClipboardItem,
    redact_sensitive: bool,
) -> Result<ClipboardPreviewPayload> {
    let mut text = None;
    let mut image_path = None;
    let mut image_exists = false;
    let mut files = Vec::new();
    let mut total_files = 0;

    match item.kind {
        ClipboardKind::Text => {
            text = Some(preview_text(&item, redact_sensitive));
        }
        ClipboardKind::Image => {
            validate_image_file_name(&item.content)?;
            let path = image_store.origin_path(&item.content);
            image_exists = path.exists();
            image_path = Some(path_to_string(&path, "image path")?);
        }
        ClipboardKind::Files => {
            total_files = count_file_paths(&item.content);
            files = build_preview_file_entries(pool, file_icon_store, &item).await?;
        }
    }
    let preview_sub_kind = preview_sub_kind(&item, redact_sensitive);

    Ok(ClipboardPreviewPayload {
        id: item.id,
        kind: item.kind,
        sub_kind: preview_sub_kind,
        updated_at: item.updated_at,
        text,
        image_path,
        image_width: item.width,
        image_height: item.height,
        size: item.size,
        is_sensitive: item.is_sensitive,
        image_exists,
        files,
        total_files,
    })
}

/// 解析 files 类型记录中的路径列表，最多返回前 64 项以控制 IPC 与 icon 抽取成本。
async fn build_preview_file_entries(
    pool: &SqlitePool,
    store: &FileIconStore,
    item: &ClipboardItem,
) -> Result<Vec<ClipboardPreviewFileEntry>> {
    let paths: Vec<&str> = item
        .content
        .split('\n')
        .filter(|path| !path.is_empty())
        .take(PREVIEW_FILE_ENTRY_LIMIT)
        .collect();

    let mut entries = Vec::with_capacity(paths.len());
    for (index, path) in paths.iter().enumerate() {
        let (icon_path, exists) =
            resolve_file_icon_path(pool, store, path, item.file_types.as_deref(), index).await?;
        let path_obj = Path::new(path);
        let is_dir = resolve_preview_file_is_dir(path_obj, item.file_types.as_deref(), index);
        let is_image = !is_dir && is_image_path(path);
        let size = resolve_preview_file_size(path_obj, is_dir);
        let name = path_obj
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| (*path).to_owned());

        entries.push(ClipboardPreviewFileEntry {
            path: (*path).to_owned(),
            name,
            is_dir,
            is_image,
            exists,
            size,
            icon_path,
        });
    }

    Ok(entries)
}

/// 统计 files payload 中的有效路径数量。
fn count_file_paths(content: &str) -> usize {
    content.split('\n').filter(|path| !path.is_empty()).count()
}

/// 解析文件是否为目录：存在时信任实时 metadata，缺失时回退到入库时保存的 file_types。
fn resolve_preview_file_is_dir(path: &Path, file_types: Option<&str>, index: usize) -> bool {
    if let Ok(metadata) = path.metadata() {
        return metadata.is_dir();
    }

    file_types
        .and_then(|types| types.split(',').nth(index))
        .map(|file_type| file_type == "d")
        .unwrap_or(false)
}

/// 解析普通文件大小；目录或缺失路径不显示 size。
fn resolve_preview_file_size(path: &Path, is_dir: bool) -> Option<i64> {
    if is_dir {
        return None;
    }

    path.metadata().ok().map(|metadata| metadata.len() as i64)
}

/// 将本地路径转为 UTF-8 字符串，失败时返回面向用户的 clipboard 错误。
fn path_to_string(path: &Path, label: &str) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| AppError::Clipboard(format!("{label} is not valid utf-8")))
}

/// 与前端 `utils/is.ts` 的 `isImage` 同义：按扩展名（大小写不敏感）判断常见图片格式。
fn is_image_path(path: &str) -> bool {
    let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) else {
        return false;
    };

    matches!(
        ext.to_ascii_lowercase().as_str(),
        "jpg"
            | "jpeg"
            | "png"
            | "webp"
            | "avif"
            | "gif"
            | "svg"
            | "bmp"
            | "ico"
            | "tif"
            | "tiff"
            | "heic"
            | "apng"
    )
}

/// 解析文件 icon 路径：优先命中缓存，未命中时在路径存在的前提下抽取并落盘缓存。
/// 返回 `(icon_path, exists)`，其中 `icon_path` 可能为 `None`（抽取失败或已删除且无缓存）。
pub async fn resolve_file_icon_path(
    pool: &SqlitePool,
    file_icon_store: &FileIconStore,
    path: &str,
    file_types: Option<&str>,
    index: usize,
) -> Result<(Option<String>, bool)> {
    let path_obj = Path::new(path);
    let exists = path_obj.exists();
    let platform = if cfg!(target_os = "macos") {
        Platform::Macos
    } else {
        Platform::Windows
    };

    let is_directory = file_types
        .and_then(|types| types.split(',').nth(index))
        .map(|t| t == "d");

    let cache_key = if is_directory == Some(true) {
        crate::clipboard::DIR_CACHE_KEY.to_string()
    } else {
        // 路径存在时实时判断（覆盖入库后类型变化的情况）；已删除时按扩展名推断。
        crate::clipboard::get_icon_cache_key(path_obj)
    };

    // DB 命中后还要确认 icon 文件仍在磁盘上：用户清缓存 / 手动删 file-icons 目录后，
    // 表里的 <hash>.png 映射就成了死引用，落到前端 convertFileSrc 会 404。缺了就当 miss 重抽。
    if let Some(icon_file) = crate::db::file_icons::get_icon(pool, &cache_key, platform).await? {
        let icon_path = file_icon_store.icon_path(&icon_file);
        if icon_path.exists() {
            return Ok((icon_path.to_str().map(str::to_owned), exists));
        }
    }

    if !exists {
        return Ok((None, false));
    }

    let path_for_extract = path_obj.to_path_buf();
    let png_bytes = tauri::async_runtime::spawn_blocking(move || {
        crate::clipboard::icon_png(&path_for_extract, None)
    })
    .await
    .map_err(|err| AppError::Clipboard(format!("icon extract task join failed: {err}")))?;

    let Some(png) = png_bytes else {
        return Ok((None, exists));
    };

    let icon_file = file_icon_store.store(&png)?;
    crate::db::file_icons::upsert_icon(pool, &cache_key, platform, &icon_file).await?;

    let icon_path = file_icon_store.icon_path(&icon_file);
    Ok((icon_path.to_str().map(str::to_owned), exists))
}

/// 合并 DB 已知应用与运行中临时应用，同 id 时保留 DB 行。
pub fn merge_clipboard_apps(
    known_apps: Vec<ClipboardApp>,
    running_apps: Vec<ClipboardApp>,
) -> Vec<ClipboardApp> {
    let mut merged = HashMap::with_capacity(known_apps.len() + running_apps.len());

    for app in running_apps {
        merged.insert(app.id.clone(), app);
    }
    for mut app in known_apps {
        if app.icon_file.is_none() {
            app.icon_file = merged
                .get(&app.id)
                .and_then(|running| running.icon_file.clone());
        }
        merged.insert(app.id.clone(), app);
    }

    merged.into_values().collect()
}

/// 按名称和 id 稳定排序来源应用列表。
pub fn sort_clipboard_apps(apps: &mut [ClipboardApp]) {
    apps.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });
}

/// 校验图片文件名：必须是单层 `<name>.png`，不含路径分隔符 / 父目录引用。
pub fn validate_image_file_name(file_name: &str) -> Result<()> {
    let invalid = file_name.is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains("..")
        || !file_name.ends_with(".png");

    if invalid {
        return Err(AppError::Clipboard(format!(
            "invalid image file name: {file_name:?}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::items::content_hash;

    fn text_item(sub_kind: Option<ClipboardSubKind>, is_sensitive: bool) -> ClipboardItem {
        let content = "<b>secret</b>".to_owned();

        ClipboardItem {
            id: "item".to_owned(),
            kind: ClipboardKind::Text,
            sub_kind,
            group_id: None,
            source_app_id: None,
            content_hash: content_hash(ClipboardKind::Text, &content),
            content,
            search_text: None,
            summary: None,
            file_types: None,
            size: None,
            width: None,
            height: None,
            use_count: 1,
            is_favorite: false,
            is_pinned: false,
            is_sensitive,
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

    fn image_item() -> ClipboardItem {
        let content = "abcdef0123.png".to_owned();
        let mut item = text_item(None, false);

        item.kind = ClipboardKind::Image;
        item.content_hash = content_hash(ClipboardKind::Image, &content);
        item.content = content;
        item
    }

    #[test]
    fn mask_sensitive_text_replaces_middle_with_stars() {
        assert_eq!(
            mask_sensitive_text("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890"),
            "sk-a********7890"
        );
    }

    #[test]
    fn mask_sensitive_text_replaces_short_lines_fully() {
        assert_eq!(mask_sensitive_text("secret"), "******");
    }

    #[test]
    fn mask_sensitive_text_masks_each_line() {
        assert_eq!(
            mask_sensitive_text("abcd1234xyz\nshort"),
            "abcd***4xyz\n*****"
        );
    }

    #[test]
    fn redact_sensitive_list_item_masks_summary_when_enabled() {
        let mut item = text_item(None, true);
        item.summary = Some("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890".to_owned());

        redact_sensitive_list_item(&mut item, true);

        assert_eq!(item.summary.as_deref(), Some("sk-a********7890"));
    }

    #[test]
    fn redact_sensitive_list_item_keeps_summary_when_disabled() {
        let mut item = text_item(None, true);
        item.summary = Some("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890".to_owned());

        redact_sensitive_list_item(&mut item, false);

        assert_eq!(
            item.summary.as_deref(),
            Some("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890")
        );
    }

    #[test]
    fn preview_sub_kind_drops_sensitive_text_subtype_when_redacted() {
        let item = text_item(Some(ClipboardSubKind::Html), true);

        assert_eq!(preview_sub_kind(&item, true), None);
    }

    #[test]
    fn preview_sub_kind_keeps_sensitive_text_subtype_when_not_redacted() {
        let item = text_item(Some(ClipboardSubKind::Html), true);

        assert_eq!(preview_sub_kind(&item, false), Some(ClipboardSubKind::Html));
    }

    #[test]
    fn preview_sub_kind_keeps_regular_text_subtype() {
        let item = text_item(Some(ClipboardSubKind::Html), false);

        assert_eq!(preview_sub_kind(&item, true), Some(ClipboardSubKind::Html));
    }

    #[test]
    fn preview_text_keeps_plain_text_content() {
        let mut item = text_item(None, false);
        item.content = "plain text".to_owned();

        assert_eq!(preview_text(&item, false), "plain text");
    }

    #[test]
    fn preview_text_uses_search_text_for_html() {
        let mut item = text_item(Some(ClipboardSubKind::Html), false);
        item.content = "<b>Hello</b> World".to_owned();
        item.search_text = Some("Hello World".to_owned());

        assert_eq!(preview_text(&item, false), "Hello World");
    }

    #[test]
    fn preview_text_uses_search_text_for_rtf() {
        let mut item = text_item(Some(ClipboardSubKind::Rtf), false);
        item.content = r"{\rtf1 Hello World}".to_owned();
        item.search_text = Some("Hello World".to_owned());

        assert_eq!(preview_text(&item, false), "Hello World");
    }

    #[test]
    fn preview_text_masks_sensitive_plain_source() {
        let mut item = text_item(Some(ClipboardSubKind::Html), true);
        item.content = "<b>sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890</b>".to_owned();
        item.search_text = Some("sk-abcdefghijklmnopqrstuvwxyzABCDE1234567890".to_owned());

        assert_eq!(preview_text(&item, true), "sk-a********7890");
    }

    #[test]
    fn image_actions_include_save_image() {
        let actions = compute_available_actions(&image_item());

        assert!(actions.contains(&ClipboardAction::SaveImage));
    }

    #[test]
    fn text_actions_do_not_include_save_image() {
        let actions = compute_available_actions(&text_item(None, false));

        assert!(!actions.contains(&ClipboardAction::SaveImage));
    }

    #[test]
    fn accepts_plain_png_file_name() {
        assert!(validate_image_file_name("abcdef0123.png").is_ok());
    }

    #[test]
    fn rejects_traversal_and_subpaths() {
        for bad in [
            "",
            "evil.txt",
            "../secret.png",
            "..\\secret.png",
            "sub/dir.png",
            "a/b.png",
            "/abs.png",
            "name..png", // 含 ".." 序列，保守拒绝
        ] {
            assert!(
                validate_image_file_name(bad).is_err(),
                "should reject: {bad:?}"
            );
        }
    }
}
