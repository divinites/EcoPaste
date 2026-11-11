//! 剪贴板相关命令：手动重新读取、解析图片路径。供前端按需触发。

use std::path::Path;
use std::sync::Arc;

use chrono::{Local, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::clipboard::{
    add_app_from_path, build_clipboard_app_view, build_clipboard_preview_payload,
    build_item_with_settings, default_saved_image_file_name, delete_unreferenced_apps,
    detect_frontmost, enrich_list_item, is_excluded_by_pattern, materialize_source,
    merge_clipboard_apps, normalize_group_icon, normalize_group_name, normalize_group_svg,
    persist_and_notify, refresh_running_apps, resolve_file_icon_path, sort_clipboard_apps,
    validate_image_file_name, AppIconStore, AppsRegistry, ClipboardAppView,
    ClipboardPreviewPayload, ClipboardReader, FileIconStore, ImageStore, ListItemContext,
    WritebackGuard, MAX_GROUP_ICON_BYTES,
};
use crate::core::{AppError, Result};
use crate::db::items::{
    clear_items, find_item_by_id, find_item_for_list_by_id, increment_item_use_count,
};
use crate::db::models::{
    ClipboardGroup, ClipboardItem, ClipboardItemPage, ClipboardItemQuery, ClipboardKind,
};
use crate::db::DatabaseState;
use crate::settings::SettingsStore;
use crate::window::{self, CLIPBOARD_WINDOW_LABEL};

/// 与前端 `src/constants/events.ts` 的 `TAURI_EVENT.CLIPBOARD_UPDATED` 一一对应。
const CLIPBOARD_UPDATED_EVENT: &str = "clipboard://updated";

/// 与前端 `src/constants/events.ts` 的 `TAURI_EVENT.CLIPBOARD_GROUPS_UPDATED` 一一对应。
const CLIPBOARD_GROUPS_UPDATED_EVENT: &str = "clipboard-groups://updated";

/// `read_clipboard` 的返回：入库后的记录 + 是否命中去重（前端据此决定提示/滚动行为）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadClipboardResult {
    pub item: ClipboardItem,
    pub deduplicated: bool,
    /// 剪贴板为空 / 无可识别内容时为 `false`，此时 `item` 为 `None`。
    pub captured: bool,
}

/// 新建或更新自定义剪贴板分组的输入。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardGroupInput {
    pub name: String,
    pub icon: String,
    #[serde(default)]
    pub is_hidden: bool,
}

/// 批量保存自定义分组排序和主界面显隐状态的输入。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardGroupLayoutInput {
    pub order: Vec<String>,
    pub visible_ids: Vec<String>,
}

/// 手动读取当前剪贴板并入库（「重新读取」按钮）。
///
/// 复用监听管线：read_all → build_item（含图片落盘）→ persist_and_notify（去重入库 + emit）。
/// 与 OS 监听走同一条入库路径，语义一致；emit 同样触发前端列表刷新。
///
/// `ClipboardReader` 持有的 `ClipboardContext` 是 `!Send`，故读取与转换全部在
/// await 之前的同步块内完成并 drop，之后才进入异步入库——保证命令 future 满足 `Send`。
#[tauri::command]
pub async fn read_clipboard(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    app_icon_store: State<'_, AppIconStore>,
    registry: State<'_, AppsRegistry>,
) -> Result<Option<ReadClipboardResult>> {
    // 先在同步段抓前台应用 + 读取剪贴板。
    // 注意：手动「重新读取」时前台应用就是 EcoPaste 自己，不像 OS 监听场景能拿到原应用——
    // 这里仍保留探测：若用户在外部应用复制后立刻命令式触发，多少能捕获到正确来源；
    // 命中我们自己也无害（apps 表只是多记一条「EcoPaste」）。
    let (item_opt, source) = {
        let source = detect_frontmost();
        let reader = ClipboardReader::new()?;
        let settings = app.state::<SettingsStore>().snapshot();
        let payload = reader.read_with_capture(&settings.clipboard.capture)?;
        let item = match payload {
            Some(payload) => build_item_with_settings(
                &store,
                &payload,
                &settings.clipboard.capture,
                &settings.clipboard.sensitive,
                settings.clipboard.content.copy_plain,
            )?,
            None => None,
        }
        .filter(|item| {
            !is_excluded_by_pattern(&settings.clipboard.filters.excluded_patterns, item)
        });
        (item, source)
        // reader 在此 drop，!Send 句柄不跨下方 await。
    };

    let Some(mut item) = item_opt else {
        return Ok(None);
    };

    let source_app = source.map(|src| materialize_source(&app_icon_store, Some(&registry), src));
    if let Some(src) = &source_app {
        item.source_app_id = Some(src.id.clone());
    }

    let pool = db.pool().await;
    let result = persist_and_notify(&app, &pool, &item, source_app.as_ref()).await?;
    Ok(Some(ReadClipboardResult {
        item,
        deduplicated: result.deduplicated,
        captured: true,
    }))
}

