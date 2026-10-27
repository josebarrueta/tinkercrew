//! USB-only temporary authority. No Keychain, saved installation keys or SSH registration.
use crate::{connection::ConnectionError, management, pairing::Forward};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};
type Result<T> = std::result::Result<T, ConnectionError>;

struct PrivateJson(Value);
impl Drop for PrivateJson {
    fn drop(&mut self) {
        fn clear(value: &mut Value) {
            match value {
                Value::String(text) => text.zeroize(),
                Value::Object(fields) => fields.values_mut().for_each(clear),
                Value::Array(items) => items.iter_mut().for_each(clear),
                _ => {}
            }
        }
        clear(&mut self.0);
    }
}
fn error(code: &str) -> ConnectionError {
    let message = match code {
        "usb_play_setup_required" => {
            "This Board needs one-time parent setup before USB play. Existing saved Pairing was not changed."
        }
        "usb_play_upgrade_required" => {
            "This Board needs the compatible Companion software update once in Parent setup. USB play does not reset the Board or upload firmware."
        }
        "authentication_busy" => {
            "The Board is limiting connection attempts. Wait one minute and choose USB Connect again; no saved Pairing was changed."
        }
        "companion_unavailable" => {
            "The USB Board's Companion did not answer securely. Check the cable and wait for startup. If it stays unavailable, use Parent setup to check the running Companion; do not reset or re-pair."
        }
        "identity_mismatch" => {
            "The selected USB Board identity changed. Unplug it and explicitly select the intended Board again."
        }
        "session_limit" => {
            "Two USB play sessions are still active. Close the other Tinkeroo window, or wait one minute before connecting again."
        }
        "usb_session_expired" => {
            "USB play ended. Reconnect the Board and choose Connect again. Your Creation is kept; outputs will not restart automatically."
        }
        _ => {
            "USB play could not start securely. Check the selected Board's data cable and running Companion; use Parent setup if software preparation is needed. No passwords or saved Pairing were read."
        }
    };
    ConnectionError::new(code, message)
}
fn bootstrap(serial: &str, mut body: Value) -> Result<Value> {
    if !management::valid_serial(serial) {
        return Err(error("identity_mismatch"));
    }
    // Fresh physical usb: evidence, UNO Q model/Debian identity and SoC every time.
    let identity = management::inspect(serial).map_err(|_| error("usb_unavailable"))?;
    if let Some(expected) = body.get("expected_serial")
        && expected.as_str() != Some(&identity.serial)
    {
        return Err(error("identity_mismatch"));
    }
    body["expected_serial"] = identity.serial.clone().into();
    let body = PrivateJson(body);
    let input =
        Zeroizing::new(serde_json::to_vec(&body.0).map_err(|_| error("usb_play_unavailable"))?);
    let path = crate::discovery::adb_path()?;
    let command = format!(
        "python3 -B -c 'import base64;exec(base64.b64decode(\"{}\"))'",
        STANDARD.encode(crate::runtime_update::usb_play_bootstrap())
    );
    let output = Zeroizing::new(
        crate::adb_transport::request(&path, serial, &command, &input, Duration::from_secs(10))
            .map_err(|_| error("usb_play_unavailable"))?,
    );
    let mut response =
        PrivateJson(serde_json::from_slice(&output).map_err(|_| error("usb_play_unavailable"))?);
    if response.0["status"] != "ok" {
        let code = response.0["error"]["code"]
            .as_str()
            .unwrap_or("usb_play_unavailable");
        return Err(error(match code {
            "usb_play_setup_required"
            | "usb_play_upgrade_required"
            | "identity_mismatch"
            | "usb_session_expired"
            | "session_limit" => code,
            _ => "usb_play_unavailable",
        }));
    }
    let result = &response.0["result"];
    if result["usb_play_version"] != 1
        || result["serial"].as_str() != Some(&identity.serial)
        || result["boot_id"].as_str() != identity.boot_id.as_deref()
    {
        return Err(error("identity_mismatch"));
    }
    Ok(response.0["result"].take())
}
pub(crate) fn begin(serial: &str) -> Result<Value> {
    bootstrap(serial, json!({"operation":"begin"}))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbBoard {
    pub serial: String,
}
#[tauri::command]
pub async fn usb_play_boards() -> Result<Vec<UsbBoard>> {
    let output = crate::discovery::adb(&["devices", "-l"]).await?;
    let mut boards = Vec::new();
    for line in output.lines() {
        if let Some(serial) = line.split_whitespace().next()
            && management::valid_serial(serial)
            && management::physical_usb_ready(&output, serial)
            && !boards.iter().any(|board: &UsbBoard| board.serial == serial)
        {
            boards.push(UsbBoard {
                serial: serial.into(),
            });
            if boards.len() >= 8 {
                break;
            }
        }
    }
    Ok(boards)
}

#[derive(Clone)]
pub(crate) struct Binding {
    pub serial: String,
    pub soc: String,
    pub board_id: String,
    pub boot_id: String,
    pub certificate_sha256: String,
}
struct Session {
    token: Zeroizing<String>,
    proof: Instant,
    last_use: Instant,
    live: bool,
}
pub(crate) struct UsbAuth {
    session: Mutex<Session>,
    binding: Binding,
    // This exact owned forward is removed on drop; unrelated forwards stay intact.
    _forward: Option<Forward>,
}
impl UsbAuth {
    pub async fn open(
        client: &reqwest::Client,
        host: &str,
        port: u16,
        mut grant: Value,
        binding: Binding,
        forward: Option<Forward>,
    ) -> Result<Arc<Self>> {
        let body = PrivateJson(
            json!({"board_id":grant["board_id"],"grant_id":grant["grant_id"],"grant_token":grant["grant_token"].take()}),
        );
        if let Value::String(text) = &mut grant["grant_token"] {
            text.zeroize();
        }
        let result = crate::session::post(client, host, port, "/v1/usb-sessions", &body.0).await;
        drop(body);
        let mut result = PrivateJson(result.map_err(|cause| match cause.code.as_str() {
            "authentication_failed" => error("usb_session_expired"),
            "authentication_busy" | "companion_unavailable" => error(&cause.code),
            _ => error("usb_play_unavailable"),
        })?);
        if result.0["usb_play_version"] != 1
            || result.0["board_id"].as_str() != Some(&binding.board_id)
            || result.0["boot_id"].as_str() != Some(&binding.boot_id)
            || result.0["expires_in_seconds"] != 150
            || result.0["idle_timeout_seconds"] != 60
        {
            return Err(error("usb_play_unavailable"));
        }
        let token = match result.0["session_token"].take() {
            Value::String(token) => Zeroizing::new(token),
            _ => return Err(error("usb_play_unavailable")),
        };
        if !token.starts_with("usb1.")
            || token.len() != 48
            || !token[5..]
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        {
            return Err(error("usb_play_unavailable"));
        }
        Ok(Arc::new(Self {
            session: Mutex::new(Session {
                token,
                proof: Instant::now(),
                last_use: Instant::now(),
                live: true,
            }),
            binding,
            _forward: forward,
        }))
    }
    pub fn token(&self) -> Result<String> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| error("usb_session_expired"))?;
        if !session.live
            || session.proof.elapsed() >= Duration::from_secs(90)
            || session.last_use.elapsed() >= Duration::from_secs(50)
        {
            session.live = false;
            return Err(error("usb_session_expired"));
        }
        session.last_use = Instant::now();
        Ok(session.token.to_string())
    }
    pub fn start_heartbeat(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(20)).await;
                let Some(auth) = weak.upgrade() else {
                    break;
                };
                let task = auth.clone();
                // Independent of gameplay I/O: a slow USB proof never queues stale
                // output/lease requests behind credential renewal.
                let result = tauri::async_runtime::spawn_blocking(move || task.refresh()).await;
                if !matches!(result, Ok(Ok(()))) {
                    if let Ok(mut state) = auth.session.lock() {
                        state.live = false;
                    }
                    break;
                }
            }
        });
    }
    fn refresh(&self) -> Result<()> {
        let token = {
            let mut session = self
                .session
                .lock()
                .map_err(|_| error("usb_session_expired"))?;
            if !session.live
                || session.last_use.elapsed() >= Duration::from_secs(50)
                || session.proof.elapsed() >= Duration::from_secs(90)
            {
                session.live = false;
                return Err(error("usb_session_expired"));
            }
            Zeroizing::new(session.token.to_string())
        };
        let result = bootstrap(
            &self.binding.serial,
            json!({"operation":"refresh","expected_serial":self.binding.soc,"board_id":self.binding.board_id,"boot_id":self.binding.boot_id,"session_token":token.as_str()}),
        )?;
        if result["board_id"].as_str() != Some(&self.binding.board_id)
            || result["boot_id"].as_str() != Some(&self.binding.boot_id)
            || result["certificate_sha256"].as_str() != Some(&self.binding.certificate_sha256)
        {
            return Err(error("identity_mismatch"));
        }
        let mut session = self
            .session
            .lock()
            .map_err(|_| error("usb_session_expired"))?;
        if !session.live {
            return Err(error("usb_session_expired"));
        }
        session.proof = Instant::now();
        Ok(())
    }
    pub fn close(&self) -> Option<Zeroizing<String>> {
        let mut session = self.session.lock().ok()?;
        session.live = false;
        Some(Zeroizing::new(std::mem::take(&mut *session.token)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    #[test]
    fn native_usb_authority_uses_real_tls_without_installation_keys_and_stays_closed() {
        let directory = tempfile::tempdir().unwrap();
        let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        std::fs::write(directory.path().join("cert.pem"), certificate.cert.pem()).unwrap();
        std::fs::write(
            directory.path().join("key.pem"),
            certificate.signing_key.serialize_pem(),
        )
        .unwrap();
        let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let mut child = std::process::Command::new("python3")
            .arg(repository.join("companion/tests/usb_play_fixture.py"))
            .arg(directory.path().join("cert.pem"))
            .arg(directory.path().join("key.pem"))
            .arg(directory.path())
            .env("PYTHONPATH", repository.join("companion/python"))
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut line = String::new();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        output.read_line(&mut line).unwrap();
        let started: Value = serde_json::from_str(&line).expect("isolated USB TLS fixture started");
        tauri::async_runtime::block_on(async {
            let board = started["grant"]["board_id"].as_str().unwrap();
            let port = started["port"].as_u64().unwrap() as u16;
            let profile = crate::connection::BoardProfile {
                board_id: board.into(),
                name: "Fixture".into(),
                adb_serial: "fixture".into(),
                tls_name: "localhost".into(),
                certificate_pem: certificate.cert.pem(),
                api_token: String::new(),
                installation_id: None,
                last_address: None,
            };
            let (client, _) = crate::connection::secure_client(
                &profile,
                &crate::connection::Candidate::new("127.0.0.1", port).unwrap(),
            )
            .await
            .unwrap();
            let usb = UsbAuth::open(
                &client,
                "localhost",
                port,
                started["grant"].clone(),
                Binding {
                    serial: "fixture".into(),
                    soc: "42".into(),
                    board_id: board.into(),
                    boot_id: started["boot_id"].as_str().unwrap().into(),
                    certificate_sha256: String::new(),
                },
                None,
            )
            .await
            .unwrap();
            let auth = crate::session::GameplayAuth::usb_play(usb);
            let token =
                Zeroizing::new(auth.token(&client, "localhost", port, board).await.unwrap());
            let hello: Value = client
                .get(format!("https://localhost:{port}/v1/hello"))
                .bearer_auth(token.as_str())
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            assert_eq!(hello["board"]["board_id"], board);
            assert_eq!(hello["health"]["safe_state"], true);
            auth.close(&client, "localhost", port).await;
            assert_eq!(
                auth.token(&client, "localhost", port, board)
                    .await
                    .err()
                    .unwrap()
                    .code,
                "usb_session_expired"
            );
            assert_eq!(
                client
                    .get(format!("https://localhost:{port}/v1/state"))
                    .bearer_auth(token.as_str())
                    .send()
                    .await
                    .unwrap()
                    .status(),
                401
            );
        });
        child.kill().ok();
        child.wait().ok();
    }
}
