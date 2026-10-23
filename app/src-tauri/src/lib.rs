mod admin_process;
mod connection;
mod discovery;
mod management;
mod progress;
mod recovery;
mod setup;

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
            let recovery_directory = directory.join("board-recovery");
            std::fs::create_dir_all(&recovery_directory)?;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&recovery_directory, std::fs::Permissions::from_mode(0o700))?;
            let connections = connection::Connections::new(directory);
            if recovery_directory
                .join("factory-restore-active.json")
                .exists()
            {
                connections
                    .maintenance
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            }
            app.manage(connections);
            app.manage(management::Management::default());
            app.manage(recovery::Recovery::new(recovery_directory));
            app.manage(setup::SetupStore::default());
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
            discovery::board_candidates,
            setup::load_setup,
            setup::save_setup,
            setup::inspect_setup_usb,
            setup::open_official_setup,
            setup::open_board_resource,
            management::admin_inspect_usb,
            management::admin_enable_ssh,
            management::admin_login,
            management::admin_action,
            management::admin_logout,
            management::admin_forget_ssh_trust,
            recovery::recovery_images,
            recovery::prepare_factory_restore,
            recovery::start_factory_restore,
            recovery::factory_restore_status,
            recovery::verify_factory_boot,
            connection::forget_board
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tinkeroo");
}
