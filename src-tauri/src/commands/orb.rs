use tauri::{State, WebviewWindow};

use crate::{
    error::Result,
    state::AppState,
    windows::{orb, positioning::DockDto},
};

#[tauri::command]
#[specta::specta]
pub fn orb_begin_drag(window: WebviewWindow) -> Result<()> {
    let result = orb::begin_drag(&window);

    if let Err(error) = &result {
        tracing::error!(error = %error, "orb drag could not start");
    }

    result
}

#[tauri::command]
#[specta::specta]
pub fn orb_dropped(
    window: WebviewWindow,
    state: State<'_, AppState>,
    physical_x: i32,
    physical_y: i32,
) -> Result<DockDto> {
    let result = orb::dropped(&window, &state, physical_x, physical_y);

    if let Err(error) = &result {
        tracing::error!(error = %error, "orb position could not be saved");
    }

    result
}
