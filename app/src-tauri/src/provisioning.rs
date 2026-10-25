//! Fixed initial source/identity preparation. Official first firmware start stays a handoff.
use crate::{
    connection::{self, Connections},
    management, runtime_update,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::Duration;
use zeroize::Zeroizing;

const HELPER: &str = include_str!("companion_provision.py");

fn usb(serial: &str, soc: &str, mut body: Value) -> Result<Value, String> {
    if management::inspect(serial)?.serial != soc {
        return Err("The physical Board changed; preparation stopped.".into());
    }
    body["expectedSerial"] = soc.into();
    let adb = crate::discovery::adb_path().map_err(|error| error.message)?;
    let command = format!(
        "python3 -c 'import base64;exec(base64.b64decode(\"{}\"))'",
        STANDARD.encode(HELPER)
    );
    let input = Zeroizing::new(
        serde_json::to_vec(&body).map_err(|_| "Invalid initial preparation request.")?,
    );
    let bytes = Zeroizing::new(crate::adb_transport::request(&adb, serial, &command, &input, Duration::from_secs(60)).map_err(|_| "Initial preparation is unsupported or interrupted. Reinspect the same Board; existing apps were not overwritten and no start/firmware operation was requested.")?);
    let response: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Incompatible initial preparation response.")?;
    if response["status"] != "ok" || response["result"]["serial"].as_str() != Some(soc) {
        return Err(
            "The exact Board did not confirm initial preparation. No setup completion is assumed."
                .into(),
        );
    }
    Ok(response["result"].clone())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialPackage {
    release: String,
    bytes: usize,
}
#[tauri::command]
pub fn initial_companion_package() -> Result<InitialPackage, String> {
    let (release, bytes) = runtime_update::baseline_bundle()?;
    Ok(InitialPackage {
        release,
        bytes: bytes.len(),
    })
}
#[tauri::command]
pub async fn inspect_initial_companion(serial: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let identity = management::inspect(&serial)?;
        usb(&serial, &identity.serial, json!({"operation":"inspect"}))
    })
    .await
    .map_err(|_| "Initial preparation inspection stopped.")?
}

#[tauri::command]
pub async fn prepare_initial_companion(
    state: tauri::State<'_, Connections>,
    serial: String,
    confirmation: String,
) -> Result<Value, String> {
    let _permit = state.begin_maintenance()?;
    connection::disconnect_board(state)
        .await
        .map_err(|error| error.message)?;
    tauri::async_runtime::spawn_blocking(move || {
        let identity = management::inspect(&serial)?;
        if confirmation != format!("PREPARE {}", identity.serial) { return Err("Type PREPARE followed by the inspected hardware serial. This prepares initial files/identity only; it never starts the app or writes MCU firmware.".into()); }
        let (release, bytes) = runtime_update::baseline_bundle()?;
        let sha: String = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect();
        let begun = usb(&serial, &identity.serial, json!({"operation":"begin","release":release,"sha256":sha,"size":bytes.len()}))?;
        if begun["phase"] != "awaiting_identity" {
            let token = Zeroizing::new(begun["token"].as_str().filter(|value| value.len() == 43).ok_or("Missing initial preparation authorization.")?.to_owned());
            let offset = begun["offset"].as_u64().and_then(|value| usize::try_from(value).ok()).filter(|value| *value <= bytes.len()).ok_or("Invalid durable preparation offset.")?;
            for at in (offset..bytes.len()).step_by(8192) {
                usb(&serial, &identity.serial, json!({"operation":"chunk","token":token.as_str(),"offset":at,"data":STANDARD.encode(&bytes[at..(at+8192).min(bytes.len())])}))?;
            }
            let published = usb(&serial, &identity.serial, json!({"operation":"publish","token":token.as_str()}))?;
            if published["release"].as_str() != Some(&release) || published["started"] != false { return Err("Initial source publication was not confirmed; no firmware/start success is assumed.".into()); }
        }
        let mut prepared = usb(&serial, &identity.serial, json!({"operation":"identity","release":release}))?;
        if prepared["phase"] != "identity_ready" || prepared["release"].as_str() != Some(&release) || prepared["started"] != false { return Err("Initial identity preparation was not confirmed.".into()); }
        prepared["message"] = "Verified initial source and private TLS identity were prepared. No app was started and no MCU firmware was written. Use official App Lab for the first start (which can upload the sketch); then return for observed Pairing/connection/self-test proof. This is not setup completion.".into();
        Ok(prepared)
    }).await.map_err(|_| "Initial preparation stopped. Reinspect and resume; success has not been assumed.")?
}

