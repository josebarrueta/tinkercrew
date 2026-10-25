//! Physical USB Pairing and restricted dedicated-key SSH proof. No arbitrary shell IPC.
use crate::{
    admin_process,
    connection::{self, BoardProfile, Connections},
    management, session,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use security_framework::passwords::{get_generic_password, set_generic_password};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    time::{Duration, Instant},
};
pub(crate) const SSH_PIN_SERVICE: &str = "com.tinkercrew.desktop.installation-ssh";
use zeroize::Zeroizing;

const COMMAND: &str = "python3 /home/arduino/ArduinoApps/tinkercrew-companion/python/management.py";

pub(crate) fn usb(serial: &str, body: &Value) -> Result<Value, String> {
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
pub(crate) struct Forward {
    path: PathBuf,
    serial: String,
    pub(crate) port: u16,
}
impl Forward {
    pub(crate) fn new(serial: &str) -> Result<Self, String> {
        Self::for_port(serial, 7443)
    }
    fn ssh(serial: &str) -> Result<Self, String> {
        Self::for_port(serial, 22)
    }
    fn for_port(serial: &str, remote: u16) -> Result<Self, String> {
        if ![22, 7443].contains(&remote) {
            return Err("Unsupported fixed Companion forwarding endpoint.".into());
        }
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
                &format!("tcp:{remote}"),
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

fn verify_dedicated_ssh(
    serial: &str,
    soc: &str,
    profile: &BoardProfile,
    keys: &session::InstallationKeys,
) -> Result<String, String> {
    let physical = usb(
        serial,
        &json!({"operation":"ssh_identity","expected_serial":soc}),
    )?;
    if physical["serial"].as_str() != Some(soc)
        || physical["board_id"].as_str() != Some(&profile.board_id)
        || physical["username"] != "arduino"
    {
        return Err(
            "The USB SSH identity/account does not match this Board. Use supported official setup."
                .into(),
        );
    }
    let host = STANDARD
        .decode(
            physical["host_key"]
                .as_str()
                .ok_or("Missing USB SSH host identity.")?,
        )
        .map_err(|_| "Invalid USB SSH host identity.")?;
    if host.len() != 51 {
        return Err("Unsupported SSH host identity.".into());
    }
    let forward = Forward::ssh(serial)?;
    let tcp = TcpStream::connect_timeout(&SocketAddr::from(([127,0,0,1], forward.port)), Duration::from_secs(3)).map_err(|_| "SSH is disabled or unreachable. Enable it explicitly over USB/official setup, then retry Pairing with the existing installation keys.")?;
    let mut ssh = ssh2::Session::new().map_err(|_| "Dedicated-key SSH could not initialize.")?;
    ssh.set_timeout(5000);
    ssh.method_pref(ssh2::MethodType::HostKey, "ssh-ed25519")
        .map_err(|_| "This SSH implementation cannot verify the supported host identity.")?;
    ssh.set_tcp_stream(tcp);
    ssh.handshake().map_err(
        |_| "Dedicated-key SSH handshake failed; no weaker host-key fallback was attempted.",
    )?;
    if ssh.host_key().map(|(key, _)| key) != Some(host.as_slice()) {
        return Err(
            "The SSH peer differs from the physically inspected host key; Pairing stopped.".into(),
        );
    }
    let pin = ssh
        .host_key_hash(ssh2::HashType::Sha256)
        .ok_or("Missing SSH fingerprint.")?;
    match get_generic_password(SSH_PIN_SERVICE, serial) {
        Ok(previous) if previous != pin => return Err("The installation SSH host key changed. Verify the physical Board and explicitly clear this app's SSH trust after an intentional restore; it was not silently replaced.".into()),
        Ok(_) => {},
        Err(error) if error.code() == -25300 => {},
        Err(_) => return Err("Unlock Keychain before verifying the dedicated SSH identity.".into()),
    }
    let private = keys.ssh_private().map_err(|error| error.message)?;
    ssh.userauth_pubkey_memory("arduino", None, &private, None).map_err(|_| "The registered dedicated key did not authenticate. Reinspect permissions/SSH support and retry; no password/agent fallback was attempted.")?;
    if !ssh.authenticated() {
        return Err("Dedicated-key SSH authorization was not completed.".into());
    }
    let mut channel = ssh
        .channel_session()
        .map_err(|_| "Restricted SSH inspection could not start.")?;
    channel
        .exec("inspect")
        .map_err(|_| "Restricted SSH inspection was rejected.")?;
    channel
        .write_all(
            &serde_json::to_vec(&json!({"operation":"inspect","expected_serial":soc}))
                .map_err(|_| "Invalid fixed SSH inspection.")?,
        )
        .map_err(|_| "Restricted SSH inspection could not be sent.")?;
    channel
        .send_eof()
        .map_err(|_| "Restricted SSH input could not close.")?;
    let mut output = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    ssh.set_blocking(false);
    let mut buffer = [0u8; 4096];
    loop {
        if Instant::now() >= deadline {
            return Err("Restricted SSH inspection exceeded its total response deadline.".into());
        }
        match channel.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                output.extend_from_slice(&buffer[..count]);
                if output.len() > 16384 {
                    return Err("Restricted SSH inspection exceeded its safe limit.".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(_) => {
                return Err(
                    "Restricted SSH inspection failed; no general shell was attempted.".into(),
                );
            }
        }
    }
    ssh.set_blocking(true);
    channel
        .wait_close()
        .map_err(|_| "Restricted SSH inspection did not close.")?;
    if channel.exit_status().map_err(|_| "Missing SSH result.")? != 0 {
        return Err("Restricted SSH inspection failed; no general shell was attempted.".into());
    }
    let proof: Value =
        serde_json::from_slice(&output).map_err(|_| "Incompatible restricted SSH proof.")?;
    let result = &proof["result"];
    if proof["status"] != "ok"
        || result["board_id"].as_str() != Some(&profile.board_id)
        || result["serial"].as_str() != Some(soc)
        || result["username"] != "arduino"
        || result["tls_name"].as_str() != Some(&profile.tls_name)
        || result["certificate_pem"].as_str() != Some(&profile.certificate_pem)
    {
        return Err(
            "Restricted SSH did not prove the same Board, account and TLS identity as USB Pairing."
                .into(),
        );
    }
    let boot = physical["boot_id"]
        .as_str()
        .filter(|value| session::valid_id(value))
        .ok_or("The host did not report a supported kernel boot identity.")?;
    if result["boot_id"].as_str() != Some(boot) {
        return Err("USB and restricted SSH did not observe the same kernel boot.".into());
    }
    set_generic_password(SSH_PIN_SERVICE, serial, pin)
        .map_err(|_| "Could not save the verified dedicated SSH fingerprint.")?;
    Ok(boot.to_owned())
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
    setup_store: tauri::State<'_, crate::setup::SetupStore>,
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
    let activation_soc = soc.clone();
    let activated = tauri::async_runtime::spawn_blocking(move || usb(&source_serial, &json!({"operation":"pair_activate","expected_serial":activation_soc,"installation_id":installation}))).await.map_err(|_| "SSH registration stopped; retry Pairing with the saved installation keys.")??;
    if activated["state"] != "active"
        || activated["installation_id"].as_str() != Some(&keys.installation_id)
    {
        return Err("The dedicated SSH key registration was not confirmed.".into());
    }
    let source_serial = serial.clone();
    let source_profile = profile.clone();
    let ssh_soc = soc.clone();
    let (keys, boot) = tauri::async_runtime::spawn_blocking(move || {
        let boot = verify_dedicated_ssh(&source_serial, &ssh_soc, &source_profile, &keys)?;
        Ok::<_, String>((keys, boot))
    })
    .await
    .map_err(|_| "Dedicated-key verification stopped; retry with the saved installation keys.")??;
    let installation_id = keys.installation_id.clone();
    let auth = session::GameplayAuth::paired(keys);
    auth.token(&client, &profile.tls_name, forward.port, &board_id)
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
        || hello["protocol"] != "1.0"
    {
        return Err(
            "Pinned HTTPS did not prove the same Board/kernel as USB and dedicated SSH.".into(),
        );
    }
    connection::remember_board(&state, &profile).map_err(|error| error.message)?;
    crate::setup::observed_step(
        &setup_store,
        7,
        crate::setup::SetupObservation {
            serial,
            soc,
            board_id: board_id.clone(),
            boot_id: boot,
            runtime_release: hello["runtime"]["release"]
                .as_str()
                .unwrap_or_default()
                .into(),
            runtime_nonce: hello["runtime"]["nonce"]
                .as_str()
                .unwrap_or_default()
                .into(),
            hardware: None,
        },
    )?;
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
