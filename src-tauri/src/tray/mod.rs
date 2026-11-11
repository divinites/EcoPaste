//! 系统托盘：Rust 侧实现。
//!
//! - icon 沿用 `assets/tray.ico`（Windows）/ `assets/tray-mac.ico`（macOS），作为 Tauri 资源打包。
//! - 菜单文案做 i18n，跟随 `Appearance.language` 即时切换（见 [`crate::i18n::tray`]）。
//! - 显隐跟随 `General.tray_icon`；语言或显隐变更后由 `commands/settings.rs` 调用 [`apply`] 同步。

use anyhow::Context;
use tauri::image::Image;
use tauri::menu::{Menu, MenuBuilder, MenuItem, PredefinedMenuItem, SubmenuBuilder};
use tauri::path::BaseDirectory;
use tauri::tray::TrayIconBuilder;
#[cfg(target_os = "windows")]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::clipboard::{ClipboardPauseState, WatcherPause};
use crate::core::Result;
use crate::i18n::tray as tray_i18n;
use crate::i18n::tray::Key;
use crate::settings::{Language, Settings};
#[cfg(target_os = "windows")]
use crate::window::CLIPBOARD_WINDOW_LABEL;
use crate::window::{self, PREFERENCE_WINDOW_LABEL};

const TRAY_ID: &str = "app-tray";
const GITHUB_URL: &str = "https://github.com/EcoPasteHub/EcoPaste";

const MENU_PREFERENCE: &str = "tray::preference";
const MENU_RESUME_LISTEN: &str = "tray::resume_listen";
const MENU_PAUSE_5: &str = "tray::pause_5";
const MENU_PAUSE_30: &str = "tray::pause_30";
const MENU_PAUSE_60: &str = "tray::pause_60";
const MENU_PAUSE_FOREVER: &str = "tray::pause_forever";
const MENU_SKIP_NEXT: &str = "tray::skip_next";
const MENU_OPEN_SOURCE: &str = "tray::open_source";
const MENU_CHECK_FOR_UPDATES: &str = "tray::check_for_updates";
const MENU_RELAUNCH: &str = "tray::relaunch";
const MENU_EXIT: &str = "tray::exit";

pub fn init(app: &AppHandle, settings: &Settings) -> Result<()> {
    let lang = settings.appearance.language;
    let version = app.package_info().version.to_string();
    let paused = pause_snapshot(app);

    let icon = load_icon(app)?;
    let menu = build_menu(app, lang, &version, &paused)?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .tooltip(format!("EcoPaste v{version}"))
        .menu(&menu)
        .on_menu_event(|app, event| handle_menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            // macOS 左键已经走 show_menu_on_left_click，不在这里处理；
            // Windows 左键单击显剪贴板窗口。
            #[cfg(target_os = "windows")]
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle().clone();
                if let Err(err) = window::show_window(&app, CLIPBOARD_WINDOW_LABEL) {
                    log::error!("tray left-click show main failed: {err:?}");
                }
            }
            #[cfg(not(target_os = "windows"))]
            let _ = (tray, event);
        })
        .build(app)
        .context("build tray icon failed")?;

    if let Err(err) = tray.set_visible(settings.general.tray_icon) {
        log::warn!("tray set_visible on init failed: {err}");
    }

    Ok(())
}

/// 根据最新 settings 更新菜单和可见性。语言变了就重建菜单；显隐位变了就 set_visible。
pub fn apply(app: &AppHandle, settings: &Settings) -> Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let version = app.package_info().version.to_string();
    let paused = pause_snapshot(app);
    let menu = build_menu(app, settings.appearance.language, &version, &paused)?;
    tray.set_menu(Some(menu)).context("tray set_menu failed")?;
    tray.set_visible(settings.general.tray_icon)
        .context("tray set_visible failed")?;
    Ok(())
}

/// 当前用户暂停状态；监听尚未初始化时视为未暂停。
fn pause_snapshot(app: &AppHandle) -> ClipboardPauseState {
    app.try_state::<WatcherPause>()
        .map(|pause| pause.snapshot())
        .unwrap_or(ClipboardPauseState {
            paused: false,
            until: None,
            skip_next: false,
        })
}

/// 重建菜单但不动可见性，用于暂停状态变化后只刷文案。
pub fn refresh_menu(app: &AppHandle) -> Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let lang = app
        .try_state::<crate::settings::SettingsStore>()
        .map(|s| s.snapshot().appearance.language)
        .unwrap_or_default();
    let version = app.package_info().version.to_string();
    let paused = pause_snapshot(app);
    let menu = build_menu(app, lang, &version, &paused)?;
    tray.set_menu(Some(menu)).context("tray set_menu failed")?;
    Ok(())
}

fn load_icon(app: &AppHandle) -> Result<Image<'static>> {
    let relative = if cfg!(target_os = "macos") {
        "assets/tray-mac.ico"
    } else {
        "assets/tray.ico"
    };
    let path = app
        .path()
        .resolve(relative, BaseDirectory::Resource)
        .with_context(|| format!("resolve tray icon resource {relative}"))?;
    Ok(Image::from_path(&path).with_context(|| format!("load tray icon from {path:?}"))?)
}

