use crate::{
    admin_process,
    connection::Connections,
    management::{BoardIdentity, Management},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, atomic::Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::io::AsyncWriteExt;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    version: String,
    os: String,
    url: String,
    sha256: String,
}
#[derive(Clone)]
struct Plan {
    id: u64,
    board: BoardIdentity,
    image: Image,
    tool: PathBuf,
    tool_hash: String,
    created: Instant,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    plan_id: u64,
    board: BoardIdentity,
    image: Image,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestoreConfirmation {
    phrase: String,
    backup: bool,
    edl: bool,
    only_target: bool,
    power: bool,
}
fn validate_confirmation(serial: &str, confirmation: &RestoreConfirmation) -> Result<(), String> {
    if serial.parse::<i64>().ok().is_none_or(|value| value <= 0)
        || confirmation.phrase != format!("ERASE {serial}")
        || !confirmation.backup
        || !confirmation.edl
        || !confirmation.only_target
        || !confirmation.power
    {
        return Err("Type ERASE followed by this Board's hardware serial and confirm backup/data loss, EDL preparation, isolated target, and stable power/disconnected loads.".into());
    }
    Ok(())
}
fn flash_args(serial: &str, image: &str) -> Vec<String> {
    ["flash", "unoq", image, "--serial", serial, "--yes"]
        .into_iter()
        .map(str::to_string)
        .collect()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreStatus {
    phase: String,
    bytes: u64,
    message: String,
}
struct Job {
    status: RestoreStatus,
    child: Option<Child>,
    marker: Marker,
}
#[derive(Clone, Serialize, Deserialize)]
struct Marker {
    serial: String,
    adb_serial: String,
    version: String,
    phase: String,
    pid: Option<u32>,
    boot_session: String,
}
#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum VerificationSource {
    TinkerooFlasher,
    HumanOfficialRecovery,
    NoFlashStarted,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootVerification {
    board: BoardIdentity,
    source: VerificationSource,
    observed_at_unix_seconds: Option<u64>,
}
pub(crate) fn boot_session() -> Result<String, String> {
    let mut bytes = [0u8; 128];
    let mut length = bytes.len();
    let status = unsafe {
        libc::sysctlbyname(
            c"kern.bootsessionuuid".as_ptr(),
            bytes.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 || !(2..=bytes.len()).contains(&length) {
        return Err("Cannot establish this Mac's boot identity; recovery ownership cannot be checked safely.".into());
    }
    let value =
        std::str::from_utf8(&bytes[..length - 1]).map_err(|_| "Unsupported Mac boot identity.")?;
    if value.len() != 36
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        return Err("Unsupported Mac boot identity.".into());
    }
    Ok(value.into())
}
fn outcome_policy(
    marker: &Marker,
    current_boot: &str,
    pid_alive: bool,
    official_recovery_confirmed: bool,
) -> Result<VerificationSource, String> {
    let same_boot = marker.boot_session == current_boot;
    if same_boot && pid_alive {
        return Err("The previous flasher may still be running. Do not unplug or retry; wait for completion.".into());
    }
    if same_boot
        && marker.pid.is_none()
        && matches!(marker.phase.as_str(), "flash_starting" | "flashing")
    {
        return Err("Flash ownership is uncertain after interruption. Do not kill or unplug an active flasher. After all flashing is stopped, restart this Mac, perform official recovery, then explicitly acknowledge its successful outcome here.".into());
    }
    match marker.phase.as_str() {
        "awaiting_boot" => Ok(VerificationSource::TinkerooFlasher),
        "downloading" | "not_started" => Ok(VerificationSource::NoFlashStarted),
        "flash_starting" | "flashing" | "flash_failed" if official_recovery_confirmed => Ok(VerificationSource::HumanOfficialRecovery),
        _ => Err("The previous flasher did not report confirmed success to this app. Use official Arduino recovery and explicitly confirm its successful destructive restore; an old image merely booting is not factory-reset evidence.".into()),
    }
}
struct RecoveryInner {
    plan: Option<Plan>,
    job: Option<Job>,
    sequence: u64,
}
pub struct Recovery {
    inner: Arc<Mutex<RecoveryInner>>,
    operations: tokio::sync::Mutex<()>,
    owner: Mutex<Option<crate::file_lock::ExclusiveFile>>,
    directory: PathBuf,
}
impl Recovery {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            operations: tokio::sync::Mutex::new(()),
            owner: Mutex::new(None),
            inner: Arc::new(Mutex::new(RecoveryInner {
                plan: None,
                job: None,
                sequence: 0,
            })),
            directory,
        }
    }
    fn require_owner(&self) -> Result<(), String> {
        let mut owner = self
            .owner
            .lock()
            .map_err(|_| "Recovery ownership is unavailable.")?;
        if owner.is_none() {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(self.directory.join("factory-restore.lock"))
                .map_err(|_| "Cannot establish private recovery ownership.")?;
            let file = crate::file_lock::ExclusiveFile::new(file).map_err(|_| "Another Tinkeroo instance owns recovery. Quit the other instance before preparing or accepting recovery; this app will not start a competing erase.")?;
            *owner = Some(file);
        }
        Ok(())
    }
}
fn marker_path(directory: &Path) -> PathBuf {
    directory.join("factory-restore-active.json")
}
fn persist(directory: &Path, marker: &Marker) -> Result<(), String> {
    persist_record(directory, "factory-restore-active.json", marker)
}
fn persist_record(directory: &Path, name: &str, record: &impl Serialize) -> Result<(), String> {
    let temp = directory.join(format!("{name}.tmp"));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temp)
        .map_err(|_| "Could not record recovery evidence; do not assume completion or start another flash.")?;
    file.write_all(&serde_json::to_vec(record).map_err(|_| "Invalid recovery state.")?)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Could not save recovery state.")?;
    fs::rename(temp, directory.join(name)).map_err(|_| "Could not persist recovery state.".into())
}
fn tool_path(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || path.file_name().and_then(|name| name.to_str()) != Some("arduino-flasher-cli")
    {
        return Err("Select an absolute path to the separately installed official arduino-flasher-cli executable.".into());
    }
    let path = path
        .canonicalize()
        .map_err(|_| "Arduino Flasher CLI is not installed at that path.")?;
    if !path.is_file()
        || fs::metadata(&path)
            .map_err(|_| "Could not inspect the flasher.")?
            .len()
            > 128 * 1024 * 1024
    {
        return Err("Unsupported flasher executable.".into());
    }
    Ok(path)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn tool_hash(path: &Path) -> Result<String, String> {
    Ok(hex(&Sha256::digest(fs::read(path).map_err(
        |_| "Could not inspect the installed flasher.",
    )?)))
}
fn catalog(path: &Path) -> Result<Vec<Image>, String> {
    let version = admin_process::run(
        path,
        &["version", "--format", "json"],
        &[],
        Duration::from_secs(20),
    )?;
    let version: serde_json::Value =
        serde_json::from_slice(&version).map_err(|_| "Unsupported flasher version response.")?;
    if version["version"]
        .as_str()
        .map(|version| version.trim_start_matches('v'))
        != Some("0.5.4")
    {
        return Err("This adapter is source-verified for Arduino Flasher CLI 0.5.4 only. Install that official version or use Arduino App Lab recovery; unknown versions are not guessed.".into());
    }
    let bytes = admin_process::run(
        path,
        &["list", "unoq", "--format", "json"],
        &[],
        Duration::from_secs(30),
    )?;
    parse_catalog(&bytes)
}
fn parse_catalog(bytes: &[u8]) -> Result<Vec<Image>, String> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| "The official image catalog is incompatible.")?;
    let releases = value["releases"]
        .as_array()
        .ok_or("No official UNO Q image catalog was returned.")?;
    let mut images = vec![];
    for release in releases.iter().take(100) {
        if release["board"].as_str() != Some("unoq") {
            continue;
        }
        let image: Image = serde_json::from_value(release.clone())
            .map_err(|_| "An image catalog entry is incomplete.")?;
        let url = reqwest::Url::parse(&image.url).map_err(|_| "Invalid official image URL.")?;
        if url.scheme() != "https"
            || url.host_str() != Some("downloads.arduino.cc")
            || !url.username().is_empty()
            || url.password().is_some()
            || image.sha256.len() != 64
            || !image.sha256.bytes().all(|c| c.is_ascii_hexdigit())
            || image.version.is_empty()
            || image.version.len() > 64
            || image.version == "latest"
            || !image
                .version
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            || image.os.len() > 128
            || !image.os.to_lowercase().contains("debian")
        {
            return Err(
                "Unsupported or unverifiable official image metadata; no flashing was started."
                    .into(),
            );
        }
        // The initial image lacks /etc/buildinfo; its exact post-restore build cannot be verified.
        if image.version != "20250807-136" {
            images.push(image);
        }
    }
    if images.is_empty() {
        return Err("No supported official UNO Q Debian image was found.".into());
    }
    images.sort_by(|left, right| right.version.cmp(&left.version));
    Ok(images)
}
#[tauri::command]
pub async fn recovery_images(tool: String) -> Result<Vec<Image>, String> {
    tauri::async_runtime::spawn_blocking(move || catalog(&tool_path(&tool)?))
        .await
        .map_err(|_| "Image catalog lookup stopped.")?
}
#[tauri::command]
pub async fn prepare_factory_restore(
    state: tauri::State<'_, Recovery>,
    serial: String,
    tool: String,
    version: String,
    os: String,
) -> Result<PlanView, String> {
    let _operation = state.operations.lock().await;
    state.require_owner()?;
    if marker_path(&state.directory).exists() {
        return Err("A prior recovery outcome is pending. Verify that Board's boot/outcome before preparing a new erase; do not blindly retry interrupted flashing.".into());
    }
    let inner = state.inner.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let board = crate::management::inspect(&serial)?;
        let tool = tool_path(&tool)?;
        let image = catalog(&tool)?
            .into_iter()
            .find(|image| image.version == version && image.os == os)
            .ok_or("Select an exact version from the official UNO Q catalog.")?;
        let hash = tool_hash(&tool)?;
        let mut stored = inner.lock().map_err(|_| "Recovery is busy.")?;
        stored.sequence += 1;
        let id = stored.sequence;
        stored.plan = Some(Plan {
            id,
            board: board.clone(),
            image: image.clone(),
            tool,
            tool_hash: hash,
            created: Instant::now(),
        });
        Ok(PlanView {
            plan_id: id,
            board,
            image,
        })
    })
    .await
    .map_err(|_| "Recovery preparation stopped.")?
}
fn update(inner: &Arc<Mutex<RecoveryInner>>, phase: &str, bytes: u64, message: &str) {
    if let Ok(mut inner) = inner.lock()
        && let Some(job) = inner.job.as_mut()
    {
        job.status = RestoreStatus {
            phase: phase.into(),
            bytes,
            message: message.into(),
        };
    }
}
async fn download_and_flash(
    inner: Arc<Mutex<RecoveryInner>>,
    directory: PathBuf,
    plan: Plan,
    maintenance: Arc<std::sync::atomic::AtomicBool>,
    mut marker: Marker,
) {
    let result: Result<(), String> = async {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| "The clock is unavailable.")?.as_nanos();
        let run_dir = directory.join(format!("restore-{nonce}"));
        fs::create_dir(&run_dir).map_err(|_| "Could not create private recovery storage.")?;
        fs::set_permissions(&run_dir, fs::Permissions::from_mode(0o700)).map_err(|_| "Could not protect recovery storage.")?;
        if fs2::available_space(&run_dir).map_err(|_| "Could not check free disk space.")? < 10 * 1024 * 1024 * 1024 { return Err("At least 10 GiB free disk space is required before downloading/extracting the image.".into()); }
        let image_path = run_dir.join("official-image.tar.zst");
        let client = reqwest::Client::builder().https_only(true).redirect(reqwest::redirect::Policy::none()).connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(1800)).build().map_err(|_| "Secure image download could not initialize.")?;
        let mut response = client.get(&plan.image.url).send().await.map_err(|_| "Image download failed; no Board writes have started.")?;
        if !response.status().is_success() { return Err("The official image download was rejected; no Board writes have started.".into()); }
        let mut output = tokio::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&image_path).await.map_err(|_| "Could not create the image file.")?;
        let mut digest = Sha256::new();
        let mut total = 0;
        while let Some(chunk) = response.chunk().await.map_err(|_| "Image download was interrupted before flashing.")? {
            total += chunk.len() as u64;
            if total > 4 * 1024 * 1024 * 1024 { return Err("Image download exceeds the 4 GiB safety limit; no flashing started.".into()); }
            digest.update(&chunk);
            output.write_all(&chunk).await.map_err(|_| "Image storage failed before flashing.")?;
            update(&inner, "downloading", total, "Downloading the exact approved official image; Board writes have not started.");
        }
        output.sync_all().await.map_err(|_| "Could not finish saving the image.")?;
        drop(output);
        if hex(&digest.finalize()) != plan.image.sha256.to_lowercase() { return Err("Official image SHA-256 verification failed. No flashing started.".into()); }
        if tool_hash(&plan.tool)? != plan.tool_hash { return Err("The flasher executable changed since approval; no flashing started.".into()); }
        let log = OpenOptions::new().write(true).create_new(true).mode(0o600).open(run_dir.join("flasher.log")).map_err(|_| "Could not create the recovery log.")?;
        marker.phase = "flash_starting".into();
        // Persist BEFORE starting a destructive process; an app crash must not look like a safe empty state.
        persist(&directory, &marker)?;
        let child = Command::new(&plan.tool).process_group(0).args(flash_args(&plan.board.serial, image_path.to_str().ok_or("Invalid image path.")?))
            .env_remove("ARDUINO_FLASHER_ADDITIONAL_URLS").env_remove("ARDUINO_FLASHER_ADDITIONAL_HEADERS")
            .env("TMPDIR", &run_dir).env("LC_ALL", "C").stdin(Stdio::null()).stdout(Stdio::from(log.try_clone().map_err(|_| "Could not open recovery log.")?)).stderr(Stdio::from(log))
            .spawn();
        let child = match child {
            Ok(child) => child,
            Err(_) => { marker.phase = "not_started".into(); persist(&directory, &marker)?; return Err("The official flasher could not start; no Board writes started. Check the official installation and macOS approval without bypassing Gatekeeper.".into()); }
        };
        marker.phase = "flashing".into(); marker.pid = Some(child.id());
        // Child owns file handles and is deliberately not killed if this app closes mid-write.
        let marker_result = persist(&directory, &marker);
        let mut stored = inner.lock().map_err(|_| "Recovery state is unavailable; do not disconnect power.")?;
        let job = stored.job.as_mut().ok_or("Recovery state is unavailable; keep the Board powered.")?;
        job.child = Some(child); job.marker = marker;
        job.status = RestoreStatus { phase: "flashing".into(), bytes: total, message: if marker_result.is_ok() { "Official flasher is running for the exact hardware serial. Do not unplug, close the app, or cancel." } else { "Flashing started but progress persistence failed. Keep the Board powered and app open; inspect official recovery before retrying." }.into() };
        Ok(())
    }.await;
    if let Err(message) = result {
        let before_flash = fs::read(marker_path(&directory))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Marker>(&bytes).ok())
            .is_some_and(|marker| matches!(marker.phase.as_str(), "downloading" | "not_started"));
        update(
            &inner,
            if before_flash {
                "failed"
            } else {
                "outcome_unknown"
            },
            0,
            &message,
        );
        if before_flash {
            let _ = fs::remove_file(marker_path(&directory));
            maintenance.store(false, Ordering::SeqCst);
        }
    }
}
#[tauri::command]
pub async fn start_factory_restore(
    state: tauri::State<'_, Recovery>,
    sessions: tauri::State<'_, Management>,
    connections: tauri::State<'_, Connections>,
    plan_id: u64,
    confirmation: RestoreConfirmation,
) -> Result<RestoreStatus, String> {
    let _operation = state.operations.lock().await;
    state.require_owner()?;
    let plan = state
        .inner
        .lock()
        .map_err(|_| "Recovery is busy.")?
        .plan
        .clone()
        .filter(|plan| plan.id == plan_id && plan.created.elapsed() <= Duration::from_secs(600))
        .ok_or("Prepare a fresh USB-verified recovery plan before confirming erase.")?;
    validate_confirmation(&plan.board.serial, &confirmation)?;
    if marker_path(&state.directory).exists() {
        return Err(
            "A recovery is already active or its outcome is pending. Do not start a second flash."
                .into(),
        );
    }
    let mut permit = connections.begin_maintenance()?;
    let maintenance = permit.flag.clone();
    crate::connection::disconnect_board(connections)
        .await
        .map_err(|error| error.message)?;
    crate::management::admin_logout(sessions).await?;
    let marker = Marker {
        serial: plan.board.serial.clone(),
        adb_serial: plan.board.adb_serial.clone(),
        version: plan.image.version.clone(),
        phase: "downloading".into(),
        pid: None,
        boot_session: boot_session()?,
    };
    persist(&state.directory, &marker)?;
    let status = RestoreStatus {
        phase: "downloading".into(),
        bytes: 0,
        message:
            "Preparing verified download. Flashing will start only after its checksum matches."
                .into(),
    };
    state.inner.lock().map_err(|_| "Recovery is busy.")?.job = Some(Job {
        status: status.clone(),
        child: None,
        marker: marker.clone(),
    });
    permit.keep = true;
    tauri::async_runtime::spawn(download_and_flash(
        state.inner.clone(),
        state.directory.clone(),
        plan,
        maintenance,
        marker,
    ));
    Ok(status)
}
#[tauri::command]
pub fn factory_restore_status(state: tauri::State<'_, Recovery>) -> Result<RestoreStatus, String> {
    let mut stored = state.inner.lock().map_err(|_| "Recovery is busy.")?;
    if let Some(job) = stored.job.as_mut() {
        if job.status.phase == "recording_failed" {
            persist(&state.directory, &job.marker)?;
            job.status.phase = job.marker.phase.clone();
        }
        if let Some(child) = job.child.as_mut()
            && let Some(exit) = child
                .try_wait()
                .map_err(|_| "Could not read flasher completion; do not assume success.")?
        {
            job.child = None;
            job.marker.phase = if exit.success() {
                "awaiting_boot"
            } else {
                "flash_failed"
            }
            .into();
            job.status.phase = job.marker.phase.clone();
            job.status.message = if exit.success() { "The official flasher reported success. Unplug, remove the jumper, reconnect, and verify the same Board and exact image version below. Setup and Pairing must be redone." } else { "The official flasher failed. Do not assume the Board is usable or automatically retry; follow official recovery, then verify the Board below." }.into();
            if let Err(error) = persist(&state.directory, &job.marker) {
                job.status.phase = "recording_failed".into();
                return Err(error);
            }
        }
        return Ok(job.status.clone());
    }
    if marker_path(&state.directory).exists() {
        let marker: Marker = serde_json::from_slice(&fs::read(marker_path(&state.directory)).map_err(|_| "Cannot read pending recovery state; do not start another flasher.")?).map_err(|_| "Pending recovery state is unreadable; use official Arduino recovery without assuming a previous flasher has stopped.")?;
        let (phase, message) = match marker.phase.as_str() {
            "awaiting_boot" => (
                "awaiting_boot",
                "The official flasher previously reported success. Remove the jumper only after completion, reconnect, and verify the exact Board and image.",
            ),
            "downloading" | "not_started" => (
                "download_interrupted",
                "The prior app session stopped before this app started a flasher. Verify the physical Board to discard this download/plan; this is not a successful factory restore.",
            ),
            "flash_failed" => (
                "flash_failed",
                "The previous flasher failed. Use official Arduino recovery, then explicitly attest its successful destructive restore and verify the Board/image here.",
            ),
            _ => (
                "outcome_unknown",
                "The app closed/interrupted during flashing. Do not start another erase or kill a running flasher. Wait for confirmed completion or use official recovery; its success must be explicitly attested, not inferred from an old image booting.",
            ),
        };
        return Ok(RestoreStatus {
            phase: phase.into(),
            bytes: 0,
            message: format!(
                "Hardware serial {}, approved image {}. {}",
                marker.serial, marker.version, message
            ),
        });
    }
    Ok(RestoreStatus {
        phase: "idle".into(),
        bytes: 0,
        message: "No factory restore is running.".into(),
    })
}
#[tauri::command]
pub async fn verify_factory_boot(
    state: tauri::State<'_, Recovery>,
    connections: tauri::State<'_, Connections>,
    serial: String,
    confirmation: String,
    official_recovery_confirmed: bool,
) -> Result<BootVerification, String> {
    let _operation = state.operations.lock().await;
    state.require_owner()?;
    if state
        .inner
        .lock()
        .map_err(|_| "Recovery is busy.")?
        .job
        .as_ref()
        .is_some_and(|job| matches!(job.status.phase.as_str(), "downloading" | "flashing"))
    {
        return Err("Recovery is still running. Do not unplug or verify/retry while image transfer or flashing is active.".into());
    }
    let directory = state.directory.clone();
    let inner = state.inner.clone();
    let flag = connections.maintenance.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let marker: Marker = serde_json::from_slice(&fs::read(marker_path(&directory)).map_err(|_| "No pending recovery outcome exists.")?).map_err(|_| "Recovery record is unreadable; use official Arduino recovery.")?;
        let prefix = if official_recovery_confirmed { "RECOVERED" } else { "VERIFY" };
        if !crate::management::valid_serial(&serial) || confirmation != format!("{prefix} {}", marker.serial) { return Err("Verify the same physical Board and type its hardware serial confirmation.".into()); }
        let current_boot = boot_session()?;
        let pid_alive = marker.boot_session == current_boot && marker.pid.is_some_and(|pid| {
            // Conservative PID check: even a reused PID on this boot blocks verification.
            (unsafe { libc::kill(pid as libc::pid_t, 0) }) == 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
        });
        let source = outcome_policy(&marker, &current_boot, pid_alive, official_recovery_confirmed)?;
        let board = crate::management::inspect(&serial)?;
        if board.serial != marker.serial { return Err("A different physical Board was connected; no recovery state was cleared.".into()); }
        if source != VerificationSource::NoFlashStarted && board.image_version != marker.version { return Err("The Board booted, but its exact image version is not the approved recovery image. Continue official recovery; success is not assumed.".into()); }
        let verification = BootVerification { board, source, observed_at_unix_seconds: SystemTime::now().duration_since(UNIX_EPOCH).ok().map(|time| time.as_secs()) };
        persist_record(&directory, "factory-restore-last-outcome.json", &verification)?;
        fs::remove_file(marker_path(&directory)).map_err(|_| "Could not finish the recovery checkpoint.")?;
        let mut stored = inner.lock().map_err(|_| "Recovery is busy.")?;
        stored.plan = None; stored.job = None;
        flag.store(false, Ordering::SeqCst);
        // Delete only this app's private image/extraction artifacts after verified process ownership/boot.
        if let Ok(entries) = fs::read_dir(&directory) { for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with("restore-") && entry.file_type().is_ok_and(|kind| kind.is_dir()) { let _ = fs::remove_dir_all(entry.path()); }
        } }
        Ok(verification)
    }).await.map_err(|_| "Boot verification stopped.")?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn interrupted_flashing_requires_exclusive_ownership_and_explicit_outcome_evidence() {
        let mut marker = Marker {
            serial: "42".into(),
            adb_serial: "USB42".into(),
            version: "test-build".into(),
            phase: "flashing".into(),
            pid: Some(123),
            boot_session: "boot-a".into(),
        };
        assert!(outcome_policy(&marker, "boot-a", true, true).is_err());
        assert!(outcome_policy(&marker, "boot-a", false, false).is_err());
        assert_eq!(
            outcome_policy(&marker, "boot-a", false, true).unwrap(),
            VerificationSource::HumanOfficialRecovery
        );
        marker.pid = None;
        assert!(outcome_policy(&marker, "boot-a", false, true).is_err());
        assert!(outcome_policy(&marker, "boot-b", false, false).is_err());
        assert_eq!(
            outcome_policy(&marker, "boot-b", false, true).unwrap(),
            VerificationSource::HumanOfficialRecovery
        );
        marker.phase = "flash_failed".into();
        assert!(outcome_policy(&marker, "boot-a", false, false).is_err());
        assert_eq!(
            outcome_policy(&marker, "boot-a", false, true).unwrap(),
            VerificationSource::HumanOfficialRecovery
        );
        marker.phase = "awaiting_boot".into();
        assert_eq!(
            outcome_policy(&marker, "boot-a", false, false).unwrap(),
            VerificationSource::TinkerooFlasher
        );
        marker.phase = "downloading".into();
        assert_eq!(
            outcome_policy(&marker, "boot-a", false, false).unwrap(),
            VerificationSource::NoFlashStarted
        );
        let directory = tempfile::tempdir().unwrap();
        let first = Recovery::new(directory.path().to_path_buf());
        first.require_owner().unwrap();
        let second = Recovery::new(directory.path().to_path_buf());
        assert!(second.require_owner().is_err());
        drop(first);
        second.require_owner().unwrap();
        persist(directory.path(), &marker).unwrap();
        let path = marker_path(directory.path());
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            serde_json::from_slice::<Marker>(&fs::read(path).unwrap())
                .unwrap()
                .phase,
            "downloading"
        );
        assert_eq!(boot_session().unwrap(), boot_session().unwrap()); // Host-only integration, never Board hardware.
    }

    #[test]
    fn only_exact_official_checksummed_debian_images_enter_a_restore_plan() {
        let good = json!({"board":"unoq", "version":"20260528-558", "os":"debian", "url":"https://downloads.arduino.cc/test.tar.zst", "sha256":"a".repeat(64)});
        let catalog =
            |entry: serde_json::Value| serde_json::to_vec(&json!({"releases":[entry]})).unwrap();
        assert_eq!(
            parse_catalog(&catalog(good.clone())).unwrap()[0].version,
            "20260528-558"
        );
        for (field, value) in [
            ("board", "ventunoq"),
            ("version", "latest"),
            ("version", "20250807-136"),
            ("os", "ubuntu"),
            ("url", "http://downloads.arduino.cc/test.tar.zst"),
            ("url", "https://elsewhere.invalid/image.tar.zst"),
            (
                "url",
                "https://user:password@downloads.arduino.cc/test.tar.zst",
            ),
            ("sha256", "not-a-hash"),
        ] {
            let mut invalid = good.clone();
            invalid[field] = value.into();
            assert!(
                parse_catalog(&catalog(invalid)).is_err(),
                "accepted unsupported {field}"
            );
        }
        assert_eq!(
            hex(&Sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    #[test]
    fn erase_requires_the_hardware_serial_all_acknowledgements_and_an_exact_version() {
        let mut confirmation = RestoreConfirmation {
            phrase: "ERASE 123456789".into(),
            backup: true,
            edl: true,
            only_target: true,
            power: true,
        };
        assert!(validate_confirmation("123456789", &confirmation).is_ok());
        confirmation.backup = false;
        assert!(validate_confirmation("123456789", &confirmation).is_err());
        confirmation.backup = true;
        assert!(validate_confirmation("987654321", &confirmation).is_err());
        let args = flash_args("123456789", "/safe/image.tar.zst");
        assert_eq!(
            args,
            [
                "flash",
                "unoq",
                "/safe/image.tar.zst",
                "--serial",
                "123456789",
                "--yes"
            ]
        );
        assert!(!args.contains(&"--preserve-user".into()));
    }
}
