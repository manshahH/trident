use tauri::State;

use crate::{
    error::Result,
    state::{AppState, HealthDto},
};

#[tauri::command]
#[specta::specta]
pub fn health(state: State<'_, AppState>) -> Result<HealthDto> {
    let result = state.health();

    if let Err(error) = &result {
        tracing::error!(error = %error, "health command failed");
    }

    result
}
