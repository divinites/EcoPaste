use tauri::{Monitor, PhysicalPosition, PhysicalRect, PhysicalSize, WebviewWindow};

use crate::core::Result;
use crate::settings::{DockMonitor, WindowPosition, DOCK_HEIGHT_PERCENT_RANGE};

/// 底部停靠面板的最小逻辑高度，保证顶栏 + 一行卡片仍可容纳。
const DOCK_MIN_LOGICAL_HEIGHT: f64 = 280.0;

struct MonitorInfo {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    work_area: PhysicalRect<i32, u32>,
    scale_factor: f64,
}

impl From<&Monitor> for MonitorInfo {
    fn from(monitor: &Monitor) -> Self {
        Self {
            position: *monitor.position(),
            size: *monitor.size(),
            work_area: *monitor.work_area(),
            scale_factor: monitor.scale_factor(),
        }
    }
}

/// 把物理坐标点换成 `monitor_from_point` 期望的坐标系：
/// macOS（tao 用 `CGDisplayBounds`）是逻辑点，Windows（`MonitorFromPoint`）是物理像素。
fn monitor_query_point(window: &WebviewWindow, point: PhysicalPosition<f64>) -> Result<(f64, f64)> {
    if cfg!(target_os = "windows") {
        return Ok((point.x, point.y));
    }

    let scale = window.scale_factor().map_err(|e| anyhow::anyhow!(e))?;
    let logical = point.to_logical::<f64>(scale);
    Ok((logical.x, logical.y))
}

fn monitor_from_cursor(
    window: &WebviewWindow,
) -> Result<Option<(MonitorInfo, PhysicalPosition<f64>)>> {
    let cursor = window.cursor_position().map_err(|e| anyhow::anyhow!(e))?;
    let (x, y) = monitor_query_point(window, cursor)?;

    let monitor = window
        .monitor_from_point(x, y)
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok(monitor.map(|monitor| (MonitorInfo::from(&monitor), cursor)))
}

/// 停靠面板要定位到哪块显示器。
#[derive(Debug, Clone, Copy)]
pub enum DockTarget {
    /// 呼出时：按设置策略解析。
    Strategy(DockMonitor),
    /// 可见时重排 / 拖拽调高：留在窗口当前所在显示器，避免跳屏。
    Current,
}

/// 停靠面板高度来源。
#[derive(Debug, Clone, Copy)]
pub enum DockHeight {
    Percent(u8),
    LogicalPx(f64),
}

