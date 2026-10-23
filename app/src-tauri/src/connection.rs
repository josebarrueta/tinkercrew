use reqwest::{Client, Method};
use security_framework::passwords::{get_generic_password, set_generic_password};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    time::Duration,
};
use tokio::sync::Mutex;

const KEYCHAIN_SERVICE: &str = "com.tinkercrew.desktop.boards";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionError {
    pub code: String,
    pub message: String,
}
impl ConnectionError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
type Result<T> = std::result::Result<T, ConnectionError>;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Candidate {
    pub host: String,
    pub port: u16,
}
impl Candidate {
    pub fn new(host: &str, port: u16) -> Result<Self> {
        let host = host.trim();
        let hostname = !host.is_empty()
            && host.len() <= 253
            && host.split('.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && label.as_bytes()[0].is_ascii_alphanumeric()
                    && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                    && label
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            });
        if port == 0 || !(hostname || host.parse::<IpAddr>().is_ok()) {
            return Err(ConnectionError::new(
                "invalid_address",
                "Enter a plain local IP or hostname and a valid port, not a URL.",
            ));
        }
        Ok(Self {
            host: host.into(),
            port,
        })
    }
}

pub fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoardProfile {
    pub board_id: String,
    pub name: String,
    pub adb_serial: String,
    pub tls_name: String,
    pub certificate_pem: String,
    pub api_token: String,
    pub last_address: Option<Candidate>,
}
impl BoardProfile {
    fn validate(&self) -> Result<()> {
        if self.board_id == "unprovisioned"
            || self.board_id.len() < 8
            || self.board_id.len() > 64
            || !self
                .board_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            || self.name.is_empty()
            || self.name.len() > 60
            || self.adb_serial.is_empty()
            || self.adb_serial.len() > 128
            || !self.adb_serial.as_bytes()[0].is_ascii_alphanumeric()
            || !self
                .adb_serial
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
            || self.api_token.len() < 32
            || self.api_token.len() > 512
            || self.api_token.contains(['\r', '\n'])
            || self.certificate_pem.len() > 8192
        {
            return Err(ConnectionError::new(
                "invalid_pairing",
                "The Board record is incomplete or invalid. Copy the provisioned record again over USB.",
            ));
        }
        Candidate::new(&self.tls_name, 7443)?;
        if self.tls_name.parse::<IpAddr>().is_ok() || self.tls_name.contains(':') {
            return Err(ConnectionError::new(
                "invalid_pairing",
                "The TLS identity must be the fixed DNS name on the USB-provisioned certificate, not an IP address.",
            ));
        }
        if let Some(candidate) = &self.last_address {
            Candidate::new(&candidate.host, candidate.port)?;
        }
        reqwest::Certificate::from_pem(self.certificate_pem.as_bytes()).map_err(|_| {
            ConnectionError::new(
                "invalid_pairing",
                "The provisioned TLS certificate is invalid.",
            )
        })?;
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardSummary {
    board_id: String,
    name: String,
}

struct ActiveBoard {
    handle: u64,
    client: Client,
    profile: BoardProfile,
    port: u16,
}

pub struct Connections {
    pub directory: PathBuf,
    active: Mutex<Option<ActiveBoard>>,
    pub forwards: Mutex<Vec<(String, u16)>>,
    sequence: std::sync::atomic::AtomicU64,
    profile_writes: std::sync::Mutex<()>,
}
impl Connections {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            active: Mutex::new(None),
            forwards: Mutex::new(vec![]),
            sequence: std::sync::atomic::AtomicU64::new(1),
            profile_writes: std::sync::Mutex::new(()),
        }
    }
    pub fn profile(&self, board_id: &str) -> Result<BoardProfile> {
        let bytes = get_generic_password(KEYCHAIN_SERVICE, board_id).map_err(|_| {
            ConnectionError::new(
                "pairing_required",
                "Unlock macOS Keychain or import the USB-verified Board record in Parent Settings.",
            )
        })?;
        let profile: BoardProfile = serde_json::from_slice(&bytes).map_err(|_| {
            ConnectionError::new(
                "invalid_pairing",
                "The saved Board record is unreadable. Recover through USB.",
            )
        })?;
        profile.validate()?;
        if profile.board_id != board_id {
            return Err(ConnectionError::new(
                "identity_mismatch",
                "The saved Board identity does not match. Recover through USB.",
            ));
        }
        Ok(profile)
    }
    fn save_profile(&self, profile: &BoardProfile) -> Result<()> {
        let bytes = serde_json::to_vec(profile).map_err(|_| {
            ConnectionError::new("invalid_pairing", "Could not encode the Board record.")
        })?;
        set_generic_password(KEYCHAIN_SERVICE, &profile.board_id, &bytes).map_err(|_| {
            ConnectionError::new(
                "keychain_unavailable",
                "Unlock macOS Keychain and retry. Credentials have not been written to disk.",
            )
        })
    }
    fn ids(&self) -> Result<Vec<String>> {
        match std::fs::read(self.directory.join("board-handles.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| {
                ConnectionError::new(
                    "storage_error",
                    "The Board index is unreadable. Existing Keychain records are unchanged.",
                )
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
            Err(_) => Err(ConnectionError::new(
                "storage_error",
                "Could not read the local Board index.",
            )),
        }
    }
}

#[tauri::command]
pub fn register_board(
    state: tauri::State<'_, Connections>,
    record: String,
    usb_confirmed: bool,
) -> Result<()> {
    if !usb_confirmed || record.len() > 16384 {
        return Err(ConnectionError::new(
            "usb_confirmation_required",
            "Verify this record directly over physical USB before trusting it.",
        ));
    }
    let profile: BoardProfile = serde_json::from_str(&record).map_err(|_| {
        ConnectionError::new(
            "invalid_pairing",
            "The Board record must be the provisioned JSON copied over USB.",
        )
    })?;
    profile.validate()?;
    let _guard = state.profile_writes.lock().map_err(|_| {
        ConnectionError::new(
            "storage_error",
            "Restart the app and retry importing the Board record.",
        )
    })?;
    let mut ids = state.ids()?;
    if !ids.contains(&profile.board_id) {
        ids.push(profile.board_id.clone());
    }
    if ids.len() > 16 {
        return Err(ConnectionError::new(
            "board_limit",
            "This development build supports up to 16 paired Boards.",
        ));
    }
    state.save_profile(&profile)?;
    let bytes = serde_json::to_vec(&ids)
        .map_err(|_| ConnectionError::new("storage_error", "Could not save the Board index."))?;
    let temp = state.directory.join("board-handles.json.tmp");
    std::fs::write(&temp, bytes)
        .and_then(|_| std::fs::rename(&temp, state.directory.join("board-handles.json")))
        .map_err(|_| {
            ConnectionError::new(
                "storage_error",
                "Could not save the Board index. Re-import the same record to retry.",
            )
        })?;
    Ok(())
}

#[tauri::command]
pub fn list_boards(state: tauri::State<'_, Connections>) -> Result<Vec<BoardSummary>> {
    state
        .ids()?
        .iter()
        .map(|id| {
            let profile = state.profile(id)?;
            Ok(BoardSummary {
                board_id: profile.board_id,
                name: profile.name,
            })
        })
        .collect()
}

async fn local_addresses(candidate: &Candidate) -> Result<Vec<SocketAddr>> {
    let validated = Candidate::new(&candidate.host, candidate.port)?;
    let addresses = tokio::time::timeout(
        Duration::from_millis(800),
        tokio::net::lookup_host((validated.host.as_str(), validated.port)),
    )
    .await
    .map_err(|_| {
        ConnectionError::new(
            "network_unavailable",
            "Address lookup timed out. Check Wi-Fi, VPN restrictions, or use USB.",
        )
    })?
    .map_err(|_| {
        ConnectionError::new(
            "network_unavailable",
            "This local address could not be found. Try USB or a manual IP address.",
        )
    })?;
    let addresses: Vec<_> = addresses
        .filter(|address| is_local(address.ip()))
        .take(8)
        .collect();
    if addresses.is_empty() {
        return Err(ConnectionError::new(
            "network_unavailable",
            "Only local-network addresses are permitted. Check the IP address or VPN routing; USB remains available.",
        ));
    }
    Ok(addresses)
}

async fn request(
    client: &Client,
    profile: &BoardProfile,
    port: u16,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value> {
    let mut authorization =
        reqwest::header::HeaderValue::from_str(&format!("Bearer {}", profile.api_token)).map_err(
            |_| {
                ConnectionError::new(
                    "invalid_pairing",
                    "The saved credential is invalid. Recover through USB.",
                )
            },
        )?;
    authorization.set_sensitive(true);
    let mut builder = client
        .request(method, format!("https://{}:{port}{path}", profile.tls_name))
        .header(reqwest::header::AUTHORIZATION, authorization);
    if let Some(body) = body {
        builder = builder.json(&body);
    }
    let mut response = builder.send().await.map_err(|_| ConnectionError::new("companion_unavailable", "The Companion could not be reached securely. Check its certificate/clock, power, Wi-Fi and VPN routing, or try USB."))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(ConnectionError::new(
            "authentication_failed",
            "The Board rejected this credential. Recover Pairing through USB; do not disable TLS verification.",
        ));
    }
    if !response.status().is_success() {
        return Err(ConnectionError::new(
            "command_failed",
            "The Board could not complete the request. Check component health and retry.",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        ConnectionError::new(
            "companion_unavailable",
            "The Board response was interrupted. Reconnect and retry.",
        )
    })? {
        if bytes.len() + chunk.len() > 65536 {
            return Err(ConnectionError::new(
                "invalid_response",
                "The Companion response exceeded its allowed size.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| {
        ConnectionError::new(
            "invalid_response",
            "The Companion returned an incompatible response. Check the installed version.",
        )
    })
}

async fn verify_candidate(
    profile: &BoardProfile,
    candidate: &Candidate,
) -> Result<(Client, Vec<SocketAddr>)> {
    let addresses = local_addresses(candidate).await?;
    let certificate =
        reqwest::Certificate::from_pem(profile.certificate_pem.as_bytes()).map_err(|_| {
            ConnectionError::new("invalid_pairing", "Invalid provisioned TLS certificate.")
        })?;
    let client = Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .tls_certs_only([certificate])
        .tls_version_min(reqwest::tls::Version::TLS_1_2)
        .resolve_to_addrs(&profile.tls_name, &addresses)
        .connect_timeout(Duration::from_millis(700))
        .timeout(Duration::from_millis(1200))
        .build()
        .map_err(|_| {
            ConnectionError::new(
                "tls_unavailable",
                "Secure local transport could not start. Restart the app and retry.",
            )
        })?;
    let hello = request(
        &client,
        profile,
        candidate.port,
        Method::GET,
        "/v1/hello",
        None,
    )
    .await?;
    if hello["board"]["board_id"].as_str() != Some(&profile.board_id) {
        return Err(ConnectionError::new(
            "identity_mismatch",
            "This is not the selected Board. Check its USB identity; no gameplay commands were sent.",
        ));
    }
    if hello["protocol"].as_str() != Some("1.0") {
        return Err(ConnectionError::new(
            "protocol_incompatible",
            "Update the Companion through parent-approved USB setup before playing.",
        ));
    }
    Ok((client, addresses))
}

#[tauri::command]
pub async fn probe_board(
    state: tauri::State<'_, Connections>,
    board_id: String,
    candidate: Candidate,
) -> Result<u64> {
    let mut profile = state.profile(&board_id)?;
    let (client, addresses) = verify_candidate(&profile, &candidate).await?;
    let mut active = state.active.lock().await;
    if let Some(old) = active.take() {
        let _ = request(
            &old.client,
            &old.profile,
            old.port,
            Method::POST,
            "/v1/commands",
            Some(json!({"operation":"outputs.clear", "arguments":{}})),
        )
        .await;
    }
    // USB loopback addresses are ephemeral; never let them displace the last validated LAN address.
    if !addresses.iter().all(|address| address.ip().is_loopback()) {
        profile.last_address = Some(candidate.clone());
        state.save_profile(&profile)?;
    }
    let handle = state
        .sequence
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    *active = Some(ActiveBoard {
        handle,
        client,
        profile,
        port: candidate.port,
    });
    Ok(handle)
}

#[tauri::command]
pub async fn disconnect_board(state: tauri::State<'_, Connections>) -> Result<()> {
    let mut active = state.active.lock().await;
    if let Some(old) = active.take() {
        let _ = request(
            &old.client,
            &old.profile,
            old.port,
            Method::POST,
            "/v1/commands",
            Some(json!({"operation":"outputs.clear", "arguments":{}})),
        )
        .await;
    }
    for (serial, port) in state.forwards.lock().await.drain(..) {
        crate::discovery::remove_forward(&serial, port).await;
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum GameplayRequest {
    State,
    Hello,
    Events { after: u64 },
    RenewControl,
    Clear,
    Pixels { color: String, brightness: f64 },
    Buzzer { frequency_hz: u32, duration_ms: u32 },
}

#[tauri::command]
pub async fn board_request(
    state: tauri::State<'_, Connections>,
    handle: u64,
    operation: GameplayRequest,
) -> Result<Value> {
    let active = state.active.lock().await;
    let selected = active
        .as_ref()
        .filter(|board| board.handle == handle)
        .ok_or_else(|| {
            ConnectionError::new(
                "disconnected",
                "The selected Board connection changed. Reconnect before continuing.",
            )
        })?;
    let (method, path, body) = match operation {
        GameplayRequest::State => (Method::GET, "/v1/state".into(), None),
        GameplayRequest::Hello => (Method::GET, "/v1/hello".into(), None),
        GameplayRequest::Events { after } => {
            (Method::GET, format!("/v1/events?after={after}"), None)
        }
        GameplayRequest::RenewControl => (
            Method::POST,
            "/v1/control-lease".into(),
            Some(json!({"lease_ms":1500})),
        ),
        GameplayRequest::Clear => (
            Method::POST,
            "/v1/commands".into(),
            Some(json!({"operation":"outputs.clear","arguments":{}})),
        ),
        GameplayRequest::Pixels { color, brightness } => {
            if color.len() != 7
                || !color.starts_with('#')
                || !color[1..].bytes().all(|c| c.is_ascii_hexdigit())
                || !(0.0..=0.5).contains(&brightness)
            {
                return Err(ConnectionError::new(
                    "command_out_of_range",
                    "Choose a hex color and brightness from 0 to 0.5.",
                ));
            }
            (
                Method::POST,
                "/v1/commands".into(),
                Some(
                    json!({"operation":"pixels.set","arguments":{"color":color,"brightness":brightness,"duration_ms":1500}}),
                ),
            )
        }
        GameplayRequest::Buzzer {
            frequency_hz,
            duration_ms,
        } => {
            if !(100..=4000).contains(&frequency_hz) || !(1..=3000).contains(&duration_ms) {
                return Err(ConnectionError::new(
                    "command_out_of_range",
                    "Use a bounded tone from 100 to 4000 Hz and at most 3000 ms.",
                ));
            }
            (
                Method::POST,
                "/v1/commands".into(),
                Some(
                    json!({"operation":"buzzer.play","arguments":{"frequency_hz":frequency_hz,"duration_ms":duration_ms,"volume":0.25}}),
                ),
            )
        }
    };
    request(
        &selected.client,
        &selected.profile,
        selected.port,
        method,
        &path,
        body,
    )
    .await
}

pub fn adb_status<'a>(output: &'a str, selected: &str) -> Option<&'a str> {
    output.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        if parts.next()? == selected {
            parts.next()
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_tls_companion_checks_certificate_identity_authentication_and_commands() {
        use std::io::{BufRead, BufReader};
        struct Fixture(std::process::Child);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let certificate =
            rcgen::generate_simple_self_signed(vec!["guardian.local".into()]).unwrap();
        let pem = certificate.cert.pem();
        let cert_path = directory.path().join("test-cert.pem");
        let key_path = directory.path().join("test-key.pem");
        std::fs::write(&cert_path, &pem).unwrap();
        std::fs::write(&key_path, certificate.signing_key.serialize_pem()).unwrap();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let child = std::process::Command::new("python3")
            .env("PYTHONPATH", root.join("companion/python"))
            .arg(root.join("companion/tests/tls_fixture.py"))
            .arg(&cert_path)
            .arg(&key_path)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut fixture = Fixture(child);
        let mut line = String::new();
        BufReader::new(fixture.0.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let port: u16 = line.trim().parse().unwrap();
        let profile = BoardProfile {
            board_id: "board-test-1234".into(),
            name: "Test Board".into(),
            adb_serial: "test-serial".into(),
            tls_name: "guardian.local".into(),
            certificate_pem: pem,
            api_token: "test-only-credential-01234567890123456789".into(),
            last_address: None,
        };
        let candidate = Candidate::new("127.0.0.1", port).unwrap();
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            let (client, _) = verify_candidate(&profile, &candidate).await.unwrap();
            request(&client, &profile, port, Method::POST, "/v1/control-lease", Some(json!({"lease_ms":1500}))).await.unwrap();
            request(&client, &profile, port, Method::POST, "/v1/commands", Some(json!({"operation":"pixels.set", "arguments":{"color":"#22c55e","brightness":0.3,"duration_ms":1000}}))).await.unwrap();
            let state = request(&client, &profile, port, Method::GET, "/v1/state", None).await.unwrap();
            assert_eq!(state["outputs"]["pixels"]["color"], "#22c55e");
            let mut wrong = profile.clone();
            wrong.board_id = "other-board".into();
            assert_eq!(verify_candidate(&wrong, &candidate).await.err().unwrap().code, "identity_mismatch");
            wrong = profile.clone();
            wrong.api_token = "wrong-credential-01234567890123456789".into();
            assert_eq!(verify_candidate(&wrong, &candidate).await.err().unwrap().code, "authentication_failed");
            wrong = profile.clone();
            wrong.certificate_pem = rcgen::generate_simple_self_signed(vec!["guardian.local".into()]).unwrap().cert.pem();
            assert_eq!(verify_candidate(&wrong, &candidate).await.err().unwrap().code, "companion_unavailable");
        });
    }

    #[test]
    fn endpoint_input_cannot_become_a_url_command_or_remote_scan() {
        assert!(Candidate::new("192.168.1.8", 7443).is_ok());
        assert!(Candidate::new("dragon.local", 7443).is_ok());
        for host in [
            "https://example.com",
            "x/path",
            "x@localhost",
            "x;whoami",
            "-s",
            "",
        ] {
            assert!(Candidate::new(host, 7443).is_err(), "{host}");
        }
        assert!(is_local("192.168.1.8".parse().unwrap()));
        assert!(!is_local("8.8.8.8".parse().unwrap()));
        assert!(!is_local("0.0.0.0".parse().unwrap()));
    }

    #[test]
    fn adb_selection_requires_the_exact_paired_serial() {
        let output = "List of devices attached\nother device product:foo\nuno-123 offline\n";
        assert_eq!(adb_status(output, "uno-123"), Some("offline"));
        assert_eq!(adb_status(output, "uno"), None);
        assert_eq!(adb_status(output, "other"), Some("device"));
    }
}
