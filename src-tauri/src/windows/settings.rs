use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

use crate::error::{AppError, Result};

pub fn show<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    if let Some(window) = app.get_webview_window("settings") {
        window
            .show()
            .map_err(|error| AppError::Internal(error.to_string()))?;
        window
            .set_focus()
            .map_err(|error| AppError::Internal(error.to_string()))?;
        return Ok(());
    }

    WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html".into()))
        .title("Trident settings")
        .inner_size(480.0, 560.0)
        .build()
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(())
}
