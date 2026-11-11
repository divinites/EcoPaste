mod app_store;
mod apps_registry;
mod cleanup;
mod content_filter;
mod detect;
mod file_icon_store;
mod groups;
mod guard;
mod icon;
mod ingest;
mod pause;
mod payload;
mod read;
mod secrets;
mod sound;
mod source;
mod storage;
mod view;
mod watcher;
mod write;

pub use app_store::AppIconStore;
pub use apps_registry::{
    add_app_from_path, delete_unreferenced_apps, refresh_running_apps, AppsRegistry,
};
pub use content_filter::{is_excluded_by_pattern, validate_excluded_patterns};
pub use detect::sanitize_css_color;
pub use file_icon_store::FileIconStore;
pub use groups::{
    normalize_group_icon, normalize_group_name, normalize_group_svg, MAX_GROUP_ICON_BYTES,
};
pub use guard::WritebackGuard;
pub use icon::{get_icon_cache_key, icon_png, DIR_CACHE_KEY};
#[cfg(test)]
pub use ingest::build_item;
pub use ingest::build_item_with_settings;
pub use pause::{
    apply_skip_next, apply_user_pause, ClipboardPauseState, WatcherPause, WatcherSuspendGuard,
};
pub use payload::{ClipboardPayload, ImagePayload, TextPayload};
pub use read::ClipboardReader;
pub use sound::play_copy_sound;
pub use source::detect_frontmost;
pub use storage::ImageStore;
pub use view::{
    build_clipboard_app_view, build_clipboard_preview_payload, default_saved_image_file_name,
    enrich_list_item, merge_clipboard_apps, resolve_file_icon_path, sort_clipboard_apps,
    validate_image_file_name, ClipboardAppView, ClipboardPreviewPayload, ListItemContext,
};
pub use watcher::{init, materialize_source, persist_and_notify};
pub use write::write_to_clipboard;

#[cfg(test)]
pub(crate) mod test_lock {
    use std::sync::{Mutex, MutexGuard};

    /// 系统剪贴板是单一全局资源，触碰它的测试不能并行（否则相互覆盖内容而 flaky）。
    /// 这些测试统一持有此锁串行执行，即使用默认多线程 runner 也稳定。
    static LOCK: Mutex<()> = Mutex::new(());

    /// 获取串行锁。即便上一个持锁测试 panic 导致锁中毒，也恢复使用——
    /// 锁仅用于串行化，内部 `()` 无状态可破坏。
    pub fn serial() -> MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