fn build_menu(
    app: &AppHandle,
    lang: Language,
    version: &str,
    paused: &ClipboardPauseState,
) -> Result<Menu<tauri::Wry>> {
    let preference = MenuItem::with_id(
        app,
        MENU_PREFERENCE,
        tray_i18n::label(lang, Key::Preference),
        true,
        None::<&str>,
    )
    .context("build preference menu item")?;
    let skip_next = MenuItem::with_id(
        app,
        MENU_SKIP_NEXT,
        tray_i18n::label(
            lang,
            if paused.skip_next {
                Key::CancelSkipNextCopy
            } else {
                Key::SkipNextCopy
            },
        ),
        !paused.paused,
        None::<&str>,
    )
    .context("build skip_next menu item")?;
    let open_source = MenuItem::with_id(
        app,
        MENU_OPEN_SOURCE,
        tray_i18n::label(lang, Key::OpenSourceAddress),
        true,
        None::<&str>,
    )
    .context("build open_source menu item")?;
    let check_for_updates = MenuItem::with_id(
        app,
        MENU_CHECK_FOR_UPDATES,
        tray_i18n::label(lang, Key::CheckForUpdates),
        true,
        None::<&str>,
    )
    .context("build check_for_updates menu item")?;
    let version_item = MenuItem::with_id(
        app,
        "tray::version",
        format!("{} {}", tray_i18n::label(lang, Key::Version), version),
        false,
        None::<&str>,
    )
    .context("build version menu item")?;
    let relaunch = MenuItem::with_id(
        app,
        MENU_RELAUNCH,
        tray_i18n::label(lang, Key::Relaunch),
        true,
        None::<&str>,
    )
    .context("build relaunch menu item")?;
    let exit = MenuItem::with_id(
        app,
        MENU_EXIT,
        tray_i18n::label(lang, Key::Exit),
        true,
        None::<&str>,
    )
    .context("build exit menu item")?;
    let sep1 = PredefinedMenuItem::separator(app).context("build separator")?;
    let sep2 = PredefinedMenuItem::separator(app).context("build separator")?;

    let builder = MenuBuilder::new(app).item(&preference);
    let builder = if paused.paused {
        let resume_label = match paused.until {
            Some(until) => format!(
                "{} ({} {})",
                tray_i18n::label(lang, Key::StartListening),
                tray_i18n::label(lang, Key::PausedUntil),
                until.with_timezone(&chrono::Local).format("%H:%M")
            ),
            None => tray_i18n::label(lang, Key::StartListening).to_owned(),
        };
        let resume = MenuItem::with_id(app, MENU_RESUME_LISTEN, resume_label, true, None::<&str>)
            .context("build resume menu item")?;
        builder.item(&resume)
    } else {
        let pause_menu = SubmenuBuilder::new(app, tray_i18n::label(lang, Key::PauseListening))
            .text(MENU_PAUSE_5, tray_i18n::label(lang, Key::PauseFiveMinutes))
            .text(
                MENU_PAUSE_30,
                tray_i18n::label(lang, Key::PauseThirtyMinutes),
            )
            .text(MENU_PAUSE_60, tray_i18n::label(lang, Key::PauseOneHour))
            .separator()
            .text(
                MENU_PAUSE_FOREVER,
                tray_i18n::label(lang, Key::PauseUntilResumed),
            )
            .build()
            .context("build pause submenu")?;
        builder.item(&pause_menu)
    };

    builder
        .items(&[
            &skip_next,
            &sep1,
            &open_source,
            &check_for_updates,
            &sep2,
            &version_item,
            &relaunch,
            &exit,
        ])
        .build()
        .context("build tray menu")
        .map_err(Into::into)
}

fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        MENU_PREFERENCE => {
            if let Err(err) = window::show_window(app, PREFERENCE_WINDOW_LABEL) {
                log::error!("tray open preference failed: {err:?}");
            }
        }
        MENU_RESUME_LISTEN => {
            crate::clipboard::apply_user_pause(app, None);
        }
        MENU_PAUSE_5 => {
            crate::clipboard::apply_user_pause(app, Some(5));
        }
        MENU_PAUSE_30 => {
            crate::clipboard::apply_user_pause(app, Some(30));
        }
        MENU_PAUSE_60 => {
            crate::clipboard::apply_user_pause(app, Some(60));
        }
        MENU_PAUSE_FOREVER => {
            crate::clipboard::apply_user_pause(app, Some(0));
        }
        MENU_SKIP_NEXT => {
            let skip = !pause_snapshot(app).skip_next;
            crate::clipboard::apply_skip_next(app, skip);
        }
        MENU_OPEN_SOURCE => {
            if let Err(err) = app.opener().open_url(GITHUB_URL, None::<&str>) {
                log::error!("tray open url failed: {err:?}");
            }
        }
        MENU_CHECK_FOR_UPDATES => {
            if let Err(err) = window::show_window(app, window::UPDATE_WINDOW_LABEL) {
                log::error!("tray open update window failed: {err:?}");
            }
        }
        MENU_RELAUNCH => app.restart(),
        MENU_EXIT => app.exit(0),
        _ => {}
    }
}
