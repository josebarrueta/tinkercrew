use crate::{admin_process, connection::Connections};
use base64::{Engine, engine::general_purpose::STANDARD};
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ssh2::{HashType, Session};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

const PIN_SERVICE: &str = "com.tinkercrew.desktop.management-ssh";
const HELPER: &str = include_str!("board_management.py");

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardIdentity {
    pub adb_serial: String,
    pub serial: String,
    pub username: String,
    pub hostname: String,
    pub os: String,
    pub os_version: String,
    pub image_version: String,
    pub boot_id: Option<String>,
}
#[derive(Deserialize)]
struct WireIdentity {
    serial: String,
    username: String,
    hostname: String,
    os: String,
    #[serde(rename = "osVersion")]
    os_version: String,
    #[serde(rename = "imageVersion")]
    image_version: String,
    #[serde(default, rename = "bootId")]
    boot_id: Option<String>,
}

pub(crate) struct AuthorizedBoard {
    identity: BoardIdentity,
    ssh: Session,
    port: u16,
    expires: Instant,
}
#[derive(Default)]
pub struct Management {
    pub(crate) session: Arc<Mutex<Option<AuthorizedBoard>>>,
}
struct PrivateJson(Value);
impl Drop for PrivateJson {
    fn drop(&mut self) {
        fn clear(value: &mut Value) {
            match value {
                Value::String(value) => value.zeroize(),
                Value::Object(object) => object.values_mut().for_each(clear),
                Value::Array(array) => array.iter_mut().for_each(clear),
                _ => {}
            }
        }
        clear(&mut self.0);
    }
}

pub(crate) fn valid_serial(serial: &str) -> bool {
    !serial.is_empty()
        && serial.len() <= 128
        && serial.as_bytes()[0].is_ascii_alphanumeric()
        && serial
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.:-".contains(&c))
}
pub(crate) fn physical_usb_ready(output: &str, serial: &str) -> bool {
    output.lines().any(|line| {
        let mut fields = line.split_whitespace();
        fields.next() == Some(serial)
            && fields.next() == Some("device")
            && fields.any(|field| field.starts_with("usb:") && field.len() > 4)
    })
}
fn valid_username(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        && !name.starts_with('-')
}
fn helper_command() -> String {
    format!(
        "python3 -c 'import base64;exec(base64.b64decode(\"{}\"))'",
        STANDARD.encode(HELPER)
    )
}
fn decode_response(bytes: &[u8]) -> Result<Value, String> {
    let response: Value = serde_json::from_slice(bytes)
        .map_err(|_| "The Board returned an incompatible management response.")?;
    if response["status"] != "ok" {
        return Err("Board operation failed or is unsupported. Check identity, credentials, permissions, and image support.".into());
    }
    Ok(response["result"].clone())
}
fn usb_request(serial: &str, body: Value) -> Result<Value, String> {
    if !valid_serial(serial) {
        return Err("Choose a valid, explicit USB serial.".into());
    }
    let path = crate::discovery::adb_path().map_err(|error| error.message)?;
    let devices = admin_process::run(
        &path,
        &["-H", "127.0.0.1", "-P", "5037", "devices", "-l"],
        &[],
        Duration::from_secs(4),
    )?;
    if !physical_usb_ready(
        std::str::from_utf8(&devices).map_err(|_| "Unreadable USB list.")?,
        serial,
    ) {
        return Err("The selected USB Board is not ready. Check boot, authorization, and the data cable; another device will never be selected instead.".into());
    }
    let body = PrivateJson(body);
    let bytes =
        Zeroizing::new(serde_json::to_vec(&body.0).map_err(|_| "Invalid management input.")?);
    let result = admin_process::run(
        &path,
        &[
            "-H",
            "127.0.0.1",
            "-P",
            "5037",
            "-s",
            serial,
            "exec-out",
            &helper_command(),
        ],
        &bytes,
        Duration::from_secs(40),
    )?;
    decode_response(&result)
}
fn identity(serial: &str, value: Value) -> Result<BoardIdentity, String> {
    let wire: WireIdentity =
        serde_json::from_value(value).map_err(|_| "The Board identity response is incomplete.")?;
    let soc = wire
        .serial
        .parse::<i64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or("The hardware serial is not supported by the official flasher.")?;
    if wire.os != "debian" || wire.hostname.len() > 64 || !valid_username(&wire.username) {
        return Err("Unsupported UNO Q identity or image. Use official Arduino recovery.".into());
    }
    Ok(BoardIdentity {
        adb_serial: serial.into(),
        serial: soc.to_string(),
        username: wire.username,
        hostname: wire.hostname,
        os: wire.os,
        os_version: wire.os_version,
        image_version: wire.image_version,
        boot_id: wire.boot_id.filter(|value| crate::session::valid_id(value)),
    })
}
pub(crate) fn inspect(serial: &str) -> Result<BoardIdentity, String> {
    identity(serial, usb_request(serial, json!({"operation":"inspect"}))?)
}
fn ssh_request(ssh: &Session, body: Value) -> Result<Value, String> {
    let body = PrivateJson(body);
    let bytes =
        Zeroizing::new(serde_json::to_vec(&body.0).map_err(|_| "Invalid management input.")?);
    if bytes.len() > 16384 {
        return Err("Management input exceeds its safe limit.".into());
    }
    let mut channel = ssh
        .channel_session()
        .map_err(|_| "SSH is unavailable; reconnect the exact Board.")?;
    channel.exec(&helper_command()).map_err(|_| "The UNO Q management helper could not start. Python 3 and the supported Debian image are required.")?;
    channel
        .write_all(&bytes)
        .map_err(|_| "Management request was interrupted.")?;
    channel
        .send_eof()
        .map_err(|_| "Management input was interrupted.")?;
    let mut output = Zeroizing::new(Vec::new());
    Read::by_ref(&mut channel).take(65537).read_to_end(&mut output).map_err(|_| "Management response timed out. Check the Board before retrying; success is not assumed.")?;
    if output.len() > 65536 {
        return Err("Management response exceeded its safe limit.".into());
    }
    channel
        .wait_close()
        .map_err(|_| "Management completion was interrupted.")?;
    if channel
        .exit_status()
        .map_err(|_| "No management exit status.")?
        != 0
    {
        return Err("The Board rejected the operation. Check credentials, permissions, image support, or Wi-Fi settings, then retry.".into());
    }
    decode_response(&output)
}
fn close_session(session: AuthorizedBoard) {
    let _ = session
        .ssh
        .disconnect(None, "Management session closed", None);
    if let Ok(path) = crate::discovery::adb_path() {
        let _ = admin_process::run(
            &path,
            &[
                "-H",
                "127.0.0.1",
                "-P",
                "5037",
                "-s",
                &session.identity.adb_serial,
                "forward",
                "--remove",
                &format!("tcp:{}", session.port),
            ],
            &[],
            Duration::from_secs(3),
        );
    }
}
fn remove_forward(serial: &str, port: u16) {
    if let Ok(path) = crate::discovery::adb_path() {
        let _ = admin_process::run(
            &path,
            &[
                "-H",
                "127.0.0.1",
                "-P",
                "5037",
                "-s",
                serial,
                "forward",
                "--remove",
                &format!("tcp:{port}"),
            ],
            &[],
            Duration::from_secs(3),
        );
    }
}

