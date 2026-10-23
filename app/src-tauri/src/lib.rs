mod progress;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            let store = progress::ProgressStore::open(&directory.join("crew-progress.sqlite"))
                .map_err(std::io::Error::other)?;
            app.manage(store);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            progress::load_progress,
            progress::save_progress
        ])
        .run(tauri::generate_context!())
        .expect("error while running TinkerCrew");
}
