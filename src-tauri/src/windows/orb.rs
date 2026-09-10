use std::fmt::Display;

use tauri::{App, Manager, Monitor, Runtime, WebviewWindow, WindowEvent};

use crate::{
    error::{AppError, Result},
    platform::win32,
    state::AppState,
};

use super::positioning::{
    clamp, dock, resolve_saved_position, DockDto, MonitorWorkArea, OrbPosition, PhysicalPoint,
    PhysicalSize, WorkArea,
};

const ORB_POSITION_SETTING_KEY: &str = "orb.position";

pub fn initialize<R: Runtime>(app: &App<R>) -> Result<()> {
    let window = app
        .get_webview_window("orb")
        .ok_or_else(|| AppError::WindowMissing("orb".to_owned()))?;
    let hwnd = window
        .hwnd()
        .map_err(|error| window_error("WebviewWindow::hwnd", error))?;
    win32::set_no_activate(hwnd.0)?;

    let state = app.state::<AppState>().inner().clone();
    restore_saved_position(&window, &state)?;
    listen_for_window_events(&window, state);
    window
        .show()
        .map_err(|error| window_error("WebviewWindow::show", error))?;
    Ok(())
}

pub fn begin_drag<R: Runtime>(window: &WebviewWindow<R>) -> Result<()> {
    ensure_orb_window(window)?;
    window
        .start_dragging()
        .map_err(|error| window_error("WebviewWindow::start_dragging", error))
}

pub fn dropped<R: Runtime>(
    window: &WebviewWindow<R>,
    state: &AppState,
    physical_x: i32,
    physical_y: i32,
) -> Result<DockDto> {
    ensure_orb_window(window)?;
    let monitor = current_monitor_work_area(window)?;
    let docked = dock(
        PhysicalPoint::new(physical_x, physical_y),
        window_size(window)?,
        &monitor.name,
        monitor.work_area,
    );
    set_window_position(window, &docked.position)?;
    save_position(state, &docked.position)?;
    Ok(DockDto::from(docked.position))
}

fn listen_for_window_events<R: Runtime>(window: &WebviewWindow<R>, state: AppState) {
    let event_window = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Moved(position) => {
            if let Err(error) = persist_moved_position(&event_window, &state, position.x, position.y) {
                tracing::error!(error = %error, "could not persist orb position after move");
            }
        }
        WindowEvent::ScaleFactorChanged { .. } => {
            if let Err(error) = restore_saved_position(&event_window, &state) {
                tracing::error!(error = %error, "could not restore orb position after display scale change");
            }
        }
        _ => {}
    });
}

fn persist_moved_position<R: Runtime>(
    window: &WebviewWindow<R>,
    state: &AppState,
    physical_x: i32,
    physical_y: i32,
) -> Result<()> {
    let monitor = current_monitor_work_area(window)?;
    let clamped = clamp(
        PhysicalPoint::new(physical_x, physical_y),
        window_size(window)?,
        monitor.work_area,
    );
    let edge = load_position(state)?.and_then(|position| position.edge);
    let position = OrbPosition::new(monitor.name, clamped.x, clamped.y, edge);

    if position.physical_x != physical_x || position.physical_y != physical_y {
        set_window_position(window, &position)?;
    }
    save_position(state, &position)
}

fn restore_saved_position<R: Runtime>(window: &WebviewWindow<R>, state: &AppState) -> Result<()> {
    let (monitors, primary_monitor_name) = available_monitor_work_areas(window)?;
    let size = window_size(window)?;
    let saved_position = load_position(state)?;
    let position = resolve_saved_position(
        &monitors,
        &primary_monitor_name,
        saved_position.as_ref(),
        size,
    )
    .ok_or_else(|| AppError::Platform {
        api: "WebviewWindow::available_monitors".to_owned(),
        code: 0,
    })?;

    set_window_position(window, &position)?;
    save_position(state, &position)
}

fn available_monitor_work_areas<R: Runtime>(
    window: &WebviewWindow<R>,
) -> Result<(Vec<MonitorWorkArea>, String)> {
    let primary_monitor = window
        .primary_monitor()
        .map_err(|error| window_error("WebviewWindow::primary_monitor", error))?
        .ok_or_else(|| AppError::Platform {
            api: "WebviewWindow::primary_monitor".to_owned(),
            code: 0,
        })?;
    let primary = monitor_work_area(primary_monitor)?;
    let mut monitors = Vec::new();

    for monitor in window
        .available_monitors()
        .map_err(|error| window_error("WebviewWindow::available_monitors", error))?
    {
        monitors.push(monitor_work_area(monitor)?);
    }

    if !monitors.iter().any(|monitor| monitor.name == primary.name) {
        monitors.push(primary.clone());
    }

    Ok((monitors, primary.name))
}