/// 把入库的图片文件名解析为磁盘绝对路径，供前端预览取图。
/// `thumbnail = true` 取缩略图，否则取原图。
///
/// 缩略图按需生成：不存在时在 `spawn_blocking` 里从原图解码 + 缩放 + 编码后落盘，再返回路径。
/// 解码/编码是 CPU 阻塞活，放到阻塞线程池避免占用 async worker；且「返回时文件已确保存在」，
/// 前端拿到路径即可安全 `convertFileSrc` 加载，不会撞到半成品文件。
///
/// `file_name` 来自前端（即记录的 `content`），是唯一的外部输入，需防路径穿越：
/// 仅允许「单层、纯 `<hash>.png` 形态」的文件名，含分隔符 / `..` / 子目录一律拒绝。
#[tauri::command]
pub async fn get_clipboard_image_path(
    store: State<'_, ImageStore>,
    file_name: String,
    thumbnail: bool,
) -> Result<String> {
    validate_image_file_name(&file_name)?;

    let path = if thumbnail {
        let store = store.inner().clone();
        tauri::async_runtime::spawn_blocking(move || store.ensure_thumbnail(&file_name))
            .await
            .map_err(|err| AppError::Clipboard(format!("thumbnail task join failed: {err}")))??
    } else {
        store.origin_path(&file_name)
    };

    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| AppError::Clipboard("image path is not valid utf-8".to_owned()))
}

/// 把 `clipboard_apps.icon_file`（形如 `<hash>.png`）解析为磁盘绝对路径，
/// 供前端用 `convertFileSrc` 渲染。文件名复用图片同款防穿越校验。
#[tauri::command]
pub async fn get_clipboard_app_icon_path(
    store: State<'_, AppIconStore>,
    file_name: String,
) -> Result<String> {
    validate_image_file_name(&file_name)?;
    store
        .icon_path(&file_name)
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| AppError::Clipboard("app icon path is not valid utf-8".to_owned()))
}

/// 将图片历史记录另存为本地 PNG 文件。
///
/// 保存对话框默认打开系统下载目录，用户可在对话框中自由选择最终位置。
#[tauri::command]
pub async fn save_clipboard_image_to_file(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    id: String,
) -> Result<Option<String>> {
    let pool = db.pool().await;
    let item = find_item_by_id(&pool, &id)
        .await?
        .ok_or_else(|| item_not_found(&id))?;

    if item.kind != ClipboardKind::Image {
        return Err(AppError::Clipboard(
            "selected item is not an image".to_owned(),
        ));
    }

    validate_image_file_name(&item.content)?;

    let source = store.origin_path(&item.content);
    if !source.is_file() {
        return Err(AppError::Clipboard("image file does not exist".to_owned()));
    }

    let _auto_hide_guard = ClipboardAutoHideSuspendGuard::new();
    let mut dialog = app
        .dialog()
        .file()
        .add_filter("PNG", &["png"])
        .set_can_create_directories(true)
        .set_file_name(default_saved_image_file_name(&item))
        .set_title(crate::i18n::clipboard_menu::label(
            crate::i18n::current_language(&app),
            crate::i18n::clipboard_menu::Key::SaveImage,
        ));

    match app.path().download_dir() {
        Ok(download_dir) => {
            dialog = dialog.set_directory(download_dir);
        }
        Err(err) => {
            log::warn!("resolve download directory for image save failed: {err}");
        }
    }

    let Some(target_path) = dialog.blocking_save_file() else {
        return Ok(None);
    };

    let target = target_path
        .into_path()
        .map_err(|err| AppError::Clipboard(format!("save path is invalid: {err}")))?;
    let source_for_copy = source.clone();
    let target_for_copy = target.clone();

    tauri::async_runtime::spawn_blocking(move || std::fs::copy(source_for_copy, target_for_copy))
        .await
        .map_err(|err| AppError::Clipboard(format!("save image task join failed: {err}")))?
        .map_err(|err| AppError::Clipboard(format!("save image failed: {err}")))?;

    target
        .to_str()
        .map(str::to_owned)
        .map(Some)
        .ok_or_else(|| AppError::Clipboard("save path is not valid utf-8".to_owned()))
}

