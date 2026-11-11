use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::core::Result;
use crate::legacy::{
    LegacyImportSelection, OnboardingLegacyDataDetection, OnboardingLegacyImportResult,
};
use crate::settings::{Settings, SettingsStore};
use crate::window;

const MAX_ONBOARDING_STEP: u32 = 20;
const CLIPBOARD_UPDATED_EVENT: &str = "clipboard://updated";

#[tauri::command]
pub async fn open_onboarding(app: AppHandle) -> Result<()> {
    window::open_onboarding(&app)
}

#[tauri::command]
pub async fn set_onboarding_step(app: AppHandle, step: u32) -> Result<Settings> {
    update_onboarding_settings(
        &app,
        json!({
            "onboarding": {
                "lastStep": step.min(MAX_ONBOARDING_STEP),
            },
        }),
    )
}

#[tauri::command]
pub async fn finish_onboarding(app: AppHandle) -> Result<Settings> {
    let next = update_onboarding_settings(
        &app,
        json!({
            "onboarding": {
                "completed": true,
                "lastStep": 0,
            },
        }),
    )?;

    if let Err(err) = window::hide_window(&app, window::ONBOARDING_WINDOW_LABEL) {
        log::warn!("hide onboarding window after finish failed: {err}");
    }

    window::show_window(&app, window::CLIPBOARD_WINDOW_LABEL)?;

    Ok(next)
}

#[tauri::command]
pub async fn detect_legacy_data(app: AppHandle) -> Result<OnboardingLegacyDataDetection> {
    let detection = crate::legacy::inspect().await?;

    update_onboarding_settings(
        &app,
        json!({
            "onboarding": {
                "legacyImport": {
                    "checked": true,
                },
            },
        }),
    )?;

    Ok(detection)
}

/// 导入旧版数据后同步引导页设置，并一次性通知列表刷新。
#[tauri::command]
pub async fn import_legacy_data(
    app: AppHandle,
    types: Vec<LegacyImportSelection>,
) -> Result<OnboardingLegacyImportResult> {
    let result = crate::legacy::import(&app, types).await?;

    update_onboarding_settings(
        &app,
        json!({
            "onboarding": {
                "legacyImport": {
                    "checked": true,
                    "imported": result.imported > 0,
                    "importTypes": result.selected_types,
                    "importedAt": result.imported_at,
                },
            },
        }),
    )?;

    if let Err(err) = app.emit(
        CLIPBOARD_UPDATED_EVENT,
        json!({
            "imported": result.imported > 0,
        }),
    ) {
        log::warn!("emit {CLIPBOARD_UPDATED_EVENT} after legacy import failed: {err}");
    }

    Ok(result)
}

fn update_onboarding_settings(app: &AppHandle, patch: serde_json::Value) -> Result<Settings> {
    let next = app.state::<SettingsStore>().update(patch)?;

    super::settings::emit_settings_updated(app, &next);

    Ok(next)
}
