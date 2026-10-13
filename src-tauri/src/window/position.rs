use tauri::{PhysicalPosition, PhysicalRect, PhysicalSize, WebviewWindow};

use crate::core::Result;
use crate::settings::WindowPosition;

/// 底部停靠面板的最小逻辑高度，保证顶栏 + 一行卡片仍可容纳。
const DOCK_MIN_LOGICAL_HEIGHT: f64 = 280.0;

struct MonitorInfo {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    work_area: PhysicalRect<i32, u32>,
    scale_factor: f64,
}

fn monitor_from_cursor(
    window: &WebviewWindow,
) -> Result<Option<(MonitorInfo, PhysicalPosition<f64>)>> {
    let cursor = window.cursor_position().map_err(|e| anyhow::anyhow!(e))?;
    let scale = window.scale_factor().map_err(|e| anyhow::anyhow!(e))?;

    let logical = cursor.to_logical::<f64>(scale);

    let monitor = window
        .monitor_from_point(logical.x, logical.y)
        .map_err(|e| anyhow::anyhow!(e))?;

    let Some(monitor) = monitor else {
        return Ok(None);
    };

    Ok(Some((
        MonitorInfo {
            position: *monitor.position(),
            size: *monitor.size(),
            work_area: *monitor.work_area(),
            scale_factor: monitor.scale_factor(),
        },
        cursor,
    )))
}

pub fn position_window(window: &WebviewWindow, position: WindowPosition) -> Result<()> {
    let Some((monitor, cursor)) = monitor_from_cursor(window)? else {
        return Ok(());
    };

    match position {
        WindowPosition::Remember => {}
        WindowPosition::FollowCursor => apply_follow(window, &monitor, &cursor)?,
        WindowPosition::Center => apply_center(window, &monitor)?,
    }

    Ok(())
}

/// 底部停靠：贴光标所在显示器工作区底边、整宽，高度按百分比取自工作区高度。
/// 使用 `work_area` 而非整屏尺寸，避免盖住 macOS Dock / Windows 任务栏。
pub fn apply_dock(window: &WebviewWindow, height_percent: u8) -> Result<()> {
    let Some((monitor, _)) = monitor_from_cursor(window)? else {
        return Ok(());
    };

    let (size, position) = dock_geometry(&monitor.work_area, monitor.scale_factor, height_percent);

    window.set_size(size).map_err(|e| anyhow::anyhow!(e))?;
    window
        .set_position(position)
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

/// 计算停靠面板的物理尺寸与左上角位置。
/// 高度不低于 [`DOCK_MIN_LOGICAL_HEIGHT`]，也不超过工作区高度；
/// Windows 宽度减 1 像素，避免精确整宽被系统当作全屏窗口触发勿扰。
fn dock_geometry(
    work_area: &PhysicalRect<i32, u32>,
    scale_factor: f64,
    height_percent: u8,
) -> (PhysicalSize<u32>, PhysicalPosition<i32>) {
    let area_height = work_area.size.height as f64;
    let min_height = (DOCK_MIN_LOGICAL_HEIGHT * scale_factor).round();
    let height = (area_height * f64::from(height_percent) / 100.0)
        .round()
        .max(min_height)
        .min(area_height) as u32;
    let width = dock_width(work_area.size.width);

    let x = work_area.position.x;
    let y = work_area.position.y + (work_area.size.height - height) as i32;

    (
        PhysicalSize::new(width, height),
        PhysicalPosition::new(x, y),
    )
}

#[cfg(target_os = "windows")]
fn dock_width(area_width: u32) -> u32 {
    area_width.saturating_sub(1)
}

#[cfg(not(target_os = "windows"))]
fn dock_width(area_width: u32) -> u32 {
    area_width
}

fn apply_follow(
    window: &WebviewWindow,
    monitor: &MonitorInfo,
    cursor: &PhysicalPosition<f64>,
) -> Result<()> {
    let win_size = window.inner_size().map_err(|e| anyhow::anyhow!(e))?;
    let mon_x = monitor.position.x as f64;
    let mon_y = monitor.position.y as f64;
    let mon_w = monitor.size.width as f64;
    let mon_h = monitor.size.height as f64;

    let x = cursor.x.min(mon_x + mon_w - win_size.width as f64);
    let y = cursor.y.min(mon_y + mon_h - win_size.height as f64);

    window
        .set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32))
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

/// 将窗口居中到当前光标所在显示器。
/// 用于存档位置已失效（显示器被拔出）时的 fallback。
pub(super) fn center_on_cursor_monitor(window: &WebviewWindow) -> Result<()> {
    let Some((monitor, _)) = monitor_from_cursor(window)? else {
        return Ok(());
    };
    apply_center(window, &monitor)
}

fn apply_center(window: &WebviewWindow, monitor: &MonitorInfo) -> Result<()> {
    let win_size = window.inner_size().map_err(|e| anyhow::anyhow!(e))?;
    let mon_x = monitor.position.x as f64;
    let mon_y = monitor.position.y as f64;
    let mon_w = monitor.size.width as f64;
    let mon_h = monitor.size.height as f64;

    let x = mon_x + (mon_w - win_size.width as f64) / 2.0;
    let y = mon_y + (mon_h - win_size.height as f64) / 2.0;

    window
        .set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32))
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work_area() -> PhysicalRect<i32, u32> {
        PhysicalRect {
            position: PhysicalPosition::new(0, 50),
            size: PhysicalSize::new(2880, 1700),
        }
    }

    #[test]
    fn dock_geometry_uses_percent_of_work_area_and_sticks_to_bottom() {
        let (size, position) = dock_geometry(&work_area(), 2.0, 40);

        assert_eq!(size.height, 680);
        assert_eq!(size.width, dock_width(2880));
        assert_eq!(position.x, 0);
        assert_eq!(position.y, 50 + 1700 - 680);
    }

    #[test]
    fn dock_geometry_enforces_minimum_logical_height() {
        let small = PhysicalRect {
            position: PhysicalPosition::new(0, 0),
            size: PhysicalSize::new(1440, 800),
        };
        let (size, position) = dock_geometry(&small, 1.0, 25);

        assert_eq!(size.height, 280);
        assert_eq!(position.y, 800 - 280);
    }

    #[test]
    fn dock_geometry_never_exceeds_work_area_height() {
        let tiny = PhysicalRect {
            position: PhysicalPosition::new(0, 0),
            size: PhysicalSize::new(800, 400),
        };
        let (size, position) = dock_geometry(&tiny, 2.0, 60);

        assert_eq!(size.height, 400);
        assert_eq!(position.y, 0);
    }
}