#[tauri::command]
pub async fn admin_inspect_usb(serial: String) -> Result<BoardIdentity, String> {
    tauri::async_runtime::spawn_blocking(move || inspect(&serial))
        .await
        .map_err(|_| "USB inspection stopped.")?
}

#[tauri::command]
pub async fn admin_enable_ssh(
    connections: tauri::State<'_, Connections>,
    serial: String,
    username: String,
    password: String,
    confirmation: String,
) -> Result<Value, String> {
    let password = Zeroizing::new(password);
    if confirmation != format!("ENABLE SSH {serial}") || !valid_username(&username) {
        return Err("Confirm enabling SSH for the explicitly selected USB Board.".into());
    }
    let _permit = connections.begin_maintenance()?;
    crate::connection::disconnect_board(connections)
        .await
        .map_err(|error| error.message)?;
    tauri::async_runtime::spawn_blocking(move || {
        let board = inspect(&serial)?;
        usb_request(&serial, json!({"operation":"enable_ssh","expectedSerial":board.serial,"username":username,"linuxPassword":password.as_str()}))
    }).await.map_err(|_| "SSH enablement stopped; check Board state before retrying.")?
}

#[tauri::command]
pub async fn admin_login(
    state: tauri::State<'_, Management>,
    connections: tauri::State<'_, Connections>,
    serial: String,
    username: String,
    password: String,
    usb_confirmed: bool,
) -> Result<BoardIdentity, String> {
    let password = Zeroizing::new(password);
    if !usb_confirmed || !valid_username(&username) || password.is_empty() || password.len() > 512 {
        return Err("Select the exact USB Board, verify it physically, and enter its Linux username/password.".into());
    }
    let _permit = connections.begin_maintenance()?;
    crate::connection::disconnect_board(connections)
        .await
        .map_err(|error| error.message)?;
    let sessions = state.session.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut stored = sessions.lock().map_err(|_| "Management is busy; restart the app.")?;
        if let Some(old) = stored.take() { close_session(old); }
        let board = inspect(&serial)?;
        let path = crate::discovery::adb_path().map_err(|error| error.message)?;
        let output = admin_process::run(&path, &["-H", "127.0.0.1", "-P", "5037", "-s", &serial, "forward", "tcp:0", "tcp:22"], &[], Duration::from_secs(4))?;
        let port = std::str::from_utf8(&output).ok().and_then(|value| value.trim().parse::<u16>().ok()).filter(|port| *port >= 1024).ok_or("ADB could not allocate a safe SSH forward.")?;
        let result = (|| {
            let tcp = TcpStream::connect_timeout(&SocketAddr::from(([127,0,0,1], port)), Duration::from_secs(3)).map_err(|_| "SSH is not available. Enable SSH explicitly over USB or use official Arduino setup.")?;
            let mut ssh = Session::new().map_err(|_| "SSH could not initialize.")?;
            ssh.set_tcp_stream(tcp);
            ssh.set_timeout(35000);
            ssh.handshake().map_err(|_| "SSH handshake failed. Check that SSH is enabled on this Board.")?;
            let pin = ssh.host_key_hash(HashType::Sha256).ok_or("The Board's SSH identity is unavailable.")?.to_vec();
            match get_generic_password(PIN_SERVICE, &serial) {
                Ok(existing) if existing != pin => return Err("The SSH host key changed. Stop and verify the Board over USB; reset only this app's SSH trust explicitly after an intentional factory restore.".into()),
                Ok(_) => {},
                Err(error) if error.code() == -25300 => {},
                Err(_) => return Err("Unlock Keychain before verifying the Board's SSH identity.".into()),
            }
            ssh.userauth_password(&username, &password).map_err(|_| "Linux authentication failed. Check the username/password and SSH password-login policy; no password has been stored.")?;
            if !ssh.authenticated() { return Err("Linux authentication was not completed.".into()); }
            let authenticated = identity(&serial, ssh_request(&ssh, json!({"operation":"inspect"}))?)?;
            if authenticated.serial != board.serial || authenticated.username != username { return Err("The authenticated SSH identity does not match the physically inspected Board/account.".into()); }
            set_generic_password(PIN_SERVICE, &serial, &pin).map_err(|_| "Could not save the USB-verified SSH fingerprint to Keychain.")?;
            *stored = Some(AuthorizedBoard { identity: authenticated.clone(), ssh, port, expires: Instant::now() + Duration::from_secs(300) });
            Ok(authenticated)
        })();
        if result.is_err() { remove_forward(&serial, port); }
        result
    }).await.map_err(|_| "Authentication stopped. Reconnect the selected USB Board.")?
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdminAction {
    Status {},
    Hostname {
        hostname: String,
        password: String,
    },
    WifiScan {},
    WifiJoin {
        ssid: String,
        password: String,
    },
    Reboot {
        password: String,
        confirmation: String,
    },
}
impl Drop for AdminAction {
    fn drop(&mut self) {
        match self {
            Self::Hostname { password, .. }
            | Self::WifiJoin { password, .. }
            | Self::Reboot { password, .. } => password.zeroize(),
            _ => {}
        }
    }
}
pub(crate) fn assert_authorized(state: &Management, serial: &str) -> Result<(), String> {
    let current = state
        .session
        .lock()
        .map_err(|_| "Management authorization is busy.")?;
    if current.as_ref().is_some_and(|session| {
        session.identity.adb_serial == serial && session.expires > Instant::now()
    }) {
        Ok(())
    } else {
        Err("Authenticate this exact Board's Linux account before scheduling its restart.".into())
    }
}

