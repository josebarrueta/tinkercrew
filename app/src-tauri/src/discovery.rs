use crate::connection::{Candidate, ConnectionError, Connections, adb_status, is_local};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use serde::Deserialize;
use std::{
    net::IpAddr,
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::process::Command;

type Result<T> = std::result::Result<T, ConnectionError>;

fn adb_path() -> Result<PathBuf> {
    let mut paths = vec![];
    if let Some(path) = std::env::var_os("TC_ADB_PATH") {
        paths.push(PathBuf::from(path));
    }
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join("Library/Android/sdk/platform-tools/adb"));
    }
    paths.extend([
        PathBuf::from("/opt/homebrew/bin/adb"),
        PathBuf::from("/usr/local/bin/adb"),
    ]);
    paths.into_iter().find(|path| path.is_absolute() && path.is_file()).ok_or_else(|| ConnectionError::new("adb_unavailable", "Install official Android Platform-Tools or set TC_ADB_PATH to its absolute adb path, then restart TinkerCrew."))
}

pub(crate) async fn adb(args: &[&str]) -> Result<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(2),
        Command::new(adb_path()?)
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| {
        ConnectionError::new(
            "usb_timeout",
            "USB did not respond. Check the data cable and Board power, then retry.",
        )
    })?
    .map_err(|_| {
        ConnectionError::new(
            "adb_unavailable",
            "ADB could not start. Check the configured official Platform-Tools installation.",
        )
    })?;
    if !output.status.success() {
        return Err(ConnectionError::new(
            "usb_unavailable",
            "ADB could not reach the selected Board. Check its cable, USB authorization, and power.",
        ));
    }
    if output.stdout.len() > 65536 {
        return Err(ConnectionError::new(
            "invalid_response",
            "ADB returned an unexpectedly large response.",
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| {
        ConnectionError::new("invalid_response", "ADB returned an unreadable response.")
    })
}

pub async fn remove_forward(serial: &str, port: u16) {
    let _ = adb(&["-s", serial, "forward", "--remove", &format!("tcp:{port}")]).await;
}

async fn usb(state: &Connections, board_id: &str) -> Result<Vec<Candidate>> {
    let profile = state.profile(board_id)?;
    let devices = adb(&["devices", "-l"]).await?;
    match adb_status(&devices, &profile.adb_serial) {
        Some("device") => {}
        Some("offline") => {
            return Err(ConnectionError::new(
                "board_booting",
                "The selected Board is visible over USB but not ready. Wait for boot, then retry.",
            ));
        }
        Some("unauthorized") => {
            return Err(ConnectionError::new(
                "usb_authorization_required",
                "Authorize USB access in the official Arduino setup, then retry.",
            ));
        }
        _ => {
            return Err(ConnectionError::new(
                "board_not_detected",
                "The selected Board is not visible over USB. It may be unpowered or unplugged; check power and a USB-C data cable.",
            ));
        }
    }
    let output = adb(&["-s", &profile.adb_serial, "forward", "tcp:0", "tcp:7443"]).await?;
    let port = output
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|port| *port >= 1024)
        .ok_or_else(|| {
            ConnectionError::new(
                "invalid_response",
                "ADB could not allocate a safe local forward port.",
            )
        })?;
    state.forwards.lock().await.push((profile.adb_serial, port));
    Ok(vec![Candidate::new("127.0.0.1", port)?])
}

fn mdns(board_id: &str) -> Result<Vec<Candidate>> {
    let daemon = ServiceDaemon::new().map_err(|_| {
        ConnectionError::new(
            "mdns_unavailable",
            "Bonjour could not start. USB and manual IP connection remain available.",
        )
    })?;
    let service = "_tinkercrew._tcp.local.";
    let receiver = daemon.browse(service).map_err(|_| {
        ConnectionError::new(
            "mdns_unavailable",
            "Bonjour discovery is unavailable on this network. Try USB or manual IP.",
        )
    })?;
    let start = Instant::now();
    let mut candidates = vec![];
    while start.elapsed() < Duration::from_millis(1800) && candidates.len() < 8 {
        if let Ok(ServiceEvent::ServiceResolved(info)) =
            receiver.recv_timeout(Duration::from_millis(100))
        {
            if info.get_property_val_str("board_id") != Some(board_id) {
                continue;
            }
            for address in info
                .get_addresses_v4()
                .into_iter()
                .filter(|ip| is_local(IpAddr::V4(*ip)))
            {
                if candidates.len() >= 8 {
                    break;
                }
                candidates.push(Candidate::new(&address.to_string(), info.get_port())?);
            }
        }
    }
    let _ = daemon.stop_browse(service);
    let _ = daemon.shutdown();
    Ok(candidates)
}

async fn neighbor_candidates() -> Result<Vec<Candidate>> {
    // Passive neighbor-cache enumeration only: no subnet sweep, port sweep, or shell expansion.
    let output = tokio::time::timeout(
        Duration::from_secs(1),
        Command::new("/usr/sbin/arp")
            .arg("-an")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| {
        ConnectionError::new(
            "local_discovery_unavailable",
            "Local neighbor discovery timed out. Use USB or enter the IP manually.",
        )
    })?
    .map_err(|_| {
        ConnectionError::new(
            "local_discovery_unavailable",
            "Local neighbor discovery is unavailable. Use USB or enter the IP manually.",
        )
    })?;
    if output.stdout.len() > 65536 {
        return Ok(vec![]);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut candidates = vec![];
    for line in text.lines() {
        let Some(host) = line
            .split('(')
            .nth(1)
            .and_then(|rest| rest.split(')').next())
        else {
            continue;
        };
        let Ok(ip) = host.parse::<IpAddr>() else {
            continue;
        };
        if is_local(ip) && !ip.is_loopback() {
            candidates.push(Candidate::new(host, 7443)?);
        }
        if candidates.len() == 8 {
            break;
        }
    }
    Ok(candidates)
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryStage {
    Known,
    Mdns,
    Local,
    Usb,
}

#[tauri::command]
pub async fn board_candidates(
    state: tauri::State<'_, Connections>,
    board_id: String,
    stage: DiscoveryStage,
    local_consent: bool,
) -> Result<Vec<Candidate>> {
    let profile = state.profile(&board_id)?;
    match stage {
        DiscoveryStage::Known => Ok(profile.last_address.into_iter().collect()),
        DiscoveryStage::Mdns => tokio::task::spawn_blocking(move || mdns(&board_id))
            .await
            .map_err(|_| {
                ConnectionError::new(
                    "mdns_unavailable",
                    "Bonjour stopped unexpectedly. Try USB or manual IP.",
                )
            })?,
        DiscoveryStage::Local if local_consent => neighbor_candidates().await,
        DiscoveryStage::Local => Ok(vec![]),
        DiscoveryStage::Usb => usb(&state, &board_id).await,
    }
}