fn same_identity_key(previous: &str, current: &str) -> Result<bool, String> {
    let previous = openssl::x509::X509::from_pem(previous.as_bytes())
        .map_err(|_| "Invalid saved Board certificate.")?
        .public_key()
        .map_err(|_| "Missing saved Board public key.")?;
    let current = openssl::x509::X509::from_pem(current.as_bytes())
        .map_err(|_| "Invalid prepared Board certificate.")?
        .public_key()
        .map_err(|_| "Missing prepared Board public key.")?;
    Ok(previous.public_eq(&current))
}

#[tauri::command]
pub async fn renew_companion_identity(
    state: tauri::State<'_, Connections>,
    serial: String,
    board_id: String,
    confirmation: String,
) -> Result<Value, String> {
    let _permit = state.begin_maintenance()?;
    connection::disconnect_board(state.clone())
        .await
        .map_err(|error| error.message)?;
    let mut profile = state.profile(&board_id).map_err(|error| error.message)?;
    if profile.adb_serial != serial || profile.installation_id.is_none() {
        return Err("Select this exact registered TBP Board before identity renewal.".into());
    }
    let auth = crate::session::GameplayAuth::from_profile(&profile, &state.directory)
        .map_err(|error| error.message)?;
    let (wanted_release, wanted_contract) = runtime_update::runtime_identity()?;
    let source_profile = profile.clone();
    let source_serial = serial.clone();
    let (soc, certificate, wanted_hash, previous_nonce, forward) = tauri::async_runtime::spawn_blocking(move || {
        let identity = management::inspect(&source_serial)?;
        if confirmation != format!("RENEW TLS {}", identity.serial) { return Err("Type RENEW TLS followed by the inspected hardware serial. Renewal preserves the private key and restarts only the owned Linux child, never MCU firmware.".into()); }
        let initial = usb(&source_serial, &identity.serial, json!({"operation":"inspect"}))?;
        let public = &initial["identity"];
        if public["board_id"].as_str() != Some(&source_profile.board_id) || public["tls_name"].as_str() != Some(&source_profile.tls_name) || !same_identity_key(&source_profile.certificate_pem, public["certificate_pem"].as_str().ok_or("Missing physical Board certificate.")?)? { return Err("The physical TLS identity differs from the saved key; automatic rekeying/repinning was refused.".into()); }
        let runtime = crate::pairing::usb(&source_serial, &json!({"operation":"runtime_status","expected_serial":identity.serial}))?;
        if runtime["board_id"].as_str() != Some(&source_profile.board_id) || runtime["serial"].as_str() != Some(&identity.serial) || runtime["current"].as_str() != Some(&wanted_release) || runtime["contract"].as_str() != Some(&wanted_contract) || runtime["supervisor_ready"] != true || runtime["observed"]["certificate_sha256"].as_str().is_none_or(|value| value.len() != 64) || ![Some("idle"), Some("committed"), Some("rolled_back"), Some("aborted")].contains(&runtime["phase"].as_str()) { return Err("The baseline does not support a safe certificate reload or runtime maintenance is pending. Resolve/update it through supported setup first; no certificate was changed.".into()); }
        let previous = runtime["observed"]["nonce"].as_str().filter(|value| value.len() == 22).ok_or("Missing current process observation.")?.to_owned();
        let release = initial["release"].as_str().ok_or("Missing verified source identity.")?;
        let renewed = usb(&source_serial, &identity.serial, json!({"operation":"renew_identity","release":release}))?;
        let certificate = renewed["certificate_pem"].as_str().ok_or("Missing renewed public certificate.")?.to_owned();
        if renewed["board_id"].as_str() != Some(&source_profile.board_id) || renewed["tls_name"].as_str() != Some(&source_profile.tls_name) || !same_identity_key(&source_profile.certificate_pem, &certificate)? { return Err("Renewal did not preserve the registered public key. Stop and recover over physical USB; no replacement trust was saved.".into()); }
        let hash: String = Sha256::digest(certificate.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect();
        let forward = crate::pairing::Forward::new(&source_serial)?;
        Ok::<_, String>((identity.serial, certificate, hash, previous, forward))
    }).await.map_err(|_| "Certificate renewal stopped; inspect the exact Board before assuming success.")??;
    let candidate =
        connection::Candidate::new("127.0.0.1", forward.port).map_err(|error| error.message)?;
    // Keep the OLD anchor for verification; never trust a replacement merely because USB returned it.
    let (client, _) = connection::secure_client(&profile, &candidate)
        .await
        .map_err(|error| error.message)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut verified = false;
    while std::time::Instant::now() < deadline {
        if let Ok(hello) = connection::request(
            &client,
            &profile,
            forward.port,
            reqwest::Method::GET,
            "/v1/hello",
            None,
            &auth,
        )
        .await
            && hello["board"]["board_id"].as_str() == Some(&board_id)
            && hello["runtime"]["certificate_sha256"].as_str() == Some(&wanted_hash)
            && hello["runtime"]["nonce"]
                .as_str()
                .is_some_and(|value| value.len() == 22 && value != previous_nonce)
            && hello["health"]["safe_state"] == true
        {
            verified = true;
            break;
        }
        auth.close(&client, &profile.tls_name, forward.port).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    auth.close(&client, &profile.tls_name, forward.port).await;
    if !verified {
        return Err("The certificate file may have renewed, but a fresh authenticated child with the old trust anchor was not verified. No local repinning or setup completion was performed; reinspect over USB.".into());
    }
    profile.certificate_pem = certificate;
    connection::remember_board(&state, &profile).map_err(|error| error.message)?;
    Ok(
        json!({"phase":"renewed","serial":soc,"boardId":board_id,"message":"The certificate renewed with the same key. A fresh disarmed Linux child was authenticated using the prior trust anchor before refreshing the saved certificate. No firmware, Pairings or private keys were replaced; reconnect/rearm explicitly when ready."}),
    )
}

#[tauri::command]
pub async fn cancel_initial_companion(
    state: tauri::State<'_, Connections>,
    serial: String,
    confirmation: String,
) -> Result<Value, String> {
    let _permit = state.begin_maintenance()?;
    connection::disconnect_board(state)
        .await
        .map_err(|error| error.message)?;
    tauri::async_runtime::spawn_blocking(move || {
        let identity = management::inspect(&serial)?;
        if confirmation != format!("ABORT PREPARE {}", identity.serial) { return Err("Type ABORT PREPARE followed by the inspected hardware serial. This cancels only a partial initial transfer and never deletes an installed app or its identity.".into()); }
        let current = usb(&serial, &identity.serial, json!({"operation":"inspect"}))?;
        let release = current["release"].as_str().filter(|value| value.len() == 64).ok_or("No identified initial preparation exists to cancel.")?;
        let mut cancelled = usb(&serial, &identity.serial, json!({"operation":"cancel","release":release}))?;
        cancelled["message"] = "Initial transfer state resolved. Already published app files, credentials and private identity were not removed. No app was started or stopped.".into();
        Ok(cancelled)
    }).await.map_err(|_| "Initial preparation cancellation stopped; reinspect before assuming completion.")?
}