/// `get_file_icon_path` 的返回：icon 绝对路径 + 文件当前是否存在于磁盘。
/// 前端据 `exists = false` 给已删除的文件显示弱化样式 / 提示。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIconResult {
    /// icon 文件磁盘绝对路径；抽取失败 / 路径已删除且未缓存时为 `None`。
    pub icon_path: Option<String>,
    /// 当前路径是否仍存在于磁盘。
    pub exists: bool,
}

/// 获取文件类型 icon 的磁盘绝对路径，供前端 FilesCard 渲染。
///
/// 懒加载：缓存 miss 时，若路径存在则抽取 icon 并缓存；路径已删除则 `iconPath = None`（前端显示弱化态）。
/// `file_types` 来自 `clipboard_items.file_types`（如 "d,f,f"），`index` 为路径在列表中的索引（0-based）。
#[tauri::command]
pub async fn get_file_icon_path(
    db: State<'_, DatabaseState>,
    file_icon_store: State<'_, FileIconStore>,
    path: String,
    file_types: Option<String>,
    index: usize,
) -> Result<FileIconResult> {
    let pool = db.pool().await;
    let (icon_path, exists) =
        resolve_file_icon_path(&pool, &file_icon_store, &path, file_types.as_deref(), index)
            .await?;

    Ok(FileIconResult { icon_path, exists })
}

/// 读取单条剪贴板记录的完整预览 payload。
///
/// 列表查询会裁剪 text content；预览必须走完整记录，再按 Content Viewer 的三类视图归一化。
#[tauri::command]
pub async fn get_clipboard_preview_payload(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    image_store: State<'_, ImageStore>,
    file_icon_store: State<'_, FileIconStore>,
    item_id: String,
) -> Result<Option<ClipboardPreviewPayload>> {
    let pool = db.pool().await;
    let Some(item) = find_item_by_id(&pool, &item_id).await? else {
        return Ok(None);
    };

    let redact_sensitive = app
        .state::<SettingsStore>()
        .snapshot()
        .clipboard
        .sensitive
        .redact_secrets;
    let payload = build_clipboard_preview_payload(
        &pool,
        &image_store,
        &file_icon_store,
        item,
        redact_sensitive,
    )
    .await?;
    Ok(Some(payload))
}

/// 播放一次复制成功提示音，用于偏好设置页试听。
#[tauri::command]
pub async fn play_copy_sound() {
    crate::clipboard::play_copy_sound();
}

/// 把指定历史记录写回系统剪贴板（不触发模拟粘贴）。
/// `plain = true` 强制纯文本，剥离 HTML/RTF。
///
/// `ClipboardContext` 是 `!Send`，写回逻辑在 `clipboard::write_to_clipboard` 内同步完成，
/// 之后无 await，命令 future 仍满足 `Send`。
#[tauri::command]
pub async fn write_to_clipboard(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    guard: State<'_, Arc<WritebackGuard>>,
    id: String,
    plain: bool,
) -> Result<()> {
    let pool = db.pool().await;
    let item = find_item_by_id(&pool, &id)
        .await?
        .ok_or_else(|| item_not_found(&id))?;

    let settings = app.state::<SettingsStore>().snapshot();
    let write_plain =
        should_write_plain_for_copy(plain, item.kind, settings.clipboard.content.copy_plain);
    let hide_after_copy = settings.clipboard.content.copy_then_hide_window;

    crate::clipboard::write_to_clipboard(&store, guard.inner().as_ref(), &item, write_plain)?;
    mark_item_reused_if_enabled(&app, &pool, &id, item.kind).await?;

    if hide_after_copy {
        hide_clipboard_window_after_copy(&app);
    }

    Ok(())
}

