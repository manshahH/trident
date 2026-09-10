use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    App, Runtime,
};

use crate::windows::settings;

pub fn create<R: Runtime>(app: &App<R>) -> tauri::Result<()> {
    let settings = MenuItemBuilder::with_id("settings", "Settings").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&settings, &quit]).build()?;

    TrayIconBuilder::with_id("trident-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "settings" => {
                if let Err(error) = settings::show(app) {
                    tracing::error!(error = %error, "could not open settings window");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}
