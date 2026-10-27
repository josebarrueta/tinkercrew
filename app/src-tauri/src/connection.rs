use reqwest::{Client, Method};
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    net::{IpAddr, SocketAddr},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
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
    #[serde(default)]
    pub api_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_id: Option<String>,
    pub last_address: Option<Candidate>,
}
impl BoardProfile {
    pub(crate) fn validate(&self) -> Result<()> {
        self.validate_identity(false)
    }
    fn validate_identity(&self, usb_play: bool) -> Result<()> {
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
            || if usb_play {
                !crate::session::valid_id(&self.board_id)
                    || !self.api_token.is_empty()
                    || self.installation_id.is_some()
            } else {
                match &self.installation_id {
                    Some(id) => {
                        !crate::session::valid_id(id)
                            || !crate::session::valid_id(&self.board_id)
                            || !self.api_token.is_empty()
                    }
                    None => {
                        self.api_token.len() < 32
                            || self.api_token.len() > 512
                            || self.api_token.contains(['\r', '\n'])
                    }
                }
            }
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
    auth: crate::session::GameplayAuth,
    port: u16,
}

pub struct Connections {
    pub directory: PathBuf,
    pub(crate) maintenance: Arc<AtomicBool>,
    active: Mutex<Option<ActiveBoard>>,
    pub forwards: Mutex<Vec<(String, u16)>>,
    sequence: std::sync::atomic::AtomicU64,
    profile_writes: std::sync::Mutex<()>,
}
impl Connections {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            maintenance: Arc::new(AtomicBool::new(false)),
            active: Mutex::new(None),
            forwards: Mutex::new(vec![]),
            sequence: std::sync::atomic::AtomicU64::new(1),
            profile_writes: std::sync::Mutex::new(()),
        }
    }
    pub(crate) fn maintenance_active(&self) -> bool {
        self.maintenance.load(Ordering::SeqCst)
            || self
                .directory
                .join("board-recovery/factory-restore-active.json")
                .exists()
            || self.directory.join("runtime-update-active.json").exists()
            || self.other_management_active()
    }
    fn other_management_active(&self) -> bool {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.directory.join("board-management.lock"))
        {
            Ok(file) => crate::file_lock::ExclusiveFile::new(file).is_err(),
            Err(error) => error.kind() != std::io::ErrorKind::NotFound,
        }
    }
    pub(crate) fn begin_maintenance(&self) -> std::result::Result<MaintenancePermit, String> {
        self.maintenance_permit(false)
    }
    pub(crate) fn begin_runtime_update(&self) -> std::result::Result<MaintenancePermit, String> {
        self.maintenance_permit(true)
    }
    fn maintenance_permit(
        &self,
        resume_runtime: bool,
    ) -> std::result::Result<MaintenancePermit, String> {
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(self.directory.join("board-management.lock"))
            .map_err(|_| "Cannot establish exclusive Board-management ownership.")?;
        let lock = crate::file_lock::ExclusiveFile::new(lock).map_err(|_| "Another app instance owns Board management. Wait before starting another operation.")?;
        if !resume_runtime && self.directory.join("runtime-update-active.json").exists() {
            return Err("A runtime update is pending verification. Resume/resolve that update before other management operations.".into());
        }
        if self
            .directory
            .join("board-recovery/factory-restore-active.json")
            .exists()
        {
            return Err("A factory restore is active or pending verification, possibly in another app instance. Resolve recovery before other management changes.".into());
        }
        self.maintenance
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "Board maintenance is already running; do not start another operation.")?;
        Ok(MaintenancePermit {
            flag: self.maintenance.clone(),
            keep: false,
            _lock: lock,
        })
    }
    pub fn profile(&self, board_id: &str) -> Result<BoardProfile> {
        let bytes = get_generic_password(KEYCHAIN_SERVICE, board_id).map_err(|error| {
            if error.code() == -25300 {
                ConnectionError::new("pairing_required", "This Mac has no saved permission for that Board. Pair it through physical USB; existing records were not replaced.")
            } else {
                ConnectionError::new("keychain_unavailable", "macOS could not allow access to Tinkeroo's saved Board permission. Unlock secure storage or approve the expected Tinkeroo prompt, then retry Connect. Do not delete or re-pair as a workaround.")
            }
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

pub(crate) struct MaintenancePermit {
    _lock: crate::file_lock::ExclusiveFile,
    pub(crate) flag: Arc<AtomicBool>,
    pub(crate) keep: bool,
}
impl Drop for MaintenancePermit {
    fn drop(&mut self) {
        if !self.keep {
            self.flag.store(false, Ordering::SeqCst);
        }
    }
}

#[tauri::command]
pub async fn register_board(
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
    if state.maintenance_active() {
        return Err(ConnectionError::new(
            "maintenance_active",
            "Finish Board maintenance/recovery verification before importing gameplay credentials.",
        ));
    }
    let devices = crate::discovery::adb(&["devices", "-l"]).await?;
    if !crate::management::physical_usb_ready(&devices, &profile.adb_serial) {
        return Err(ConnectionError::new(
            "usb_confirmation_required",
            "The exact record's Board must be ready on physical USB through this Mac's local ADB server. Wireless ADB is not USB proof.",
        ));
    }
    remember_board(&state, &profile)
}

pub(crate) fn remember_board(state: &Connections, profile: &BoardProfile) -> Result<()> {
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
    state.save_profile(profile)?;
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

pub(crate) async fn request(
    client: &Client,
    profile: &BoardProfile,
    port: u16,
    method: Method,
    path: &str,
    body: Option<Value>,
    auth: &crate::session::GameplayAuth,
) -> Result<Value> {
    let token = zeroize::Zeroizing::new(
        auth.token(client, &profile.tls_name, port, &profile.board_id)
            .await?,
    );
    let mut authorization =
        reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token.as_str())).map_err(
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
            "The Board session expired or was rejected. Reconnect your saved Board first. Recover Pairing through USB only if registration was revoked; keep TLS verification enabled.",
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

pub(crate) async fn secure_client(
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
    Ok((client, addresses))
}

async fn verify_candidate(
    profile: &BoardProfile,
    candidate: &Candidate,
    auth: &crate::session::GameplayAuth,
) -> Result<(Client, Vec<SocketAddr>)> {
    let (client, addresses) = secure_client(profile, candidate).await?;
    let hello = request(
        &client,
        profile,
        candidate.port,
        Method::GET,
        "/v1/hello",
        None,
        auth,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbConnection {
    handle: u64,
    board_id: String,
}

#[tauri::command]
pub async fn connect_usb_play(
    state: tauri::State<'_, Connections>,
    serial: String,
) -> Result<UsbConnection> {
    connect_usb(&state, serial).await
}

pub(crate) async fn connect_usb(state: &Connections, serial: String) -> Result<UsbConnection> {
    let _permit = state.begin_maintenance().map_err(|_| ConnectionError::new("maintenance_active", "Another Board operation is active or needs recovery. Finish Parent setup before connecting."))?;
    disconnect(state).await?;
    let selected = serial.clone();
    let mut bootstrap =
        tauri::async_runtime::spawn_blocking(move || crate::usb_play::begin(&selected))
            .await
            .map_err(|_| {
                ConnectionError::new(
                    "usb_play_unavailable",
                    "USB verification stopped; select the Board again.",
                )
            })??;
    let text = |name: &str| {
        bootstrap[name].as_str().map(str::to_owned).ok_or_else(|| {
            ConnectionError::new(
                "invalid_response",
                "USB play returned an incomplete identity.",
            )
        })
    };
    let binding = crate::usb_play::Binding {
        serial: serial.clone(),
        soc: text("serial")?,
        board_id: text("board_id")?,
        boot_id: text("boot_id")?,
        certificate_sha256: text("certificate_sha256")?,
    };
    let profile = BoardProfile {
        board_id: binding.board_id.clone(),
        name: "USB Board".into(),
        adb_serial: serial.clone(),
        tls_name: text("tls_name")?,
        certificate_pem: text("certificate_pem")?,
        api_token: String::new(),
        installation_id: None,
        last_address: None,
    };
    profile.validate_identity(true)?;
    if !crate::session::valid_id(&binding.boot_id)
        || binding.certificate_sha256.len() != 64
        || !binding
            .certificate_sha256
            .bytes()
            .all(|c| c.is_ascii_hexdigit())
    {
        return Err(ConnectionError::new(
            "invalid_response",
            "USB play returned an invalid boot or certificate identity.",
        ));
    }
    let forward =
        tauri::async_runtime::spawn_blocking(move || crate::pairing::Forward::usb_play(&serial))
            .await
            .map_err(|_| ConnectionError::new("usb_unavailable", "USB forwarding stopped."))?
            .map_err(|_| {
                ConnectionError::new(
                    "usb_unavailable",
                    "USB forwarding could not start for the selected Board.",
                )
            })?;
    let candidate = Candidate::new("127.0.0.1", forward.port)?;
    let (client, _) = secure_client(&profile, &candidate).await?;
    let usb = crate::usb_play::UsbAuth::open(
        &client,
        &profile.tls_name,
        candidate.port,
        bootstrap.take(),
        binding.clone(),
        Some(forward),
    )
    .await?;
    let auth = crate::session::GameplayAuth::usb_play(usb.clone());
    let hello = request(
        &client,
        &profile,
        candidate.port,
        Method::GET,
        "/v1/hello",
        None,
        &auth,
    )
    .await;
    let hello = match hello {
        Ok(hello) => hello,
        Err(error) => {
            auth.close(&client, &profile.tls_name, candidate.port).await;
            return Err(error);
        }
    };
    let verified = hello["board"]["board_id"].as_str() == Some(&profile.board_id)
        && hello["protocol"] == "1.0"
        && hello["runtime"]["boot_id"].as_str() == Some(&binding.boot_id)
        && hello["runtime"]["release"].as_str()
            == crate::runtime_update::release_id().ok().as_deref();
    if !verified {
        auth.close(&client, &profile.tls_name, candidate.port).await;
        return Err(ConnectionError::new(
            "protocol_incompatible",
            "The running Companion did not prove the expected USB play identity/version. Use Parent setup to check its software; no outputs were enabled.",
        ));
    }
    usb.start_heartbeat();
    let mut active = state.active.lock().await;
    let handle = state.sequence.fetch_add(1, Ordering::Relaxed);
    let board_id = profile.board_id.clone();
    *active = Some(ActiveBoard {
        handle,
        client,
        profile,
        auth,
        port: candidate.port,
    });
    Ok(UsbConnection { handle, board_id })
}

#[tauri::command]
pub async fn probe_board(
    state: tauri::State<'_, Connections>,
    board_id: String,
    candidate: Candidate,
) -> Result<u64> {
    if state.maintenance_active() {
        return Err(ConnectionError::new(
            "maintenance_active",
            "Board maintenance is active. Finish it before connecting for gameplay.",
        ));
    }
    let mut profile = state.profile(&board_id)?;
    let auth = crate::session::GameplayAuth::from_profile(&profile, &state.directory)?;
    let (client, addresses) = verify_candidate(&profile, &candidate, &auth).await?;
    let mut active = state.active.lock().await;
    if state.maintenance_active() {
        return Err(ConnectionError::new(
            "maintenance_active",
            "Board maintenance started while connecting; gameplay remains paused.",
        ));
    }
    if let Some(old) = active.take() {
        clear_connection(&old).await;
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
        auth,
        port: candidate.port,
    });
    Ok(handle)
}

async fn clear_connection(old: &ActiveBoard) {
    let _ = request(
        &old.client,
        &old.profile,
        old.port,
        Method::POST,
        "/v1/commands",
        Some(json!({"operation":"outputs.clear", "arguments":{}})),
        &old.auth,
    )
    .await;
    old.auth
        .close(&old.client, &old.profile.tls_name, old.port)
        .await;
}

#[tauri::command]
pub async fn disconnect_play_handle(
    state: tauri::State<'_, Connections>,
    handle: u64,
) -> Result<()> {
    close_play(&state, handle).await
}

pub(crate) async fn close_play(state: &Connections, handle: u64) -> Result<()> {
    let mut active = state.active.lock().await;
    if active.as_ref().is_some_and(|board| board.handle == handle)
        && let Some(old) = active.take()
    {
        // Session revocation disarms its lease; cancelling an unpublished
        // connection must not issue an output command.
        old.auth
            .close(&old.client, &old.profile.tls_name, old.port)
            .await;
    }
    Ok(())
}

#[tauri::command]
pub async fn disconnect_board(state: tauri::State<'_, Connections>) -> Result<()> {
    disconnect(&state).await
}

async fn disconnect(state: &Connections) -> Result<()> {
    let mut active = state.active.lock().await;
    if let Some(old) = active.take() {
        clear_connection(&old).await;
    }
    for (serial, port) in state.forwards.lock().await.drain(..) {
        crate::discovery::remove_forward(&serial, port).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn forget_board(
    state: tauri::State<'_, Connections>,
    board_id: String,
    confirmation: String,
) -> Result<()> {
    if confirmation != format!("FORGET {board_id}") {
        return Err(ConnectionError::new(
            "confirmation_required",
            "Type FORGET followed by the selected Board ID. This removes only this Mac's Tinkeroo record, not the Board or other installations.",
        ));
    }
    disconnect_board(state.clone()).await?;
    let _guard = state.profile_writes.lock().map_err(|_| {
        ConnectionError::new("storage_error", "Restart the app before removing a record.")
    })?;
    let _ = state.profile(&board_id)?;
    let ids: Vec<_> = state
        .ids()?
        .into_iter()
        .filter(|id| id != &board_id)
        .collect();
    match delete_generic_password(KEYCHAIN_SERVICE, &board_id) {
        Ok(()) => {}
        Err(error) if error.code() == -25300 => {}
        Err(_) => {
            return Err(ConnectionError::new(
                "keychain_unavailable",
                "Unlock Keychain before removing this Mac's Board record.",
            ));
        }
    }
    let temp = state.directory.join("board-handles.json.tmp");
    std::fs::write(&temp, serde_json::to_vec(&ids).map_err(|_| ConnectionError::new("storage_error", "Could not update the Board index."))?).and_then(|_| std::fs::rename(&temp, state.directory.join("board-handles.json"))).map_err(|_| ConnectionError::new("storage_error", "The credential was removed but the Board index update failed. Re-import/recover the record; no Board data was erased."))?;
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
    gameplay_request(&state, handle, operation).await
}

pub(crate) async fn gameplay_request(
    state: &Connections,
    handle: u64,
    operation: GameplayRequest,
) -> Result<Value> {
    if state.maintenance_active() {
        return Err(ConnectionError::new(
            "maintenance_active",
            "Board maintenance is active; gameplay commands are paused.",
        ));
    }
    let active = state.active.lock().await;
    if state.maintenance_active() {
        return Err(ConnectionError::new(
            "maintenance_active",
            "Board maintenance is active; gameplay remains paused.",
        ));
    }
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
        &selected.auth,
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
    #[ignore = "Owner-only exact USB play read-only acceptance; never run blanket --ignored."]
    fn physical_password_free_usb_play_read_only_owner_only() {
        assert_eq!(
            std::env::var("TC_ACCEPTANCE_CONSENT").as_deref(),
            Ok("usb-play-read-only")
        );
        assert_eq!(
            std::env::var("TC_ACCEPTANCE_USB_SERIAL").as_deref(),
            Ok("1158338022")
        );
        assert!(
            !std::process::Command::new("pgrep")
                .args(["-x", "tinkercrew"])
                .status()
                .unwrap()
                .success(),
            "Quit the app normally before isolated native acceptance."
        );
        let directory = PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("Library/Application Support/com.tinkercrew.desktop");
        assert!(directory.is_dir() && !directory.is_symlink());
        #[link(name = "Security", kind = "framework")]
        unsafe extern "C" {
            fn SecKeychainSetUserInteractionAllowed(state: u8) -> i32;
            fn SecKeychainGetUserInteractionAllowed(state: *mut u8) -> i32;
        }
        let mut previous = 1u8;
        assert_eq!(
            unsafe { SecKeychainGetUserInteractionAllowed(&mut previous) },
            0
        );
        struct Restore(u8);
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = unsafe { SecKeychainSetUserInteractionAllowed(self.0) };
            }
        }
        let _restore = Restore(previous);
        assert_eq!(unsafe { SecKeychainSetUserInteractionAllowed(0) }, 0);
        let connections = Connections::new(directory);
        tauri::async_runtime::block_on(async {
            for cycle in 0..2 {
                let opened = connect_usb(&connections, "1158338022".into()).await.expect(
                    "Native USB Connect failed; no credential or source fallback permitted.",
                );
                assert_eq!(opened.board_id, "43ddf2b6-1679-498d-89c8-4f4c96852b4a");
                let hello = gameplay_request(&connections, opened.handle, GameplayRequest::Hello)
                    .await
                    .unwrap();
                let snapshot =
                    gameplay_request(&connections, opened.handle, GameplayRequest::State)
                        .await
                        .unwrap();
                assert_eq!(hello["health"]["safe_state"], true);
                assert_eq!(snapshot["health"]["state"], "safe_state");
                if cycle == 0 {
                    // Beyond the native 90s proof deadline: success requires the
                    // real background USB reproof, not just the initial token.
                    let deadline = std::time::Instant::now() + Duration::from_secs(95);
                    while std::time::Instant::now() < deadline {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        let state =
                            gameplay_request(&connections, opened.handle, GameplayRequest::State)
                                .await
                                .unwrap();
                        assert_eq!(state["health"]["state"], "safe_state");
                    }
                }
                close_play(&connections, opened.handle).await.unwrap();
            }
        });
        let evidence = json!({"board_id":"43ddf2b6-1679-498d-89c8-4f4c96852b4a","nativeConnectCloseCycles":2,"sustainedReadOnlySeconds":95,"transport":"ADB private Unix TLS","interactiveKeychainDisabledInThisProcess":true,"noLeaseOrOutputCommandsRequested":true,"reportedSafeState":true,"physicalModuleAcceptance":"not tested"});
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::write(
            root.join("artifacts/qa/uno-q-usb-play-native.json"),
            serde_json::to_vec_pretty(&evidence).unwrap(),
        )
        .unwrap();
    }

    #[test]
    #[ignore = "Owner-only authenticated sensor observation; never enable in normal CI or run blanket --ignored."]
    fn physical_authenticated_sensor_inspection_owner_only() {
        assert!(
            std::env::var("TC_ACCEPTANCE_CONSENT").as_deref() == Ok("read-only-sensors"),
            "Explicit owner read-only sensor scope required."
        );
        let serial = std::env::var("TC_ACCEPTANCE_USB_SERIAL")
            .expect("Explicit exact physical USB serial required.");
        assert!(
            crate::management::valid_serial(&serial),
            "Invalid selected USB identity."
        );
        let running = std::process::Command::new("pgrep")
            .args(["-x", "tinkercrew"])
            .output()
            .expect("Could not check parent app state.");
        assert!(
            !running.status.success(),
            "Quit Tinkeroo before the isolated native diagnostic; no process was stopped."
        );
        let directory =
            std::path::PathBuf::from(std::env::var_os("HOME").expect("Missing Mac home."))
                .join("Library/Application Support/com.tinkercrew.desktop");
        assert!(
            directory.is_dir() && !directory.is_symlink(),
            "Expected existing app data; no replacement storage created."
        );
        // Fail closed if macOS requires interactive credential permission. Never leave
        // an unattended acceptance run waiting on SecurityAgent, and never bypass its ACL.
        #[link(name = "Security", kind = "framework")]
        unsafe extern "C" {
            fn SecKeychainSetUserInteractionAllowed(state: u8) -> i32;
            fn SecKeychainGetUserInteractionAllowed(state: *mut u8) -> i32;
        }
        let mut previous_interaction = 1u8;
        assert_eq!(
            unsafe { SecKeychainGetUserInteractionAllowed(&mut previous_interaction) },
            0,
            "Could not read Keychain interaction policy."
        );
        struct RestoreInteraction(u8);
        impl Drop for RestoreInteraction {
            fn drop(&mut self) {
                let _ = unsafe { SecKeychainSetUserInteractionAllowed(self.0) };
            }
        }
        let _interaction = RestoreInteraction(previous_interaction);
        assert_eq!(
            unsafe { SecKeychainSetUserInteractionAllowed(0) },
            0,
            "Could not establish noninteractive Keychain failure mode."
        );
        let state = Connections::new(directory);
        let _permit = state
            .begin_maintenance()
            .expect("Another operation is unresolved; diagnostic refused.");
        println!("Owner read-only diagnostic phase: physical USB inspection");
        let physical =
            crate::management::inspect(&serial).expect("Exact physical Board inspection failed.");
        let observed = crate::pairing::usb(
            &serial,
            &json!({"operation":"inspect","expected_serial":physical.serial}),
        )
        .expect("Fixed read-only USB inspection failed.");
        let board_id = observed["board_id"]
            .as_str()
            .expect("Missing USB Board identity.");
        println!("Owner read-only diagnostic phase: existing Board Keychain record");
        let profile = state
            .profile(board_id)
            .expect("Existing Board credentials are not accessible to this noninteractive diagnostic. Use Tinkeroo's normal Keychain prompt; do not delete or re-pair as a workaround.");
        assert!(
            profile.adb_serial == serial
                && profile.installation_id.is_some()
                && profile.api_token.is_empty(),
            "Selected Board does not have the expected installed TBP Pairing."
        );
        assert!(
            observed["tls_name"].as_str() == Some(profile.tls_name.as_str())
                && observed["certificate_pem"].as_str() == Some(profile.certificate_pem.as_str()),
            "USB and saved TLS identities differ; diagnostic stopped."
        );
        let forward = crate::pairing::Forward::new(&serial).expect("Fixed HTTPS forward failed.");
        let result=tauri::async_runtime::block_on(async {
            let candidate=Candidate::new("127.0.0.1",forward.port)?;
            println!("Owner read-only diagnostic phase: pinned TLS client");
            let (client,_)=secure_client(&profile,&candidate).await?;
            println!("Owner read-only diagnostic phase: existing installation Keychain keys");
            let auth=crate::session::GameplayAuth::from_profile(&profile,&state.directory)?;
            let observation=async {
                println!("Owner read-only diagnostic phase: signed authenticated hello");
                let hello=request(&client,&profile,forward.port,Method::GET,"/v1/hello",None,&auth).await?;
                let sensors=request(&client,&profile,forward.port,Method::GET,"/v1/state",None,&auth).await?;
                let events=request(&client,&profile,forward.port,Method::GET,"/v1/events?after=0",None,&auth).await?;
                if hello["board"]["board_id"].as_str()!=Some(profile.board_id.as_str()) || hello["runtime"]["boot_id"]!=observed["boot_id"] {
                    return Err(ConnectionError::new("identity_mismatch","Signed TLS and physical USB did not prove the same Board/kernel."));
                }
                Ok::<_,ConnectionError>(json!({"authenticatedTlsAndUsbIdentityVerified":true,"health":sensors["health"],"observables":sensors["observables"],"capabilities":hello["capabilities"],"runtimeRelease":hello["runtime"]["release"],"eventCount":events["events"].as_array().map(|items|items.len()),"noOutputControlLeaseRearmOrSelfTestRequested":true,"scope":"read-only current observations, not human input/output acceptance"}))
            }.await;
            auth.close(&client,&profile.tls_name,forward.port).await;
            observation
        }).expect("Authenticated read-only observation failed; no Pairing reset or output fallback attempted.");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/qa");
        std::fs::write(
            root.join("uno-q-native-sensor-inspection.json"),
            serde_json::to_vec_pretty(&result).unwrap(),
        )
        .expect("Could not save sanitized observation.");
        println!("{}", serde_json::to_string(&result).unwrap());
    }

    #[test]
    fn maintenance_blocks_competing_operations_and_other_instances_with_pending_restore() {
        let directory = tempfile::tempdir().unwrap();
        let first = Connections::new(directory.path().to_path_buf());
        let second = Connections::new(directory.path().to_path_buf());
        let permit = first.begin_maintenance().unwrap();
        assert!(first.maintenance_active());
        assert!(second.maintenance_active());
        assert!(second.begin_maintenance().is_err());
        assert!(first.begin_maintenance().is_err());
        std::fs::create_dir(directory.path().join("board-recovery")).unwrap();
        let marker = directory
            .path()
            .join("board-recovery/factory-restore-active.json");
        std::fs::write(&marker, b"{}").unwrap();
        assert!(second.maintenance_active());
        assert!(second.begin_maintenance().is_err());
        drop(permit);
        assert!(first.maintenance_active());
        std::fs::remove_file(marker).unwrap();
        assert!(!first.maintenance_active());
        assert!(!second.maintenance_active());
        let runtime = directory.path().join("runtime-update-active.json");
        std::fs::write(&runtime, b"{}").unwrap();
        assert!(second.maintenance_active());
        assert!(second.begin_maintenance().is_err());
        let runtime_permit = first.begin_runtime_update().unwrap();
        assert!(second.begin_runtime_update().is_err());
        drop(runtime_permit);
        std::fs::remove_file(runtime).unwrap();
        assert!(!second.maintenance_active());
    }

    #[test]
    fn renewed_certificate_keeps_working_with_the_existing_pinned_identity() {
        use std::io::{BufRead, BufReader};
        struct Peer(std::process::Child);
        impl Drop for Peer {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut peer = Peer(
            std::process::Command::new("python3")
                .arg(root.join("companion/tests/renewal_fixture.py"))
                .arg(directory.path())
                .env("PYTHONPATH", root.join("companion/python"))
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let mut line = String::new();
        BufReader::new(peer.0.stdout.as_mut().unwrap())
            .read_line(&mut line)
            .unwrap();
        let fixture: Value = serde_json::from_str(&line).unwrap();
        let profile = BoardProfile {
            board_id: fixture["boardId"].as_str().unwrap().into(),
            name: "Renewal fixture".into(),
            adb_serial: "FIXTURE".into(),
            tls_name: fixture["tlsName"].as_str().unwrap().into(),
            certificate_pem: fixture["previousCertificate"].as_str().unwrap().into(),
            api_token: "fixture-only-renewal-token-32-characters".into(),
            installation_id: None,
            last_address: None,
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let port = fixture["port"].as_u64().unwrap() as u16;
            let candidate = Candidate::new("127.0.0.1", port).unwrap();
            let (client, _) = secure_client(&profile, &candidate).await.unwrap();
            let auth = crate::session::GameplayAuth::legacy(&profile.api_token);
            let hello = request(
                &client,
                &profile,
                port,
                Method::GET,
                "/v1/hello",
                None,
                &auth,
            )
            .await
            .unwrap();
            assert_eq!(hello["protocol"], "1.0");
            assert_eq!(hello["board"]["board_id"], profile.board_id);
        });
    }

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
            installation_id: None,
            last_address: None,
        };
        let candidate = Candidate::new("127.0.0.1", port).unwrap();
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            let auth = crate::session::GameplayAuth::legacy(&profile.api_token);
            let (client, _) = verify_candidate(&profile, &candidate, &auth).await.unwrap();
            request(&client, &profile, port, Method::POST, "/v1/control-lease", Some(json!({"lease_ms":1500})), &auth).await.unwrap();
            request(&client, &profile, port, Method::POST, "/v1/commands", Some(json!({"operation":"pixels.set", "arguments":{"color":"#22c55e","brightness":0.3,"duration_ms":1000}})), &auth).await.unwrap();
            let state = request(&client, &profile, port, Method::GET, "/v1/state", None, &auth).await.unwrap();
            assert_eq!(state["outputs"]["pixels"]["color"], "#22c55e");
            let mut wrong = profile.clone();
            wrong.board_id = "other-board".into();
            assert_eq!(verify_candidate(&wrong, &candidate, &crate::session::GameplayAuth::legacy(&wrong.api_token)).await.err().unwrap().code, "identity_mismatch");
            wrong = profile.clone();
            wrong.api_token = "wrong-credential-01234567890123456789".into();
            assert_eq!(verify_candidate(&wrong, &candidate, &crate::session::GameplayAuth::legacy(&wrong.api_token)).await.err().unwrap().code, "authentication_failed");
            wrong = profile.clone();
            wrong.certificate_pem = rcgen::generate_simple_self_signed(vec!["guardian.local".into()]).unwrap().cert.pem();
            assert_eq!(verify_candidate(&wrong, &candidate, &crate::session::GameplayAuth::legacy(&wrong.api_token)).await.err().unwrap().code, "companion_unavailable");
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