/// 「点击列表项 → 自动粘贴」的组合命令：写回剪贴板 + 隐藏剪贴板窗口 + 触发系统级粘贴。
///
/// 窗口已是非激活面板（macOS NSPanel `nonactivating_panel` / Windows `focusable=false`），
/// show 时不会把前台 App 推走，前台焦点始终在用户原窗口。
/// macOS 上 panel 会成为 key window，CGEvent ⌘V 若不先 hide 会被 panel 自己吞掉，
/// hide 后插入 50ms 让 panel 真正 order_out（hide_window 是 run_on_main_thread 异步派发，
/// 右键菜单触发时主线程仍在处理菜单关闭，不等会出现 ⌘V 早于 hide 完成的竞态）。
#[tauri::command]
pub async fn paste_clipboard_item(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    guard: State<'_, Arc<WritebackGuard>>,
    id: String,
    plain: bool,
) -> Result<()> {
    ensure_paste_permission(&app).await?;

    let pool = db.pool().await;
    let item = find_item_by_id(&pool, &id)
        .await?
        .ok_or_else(|| item_not_found(&id))?;

    let settings = app.state::<SettingsStore>().snapshot();
    let write_plain = should_write_plain_for_paste(
        plain,
        item.kind,
        settings.clipboard.content.paste_plain,
        settings.clipboard.content.paste_files_as_path,
    );

    crate::clipboard::write_to_clipboard(&store, guard.inner().as_ref(), &item, write_plain)?;

    if window::is_clipboard_window_pinned() {
        // 固定时窗口保持可见：macOS 上 panel 仍是 key window 会吞掉 ⌘V，需先 resign key
        // 让键焦点回到前台 App 的窗口；Windows 剪贴板窗口 focusable=false，无需处理。
        #[cfg(target_os = "macos")]
        if let Err(err) = window::macos::resign_clipboard_panel_key(&app) {
            log::warn!("resign clipboard panel key before paste failed: {err:?}");
        }
    } else if let Err(err) = window::hide_window(&app, CLIPBOARD_WINDOW_LABEL) {
        log::warn!("hide clipboard window before paste failed: {err:?}");
    }

    // hide / resign 都是 run_on_main_thread 异步派发；不等一拍，simulate_paste 的 ⌘V
    // 会赶在 panel 真正 order_out / 让出 key 前命中 panel 自己（webview 吞掉）。
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    crate::keystroke::simulate_paste()?;

    // 固定窗口下粘贴完把 key 拿回来，让用户继续用键盘 / 列表操作；
    // 再等一拍让 ⌘V 事件被目标 App 消费完，避免 make_key 抢回焦点把按键吞回 panel。
    if window::is_clipboard_window_pinned() {
        #[cfg(target_os = "macos")]
        {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Err(err) = window::macos::make_clipboard_panel_key(&app) {
                log::warn!("restore clipboard panel key after paste failed: {err:?}");
            }
        }
    }

    // 复用计数放在按键注入之后：DB 写入不拖慢粘贴，失败也不影响已完成的粘贴。
    if let Err(err) = mark_item_reused_if_enabled(&app, &pool, &id, item.kind).await {
        log::warn!("mark clipboard item reused after paste failed: {err}");
    }

    Ok(())
}

/// 计算复制写回是否强制走纯文本；默认复制纯文本只作用于文本记录。
fn should_write_plain_for_copy(force_plain: bool, kind: ClipboardKind, copy_plain: bool) -> bool {
    force_plain || kind == ClipboardKind::Text && copy_plain
}

/// 计算粘贴写回是否强制走纯文本；文本去格式与文件路径粘贴分别由各自设置控制。
fn should_write_plain_for_paste(
    force_plain: bool,
    kind: ClipboardKind,
    paste_plain: bool,
    paste_files_as_path: bool,
) -> bool {
    force_plain
        || kind == ClipboardKind::Text && paste_plain
        || kind == ClipboardKind::Files && paste_files_as_path
}

/// 按设置决定是否把复制 / 粘贴历史记录计为一次复用。
async fn mark_item_reused_if_enabled(
    app: &AppHandle,
    pool: &SqlitePool,
    id: &str,
    kind: ClipboardKind,
) -> Result<()> {
    let settings = app.state::<SettingsStore>().snapshot();
    if !settings.clipboard.content.update_on_reuse {
        return Ok(());
    }

    increment_item_use_count(pool, id).await?;
    if let Err(err) = app.emit(
        CLIPBOARD_UPDATED_EVENT,
        serde_json::json!({
            "id": id,
            "kind": kind,
            "deduplicated": true,
        }),
    ) {
        log::warn!("emit {CLIPBOARD_UPDATED_EVENT} after item reuse failed: {err}");
    }

    Ok(())
}

/// 从历史复制后按设置隐藏剪贴板窗口；固定状态下尊重用户显式 pin。
fn hide_clipboard_window_after_copy(app: &AppHandle) {
    if window::is_clipboard_window_pinned() {
        return;
    }

    if let Err(err) = window::hide_window(app, CLIPBOARD_WINDOW_LABEL) {
        log::warn!("hide clipboard window after copy failed: {err:?}");
    }
}

