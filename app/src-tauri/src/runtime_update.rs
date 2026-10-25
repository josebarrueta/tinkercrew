//! Fixed bundled runtime payload. No user-selected archives, paths, dependencies, or firmware.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

macro_rules! source {
    ($name:literal) => {
        (
            $name,
            include_bytes!(concat!("../../../companion/", $name)).as_slice(),
        )
    };
}
const RUNTIME: &[(&str, &[u8])] = &[
    source!("python/tinkercrew_companion/__init__.py"),
    source!("python/tinkercrew_companion/auth.py"),
    source!("python/tinkercrew_companion/core.py"),
    source!("python/tinkercrew_companion/discovery.py"),
    source!("python/tinkercrew_companion/hardware.py"),
    source!("python/tinkercrew_companion/server.py"),
];
const CONTRACT: &[(&str, &[u8])] = &[
    source!("app.yaml"),
    source!("sketch/sketch.ino"),
    source!("sketch/sketch.yaml"),
    source!("python/main.py"),
    source!("python/management.py"),
    source!("python/requirements.txt"),
    source!("python/requirements-auth.txt"),
    source!("python/tinkercrew_companion/management.py"),
    source!("python/tinkercrew_companion/deployment.py"),
    source!("python/tinkercrew_companion/supervisor.py"),
    source!("python/tinkercrew_companion/auth.py"),
];
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn tree(files: &[(&str, &[u8])]) -> String {
    let sorted: BTreeMap<_, _> = files.iter().copied().collect();
    let mut metadata = Vec::new();
    for (name, bytes) in sorted {
        metadata
            .extend_from_slice(format!("{name}\0{}\0{}\n", bytes.len(), digest(bytes)).as_bytes());
    }
    digest(&metadata)
}
struct Bundle {
    release: String,
    contract: String,
    bytes: Vec<u8>,
}
fn bundled_runtime() -> Result<Bundle, String> {
    let contract = tree(CONTRACT);
    let release = digest(format!("TC-RUNTIME-1\n{contract}\n{}\n", tree(RUNTIME)).as_bytes());
    let files: BTreeMap<_, _> = RUNTIME
        .iter()
        .map(|(name, bytes)| (*name, STANDARD.encode(bytes)))
        .collect();
    let bytes = serde_json::to_vec(
        &json!({"format":1,"contract":contract,"release":release,"files":files}),
    )
    .map_err(|_| "Could not prepare the compiled runtime bundle.")?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("The compiled runtime exceeds the supported update size.".into());
    }
    Ok(Bundle {
        release,
        contract,
        bytes,
    })
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeBundleInfo {
    release: String,
    contract: String,
    bytes: usize,
}
#[tauri::command]
pub fn runtime_bundle_info() -> Result<RuntimeBundleInfo, String> {
    let bundle = bundled_runtime()?;
    Ok(RuntimeBundleInfo {
        release: bundle.release,
        contract: bundle.contract,
        bytes: bundle.bytes.len(),
    })
}

