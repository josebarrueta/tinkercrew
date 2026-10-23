mod connection;
mod discovery;
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
            app.manage(connection::Connections::new(directory));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            progress::load_progress,
            progress::save_progress,
            connection::register_board,
            connection::list_boards,
            connection::probe_board,
            connection::board_request,
            connection::disconnect_board,
            discovery::board_candidates
        ])
        .run(tauri::generate_context!())
        .expect("error while running TinkerCrew");
}