/// 列表查询命令（薄封装）：参数缺省时走 Rust 端默认（limit=20, offset=0, updatedAtDesc）；
/// `keyword` 非空时由 `query_items` 内部自动委派 FTS5。
/// 返回 [`ClipboardItemPage`]：顶页项 + 同过滤下的总数 + `hasMore`，
/// 供前端一次 IPC 同时拿到「列表 / Footer 总数 / 是否还有下一页」。
/// 顺带把 `source_app_icon_file` 解析为绝对路径写到 `source_app_icon_path`，
/// 前端无需再拉 `get_clipboard_app_icon_path`。
#[tauri::command]
pub async fn list_clipboard_items(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    image_store: State<'_, ImageStore>,
    app_icon_store: State<'_, AppIconStore>,
    file_icon_store: State<'_, FileIconStore>,
    query: Option<ClipboardItemQuery>,
) -> Result<ClipboardItemPage> {
    let pool = db.pool().await;
    let settings = app.state::<SettingsStore>().snapshot();
    let mut q = query.unwrap_or_default();
    q.relevance = settings.clipboard.search.sort_by_relevance;
    let (mut items, total) = crate::db::items::query_items_page(&pool, &q).await?;
    let ctx = ListItemContext {
        pool: &pool,
        image_store: &image_store,
        app_icon_store: &app_icon_store,
        file_icon_store: &file_icon_store,
        file_entry_limit: settings.clipboard.display.file_entry_limit(),
        redact_sensitive: settings.clipboard.sensitive.redact_secrets,
        now: Local::now(),
    };
    for item in &mut items {
        enrich_list_item(&ctx, item).await?;
    }
    let has_more = q.offset + (items.len() as i64) < total;
    Ok(ClipboardItemPage {
        list: items,
        total,
        has_more,
    })
}

/// 单条查询命令：监听到 `clipboard://updated` 后前端按 id 拉单条，
/// 避免事件驱动刷新时整页 refetch。不存在返回 `None`，前端按需降级。
///
/// 这里故意走列表视图（text 类型 content 置空）保持与列表查询同款裁剪；
/// 需要完整 content 的写回/预览走各自专用命令。
#[tauri::command]
pub async fn get_clipboard_item(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    image_store: State<'_, ImageStore>,
    app_icon_store: State<'_, AppIconStore>,
    file_icon_store: State<'_, FileIconStore>,
    id: String,
) -> Result<Option<ClipboardItem>> {
    let pool = db.pool().await;
    let mut item = find_item_for_list_by_id(&pool, &id).await?;
    if let Some(item) = item.as_mut() {
        let settings = app.state::<SettingsStore>().snapshot();
        let ctx = ListItemContext {
            pool: &pool,
            image_store: &image_store,
            app_icon_store: &app_icon_store,
            file_icon_store: &file_icon_store,
            file_entry_limit: settings.clipboard.display.file_entry_limit(),
            redact_sensitive: settings.clipboard.sensitive.redact_secrets,
            now: Local::now(),
        };
        enrich_list_item(&ctx, item).await?;
    }
    Ok(item)
}

/// macOS 模拟粘贴依赖辅助功能权限；未授权时 CGEvent 会被系统静默丢弃，用户只看到"没反应"。
/// 这里先弹系统授权请求，再返回可读错误让前端 toast 提示；写回剪贴板也一并跳过，
/// 避免用户误以为已粘贴。Windows 无此权限模型，直接放行。
async fn ensure_paste_permission(app: &AppHandle) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        if tauri_plugin_macos_permissions::check_accessibility_permission().await {
            return Ok(());
        }

        tauri_plugin_macos_permissions::request_accessibility_permission().await;

        let lang = crate::i18n::current_language(app);
        Err(AppError::Clipboard(
            crate::i18n::commands::label(lang, crate::i18n::commands::Key::AccessibilityRequired)
                .to_owned(),
        ))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(())
    }
}

/// 保存文件对话框会短暂转移焦点；期间暂停剪贴板窗口失焦/外部点击自动隐藏。
struct ClipboardAutoHideSuspendGuard;

impl ClipboardAutoHideSuspendGuard {
    fn new() -> Self {
        window::set_clipboard_window_auto_hide_suspended(true);

        Self
    }
}

impl Drop for ClipboardAutoHideSuspendGuard {
    fn drop(&mut self) {
        window::set_clipboard_window_auto_hide_suspended(false);
    }
}

/// 按 id 列表批量取来源应用——前端渲染卡片时一次性补齐图标/名称。
#[tauri::command]
pub async fn list_clipboard_apps(
    db: State<'_, DatabaseState>,
    ids: Vec<String>,
) -> Result<Vec<crate::db::models::ClipboardApp>> {
    let pool = db.pool().await;
    crate::db::apps::list_apps_by_ids(&pool, &ids).await
}