#[tauri::command]
pub async fn admin_action(
    state: tauri::State<'_, Management>,
    connections: tauri::State<'_, Connections>,
    serial: String,
    action: AdminAction,
) -> Result<Value, String> {
    let _permit = connections.begin_maintenance()?;
    crate::connection::disconnect_board(connections)
        .await
        .map_err(|error| error.message)?;
    let sessions = state.session.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut stored = sessions.lock().map_err(|_| "Management is busy.")?;
        if stored.as_ref().is_some_and(|session| session.expires <= Instant::now()) && let Some(old) = stored.take() { close_session(old); }
        let session = stored.as_ref().filter(|session| session.identity.adb_serial == serial).ok_or("Authenticate this exact USB Board again; management sessions expire after five minutes.")?;
        let fresh = identity(&serial, ssh_request(&session.ssh, json!({"operation":"inspect"}))?)?;
        if fresh.serial != session.identity.serial { return Err("Board identity changed; reconnect over USB.".into()); }
        let mut reboot = false;
        let body = match &action {
            AdminAction::Status {} => return Ok(serde_json::to_value(fresh).map_err(|_| "Could not read Board status.")?),
            AdminAction::Hostname { hostname, password } => json!({"operation":"hostname","expectedSerial":fresh.serial,"hostname":hostname,"linuxPassword":password}),
            AdminAction::WifiScan {} => json!({"operation":"wifi_scan","expectedSerial":fresh.serial}),
            AdminAction::WifiJoin { ssid, password } => json!({"operation":"wifi_join","expectedSerial":fresh.serial,"ssid":ssid,"wifiPassword":password}),
            AdminAction::Reboot { password, confirmation } => {
                if confirmation != &format!("RESTART {}", fresh.serial) { return Err("Type the exact restart confirmation for this hardware serial.".into()); }
                reboot = true;
                json!({"operation":"reboot","expectedSerial":fresh.serial,"linuxPassword":password})
            }
        };
        let result = ssh_request(&session.ssh, body)?;
        if reboot && let Some(old) = stored.take() { close_session(old); }
        Ok(result)
    }).await.map_err(|_| "Management stopped. Check Board state before retrying.")?
}

