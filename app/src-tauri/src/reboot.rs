//! Intent-before-schedule and observed restart proof. Transport loss is never completion.
use crate::{
    connection::{self, Candidate, Connections},
    management::{self, AdminAction, BoardIdentity, Management},
    pairing, session,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RestartReceipt {
    schema: u32,
    serial: String,
    soc: String,
    board_id: String,
    image: String,
    before_boot: String,
    phase: String,
    observed_boot: Option<String>,
}
fn location(directory: &Path, soc: &str) -> Result<PathBuf, String> {
    if soc.parse::<u64>().ok().filter(|value| *value > 0).is_none() || soc.len() > 20 {
        return Err("Invalid restart target identity.".into());
    }
    let parent = directory.join("board-restarts");
    if parent.is_symlink() {
        return Err("Unsupported restart evidence storage; no files were changed.".into());
    }
    Ok(parent.join(format!("{soc}.json")))
}
fn save(directory: &Path, record: &RestartReceipt) -> Result<(), String> {
    let path = location(directory, &record.soc)?;
    std::fs::create_dir_all(path.parent().unwrap())
        .map_err(|_| "Cannot prepare private restart evidence.")?;
    std::fs::set_permissions(
        path.parent().unwrap(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .map_err(|_| "Cannot protect restart evidence.")?;
    let temporary = path.with_extension("tmp");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temporary)
        .map_err(|_| "Cannot persist restart intent.")?;
    file.write_all(&serde_json::to_vec(record).map_err(|_| "Invalid restart evidence.")?)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot sync restart intent.")?;
    std::fs::rename(temporary, &path).map_err(|_| "Cannot publish restart evidence.")?;
    std::fs::File::open(path.parent().unwrap())
        .and_then(|directory| directory.sync_all())
        .map_err(|_| "Cannot sync restart evidence directory.")?;
    Ok(())
}
fn load(directory: &Path, soc: &str) -> Result<Option<RestartReceipt>, String> {
    let path = location(directory, soc)?;
    if !path.exists() {
        return Ok(None);
    }
    if path.is_symlink()
        || std::fs::metadata(&path)
            .map_err(|_| "Cannot inspect restart evidence.")?
            .len()
            > 4096
    {
        return Err("Restart evidence is unsupported; it was not reset.".into());
    }
    let record: RestartReceipt =
        serde_json::from_slice(&std::fs::read(path).map_err(|_| "Cannot read restart evidence.")?)
            .map_err(|_| "Restart evidence is corrupt; it was not reset.")?;
    if record.schema != 1
        || record.soc != soc
        || !session::valid_id(&record.before_boot)
        || !session::valid_id(&record.board_id)
        || !["requested", "scheduled", "verified"].contains(&record.phase.as_str())
    {
        return Err("Unsupported restart evidence; it was not overwritten.".into());
    }
    Ok(Some(record))
}
fn lock(directory: &Path) -> Result<crate::file_lock::ExclusiveFile, String> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(directory.join("board-restarts.lock"))
        .map_err(|_| "Cannot own restart verification.")?;
    crate::file_lock::ExclusiveFile::new(file).map_err(|_| {
        "Another app instance is tracking a restart. Wait before requesting another.".into()
    })
}
fn validate(
    record: &RestartReceipt,
    board: &BoardIdentity,
    board_id: &str,
    usb: &Value,
    lan: &Value,
) -> Result<String, String> {
    let boot = board
        .boot_id
        .as_ref()
        .filter(|value| session::valid_id(value))
        .ok_or(
            "The Board did not report a kernel boot identity; no restart completion is claimed.",
        )?;
    if board.serial != record.soc
        || board.image_version != record.image
        || board_id != record.board_id
        || boot == &record.before_boot
    {
        return Err("The same hardware/build has not demonstrated a new kernel boot. A disconnect, timeout or scheduled request is not success.".into());
    }
    for hello in [usb, lan] {
        if hello["board"]["board_id"].as_str() != Some(board_id)
            || hello["protocol"] != "1.0"
            || hello["runtime"]["boot_id"].as_str() != Some(boot)
            || hello["health"]["safe_state"] != true
        {
            return Err(
                "Fresh authenticated disarmed LAN and USB identity/boot evidence is incomplete."
                    .into(),
            );
        }
    }
    if usb["runtime"]["nonce"]
        .as_str()
        .is_none_or(|value| value.len() != 22)
        || usb["runtime"]["nonce"] != lan["runtime"]["nonce"]
        || usb["runtime"]["release"].as_str().is_none_or(|value| {
            value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        || usb["runtime"]["release"] != lan["runtime"]["release"]
    {
        return Err("LAN and USB did not observe the same current Companion process.".into());
    }
    Ok(boot.clone())
}

#[tauri::command]
pub async fn request_verified_board_restart(
    connections: tauri::State<'_, Connections>,
    management_state: tauri::State<'_, Management>,
    serial: String,
    board_id: String,
    password: String,
    confirmation: String,
) -> Result<Value, String> {
    let password = zeroize::Zeroizing::new(password);
    let preflight = connections.begin_maintenance()?;
    connection::disconnect_board(connections.clone())
        .await
        .map_err(|error| error.message)?;
    let profile = connections
        .profile(&board_id)
        .map_err(|error| error.message)?;
    if profile.adb_serial != serial || profile.installation_id.is_none() {
        return Err(
            "Select this exact registered TBP Board before requesting a verified restart.".into(),
        );
    }
    management::assert_authorized(&management_state, &serial)?;
    let auth = session::GameplayAuth::from_profile(&profile, &connections.directory)
        .map_err(|error| error.message)?;
    let directory = connections.directory.clone();
    let _lock = lock(&directory)?;
    let source_serial = serial.clone();
    let board = tauri::async_runtime::spawn_blocking(move || management::inspect(&source_serial))
        .await
        .map_err(|_| "Restart preflight stopped.")??;
    if confirmation != format!("RESTART {}", board.serial) {
        return Err("Type RESTART followed by the inspected hardware serial.".into());
    }
    let boot = board
        .boot_id
        .clone()
        .filter(|value| session::valid_id(value))
        .ok_or(
            "Kernel boot identity is unavailable. Use official supported restart/recovery instead.",
        )?;
    if board.image_version.is_empty() || board.image_version == "unknown" {
        return Err("An observable OS build is required before a tracked restart.".into());
    }
    if let Some(previous) = load(&directory, &board.serial)?
        && previous.phase != "verified"
        && previous.before_boot == boot
    {
        return Err("A restart intent for this kernel boot is already unresolved. Wait/verify rather than issuing a duplicate. If scheduling failed, use explicit official recovery/restart; no successful reboot is assumed.".into());
    }
    let source_serial = serial.clone();
    let forward =
        tauri::async_runtime::spawn_blocking(move || pairing::Forward::new(&source_serial))
            .await
            .map_err(|_| "Restart TLS preflight stopped.")??;
    let candidate = Candidate::new("127.0.0.1", forward.port).map_err(|error| error.message)?;
    let (client, _) = connection::secure_client(&profile, &candidate)
        .await
        .map_err(|error| error.message)?;
    let hello = connection::request(
        &client,
        &profile,
        forward.port,
        reqwest::Method::GET,
        "/v1/hello",
        None,
        &auth,
    )
    .await;
    auth.close(&client, &profile.tls_name, forward.port).await;
    let hello = hello.map_err(|error| error.message)?;
    if hello["board"]["board_id"].as_str() != Some(&board_id)
        || hello["runtime"]["boot_id"].as_str() != Some(&boot)
        || hello["health"]["safe_state"] != true
    {
        return Err("The selected paired Companion/host did not agree on a disarmed current kernel boot. No restart was requested.".into());
    }
    let mut receipt = RestartReceipt {
        schema: 1,
        serial: serial.clone(),
        soc: board.serial,
        board_id,
        image: board.image_version,
        before_boot: boot,
        phase: "requested".into(),
        observed_boot: None,
    };
    save(&directory, &receipt)?; // BEFORE the request: lost replies must not invite duplicate schedules.
    drop(preflight); // The existing admin adapter acquires its own exclusive management scope.
    let result = management::admin_action(
        management_state,
        connections,
        serial,
        AdminAction::Reboot {
            password: password.to_string(),
            confirmation,
        },
    )
    .await;
    match result {
        Ok(mut scheduled) => { receipt.phase = "scheduled".into(); save(&directory, &receipt)?; scheduled["verification"] = "pending_new_boot_and_authenticated_lan_usb".into(); Ok(scheduled) },
        Err(_) => Err("Restart scheduling has an unresolved outcome. Its intent was retained; do not schedule duplicates or treat transport loss as success. Reinspect/verify or use the explicit official recovery handoff.".into()),
    }
}

#[tauri::command]
pub async fn verify_board_restart(
    connections: tauri::State<'_, Connections>,
    setup_store: tauri::State<'_, crate::setup::SetupStore>,
    serial: String,
    board_id: String,
    lan_host: String,
    lan_port: u16,
    local_consent: bool,
) -> Result<Value, String> {
    if !local_consent {
        return Err("Approve contact with the explicitly entered local-network endpoint before verification.".into());
    }
    let _permit = connections.begin_maintenance()?;
    connection::disconnect_board(connections.clone())
        .await
        .map_err(|error| error.message)?;
    let _lock = lock(&connections.directory)?;
    let profile = connections
        .profile(&board_id)
        .map_err(|error| error.message)?;
    if profile.installation_id.is_none() {
        return Err("Authenticated restart proof requires installation-key Pairing, not a legacy bearer record.".into());
    }
    let source_serial = serial.clone();
    let (board, forward) = tauri::async_runtime::spawn_blocking(move || {
        Ok::<_, String>((
            management::inspect(&source_serial)?,
            pairing::Forward::new(&source_serial)?,
        ))
    })
    .await
    .map_err(|_| "Restart observation stopped.")??;
    let mut receipt = load(&connections.directory, &board.serial)?.ok_or(
        "No matching native restart intent exists. Do not invent a pre-reboot observation.",
    )?;
    let auth = session::GameplayAuth::from_profile(&profile, &connections.directory)
        .map_err(|error| error.message)?;
    let usb_candidate = Candidate::new("127.0.0.1", forward.port).map_err(|error| error.message)?;
    let (usb_client, _) = connection::secure_client(&profile, &usb_candidate)
        .await
        .map_err(|error| error.message)?;
    let usb = connection::request(
        &usb_client,
        &profile,
        forward.port,
        reqwest::Method::GET,
        "/v1/hello",
        None,
        &auth,
    )
    .await;
    auth.close(&usb_client, &profile.tls_name, forward.port)
        .await;
    let usb = usb.map_err(|error| error.message)?;
    let lan_candidate = Candidate::new(&lan_host, lan_port).map_err(|error| error.message)?;
    let (lan_client, addresses) = connection::secure_client(&profile, &lan_candidate)
        .await
        .map_err(|error| error.message)?;
    if addresses.iter().any(|address| address.ip().is_loopback() || matches!(address.ip(), std::net::IpAddr::V6(ip) if ip.to_ipv4_mapped().is_some_and(|ip| ip.is_loopback()))) { return Err("A loopback/ADB endpoint cannot count as LAN verification.".into()); }
    let lan = connection::request(
        &lan_client,
        &profile,
        lan_port,
        reqwest::Method::GET,
        "/v1/hello",
        None,
        &auth,
    )
    .await;
    auth.close(&lan_client, &profile.tls_name, lan_port).await;
    let boot = validate(
        &receipt,
        &board,
        &board_id,
        &usb,
        &lan.map_err(|error| error.message)?,
    )?;
    receipt.phase = "verified".into();
    receipt.observed_boot = Some(boot.clone());
    save(&connections.directory, &receipt)?;
    let mut updated = profile;
    updated.adb_serial = serial;
    connection::remember_board(&connections, &updated).map_err(|error| error.message)?;
    crate::setup::observed_step(
        &setup_store,
        9,
        crate::setup::SetupObservation {
            serial: updated.adb_serial.clone(),
            soc: board.serial.clone(),
            board_id: board_id.clone(),
            boot_id: boot.clone(),
            runtime_release: usb["runtime"]["release"]
                .as_str()
                .unwrap_or_default()
                .into(),
            runtime_nonce: usb["runtime"]["nonce"].as_str().unwrap_or_default().into(),
            hardware: None,
        },
    )?;
    Ok(
        json!({"state":"restart_verified","boardId":board_id,"serial":board.serial,"bootId":boot,"source":"observed_kernel_and_pinned_lan_usb","message":"The same hardware/build demonstrated a new kernel boot and the same disarmed Companion over authenticated LAN and USB. No hardware self-test or automatic rearm was performed."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const BEFORE: &str = "98f6411f-20ec-438a-a1de-6ce20fb0370e";
    const AFTER: &str = "1f33fd77-7015-41c6-a882-650f37a06c18";
    const BOARD: &str = "e248c62d-f761-4ac5-9626-67bf77b59205";
    fn host() -> BoardIdentity {
        BoardIdentity {
            adb_serial: "USB".into(),
            serial: "42".into(),
            username: "arduino".into(),
            hostname: "fixture".into(),
            os: "debian".into(),
            os_version: "13".into(),
            image_version: "known-build".into(),
            boot_id: Some(AFTER.into()),
        }
    }
    #[test]
    fn restart_requires_persisted_intent_new_kernel_and_matching_authenticated_peers() {
        let directory = tempfile::tempdir().unwrap();
        let mut record = RestartReceipt {
            schema: 1,
            serial: "USB".into(),
            soc: "42".into(),
            board_id: BOARD.into(),
            image: "known-build".into(),
            before_boot: BEFORE.into(),
            phase: "requested".into(),
            observed_boot: None,
        };
        save(directory.path(), &record).unwrap();
        record = load(directory.path(), "42").unwrap().unwrap();
        let hello = json!({"protocol":"1.0","board":{"board_id":BOARD},"runtime":{"boot_id":AFTER,"nonce":"abcdefghijklmnopqrstuv","release":"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"},"health":{"safe_state":true}});
        assert_eq!(
            validate(&record, &host(), BOARD, &hello, &hello).unwrap(),
            AFTER
        );
        let mut unchanged = host();
        unchanged.boot_id = Some(BEFORE.into());
        assert!(validate(&record, &unchanged, BOARD, &hello, &hello).is_err());
        let mut wrong = hello.clone();
        wrong["board"]["board_id"] = BEFORE.into();
        assert!(validate(&record, &host(), BOARD, &hello, &wrong).is_err());
        wrong = hello.clone();
        wrong["runtime"]["nonce"] = "different-live-process".into();
        assert!(validate(&record, &host(), BOARD, &hello, &wrong).is_err());
        let mut other_build = host();
        other_build.image_version = "old-image".into();
        assert!(validate(&record, &other_build, BOARD, &hello, &hello).is_err());
    }
}