/// 列出可过滤应用：DB 已知应用加上本次运行中枚举到的临时应用。
#[tauri::command]
pub async fn list_all_apps(
    app_icon_store: State<'_, AppIconStore>,
    db: State<'_, DatabaseState>,
    registry: State<'_, AppsRegistry>,
) -> Result<Vec<ClipboardAppView>> {
    let pool = db.pool().await;
    let running_apps = refresh_running_apps(registry.inner().clone()).await?;
    let known_apps = crate::db::apps::list_all_apps(&pool).await?;
    let mut apps = merge_clipboard_apps(known_apps, running_apps);

    sort_clipboard_apps(&mut apps);
    Ok(apps
        .into_iter()
        .map(|app| build_clipboard_app_view(&app_icon_store, app))
        .collect())
}

/// 从用户手动选择的应用路径写入来源应用注册表。
#[tauri::command]
pub async fn add_clipboard_app_from_path(
    app_icon_store: State<'_, AppIconStore>,
    registry: State<'_, AppsRegistry>,
    path: String,
) -> Result<ClipboardAppView> {
    let app = add_app_from_path(registry.inner().clone(), path).await?;

    Ok(build_clipboard_app_view(&app_icon_store, app))
}

/// 删除未被历史记录引用的来源应用，通常用于从忽略列表移除手动/默认物化应用。
#[tauri::command]
pub async fn delete_unreferenced_clipboard_apps(
    registry: State<'_, AppsRegistry>,
    ids: Vec<String>,
) -> Result<Vec<String>> {
    delete_unreferenced_apps(registry.inner().clone(), ids).await
}

/// 广播自定义分组变更，打开的前端窗口据此刷新分组列表。
fn emit_clipboard_groups_updated(app: &AppHandle) {
    if let Err(err) = app.emit(CLIPBOARD_GROUPS_UPDATED_EVENT, ()) {
        log::warn!("emit clipboard groups updated failed: {err}");
    }
}

/// 列出全部分组（薄封装），供前端构建分组 tab 栏。
/// 按 `sort_order` 升序，同序按 `created_at` 兜底。
#[tauri::command]
pub async fn list_clipboard_groups(db: State<'_, DatabaseState>) -> Result<Vec<ClipboardGroup>> {
    let pool = db.pool().await;
    crate::db::groups::list_groups(&pool).await
}

/// 新建自定义剪贴板分组。
#[tauri::command]
pub async fn create_clipboard_group(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    input: ClipboardGroupInput,
) -> Result<ClipboardGroup> {
    let pool = db.pool().await;
    let name = normalize_group_name(&input.name)?;
    let icon = normalize_group_icon(&input.icon)?;
    let now = Utc::now();
    let group = ClipboardGroup {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        icon,
        is_hidden: input.is_hidden,
        sort_order: crate::db::groups::next_group_sort_order(&pool).await?,
        created_at: now,
        updated_at: now,
    };

    crate::db::groups::insert_group(&pool, &group).await?;
    emit_clipboard_groups_updated(&app);

    Ok(group)
}

/// 更新自定义剪贴板分组名称、图标和显示状态。
#[tauri::command]
pub async fn update_clipboard_group(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    id: String,
    input: ClipboardGroupInput,
) -> Result<()> {
    let pool = db.pool().await;
    let name = normalize_group_name(&input.name)?;
    let icon = normalize_group_icon(&input.icon)?;

    crate::db::groups::update_group(&pool, &id, &name, &icon, input.is_hidden).await?;
    emit_clipboard_groups_updated(&app);

    Ok(())
}

/// 批量更新自定义剪贴板分组排序和显示状态。
#[tauri::command]
pub async fn update_clipboard_groups_layout(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    input: ClipboardGroupLayoutInput,
) -> Result<()> {
    let pool = db.pool().await;

    crate::db::groups::update_group_layout(&pool, &input.order, &input.visible_ids).await?;
    emit_clipboard_groups_updated(&app);

    Ok(())
}

/// 删除自定义剪贴板分组。
#[tauri::command]
pub async fn delete_clipboard_group(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    id: String,
) -> Result<()> {
    let pool = db.pool().await;

    crate::db::groups::delete_group(&pool, &id).await?;
    emit_clipboard_groups_updated(&app);

    Ok(())
}