/// 按目标解析显示器；策略取不到时一律回落到光标所在显示器。
fn resolve_dock_monitor(window: &WebviewWindow, target: DockTarget) -> Result<Option<MonitorInfo>> {
    let resolved = match target {
        DockTarget::Strategy(DockMonitor::Cursor) => None,
        DockTarget::Strategy(DockMonitor::Primary) => window
            .primary_monitor()
            .map_err(|e| anyhow::anyhow!(e))?
            .map(|monitor| MonitorInfo::from(&monitor)),
        DockTarget::Strategy(DockMonitor::ActiveWindow) => match active_window_center() {
            Some((x, y)) => window
                .monitor_from_point(x, y)
                .map_err(|e| anyhow::anyhow!(e))?
                .map(|monitor| MonitorInfo::from(&monitor)),
            None => None,
        },
        DockTarget::Current => window
            .current_monitor()
            .map_err(|e| anyhow::anyhow!(e))?
            .map(|monitor| MonitorInfo::from(&monitor)),
    };

    if resolved.is_some() {
        return Ok(resolved);
    }

    Ok(monitor_from_cursor(window)?.map(|(monitor, _)| monitor))
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

/// 底部停靠：贴目标显示器工作区底边、整宽，高度按百分比或像素取值。
/// 使用 `work_area` 而非整屏尺寸，避免盖住 macOS Dock / Windows 任务栏。
/// 返回实际生效高度对应的整数百分比，供拖拽调高落盘。
pub fn apply_dock(
    window: &WebviewWindow,
    target: DockTarget,
    height: DockHeight,
) -> Result<Option<u8>> {
    let Some(monitor) = resolve_dock_monitor(window, target)? else {
        return Ok(None);
    };

    let (size, position) = dock_geometry(&monitor.work_area, monitor.scale_factor, height);

    window.set_size(size).map_err(|e| anyhow::anyhow!(e))?;
    window
        .set_position(position)
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(Some(height_to_percent(
        monitor.work_area.size.height,
        size.height,
    )))
}

/// 计算停靠面板的物理尺寸与左上角位置。
/// 高度不低于 [`DOCK_MIN_LOGICAL_HEIGHT`]，也不超过工作区高度；像素高度另钳到百分比上下限内。
/// Windows 宽度减 1 像素，避免精确整宽被系统当作全屏窗口触发勿扰。
fn dock_geometry(
    work_area: &PhysicalRect<i32, u32>,
    scale_factor: f64,
    height: DockHeight,
) -> (PhysicalSize<u32>, PhysicalPosition<i32>) {
    let area_height = work_area.size.height as f64;
    let min_height = (DOCK_MIN_LOGICAL_HEIGHT * scale_factor).round();
    let percent_height = |percent: u8| area_height * f64::from(percent) / 100.0;
    let raw = match height {
        DockHeight::Percent(percent) => percent_height(percent),
        DockHeight::LogicalPx(px) => (px * scale_factor).clamp(
            percent_height(*DOCK_HEIGHT_PERCENT_RANGE.start()),
            percent_height(*DOCK_HEIGHT_PERCENT_RANGE.end()),
        ),
    };
    let height = raw.round().max(min_height).min(area_height) as u32;
    let width = dock_width(work_area.size.width);

    let x = work_area.position.x;
    let y = work_area.position.y + (work_area.size.height - height) as i32;

    (
        PhysicalSize::new(width, height),
        PhysicalPosition::new(x, y),
    )
}

/// 物理高度 → 整数百分比，四舍五入后钳到 [`DOCK_HEIGHT_PERCENT_RANGE`]。
fn height_to_percent(work_area_height: u32, height: u32) -> u8 {
    if work_area_height == 0 {
        return *DOCK_HEIGHT_PERCENT_RANGE.start();
    }

    let percent = (f64::from(height) * 100.0 / f64::from(work_area_height)).round();
    percent.clamp(
        f64::from(*DOCK_HEIGHT_PERCENT_RANGE.start()),
        f64::from(*DOCK_HEIGHT_PERCENT_RANGE.end()),
    ) as u8
}

/// 前台应用窗口中心点，坐标系与 `monitor_from_point` 一致；取不到时返回 `None`。
#[cfg(target_os = "macos")]
fn active_window_center() -> Option<(f64, f64)> {
    super::macos::frontmost_window_center()
}

#[cfg(target_os = "windows")]
fn active_window_center() -> Option<(f64, f64)> {
    super::windows::foreground_window_center()
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
        let (size, position) = dock_geometry(&work_area(), 2.0, DockHeight::Percent(40));

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
        let (size, position) = dock_geometry(&small, 1.0, DockHeight::Percent(25));

        assert_eq!(size.height, 280);
        assert_eq!(position.y, 800 - 280);
    }

    #[test]
    fn dock_geometry_never_exceeds_work_area_height() {
        let tiny = PhysicalRect {
            position: PhysicalPosition::new(0, 0),
            size: PhysicalSize::new(800, 400),
        };
        let (size, position) = dock_geometry(&tiny, 2.0, DockHeight::Percent(60));

        assert_eq!(size.height, 400);
        assert_eq!(position.y, 0);
    }

    #[test]
    fn dock_geometry_clamps_pixel_height_to_percent_range() {
        let (size, _) = dock_geometry(&work_area(), 2.0, DockHeight::LogicalPx(10_000.0));
        assert_eq!(size.height, 1020);

        let (size, _) = dock_geometry(&work_area(), 2.0, DockHeight::LogicalPx(10.0));
        assert_eq!(size.height, 560);

        let (size, _) = dock_geometry(&work_area(), 2.0, DockHeight::LogicalPx(400.0));
        assert_eq!(size.height, 800);
    }

    #[test]
    fn height_to_percent_rounds_and_clamps() {
        assert_eq!(height_to_percent(1700, 680), 40);
        assert_eq!(height_to_percent(1700, 1700), 60);
        assert_eq!(height_to_percent(1700, 10), 25);
        assert_eq!(height_to_percent(0, 10), 25);
    }
}
