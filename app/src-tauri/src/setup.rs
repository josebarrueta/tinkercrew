use security_framework::passwords::{get_generic_password, set_generic_password};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

const SERVICE: &str = "com.tinkercrew.desktop.setup";
const ACCOUNT: &str = "parent-checkpoints-v1";
const STAGES: [&str; 12] = [
    "connect-usb",
    "identify",
    "boot",
    "name",
    "credentials",
    "wifi",
    "network",
    "pair",
    "deploy",
    "rediscover",
    "hardware",
    "crew",
];

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Checkpoint {
    stage_id: String,
    source: String,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetupProgress {
    schema_version: u32,
    selected_serial: Option<String>,
    stage_index: usize,
    checkpoints: Vec<Checkpoint>,
}
fn validate_payload(payload: &str) -> Result<(), String> {
    validate_checkpoints(payload, false)
}
fn validate_checkpoints(payload: &str, native: bool) -> Result<(), String> {
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
        || progress.stage_index > if native { STAGES.len() } else { 7 }
        || progress.checkpoints.len() != progress.stage_index
        || (progress.stage_index > 0 && progress.selected_serial.is_none())
        || progress
            .checkpoints
            .iter()
            .enumerate()
            .any(|(index, checkpoint)| {
                checkpoint.stage_id != STAGES[index]
                    || checkpoint.source
                        != if index == 0 || (7..=10).contains(&index) {
                            "observed"
                        } else {
                            "human"
                        }
            })
    {
        return Err(
            "Unsupported setup checkpoints; existing progress has not been replaced.".into(),
        );
    }
    Ok(())
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct HardwareObservation {
    pub test_id: String,
    pub mac_boot: String,
    pub monotonic_ms: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SetupObservation {
    pub serial: String,
    pub soc: String,
    pub board_id: String,
    pub boot_id: String,
    pub runtime_release: String,
    pub runtime_nonce: String,
    pub hardware: Option<HardwareObservation>,
}
fn validate_observation(observed: &SetupObservation) -> Result<(), String> {
    if !crate::management::valid_serial(&observed.serial)
        || observed
            .soc
            .parse::<u64>()
            .ok()
            .filter(|value| *value > 0)
            .is_none()
        || !crate::session::valid_id(&observed.board_id)
        || !crate::session::valid_id(&observed.boot_id)
        || observed.runtime_release.len() != 64
        || !observed
            .runtime_release
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || observed.runtime_nonce.len() != 22
        || !observed
            .runtime_nonce
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
        || observed.hardware.as_ref().is_some_and(|hardware| {
            !crate::session::valid_id(&hardware.test_id)
                || !crate::session::valid_id(&hardware.mac_boot)
        })
    {
        return Err("Native Board setup evidence is invalid; no checkpoint was recorded.".into());
    }
    Ok(())
}
pub(crate) fn observed_step(
    store: &SetupStore,
    expected: usize,
    observed: SetupObservation,
) -> Result<bool, String> {
    validate_observation(&observed)?;
    if !(7..12).contains(&expected) {
        return Err("Unsupported observed setup transition.".into());
    }
    let _guard = store.0.lock().map_err(|_| "Native setup is busy.")?;
    let _write = store.write_lock()?;
    let Some(saved) = read_setup()? else {
        return Ok(false);
    };
    let Some(next) = saved.with_observation(expected, observed)? else {
        return Ok(false);
    };
    // One Keychain item: checkpoint and evidence cannot diverge across a crash.
    set_generic_password(
        SERVICE,
        ACCOUNT,
        &serde_json::to_vec(&next).map_err(|_| "Cannot encode native checkpoint.")?,
    )
    .map_err(
        |_| "Native proof was observed but the checkpoint was not saved; reload and reverify.",
    )?;
    Ok(true)
}
impl StoredSetup {
    /// Native transition policy. Not an IPC writer: callers must supply real
    /// authenticated observations and commit the returned snapshot atomically.
    pub(crate) fn with_observation(
        self,
        expected: usize,
        observed: SetupObservation,
    ) -> Result<Option<Self>, String> {
        validate_observation(&observed)?;
        if !(7..12).contains(&expected) {
            return Err("Unsupported native setup transition.".into());
        }
        let saved = self;
        validate_checkpoints(&saved.payload, true)?;
        let mut progress: SetupProgress =
            serde_json::from_str(&saved.payload).map_err(|_| "Saved setup is unreadable.")?;
        if expected == 9
            && progress.stage_index > 9
            && let Some(previous) = saved.observation.as_ref()
            && previous.board_id == observed.board_id
            && previous.soc == observed.soc
            && previous.boot_id != observed.boot_id
        {
            // A freshly verified later kernel invalidates only downstream live evidence.
            progress.stage_index = 9;
            progress.checkpoints.truncate(9);
        }
        if progress.stage_index != expected {
            return Ok(None);
        }
        if expected == 7 {
            if progress.selected_serial.as_deref() != Some(&observed.serial) {
                return Ok(None);
            }
        } else {
            let previous = saved
                .observation
                .as_ref()
                .ok_or("Pair the exact Board in this guide before proceeding.")?;
            if previous.board_id != observed.board_id
                || previous.soc != observed.soc
                || (expected != 9 && previous.serial != observed.serial)
            {
                return Err(
                    "Observed evidence belongs to another Board; checkpoints cannot transfer."
                        .into(),
                );
            }
            if expected == 9
                && (previous.boot_id == observed.boot_id
                    || previous.runtime_release != observed.runtime_release)
            {
                return Err("The expected runtime has not demonstrated a new kernel boot.".into());
            }
            if expected >= 10
                && (previous.boot_id != observed.boot_id
                    || previous.runtime_release != observed.runtime_release
                    || observed.hardware.is_none())
            {
                return Err(
                    "Fresh hardware evidence must belong to the verified boot/runtime.".into(),
                );
            }
            if expected == 11
                && (previous.runtime_nonce != observed.runtime_nonce
                    || previous.hardware != observed.hardware)
            {
                return Err(
                "Hardware process/evidence changed before Crew completion; repeat the self-test."
                    .into(),
            );
            }
        }
        progress.selected_serial = Some(observed.serial.clone());
        progress.checkpoints.push(Checkpoint {
            stage_id: STAGES[expected].into(),
            source: if expected == 11 { "human" } else { "observed" }.into(),
        });
        progress.stage_index += 1;
        let payload = serde_json::to_string(&progress)
            .map_err(|_| "Cannot encode observed setup progress.")?;
        validate_checkpoints(&payload, true)?;
        let revision = saved
            .revision
            .checked_add(1)
            .ok_or("Invalid native setup revision.")?;
        Ok(Some(Self {
            payload,
            revision,
            observation: Some(observed),
        }))
    }
}
pub(crate) fn require_observed_step(
    store: &SetupStore,
    expected: usize,
    board_id: &str,
) -> Result<SetupObservation, String> {
    let _guard = store.0.lock().map_err(|_| "Setup is busy.")?;
    let saved = read_setup()?
        .ok_or("Open the setup guide and complete the preceding checkpoints first.")?;
    let progress: SetupProgress =
        serde_json::from_str(&saved.payload).map_err(|_| "Invalid saved setup.")?;
    let observed = saved
        .observation
        .ok_or("Native setup evidence is missing.")?;
    if progress.stage_index != expected
        || observed.board_id != board_id
        || progress.selected_serial.as_deref() != Some(&observed.serial)
    {
        return Err(
            "Complete the preceding observed guide steps for this exact Board first.".into(),
        );
    }
    Ok(observed)
}
pub(crate) fn prepare_hardware_step(
    store: &SetupStore,
    board_id: &str,
) -> Result<SetupObservation, String> {
    let _guard = store.0.lock().map_err(|_| "Setup is busy.")?;
    let _write = store.write_lock()?;
    let saved = read_setup()?.ok_or("Complete the preceding guide steps first.")?;
    let mut progress: SetupProgress =
        serde_json::from_str(&saved.payload).map_err(|_| "Invalid setup data.")?;
    let mut observation = saved
        .observation
        .ok_or("Native setup evidence is missing.")?;
    if ![10, 11].contains(&progress.stage_index)
        || observation.board_id != board_id
        || progress.selected_serial.as_deref() != Some(&observation.serial)
    {
        return Err(
            "Complete Pairing, software and observed restart proof for this Board first.".into(),
        );
    }
    if progress.stage_index == 11 {
        progress.stage_index = 10;
        progress.checkpoints.truncate(10);
        observation.hardware = None;
        let payload =
            serde_json::to_string(&progress).map_err(|_| "Cannot encode hardware retry.")?;
        let revision = saved
            .revision
            .checked_add(1)
            .ok_or("Invalid setup revision.")?;
        set_generic_password(
            SERVICE,
            ACCOUNT,
            &serde_json::to_vec(&StoredSetup {
                payload,
                revision,
                observation: Some(observation.clone()),
            })
            .map_err(|_| "Cannot encode hardware retry.")?,
        )
        .map_err(|_| "Unlock Keychain before explicitly repeating the hardware step.")?;
    }
    Ok(observation)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredSetup {
    payload: String,
    revision: u64,
    #[serde(default)]
    observation: Option<SetupObservation>,
}
pub struct SetupStore(Mutex<()>, std::path::PathBuf);
impl SetupStore {
    pub fn new(directory: std::path::PathBuf) -> Self {
        Self(Mutex::new(()), directory)
    }
    fn write_lock(&self) -> Result<crate::file_lock::ExclusiveFile, String> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(self.1.join("setup-checkpoints.lock"))
            .map_err(|_| "Cannot establish setup checkpoint ownership.")?;
        crate::file_lock::ExclusiveFile::new(file)
            .map_err(|_| "Setup changed in another app instance; reload before continuing.".into())
    }
}

fn read_setup() -> Result<Option<StoredSetup>, String> {
    match get_generic_password(SERVICE, ACCOUNT) {
        Ok(bytes) => {
            let saved: StoredSetup = serde_json::from_slice(&bytes).map_err(|_| "Saved setup checkpoints are unreadable. Do not delete existing Keychain records; ask a parent for recovery.")?;
            validate_checkpoints(&saved.payload, true)?;
            let progress: SetupProgress =
                serde_json::from_str(&saved.payload).map_err(|_| "Invalid saved setup.")?;
            if progress.stage_index > 7 {
                let observed = saved.observation.as_ref().ok_or(
                    "Native setup evidence is missing; no completion has been reset or invented.",
                )?;
                validate_observation(observed)?;
                if (progress.stage_index >= 11) != observed.hardware.is_some() {
                    return Err("Hardware checkpoint and native evidence disagree; preserve the records for recovery.".into());
                }
                if progress.selected_serial.as_deref() != Some(observed.serial.as_str()) {
                    return Err("Setup and native Board evidence disagree; preserve the records for recovery.".into());
                }
            }
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
    connections: tauri::State<'_, crate::connection::Connections>,
    store: tauri::State<'_, SetupStore>,
    payload: String,
    expected_revision: u64,
) -> Result<u64, String> {
    validate_payload(&payload)?;
    if connections.maintenance_active() {
        return Err(
            "Native Board setup/maintenance is active; do not overwrite its observed checkpoints."
                .into(),
        );
    }
    let _guard = store
        .0
        .lock()
        .map_err(|_| "Restart the app before resuming setup.")?;
    let _write = store.write_lock()?;
    let current = read_setup()?.map_or(0, |saved| saved.revision);
    if current != expected_revision {
        return Err("Setup changed in another window. Reopen setup before continuing.".into());
    }
    let revision = current.checked_add(1).ok_or("Invalid setup revision.")?;
    let bytes = serde_json::to_vec(&StoredSetup {
        payload,
        revision,
        observation: None,
    })
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
    fn prefix() -> StoredSetup {
        let progress = serde_json::json!({"schemaVersion":1,"selectedSerial":"USB_TEST","stageIndex":7,"checkpoints":STAGES[..7].iter().enumerate().map(|(index,id)|serde_json::json!({"stageId":id,"source":if index==0 {"observed"}else{"human"}})).collect::<Vec<_>>()});
        StoredSetup {
            payload: progress.to_string(),
            revision: 1,
            observation: None,
        }
    }
    fn observed() -> SetupObservation {
        SetupObservation {
            serial: "USB_TEST".into(),
            soc: "42".into(),
            board_id: "00000000-0000-4000-8000-000000000042".into(),
            boot_id: "00000000-0000-4000-8000-000000000001".into(),
            runtime_release: "a".repeat(64),
            runtime_nonce: "n".repeat(22),
            hardware: None,
        }
    }
    fn through_runtime() -> StoredSetup {
        prefix()
            .with_observation(7, observed())
            .unwrap()
            .unwrap()
            .with_observation(8, observed())
            .unwrap()
            .unwrap()
    }
    fn restarted() -> SetupObservation {
        let mut value = observed();
        value.boot_id = "00000000-0000-4000-8000-000000000002".into();
        value.serial = "USB_AFTER_BOOT".into();
        value
    }
    fn tested() -> SetupObservation {
        let mut value = restarted();
        value.hardware = Some(HardwareObservation {
            test_id: "00000000-0000-4000-8000-000000000003".into(),
            mac_boot: "00000000-0000-4000-8000-000000000004".into(),
            monotonic_ms: 1234,
        });
        value
    }
    #[test]
    fn native_snapshots_bind_ordered_proofs_and_are_not_frontend_writable() {
        let saved = through_runtime()
            .with_observation(9, restarted())
            .unwrap()
            .unwrap()
            .with_observation(10, tested())
            .unwrap()
            .unwrap()
            .with_observation(11, tested())
            .unwrap()
            .unwrap();
        let roundtrip: StoredSetup =
            serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        let progress: SetupProgress = serde_json::from_str(&roundtrip.payload).unwrap();
        assert_eq!(progress.stage_index, 12);
        assert_eq!(progress.selected_serial.as_deref(), Some("USB_AFTER_BOOT"));
        assert_eq!(roundtrip.revision, 6);
        assert!(roundtrip.observation.unwrap().hardware.is_some());
        assert!(validate_payload(&saved.payload).is_err());
        assert!(validate_checkpoints(&saved.payload, true).is_ok());
    }
    #[test]
    fn native_proofs_cannot_skip_or_transfer_to_other_boards_or_software() {
        assert!(prefix().with_observation(8, observed()).unwrap().is_none());
        for field in ["board", "soc", "serial"] {
            let mut other = observed();
            match field {
                "board" => other.board_id = "00000000-0000-4000-8000-000000000099".into(),
                "soc" => other.soc = "99".into(),
                _ => other.serial = "OTHER_USB".into(),
            };
            assert!(
                prefix()
                    .with_observation(7, observed())
                    .unwrap()
                    .unwrap()
                    .with_observation(8, other)
                    .is_err()
            );
        }
        assert!(through_runtime().with_observation(9, observed()).is_err());
        let mut changed = restarted();
        changed.runtime_release = "b".repeat(64);
        assert!(through_runtime().with_observation(9, changed).is_err());
    }
    #[test]
    fn hardware_and_crew_cannot_reuse_missing_or_changed_process_evidence() {
        assert!(
            through_runtime()
                .with_observation(9, restarted())
                .unwrap()
                .unwrap()
                .with_observation(10, restarted())
                .is_err()
        );
        let mut changed = tested();
        changed.runtime_nonce = "x".repeat(22);
        assert!(
            through_runtime()
                .with_observation(9, restarted())
                .unwrap()
                .unwrap()
                .with_observation(10, tested())
                .unwrap()
                .unwrap()
                .with_observation(11, changed)
                .is_err()
        );
    }
    #[test]
    fn observed_later_kernel_invalidates_only_downstream_hardware_evidence() {
        let saved = through_runtime()
            .with_observation(9, restarted())
            .unwrap()
            .unwrap()
            .with_observation(10, tested())
            .unwrap()
            .unwrap();
        let mut later = restarted();
        later.boot_id = "00000000-0000-4000-8000-000000000005".into();
        let next = saved.with_observation(9, later).unwrap().unwrap();
        let progress: SetupProgress = serde_json::from_str(&next.payload).unwrap();
        assert_eq!(progress.stage_index, 10);
        assert_eq!(progress.checkpoints.len(), 10);
        assert!(next.observation.unwrap().hardware.is_none());
    }
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