/// 读取用户通过 Tauri dialog 选择的 SVG 文件内容，供前端作为自定义分组图标保存。
#[tauri::command]
pub async fn import_clipboard_group_svg(path: String) -> Result<String> {
    let path = Path::new(&path);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "svg" {
        return Err(AppError::localized(
            crate::i18n::commands::Key::SvgFileRequired,
        ));
    }

    let bytes = std::fs::read(path).map_err(|err| {
        AppError::localized_with(crate::i18n::commands::Key::SvgFileUnreadable, err)
    })?;
    if bytes.len() > MAX_GROUP_ICON_BYTES {
        return Err(AppError::localized(
            crate::i18n::commands::Key::GroupIconTooLarge,
        ));
    }

    let icon = String::from_utf8(bytes)
        .map_err(|_| AppError::localized(crate::i18n::commands::Key::SvgFileNotUtf8))?;
    normalize_group_svg(&icon)?;

    Ok(icon)
}

/// 翻转收藏状态（薄封装）。前端在调用前/后做乐观更新；这里不返回新状态，
/// 失败时前端按需回滚 / 重拉单条。
#[tauri::command]
pub async fn toggle_clipboard_item_favorite(
    db: State<'_, DatabaseState>,
    id: String,
) -> Result<bool> {
    let pool = db.pool().await;
    crate::db::items::toggle_item_favorite(&pool, &id).await
}

/// 翻转置顶状态（薄封装）。不更新 `updated_at`，避免污染最近使用排序。
#[tauri::command]
pub async fn toggle_clipboard_item_pinned(
    db: State<'_, DatabaseState>,
    id: String,
) -> Result<bool> {
    let pool = db.pool().await;
    crate::db::items::toggle_item_pinned(&pool, &id).await
}

/// 删除单条记录（薄封装）。若删的是图片记录，连带删除其落盘文件（原图 + 缩略图）。
/// 行已删成功，删文件失败仅记日志、不回滚——残留文件最坏占用磁盘，不影响功能。
#[tauri::command]
pub async fn delete_clipboard_item(
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    id: String,
) -> Result<()> {
    let pool = db.pool().await;
    if let Some(file_name) = crate::db::items::delete_item(&pool, &id).await? {
        if let Err(err) = store.remove(&file_name) {
            log::warn!("remove deleted image {file_name} failed: {err}");
        }
    }
    Ok(())
}

/// 清空全部历史记录，并删除对应图片资源。完成后广播列表刷新事件。
#[tauri::command]
pub async fn clear_clipboard_items(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    store: State<'_, ImageStore>,
    delete_favorites: bool,
    delete_pinned: bool,
) -> Result<u64> {
    let pool = db.pool().await;
    let outcome = clear_items(&pool, delete_favorites, delete_pinned).await?;

    for file_name in &outcome.image_files {
        if let Err(err) = store.remove(file_name) {
            log::warn!("remove cleared image {file_name} failed: {err}");
        }
    }

    if let Err(err) = app.emit(
        CLIPBOARD_UPDATED_EVENT,
        serde_json::json!({
            "cleanup": outcome.removed,
        }),
    ) {
        log::warn!("emit {CLIPBOARD_UPDATED_EVENT} after clear failed: {err}");
    }

    Ok(outcome.removed)
}

/// 更新备注（薄封装）。`note = None` 或空串清空备注；空串归一化为 None，
/// 保证「无备注」在库里只有一种表示（NULL），避免后续筛选/展示判别两套逻辑。
///
/// 备注更新结果：归一化后的备注串（去前后空白；纯空白返回 `None`）+ 是否触发 auto-favorite。
/// 前端用 `note` 直接回填本地镜像，避免「输入 `   ` 时前端镜像非空但 DB 为 NULL」的漂移。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNoteResult {
    pub note: Option<String>,
    pub auto_favorited: bool,
}

/// 写入备注；`note` 由 Rust 统一 trim + 空串归一化为 NULL。
/// auto-favorite：写入非空备注时，若 `settings.clipboard.content.autoFavorite` 开启，
/// 顺带把 `is_favorite` 置为 true（已收藏的无变化；清空备注不触发）。
#[tauri::command]
pub async fn update_clipboard_item_note(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    id: String,
    note: Option<String>,
) -> Result<UpdateNoteResult> {
    let pool = db.pool().await;
    let normalized = note.as_deref().and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    crate::db::items::update_item_note(&pool, &id, normalized).await?;

    let mut auto_favorited = false;
    if normalized.is_some() {
        let auto_favorite = app
            .try_state::<crate::settings::SettingsStore>()
            .map(|s| s.snapshot().clipboard.content.auto_favorite)
            .unwrap_or(false);
        if auto_favorite {
            crate::db::items::mark_item_favorite(&pool, &id).await?;
            auto_favorited = true;
        }
    }
    Ok(UpdateNoteResult {
        note: normalized.map(str::to_owned),
        auto_favorited,
    })
}

