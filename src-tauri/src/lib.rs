pub mod commands;
pub mod config;
pub mod db;
pub mod error;
pub mod logging;
pub mod platform;
pub mod state;
pub mod tray;
pub mod windows;

use tauri::{App, Manager, Runtime};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta =
        tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
            commands::orb::orb_begin_drag,
            commands::orb::orb_dropped,
            commands::system::health
        ]);

    #[cfg(debug_assertions)]
    if let Err(error) = specta.export(
        specta_typescript::Typescript::default()
            .bigint(specta_typescript::BigIntExportBehavior::Number),
        "../src/ipc/bindings.ts",
    ) {
        eprintln!("Trident could not generate IPC bindings: {error}");
    }

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if let Err(error) = initialize(app) {
                show_startup_error(app, &error);
            }
            Ok(())
        })
        .invoke_handler(specta.invoke_handler())
        .run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("Trident could not start: {error}");
    }
}

fn initialize<R: Runtime + 'static>(app: &mut App<R>) -> error::Result<()> {
    let paths = config::AppPaths::for_app(app)?;
    paths
        .ensure_directories()
        .map_err(|error| error::AppError::Io(format!("{}: {error}", paths.root().display())))?;
    let logging = logging::init(&paths.log_dir())
        .map_err(|error| error::AppError::Io(format!("{}: {error}", paths.log_dir().display())))?;
    logging::install_panic_hook(app.handle().clone(), paths.log_dir());
    let database = db::Db::open(&paths.database_path(), unix_millis()?).map_err(|error| {
        error::AppError::Db(format!("{}: {error}", paths.database_path().display()))
    })?;
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        database_path = %paths.database_path().display(),
        "Trident started"
    );
    app.manage(logging);
    app.manage(state::AppState::new(database));
    windows::orb::initialize(app)?;
    tray::create(app).map_err(|error| error::AppError::Internal(error.to_string()))?;
    Ok(())
}

fn show_startup_error<R: Runtime>(app: &App<R>, error: &error::AppError) {
    let handle = app.handle().clone();
    app.dialog()
        .message(error.to_string())
        .kind(MessageDialogKind::Error)
        .title("Trident could not start")
        .show(move |_| handle.exit(1));
}

fn unix_millis() -> error::Result<i64> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error::AppError::Internal(error.to_string()))?;
    i64::try_from(duration.as_millis())
        .map_err(|error| error::AppError::Internal(error.to_string()))
}
