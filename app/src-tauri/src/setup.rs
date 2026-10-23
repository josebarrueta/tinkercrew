use security_framework::passwords::{get_generic_password, set_generic_password};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

const SERVICE: &str = "com.tinkercrew.desktop.setup";
const ACCOUNT: &str = "parent-checkpoints-v1";
const STAGES: [&str; 7] = [
    "connect-usb",
    "identify",
    "boot",
    "name",
    "credentials",
    "wifi",
    "network",
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Checkpoint {
    stage_id: String,
    source: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetupProgress {
    schema_version: u32,
    selected_serial: Option<String>,
    stage_index: usize,
    checkpoints: Vec<Checkpoint>,
}
fn validate_payload(payload: &str) -> Result<(), String> {
    if payload.len() > 16384 {
        return Err("Setup checkpoint data is too large.".into());
    }
    let progress: SetupProgress = serde_json::from_str(payload).map_err(
        |_| "Invalid setup checkpoint data. Passwords/credentials are never accepted here.",
    )?;
    let serial_valid = progress.selected_serial.as_ref().is_none_or(|serial| {
        !serial.is_empty()
            && serial.len() <= 128
            && serial.as_bytes()[0].is_ascii_alphanumeric()
            && serial
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
    });
    if progress.schema_version != 1
        || !serial_valid
        || progress.stage_index > STAGES.len()
        || progress.checkpoints.len() != progress.stage_index
        || (progress.stage_index > 0 && progress.selected_serial.is_none())
        || progress
            .checkpoints
            .iter()
            .enumerate()
            .any(|(index, checkpoint)| {
                checkpoint.stage_id != STAGES[index]
                    || checkpoint.source != if index == 0 { "observed" } else { "human" }
            })
    {
        return Err(
            "Unsupported setup checkpoints; existing progress has not been replaced.".into(),
        );
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct StoredSetup {
    payload: String,
    revision: u64,
}
#[derive(Default)]
pub struct SetupStore(Mutex<()>);

fn read_setup() -> Result<Option<StoredSetup>, String> {
    match get_generic_password(SERVICE, ACCOUNT) {
        Ok(bytes) => {
            let saved: StoredSetup = serde_json::from_slice(&bytes).map_err(|_| "Saved setup checkpoints are unreadable. Do not delete existing Keychain records; ask a parent for recovery.")?;
            validate_payload(&saved.payload)?;
            Ok(Some(saved))
        }
        Err(error) if error.code() == -25300 => Ok(None), // errSecItemNotFound; locked Keychain is not an empty save.
        Err(_) => Err(
            "Unlock macOS Keychain and retry opening setup. No credentials were written to disk."
                .into(),
        ),
    }
}

#[tauri::command]
pub fn load_setup(store: tauri::State<'_, SetupStore>) -> Result<Option<StoredSetup>, String> {
    let _guard = store
        .0
        .lock()
        .map_err(|_| "Restart the app before resuming setup.")?;
    read_setup()
}

#[tauri::command]
pub fn save_setup(
    store: tauri::State<'_, SetupStore>,
    payload: String,
    expected_revision: u64,
) -> Result<u64, String> {
    validate_payload(&payload)?;
    let _guard = store
        .0
        .lock()
        .map_err(|_| "Restart the app before resuming setup.")?;
    let current = read_setup()?.map_or(0, |saved| saved.revision);
    if current != expected_revision {
        return Err("Setup changed in another window. Reopen setup before continuing.".into());
    }
    let revision = current.checked_add(1).ok_or("Invalid setup revision.")?;
    let bytes = serde_json::to_vec(&StoredSetup { payload, revision })
        .map_err(|_| "Could not encode setup checkpoints.")?;
    set_generic_password(SERVICE, ACCOUNT, &bytes)
        .map_err(|_| "Unlock macOS Keychain and retry saving setup.")?;
    Ok(revision)
}

#[derive(Serialize)]
pub struct UsbDevice {
    serial: String,
    state: String,
}

#[tauri::command]
pub async fn inspect_setup_usb() -> Result<Vec<UsbDevice>, String> {
    let output = crate::discovery::adb(&["devices", "-l"])
        .await
        .map_err(|error| error.message)?;
    let devices = output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let serial = fields.next()?;
            let state = fields.next()?;
            if !["device", "offline", "unauthorized"].contains(&state)
                || serial.len() > 128
                || !serial.as_bytes()[0].is_ascii_alphanumeric()
                || !serial
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
                || !fields.any(|field| field.starts_with("usb:") && field.len() > 4)
            {
                return None;
            }
            Some(UsbDevice {
                serial: serial.into(),
                state: state.into(),
            })
        })
        .take(16)
        .collect();
    Ok(devices)
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardResource {
    Configure,
    Recovery,
    FlasherDownload,
}

#[tauri::command]
pub async fn open_official_setup() -> Result<(), String> {
    open_board_resource(BoardResource::Configure).await
}

#[tauri::command]
pub async fn open_board_resource(resource: BoardResource) -> Result<(), String> {
    let url = match resource {
        BoardResource::Configure => "https://docs.arduino.cc/software/app-lab/configure/config/",
        BoardResource::Recovery => "https://docs.arduino.cc/software/app-lab/configure/flash/",
        BoardResource::FlasherDownload => "https://www.arduino.cc/en/software/#flasher-tool",
    };
    let status = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::process::Command::new("/usr/bin/open")
            .arg(url)
            .kill_on_drop(true)
            .status(),
    )
    .await
    .map_err(|_| "Opening the official setup guide timed out.")?
    .map_err(|_| "Open docs.arduino.cc/software/app-lab/configure/config/ in your browser.")?;
    if !status.success() {
        return Err(
            "Open docs.arduino.cc/software/app-lab/configure/config/ in your browser.".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkpoints_accept_no_secrets_or_fake_completion() {
        assert!(
            validate_payload(
                r#"{"schemaVersion":1,"selectedSerial":"uno-1234","stageIndex":0,"checkpoints":[]}"#
            )
            .is_ok()
        );
        assert!(validate_payload(r#"{"schemaVersion":1,"selectedSerial":"uno-1234","stageIndex":0,"checkpoints":[],"password":"not-accepted"}"#).is_err());
        assert!(
            validate_payload(
                r#"{"schemaVersion":1,"selectedSerial":"uno-1234","stageIndex":8,"checkpoints":[]}"#
            )
            .is_err()
        );
    }
}