#[tauri::command]
pub async fn admin_logout(state: tauri::State<'_, Management>) -> Result<(), String> {
    let sessions = state.session.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(session) = sessions.lock().map_err(|_| "Management is busy.")?.take() {
            close_session(session);
        }
        Ok(())
    })
    .await
    .map_err(|_| "Management logout stopped.")?
}

#[tauri::command]
pub async fn admin_forget_ssh_trust(
    connections: tauri::State<'_, Connections>,
    state: tauri::State<'_, Management>,
    serial: String,
    confirmation: String,
) -> Result<(), String> {
    if confirmation != format!("FORGET SSH {serial}") {
        return Err("Confirm the exact USB serial before clearing this app's SSH trust.".into());
    }
    let _permit = connections.begin_maintenance()?;
    crate::connection::disconnect_board(connections)
        .await
        .map_err(|error| error.message)?;
    admin_logout(state).await?;
    tauri::async_runtime::spawn_blocking(move || {
        inspect(&serial)?;
        for service in [PIN_SERVICE, crate::pairing::SSH_PIN_SERVICE] {
            match delete_generic_password(service, &serial) {
                Ok(()) => {}
                Err(error) if error.code() == -25300 => {}
                Err(_) => {
                    return Err("Unlock Keychain and retry clearing this app's SSH trust.".into());
                }
            }
        }
        Ok(())
    })
    .await
    .map_err(|_| "SSH trust reset stopped.")?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn management_has_no_shell_reset_or_unknown_argument_endpoint() {
        for serial in ["", "-s", "serial;reboot", "serial\nother", "serial/other"] {
            assert!(!valid_serial(serial));
        }
        for name in ["", "-root", "user;reboot", "user name", "user\nroot"] {
            assert!(!valid_username(name));
        }
        assert!(valid_serial("USB_123:1"));
        assert!(valid_username("arduino"));
        assert!(physical_usb_ready(
            "USB42 device usb:123 model:UNO_Q transport_id:1",
            "USB42"
        ));
        assert!(!physical_usb_ready(
            "USB42 device model:UNO_Q transport_id:1",
            "USB42"
        ));
        assert!(!physical_usb_ready(
            "192.168.1.9:5555 device product:unoq transport_id:2",
            "192.168.1.9:5555"
        ));
        assert!(!physical_usb_ready("OTHER device usb:123", "USB42"));
        for operation in ["shell", "exec", "factory_reset", "delete_files"] {
            assert!(
                serde_json::from_value::<AdminAction>(
                    json!({"operation":operation,"command":"anything"})
                )
                .is_err()
            );
        }
        assert!(
            serde_json::from_value::<AdminAction>(
                json!({"operation":"status","command":"anything"})
            )
            .is_err()
        );
        assert!(
            decode_response(br#"{"status":"error","message":"fixture-only-secret"}"#)
                .unwrap_err()
                .find("fixture-only-secret")
                .is_none()
        );
    }
}