use crate::{
    connection::{self, BoardProfile, Connections},
    pairing,
    session::GameplayAuth,
};
use serde_json::Value;
use std::{
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

async fn board(
    serial: &str,
    soc: &str,
    board_id: &str,
    action: &str,
    mut body: Value,
) -> Result<Value, String> {
    body["operation"] = format!("runtime_{action}").into();
    body["expected_serial"] = soc.into();
    let serial = serial.to_owned();
    let result = tauri::async_runtime::spawn_blocking(move || pairing::usb(&serial, &body)).await.map_err(|_| "Runtime management stopped. Reconnect USB and resume; success has not been assumed.")??;
    if result["serial"].as_str() != Some(soc)
        || result["board_id"].as_str() != Some(board_id)
        || result["protocol"] != 1
    {
        return Err(
            "Runtime management identity or protocol changed. Stop and inspect the exact Board."
                .into(),
        );
    }
    Ok(result)
}
fn pending_matches(
    directory: &Path,
    serial: &str,
    soc: &str,
    board_id: &str,
    release: Option<&str>,
) -> Result<bool, String> {
    let path = directory.join("runtime-update-active.json");
    if !path.exists() {
        return Ok(false);
    }
    let bytes = std::fs::read(path).map_err(|_| "Cannot inspect the pending runtime update.")?;
    if bytes.len() > 4096 {
        return Err("Pending runtime ownership is unreadable; do not overwrite it.".into());
    }
    let record: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "Pending runtime ownership is corrupt; it has not been reset.")?;
    if record["serial"] != serial || record["soc"] != soc || record["boardId"] != board_id {
        return Err("An update for a different physical Board is pending. Resolve that exact Board before another update.".into());
    }
    if release.is_some_and(|value| record["release"].as_str() != Some(value)) {
        return Err("An older app build's update is pending. Use Abort/recover to resolve it before installing this bundle.".into());
    }
    Ok(true)
}
fn pending(
    directory: &Path,
    serial: &str,
    soc: &str,
    board_id: &str,
    release: &str,
) -> Result<PathBuf, String> {
    let path = directory.join("runtime-update-active.json");
    let temporary = directory.join("runtime-update-active.tmp");
    let mut output = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|_| "Cannot record pending runtime update.")?;
    output
        .write_all(
            &serde_json::to_vec(
                &json!({"serial":serial,"soc":soc,"boardId":board_id,"release":release}),
            )
            .map_err(|_| "Invalid runtime update record.")?,
        )
        .and_then(|_| output.sync_all())
        .map_err(|_| "Cannot persist runtime update ownership.")?;
    std::fs::rename(temporary, &path).map_err(|_| "Cannot persist pending runtime update.")?;
    std::fs::File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| "Cannot sync pending runtime ownership; no remote update has been started.")?;
    Ok(path)
}
async fn healthy(
    client: &reqwest::Client,
    profile: &BoardProfile,
    port: u16,
    auth: &GameplayAuth,
    status: &Value,
) -> Result<Value, String> {
    let hello = connection::request(
        client,
        profile,
        port,
        reqwest::Method::GET,
        "/v1/hello",
        None,
        auth,
    )
    .await
    .map_err(|error| error.message)?;
    let nonce = hello["runtime"]["nonce"]
        .as_str()
        .filter(|value| value.len() == 22)
        .ok_or("The runtime did not report supervisor-bound health.")?;
    if hello["board"]["board_id"].as_str() != Some(&profile.board_id)
        || hello["protocol"] != "1.0"
        || hello["health"]["safe_state"] != true
        || hello["runtime"]["release"] != status["current"]
        || status["observed"]["release"] != status["current"]
        || status["observed"]["nonce"].as_str() != Some(nonce)
        || status["observed"]["state"] != "launched"
    {
        return Err("Fresh authenticated, disarmed runtime health has not been verified.".into());
    }
    Ok(hello)
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeUpdateResult {
    phase: String,
    release: String,
    message: String,
}

#[tauri::command]
pub async fn runtime_update_status(serial: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let identity = crate::management::inspect(&serial)?;
        pairing::usb(
            &serial,
            &json!({"operation":"runtime_status","expected_serial":identity.serial}),
        )
    })
    .await
    .map_err(|_| "Runtime status inspection stopped.")?
}

