//! Parent-approved manual app upgrades. No download, installer, feed, or arbitrary URL IPC.
use crate::connection::{self, Connections};
use serde::Serialize;

const RELEASES: &str = "https://github.com/josebarrueta/tinkercrew/releases";
#[derive(Serialize)]
pub struct AppUpdateInfo {
    version: &'static str,
}
#[tauri::command]
pub fn app_update_info() -> AppUpdateInfo {
    AppUpdateInfo {
        version: env!("CARGO_PKG_VERSION"),
    }
}
#[tauri::command]
pub async fn open_app_releases(state: tauri::State<'_, Connections>) -> Result<(), String> {
    let _permit = state.begin_maintenance()?;
    connection::disconnect_board(state)
        .await
        .map_err(|error| error.message)?;
    let status = tokio::time::timeout(std::time::Duration::from_secs(3), tokio::process::Command::new("/usr/bin/open").arg(RELEASES).kill_on_drop(true).status()).await.map_err(|_| "Opening the official releases page timed out; no app was installed.")?.map_err(|_| "Open https://github.com/josebarrueta/tinkercrew/releases manually; no app was installed.")?;
    if !status.success() {
        return Err(
            "The browser could not open the official releases page; no app was installed.".into(),
        );
    }
    Ok(())
}