/// 更新条目所属分组；分组 id 来自右键菜单的实时分组列表。
#[tauri::command]
pub async fn update_clipboard_item_group(
    db: State<'_, DatabaseState>,
    id: String,
    group_id: String,
) -> Result<()> {
    let pool = db.pool().await;
    let exists = crate::db::groups::list_groups(&pool)
        .await?
        .iter()
        .any(|group| group.id == group_id);
    if !exists {
        return Err(AppError::localized(
            crate::i18n::commands::Key::GroupNotFound,
        ));
    }

    crate::db::items::update_item_group(&pool, &id, Some(&group_id)).await
}

/// 打开条目 URL：按 `id` 取完整 `content`，trim 后用系统默认浏览器/邮件 client 打开。
/// `mailto = true` 时自动补 `mailto:` 前缀，供右键菜单「发送邮件」复用。
/// 仅适用于 text 类条目；files 类调用方应改用 `reveal_clipboard_item`。
#[tauri::command]
pub async fn open_clipboard_item_link(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    id: String,
    mailto: bool,
) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;

    let pool = db.pool().await;
    let item = find_item_by_id(&pool, &id)
        .await?
        .ok_or_else(|| item_not_found(&id))?;

    let value = item.content.trim();
    if value.is_empty() {
        return Ok(());
    }

    let url = if mailto {
        format!("mailto:{value}")
    } else if value.starts_with("www.") {
        format!("https://{value}")
    } else {
        value.to_owned()
    };

    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|err| AppError::Clipboard(err.to_string()))
}

/// 在系统文件管理器中显示条目对应文件：files 类取第一条路径，text 类（subKind=path）按 trim 后的字面量。
#[tauri::command]
pub async fn reveal_clipboard_item(
    app: AppHandle,
    db: State<'_, DatabaseState>,
    id: String,
) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;

    let pool = db.pool().await;
    let item = find_item_by_id(&pool, &id)
        .await?
        .ok_or_else(|| item_not_found(&id))?;

    let target = if item.kind == ClipboardKind::Files {
        item.content
            .split('\n')
            .find(|s| !s.is_empty())
            .unwrap_or("")
            .to_owned()
    } else {
        item.content.trim().to_owned()
    };

    if target.is_empty() {
        return Ok(());
    }

    app.opener()
        .reveal_item_in_dir(&target)
        .map_err(|err| AppError::Clipboard(err.to_string()))
}

/// 记录已被删除 / 清理时的用户可见错误；id 只写日志。
fn item_not_found(id: &str) -> AppError {
    log::warn!("clipboard item not found: {id}");
    AppError::localized(crate::i18n::commands::Key::ClipboardItemNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_plain_default_only_affects_text_items() {
        assert!(should_write_plain_for_copy(
            false,
            ClipboardKind::Text,
            true
        ));
        assert!(!should_write_plain_for_copy(
            false,
            ClipboardKind::Files,
            true
        ));
        assert!(!should_write_plain_for_copy(
            false,
            ClipboardKind::Image,
            true
        ));
        assert!(!should_write_plain_for_copy(
            false,
            ClipboardKind::Text,
            false
        ));
    }

    #[test]
    fn force_plain_copy_overrides_item_kind() {
        assert!(should_write_plain_for_copy(
            true,
            ClipboardKind::Files,
            false
        ));
        assert!(should_write_plain_for_copy(
            true,
            ClipboardKind::Image,
            false
        ));
    }

    #[test]
    fn paste_plain_defaults_follow_item_kind() {
        assert!(should_write_plain_for_paste(
            false,
            ClipboardKind::Text,
            true,
            false,
        ));
        assert!(should_write_plain_for_paste(
            false,
            ClipboardKind::Files,
            false,
            true,
        ));
        assert!(!should_write_plain_for_paste(
            false,
            ClipboardKind::Files,
            true,
            false,
        ));
        assert!(!should_write_plain_for_paste(
            false,
            ClipboardKind::Image,
            true,
            true,
        ));
    }

    #[test]
    fn force_plain_paste_overrides_item_kind() {
        assert!(should_write_plain_for_paste(
            true,
            ClipboardKind::Image,
            false,
            false,
        ));
    }
}
