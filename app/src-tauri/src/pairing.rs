//! Fixed physical USB bootstrap for an already installed TBP Companion. No deploy/shell IPC.
use crate::{
    admin_process,
    connection::{self, BoardProfile, Connections},
    management, session,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};
use zeroize::Zeroizing;

const COMMAND: &str = "python3 /home/arduino/ArduinoApps/tinkercrew-companion/python/management.py";

fn usb(serial: &str, body: &Value) -> Result<Value, String> {
    management::inspect(serial)?; // Requires fresh physical usb: evidence and UNO Q/Debian/SoC identity.
    let path = crate::discovery::adb_path().map_err(|error| error.message)?;
    let input = Zeroizing::new(serde_json::to_vec(body).map_err(|_| "Invalid Pairing input.")?);
    let output = Zeroizing::new(admin_process::run(&path, &["-H", "127.0.0.1", "-P", "5037", "-s", serial, "exec-out", COMMAND], &input, Duration::from_secs(10)).map_err(|_| "The updated TBP Companion host helper is unavailable or rejected the request. Install/configure the compatible Companion through official tooling; no guessed helper/path fallback was attempted.")?);
    let response: Value = serde_json::from_slice(&output)
        .map_err(|_| "Incompatible Companion management response.")?;
    if response["status"] != "ok" {
        return Err("Companion management failed; check physical identity, permissions and TBP configuration. No completion is assumed.".into());
    }
    Ok(response["result"].clone())
}
struct Forward {
    path: PathBuf,
    serial: String,
    port: u16,
}
impl Forward {
    fn new(serial: &str) -> Result<Self, String> {
        let path = crate::discovery::adb_path().map_err(|error| error.message)?;
        let output = admin_process::run(
            &path,
            &[
                "-H",
                "127.0.0.1",
                "-P",
                "5037",
                "-s",
                serial,
                "forward",
                "tcp:0",
                "tcp:7443",
            ],
            &[],
            Duration::from_secs(4),
        )?;
        let port = std::str::from_utf8(&output)
            .ok()
            .and_then(|value| value.trim().parse::<u16>().ok())
            .filter(|value| *value >= 1024)
            .ok_or("ADB did not allocate a safe Companion port.")?;
        Ok(Self {
            path,
            serial: serial.into(),
            port,
        })
    }
}
impl Drop for Forward {
    fn drop(&mut self) {
        let _ = admin_process::run(
            &self.path,
            &[
                "-H",
                "127.0.0.1",
                "-P",
                "5037",
                "-s",
                &self.serial,
                "forward",
                "--remove",
                &format!("tcp:{}", self.port),
            ],
            &[],
            Duration::from_secs(3),
        );
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingResult {
    board_id: String,
    installation_id: String,
    state: &'static str,
}

#[tauri::command]
pub async fn pair_installed_companion(
    state: tauri::State<'_, Connections>,
    serial: String,
    confirmation: String,
    installation_label: String,
) -> Result<PairingResult, String> {
    let _permit = state.begin_maintenance()?;
    connection::disconnect_board(state.clone())
        .await
        .map_err(|error| error.message)?;
    let directory = state.directory.clone();
    let source_serial = serial.clone();
    let (keys, registration, bootstrap, forward, soc) = tauri::async_runtime::spawn_blocking(move || {
        let identity = management::inspect(&source_serial)?;
        if confirmation != format!("PAIR {}", identity.serial) { return Err("Type PAIR followed by the physically inspected hardware serial. This registers installation keys; it does not install the Companion.".into()); }
        // Persist installation keys BEFORE remote changes so retries retain the same identity/key pair.
        let keys = session::installation(&directory, true).map_err(|error| error.message)?;
        let registration = keys.registration(&installation_label).map_err(|error| error.message)?;
        let bootstrap = usb(&source_serial, &json!({"operation":"pair_begin","expected_serial":identity.serial,"registration":registration}))?;
        if bootstrap["serial"].as_str() != Some(&identity.serial) || bootstrap["protocol_versions"] != json!(["1.0"]) { return Err("The bootstrap identity or protocol changed; Pairing stopped.".into()); }
        let forward = Forward::new(&source_serial)?;
        Ok::<_, String>((keys, registration, bootstrap, forward, identity.serial))
    }).await.map_err(|_| "USB Pairing bootstrap stopped; retry with the same installation keys.")??;
    let board_id = bootstrap["board_id"]
        .as_str()
        .filter(|value| session::valid_id(value))
        .ok_or("Invalid bootstrap Board identity.")?
        .to_owned();
    let profile = BoardProfile {
        board_id: board_id.clone(),
        name: "Paired UNO Q".into(),
        adb_serial: serial.clone(),
        tls_name: bootstrap["tls_name"]
            .as_str()
            .ok_or("Missing USB TLS identity.")?
            .into(),
        certificate_pem: bootstrap["certificate_pem"]
            .as_str()
            .ok_or("Missing USB TLS certificate.")?
            .into(),
        api_token: String::new(),
        installation_id: Some(keys.installation_id.clone()),
        last_address: None,
    };
    profile.validate().map_err(|error| error.message)?;
    let candidate =
        connection::Candidate::new("127.0.0.1", forward.port).map_err(|error| error.message)?;
    let (client, _) = connection::secure_client(&profile, &candidate)
        .await
        .map_err(|error| error.message)?;
    let mut request = registration;
    request["pairing_token"] = bootstrap["pairing_token"].clone();
    let result = session::post(
        &client,
        &profile.tls_name,
        forward.port,
        "/v1/pairings",
        &request,
    )
    .await
    .map_err(|error| error.message)?;
    if result["board_id"].as_str() != Some(&board_id)
        || result["installation_id"].as_str() != Some(&keys.installation_id)
        || ![Some("pending_ssh"), Some("active")].contains(&result["state"].as_str())
    {
        return Err("The Pairing response did not match this installation and Board.".into());
    }
    let installation = keys.installation_id.clone();
    let source_serial = serial.clone();
    let activated = tauri::async_runtime::spawn_blocking(move || usb(&source_serial, &json!({"operation":"pair_activate","expected_serial":soc,"installation_id":installation}))).await.map_err(|_| "SSH registration stopped; retry Pairing with the saved installation keys.")??;
    if activated["state"] != "active"
        || activated["installation_id"].as_str() != Some(&keys.installation_id)
    {
        return Err("The dedicated SSH key registration was not confirmed.".into());
    }
    let installation_id = keys.installation_id.clone();
    let auth = session::GameplayAuth::paired(keys);
    auth.token(&client, &profile.tls_name, forward.port, &board_id)
        .await
        .map_err(|error| error.message)?;
    auth.close(&client, &profile.tls_name, forward.port).await;
    connection::remember_board(&state, &profile).map_err(|error| error.message)?;
    Ok(PairingResult {
        board_id,
        installation_id,
        state: "registered",
    })
}

#[tauri::command]
pub async fn list_board_pairings(serial: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let identity = management::inspect(&serial)?;
        let mut result = usb(
            &serial,
            &json!({"operation":"list_pairings","expected_serial":identity.serial}),
        )?;
        result["serial"] = identity.serial.into();
        Ok(result)
    })
    .await
    .map_err(|_| "Physical Pairing inspection stopped.")?
}

#[tauri::command]
pub async fn revoke_board_pairing(
    state: tauri::State<'_, Connections>,
    serial: String,
    installation_id: String,
    confirmation: String,
) -> Result<Value, String> {
    if !session::valid_id(&installation_id) {
        return Err("Select a valid installation identity for physical revocation.".into());
    }
    let _permit = state.begin_maintenance()?;
    connection::disconnect_board(state)
        .await
        .map_err(|error| error.message)?;
    tauri::async_runtime::spawn_blocking(move || {
        let identity = management::inspect(&serial)?;
        if confirmation != format!("REVOKE {installation_id} ON {}", identity.serial) { return Err("Type REVOKE followed by the installation ID, ON, and the inspected hardware serial. This changes the physical Board, not just this Mac's record.".into()); }
        let result = usb(&serial, &json!({"operation":"revoke","expected_serial":identity.serial,"installation_id":installation_id}))?;
        if result["revoked"] != true || result["installation_id"].as_str() != Some(&installation_id) || !result["ssh_key_removed"].is_boolean() { return Err("The Board did not confirm the selected revocation. Reinspect its Pairing list.".into()); }
        Ok(result)
    }).await.map_err(|_| "USB revocation stopped; check the Board's Pairing list before assuming completion.")?
}