fn current_monitor_work_area<R: Runtime>(window: &WebviewWindow<R>) -> Result<MonitorWorkArea> {
    let current_monitor = window
        .current_monitor()
        .map_err(|error| window_error("WebviewWindow::current_monitor", error))?;
    let monitor = match current_monitor {
        Some(monitor) => monitor,
        None => window
            .primary_monitor()
            .map_err(|error| window_error("WebviewWindow::primary_monitor", error))?
            .ok_or_else(|| AppError::Platform {
                api: "WebviewWindow::current_monitor".to_owned(),
                code: 0,
            })?,
    };
    monitor_work_area(monitor)
}

fn monitor_work_area(monitor: Monitor) -> Result<MonitorWorkArea> {
    let name = monitor.name().cloned().ok_or_else(|| AppError::Platform {
        api: "Monitor::name".to_owned(),
        code: 0,
    })?;
    let work_area = monitor.work_area();
    let width = i32::try_from(work_area.size.width)
        .map_err(|error| AppError::Internal(format!("Monitor::work_area width: {error}")))?;
    let height = i32::try_from(work_area.size.height)
        .map_err(|error| AppError::Internal(format!("Monitor::work_area height: {error}")))?;
    let right = work_area
        .position
        .x
        .checked_add(width)
        .ok_or_else(|| AppError::Platform {
            api: "Monitor::work_area".to_owned(),
            code: 0,
        })?;
    let bottom = work_area
        .position
        .y
        .checked_add(height)
        .ok_or_else(|| AppError::Platform {
            api: "Monitor::work_area".to_owned(),
            code: 0,
        })?;

    Ok(MonitorWorkArea::new(
        name,
        WorkArea::new(work_area.position.x, work_area.position.y, right, bottom),
    ))
}

fn window_size<R: Runtime>(window: &WebviewWindow<R>) -> Result<PhysicalSize> {
    let size = window
        .outer_size()
        .map_err(|error| window_error("WebviewWindow::outer_size", error))?;
    Ok(PhysicalSize::new(size.width, size.height))
}

fn set_window_position<R: Runtime>(
    window: &WebviewWindow<R>,
    position: &OrbPosition,
) -> Result<()> {
    window
        .set_position(tauri::PhysicalPosition::new(
            position.physical_x,
            position.physical_y,
        ))
        .map_err(|error| window_error("WebviewWindow::set_position", error))
}

fn load_position(state: &AppState) -> Result<Option<OrbPosition>> {
    let value = state.database().setting_value(ORB_POSITION_SETTING_KEY)?;
    let Some(value) = value else {
        return Ok(None);
    };

    match serde_json::from_str(&value) {
        Ok(position) => Ok(Some(position)),
        Err(_) => {
            tracing::warn!(
                setting = ORB_POSITION_SETTING_KEY,
                "ignoring malformed saved orb position"
            );
            Ok(None)
        }
    }
}

fn save_position(state: &AppState, position: &OrbPosition) -> Result<()> {
    let value = serde_json::to_string(position)
        .map_err(|error| AppError::Internal(format!("orb position serialization: {error}")))?;
    state
        .database()
        .set_setting_value(ORB_POSITION_SETTING_KEY, &value)
}

fn ensure_orb_window<R: Runtime>(window: &WebviewWindow<R>) -> Result<()> {
    ensure_orb_label(window.label())
}

fn ensure_orb_label(label: &str) -> Result<()> {
    if label == "orb" {
        Ok(())
    } else {
        Err(AppError::InvalidState(
            "orb command invoked from a non-orb window".to_owned(),
        ))
    }
}

fn window_error(api: &str, error: impl Display) -> AppError {
    AppError::Internal(format!("{api}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::ensure_orb_label;

    #[test]
    fn rejects_orb_commands_from_other_windows() {
        let result = ensure_orb_label("panel");

        assert!(matches!(
            result,
            Err(crate::error::AppError::InvalidState(_))
        ));
    }
}