#[tauri::command]
pub async fn update_companion_runtime(
    state: tauri::State<'_, Connections>,
    setup_store: tauri::State<'_, crate::setup::SetupStore>,
    serial: String,
    board_id: String,
    confirmation: String,
) -> Result<RuntimeUpdateResult, String> {
    let bundle = bundled_runtime()?;
    let profile = state.profile(&board_id).map_err(|error| error.message)?;
    if profile.adb_serial != serial || profile.installation_id.is_none() {
        return Err("Select this exact USB Board's registered TBP profile before updating. Legacy bearer migration and first installation require separate supported setup.".into());
    }
    let source_serial = serial.clone();
    let identity =
        tauri::async_runtime::spawn_blocking(move || crate::management::inspect(&source_serial))
            .await
            .map_err(|_| "USB identity inspection stopped.")??;
    if confirmation != format!("UPDATE {}", identity.serial) {
        return Err("Type UPDATE followed by the inspected hardware serial. This replaces only the fixed bundled Linux runtime, not firmware, dependencies or Board credentials.".into());
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(state.directory.join("runtime-update.lock"))
        .map_err(|_| "Cannot establish runtime-update ownership.")?;
    let _lock = crate::file_lock::ExclusiveFile::new(lock).map_err(
        |_| "Another app instance owns the runtime update. Do not start a competing transfer.",
    )?;
    let _permit = state.begin_runtime_update()?;
    connection::disconnect_board(state.clone())
        .await
        .map_err(|error| error.message)?;
    let soc = &identity.serial;
    let initial = board(&serial, soc, &board_id, "status", json!({})).await?;
    if initial["contract"].as_str() != Some(&bundle.contract) || initial["supervisor_ready"] != true
    {
        return Err("The installed baseline, dependencies, firmware or supervisor contract is unsupported. Use official tooling to install a compatible baseline; this updater never guesses a firmware/rebuild command.".into());
    }
    let auth =
        GameplayAuth::from_profile(&profile, &state.directory).map_err(|error| error.message)?;
    let source_serial = serial.clone();
    let forward =
        tauri::async_runtime::spawn_blocking(move || pairing::Forward::new(&source_serial))
            .await
            .map_err(|_| "USB forwarding stopped.")??;
    let candidate =
        connection::Candidate::new("127.0.0.1", forward.port).map_err(|error| error.message)?;
    let (client, _) = connection::secure_client(&profile, &candidate)
        .await
        .map_err(|error| error.message)?;
    let recovering = pending_matches(
        &state.directory,
        &serial,
        soc,
        &board_id,
        Some(&bundle.release),
    )?;
    if !recovering && initial["phase"] != "awaiting_health" {
        healthy(&client, &profile, forward.port, &auth, &initial).await?;
    }
    auth.close(&client, &profile.tls_name, forward.port).await;
    let marker = pending(&state.directory, &serial, soc, &board_id, &bundle.release)?;
    let result = async {
        let begun = board(&serial, soc, &board_id, "begin", json!({"release":bundle.release,"sha256":digest(&bundle.bytes),"size":bundle.bytes.len(),"contract":bundle.contract})).await?;
        if begun["phase"] == "current" {
            healthy(&client, &profile, forward.port, &auth, &begun).await?;
            return Ok::<_, String>(RuntimeUpdateResult { phase:"current".into(), release:bundle.release.clone(), message:"The bundled runtime is already active; authenticated disarmed health was verified. No update was applied.".into() });
        }
        let token = Zeroizing::new(begun["token"].as_str().filter(|value| value.len() == 43).ok_or("No bounded runtime-update authorization was returned.")?.to_owned());
        if begun["phase"] != "awaiting_health" {
            let offset = begun["offset"].as_u64().and_then(|value| usize::try_from(value).ok()).filter(|value| *value <= bundle.bytes.len()).ok_or("Invalid durable transfer offset.")?;
            for at in (offset..bundle.bytes.len()).step_by(8192) {
                board(&serial, soc, &board_id, "chunk", json!({"token":token.as_str(),"offset":at,"data":STANDARD.encode(&bundle.bytes[at..(at+8192).min(bundle.bytes.len())])})).await?;
            }
            board(&serial, soc, &board_id, "stage", json!({"token":token.as_str()})).await?;
            board(&serial, soc, &board_id, "commit", json!({"token":token.as_str()})).await?;
        }
        let deadline = Instant::now() + Duration::from_secs(55);
        while Instant::now() < deadline {
            let status = board(&serial, soc, &board_id, "status", json!({})).await?;
            if status["phase"] == "rolled_back" {
                if healthy(&client, &profile, forward.port, &auth, &status).await.is_ok() {
                    return Ok(RuntimeUpdateResult { phase:"rolled_back".into(), release:status["current"].as_str().unwrap_or_default().into(), message:"The trial was rejected or unacknowledged. The prior runtime's authenticated disarmed health was verified; no successful update is claimed.".into() });
                }
            } else if status["current"].as_str() == Some(&bundle.release) && status["phase"] == "awaiting_health"
                && let Ok(hello) = healthy(&client, &profile, forward.port, &auth, &status).await {
                    let finished = board(&serial, soc, &board_id, "finish", json!({"token":token.as_str(),"nonce":hello["runtime"]["nonce"]})).await?;
                    if finished["phase"] == "committed" && finished["current"].as_str() == Some(&bundle.release) {
                        return Ok(RuntimeUpdateResult { phase:"committed".into(), release:bundle.release.clone(), message:"The bundled Linux runtime was committed after fresh authenticated disarmed health. Firmware, dependencies, TLS identity and Pairings were preserved. Hardware acceptance/setup completion is still separate.".into() });
                    }
            }
            auth.close(&client, &profile.tls_name, forward.port).await;
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        Err("Runtime acceptance is unresolved. The Board supervisor rolls back unacknowledged trials independently; reconnect USB and inspect/resume rather than assuming success.".into())
    }.await;
    let proof = async {
    if let Ok(outcome) = &result
        && ["current", "committed"].contains(&outcome.phase.as_str())
    {
        let hello = connection::request(
            &client,
            &profile,
            forward.port,
            reqwest::Method::GET,
            "/v1/hello",
            None,
            &auth,
        )
        .await
        .map_err(|error| error.message)?;
        if hello["board"]["board_id"].as_str() != Some(&board_id)
            || hello["health"]["safe_state"] != true
            || hello["runtime"]["release"].as_str() != Some(&bundle.release)
        {
            return Err("The runtime committed, but fresh setup proof is no longer valid. Reinspect/resume rather than claiming completion.".into());
        }
        crate::setup::observed_step(
            &setup_store,
            8,
            crate::setup::SetupObservation {
                serial: serial.clone(),
                soc: soc.clone(),
                board_id: board_id.clone(),
                boot_id: hello["runtime"]["boot_id"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                runtime_release: bundle.release.clone(),
                runtime_nonce: hello["runtime"]["nonce"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                hardware: None,
            },
        )?;
    }
    Ok::<(), String>(())
    }.await;
    auth.close(&client, &profile.tls_name, forward.port).await;
    proof?;
    if result.is_ok() {
        std::fs::remove_file(marker).map_err(|_| "Runtime outcome verified but the local pending record could not be cleared. Resolve it before gameplay.")?;
    }
    result
}

#[tauri::command]
pub async fn abort_companion_runtime_update(
    state: tauri::State<'_, Connections>,
    serial: String,
    board_id: String,
    confirmation: String,
) -> Result<RuntimeUpdateResult, String> {
    let profile = state.profile(&board_id).map_err(|error| error.message)?;
    if profile.adb_serial != serial || profile.installation_id.is_none() {
        return Err("Select the registered TBP profile for this exact USB Board.".into());
    }
    let source = serial.clone();
    let identity =
        tauri::async_runtime::spawn_blocking(move || crate::management::inspect(&source))
            .await
            .map_err(|_| "USB inspection stopped.")??;
    if confirmation != format!("ABORT UPDATE {}", identity.serial) {
        return Err("Type ABORT UPDATE followed by the inspected hardware serial. This cancels only this Board's pending runtime transfer/trial.".into());
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(state.directory.join("runtime-update.lock"))
        .map_err(|_| "Cannot establish recovery ownership.")?;
    let _lock = crate::file_lock::ExclusiveFile::new(lock).map_err(
        |_| "An update is still running in another app instance. Wait before requesting recovery.",
    )?;
    let _permit = state.begin_runtime_update()?;
    connection::disconnect_board(state.clone())
        .await
        .map_err(|error| error.message)?;
    pending_matches(&state.directory, &serial, &identity.serial, &board_id, None)?;
    let status = board(&serial, &identity.serial, &board_id, "status", json!({})).await?;
    let release = status["release"].as_str().unwrap_or("");
    if release.len() != 64 {
        return Err("No identified runtime job exists to cancel. Inspect pending ownership rather than clearing it blindly.".into());
    }
    pending(
        &state.directory,
        &serial,
        &identity.serial,
        &board_id,
        release,
    )?;
    board(
        &serial,
        &identity.serial,
        &board_id,
        "cancel",
        json!({"release":release}),
    )
    .await?;
    let source = serial.clone();
    let forward = tauri::async_runtime::spawn_blocking(move || pairing::Forward::new(&source))
        .await
        .map_err(|_| "Recovery forwarding stopped.")??;
    let candidate =
        connection::Candidate::new("127.0.0.1", forward.port).map_err(|error| error.message)?;
    let (client, _) = connection::secure_client(&profile, &candidate)
        .await
        .map_err(|error| error.message)?;
    let auth =
        GameplayAuth::from_profile(&profile, &state.directory).map_err(|error| error.message)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let observed = board(&serial, &identity.serial, &board_id, "status", json!({})).await?;
        if [Some("aborted"), Some("rolled_back"), Some("committed")]
            .contains(&observed["phase"].as_str())
            && healthy(&client, &profile, forward.port, &auth, &observed)
                .await
                .is_ok()
        {
            auth.close(&client, &profile.tls_name, forward.port).await;
            let marker = state.directory.join("runtime-update-active.json");
            if marker.exists() {
                std::fs::remove_file(marker).map_err(
                    |_| "Recovery verified, but local pending ownership could not be cleared.",
                )?;
            }
            return Ok(RuntimeUpdateResult { phase:observed["phase"].as_str().unwrap_or_default().into(), release:observed["current"].as_str().unwrap_or_default().into(), message:"The pending job was resolved and the active runtime's authenticated disarmed health was verified. An already committed update was not undone.".into() });
        }
        auth.close(&client, &profile.tls_name, forward.port).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Err("Cancellation was requested, but active runtime health remains unverified. Keep the local guard and use explicit USB/official recovery.".into())
}

pub(crate) fn runtime_identity() -> Result<(String, String), String> {
    let bundle = bundled_runtime()?;
    Ok((bundle.release, bundle.contract))
}

pub(crate) fn baseline_bundle() -> Result<(String, Vec<u8>), String> {
    let source: BTreeMap<_, _> = RUNTIME.iter().chain(CONTRACT.iter()).copied().collect();
    let files = source
        .iter()
        .map(|(name, bytes)| (*name, STANDARD.encode(bytes)))
        .collect::<BTreeMap<_, _>>();
    let release = tree(&source.into_iter().collect::<Vec<_>>());
    let bytes = serde_json::to_vec(&json!({"format":1,"release":release,"files":files}))
        .map_err(|_| "Could not prepare the fixed initial Companion package.")?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("The fixed initial package exceeds its safe size.".into());
    }
    Ok((release, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn initial_package_is_accepted_by_the_fixed_provisioner_without_starting_firmware() {
        let directory = tempfile::tempdir().unwrap();
        let (release, bytes) = baseline_bundle().unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut process = std::process::Command::new("python3")
            .arg(root.join("scripts/tests/provision_fixture.py"))
            .arg(directory.path())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        process.stdin.take().unwrap().write_all(&bytes).unwrap();
        let output = process.wait_with_output().unwrap();
        assert!(output.status.success());
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["release"].as_str(), Some(release.as_str()));
        assert_eq!(result["phase"], "awaiting_identity");
        assert_eq!(result["started"], false);
    }

    #[test]
    fn compiled_runtime_bundle_is_accepted_by_the_real_board_stager() {
        let directory = tempfile::tempdir().unwrap();
        let bundle = bundled_runtime().unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut process = std::process::Command::new("python3")
            .arg(root.join("companion/tests/deployment_fixture.py"))
            .arg(directory.path())
            .env("PYTHONPATH", root.join("companion/python"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        process
            .stdin
            .take()
            .unwrap()
            .write_all(&bundle.bytes)
            .unwrap();
        let output = process.wait_with_output().unwrap();
        assert!(output.status.success());
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["phase"], "staged");
        assert_eq!(result["release"].as_str(), Some(bundle.release.as_str()));
        assert_eq!(result["credentials"], "untouched");
    }
}
