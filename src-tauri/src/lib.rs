pub mod db;
pub mod error;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default().run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("Trident could not start: {error}");
    }
}
