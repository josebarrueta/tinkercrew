use crate::connection::ConnectionError;
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use ring::{
    rand::{SecureRandom, SystemRandom},
    signature::{Ed25519KeyPair, KeyPair},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use zeroize::{Zeroize, Zeroizing};

type Result<T> = std::result::Result<T, ConnectionError>;
fn failure() -> ConnectionError {
    ConnectionError::new(
        "authentication_failed",
        "The Board rejected installation authentication. Recover Pairing through physical USB; never weaken TLS.",
    )
}
fn entropy<const N: usize>() -> Result<[u8; N]> {
    let mut value = [0; N];
    SystemRandom::new()
        .fill(&mut value)
        .map_err(|_| failure())?;
    Ok(value)
}
pub(crate) fn valid_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, byte)| {
            if [8, 13, 18, 23].contains(&i) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InstallationKeys {
    pub installation_id: String,
    gameplay_seed: String,
    ssh_seed: String,
}
impl Drop for InstallationKeys {
    fn drop(&mut self) {
        self.gameplay_seed.zeroize();
        self.ssh_seed.zeroize();
    }
}
impl InstallationKeys {
    pub fn generate() -> Result<Self> {
        let mut id = entropy::<16>()?;
        id[6] = (id[6] & 15) | 64;
        id[8] = (id[8] & 63) | 128;
        let hex: String = id.iter().map(|byte| format!("{byte:02x}")).collect();
        Ok(Self {
            installation_id: format!(
                "{}-{}-{}-{}-{}",
                &hex[..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..]
            ),
            gameplay_seed: URL_SAFE_NO_PAD.encode(Zeroizing::new(entropy::<32>()?).as_ref()),
            ssh_seed: URL_SAFE_NO_PAD.encode(Zeroizing::new(entropy::<32>()?).as_ref()),
        })
    }
    fn key(seed: &str) -> Result<Ed25519KeyPair> {
        let bytes = Zeroizing::new(URL_SAFE_NO_PAD.decode(seed).map_err(|_| failure())?);
        Ed25519KeyPair::from_seed_unchecked(&bytes).map_err(|_| failure())
    }
    pub fn validate(&self) -> Result<()> {
        if !valid_id(&self.installation_id) || self.gameplay_seed == self.ssh_seed {
            return Err(failure());
        }
        Self::key(&self.gameplay_seed)?;
        Self::key(&self.ssh_seed)?;
        Ok(())
    }
    pub fn ssh_private(&self) -> Result<Zeroizing<String>> {
        self.validate()?;
        let bytes = Zeroizing::new(
            URL_SAFE_NO_PAD
                .decode(&self.ssh_seed)
                .map_err(|_| failure())?,
        );
        let mut seed = Zeroizing::new([0u8; 32]);
        if bytes.len() != 32 {
            return Err(failure());
        }
        seed.copy_from_slice(&bytes);
        let pair = ssh_key::private::Ed25519Keypair::from_seed(&seed);
        ssh_key::PrivateKey::new(ssh_key::private::KeypairData::Ed25519(pair), "")
            .map_err(|_| failure())?
            .to_openssh(ssh_key::LineEnding::LF)
            .map_err(|_| failure())
    }
    pub fn registration(&self, label: &str) -> Result<Value> {
        self.validate()?;
        if label.is_empty() || label.chars().count() > 60 || label.chars().any(char::is_control) {
            return Err(failure());
        }
        let ssh = Self::key(&self.ssh_seed)?;
        let mut blob = vec![];
        blob.extend_from_slice(&11u32.to_be_bytes());
        blob.extend_from_slice(b"ssh-ed25519");
        blob.extend_from_slice(&32u32.to_be_bytes());
        blob.extend_from_slice(ssh.public_key().as_ref());
        Ok(
            json!({"installation_id":self.installation_id,"label":label,"public_key":URL_SAFE_NO_PAD.encode(Self::key(&self.gameplay_seed)?.public_key().as_ref()),"ssh_public_key":format!("ssh-ed25519 {}", STANDARD.encode(blob))}),
        )
    }
}

pub(crate) fn installation(directory: &std::path::Path, create: bool) -> Result<InstallationKeys> {
    use security_framework::passwords::{get_generic_password, set_generic_password};
    use std::os::unix::fs::OpenOptionsExt;
    const SERVICE: &str = "com.tinkercrew.desktop.installation-keys";
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(directory.join("installation-keys.lock"))
        .map_err(|_| failure())?;
    let _lock = crate::file_lock::ExclusiveFile::new(file).map_err(|_| {
        ConnectionError::new(
            "keychain_unavailable",
            "Another app instance is updating installation keys. Retry after it finishes.",
        )
    })?;
    match get_generic_password(SERVICE, "ed25519-v1") {
        Ok(bytes) => {
            let bytes = Zeroizing::new(bytes);
            let keys: InstallationKeys = serde_json::from_slice(&bytes).map_err(|_| failure())?;
            keys.validate()?;
            Ok(keys)
        }
        Err(error) if error.code() == -25300 && create => {
            let keys = InstallationKeys::generate()?;
            let bytes = Zeroizing::new(serde_json::to_vec(&keys).map_err(|_| failure())?);
            set_generic_password(SERVICE, "ed25519-v1", &bytes).map_err(|_| {
                ConnectionError::new(
                    "keychain_unavailable",
                    "Unlock Keychain before creating installation credentials.",
                )
            })?;
            Ok(keys)
        }
        Err(error) if error.code() != -25300 => Err(ConnectionError::new(
            "keychain_unavailable",
            "macOS could not allow access to Tinkeroo's installation permission. Unlock secure storage or approve the expected app prompt, then retry Connect. Existing keys were not changed.",
        )),
        Err(_) => Err(ConnectionError::new(
            "pairing_required",
            "This installation has no saved private key. Establish permission through physical USB Pairing; existing Board records were not overwritten.",
        )),
    }
}

enum AuthMode {
    Legacy(Zeroizing<String>),
    Paired(InstallationKeys),
}
struct CachedSession {
    token: Zeroizing<String>,
    created: Instant,
    last_use: Instant,
    absolute: Duration,
    idle: Duration,
}
pub(crate) struct GameplayAuth {
    mode: AuthMode,
    cached: Mutex<Option<CachedSession>>,
}
impl GameplayAuth {
    pub fn from_profile(
        profile: &crate::connection::BoardProfile,
        directory: &std::path::Path,
    ) -> Result<Self> {
        if let Some(id) = &profile.installation_id {
            let keys = installation(directory, false)?;
            if &keys.installation_id != id {
                return Err(ConnectionError::new(
                    "pairing_required",
                    "This Board belongs to a different installation key. Recover through USB.",
                ));
            }
            Ok(Self::paired(keys))
        } else {
            Ok(Self::legacy(&profile.api_token))
        }
    }
    pub fn legacy(token: &str) -> Self {
        Self {
            mode: AuthMode::Legacy(Zeroizing::new(token.into())),
            cached: Mutex::new(None),
        }
    }
    pub fn paired(keys: InstallationKeys) -> Self {
        Self {
            mode: AuthMode::Paired(keys),
            cached: Mutex::new(None),
        }
    }
    pub async fn close(&self, client: &reqwest::Client, host: &str, port: u16) {
        if let Some(session) = self.cached.lock().await.take()
            && let Ok(mut header) = reqwest::header::HeaderValue::from_str(&format!(
                "Bearer {}",
                session.token.as_str()
            ))
        {
            header.set_sensitive(true);
            let _ = client
                .delete(format!("https://{host}:{port}/v1/sessions/current"))
                .header(reqwest::header::AUTHORIZATION, header)
                .send()
                .await;
        }
    }
    pub async fn token(
        &self,
        client: &reqwest::Client,
        host: &str,
        port: u16,
        board: &str,
    ) -> Result<String> {
        let AuthMode::Paired(keys) = &self.mode else {
            if let AuthMode::Legacy(value) = &self.mode {
                return Ok(value.to_string());
            }
            unreachable!()
        };
        let mut cached = self.cached.lock().await;
        let now = Instant::now();
        if let Some(session) = cached.as_mut()
            && now.duration_since(session.created) < session.absolute
            && now.duration_since(session.last_use) < session.idle
        {
            session.last_use = now;
            return Ok(session.token.to_string());
        }
        if !valid_id(board) {
            return Err(failure());
        }
        let challenge = post(
            client,
            host,
            port,
            "/v1/sessions/challenge",
            &json!({"board_id":board,"installation_id":keys.installation_id}),
        )
        .await?;
        let id = challenge["challenge_id"]
            .as_str()
            .filter(|value| valid_id(value))
            .ok_or_else(failure)?;
        let nonce = challenge["nonce"]
            .as_str()
            .filter(|value| {
                value.len() == 43
                    && URL_SAFE_NO_PAD
                        .decode(value)
                        .is_ok_and(|raw| raw.len() == 32)
            })
            .ok_or_else(failure)?;
        let expires = challenge["expires_at"]
            .as_str()
            .filter(|value| value.len() <= 32 && value.bytes().all(|byte| byte.is_ascii_graphic()))
            .ok_or_else(failure)?;
        if challenge["board_id"].as_str() != Some(board)
            || challenge["installation_id"].as_str() != Some(&keys.installation_id)
        {
            return Err(failure());
        }
        let message = format!(
            "TBP1\n{board}\n{}\n{id}\n{nonce}\n{expires}\n",
            keys.installation_id
        );
        let signature = Self::sign(keys, message.as_bytes())?;
        let previous = cached.as_ref().map(|value| value.token.as_str());
        let result = post_authorized(client, host, port, "/v1/sessions", &json!({"board_id":board,"installation_id":keys.installation_id,"challenge_id":id,"signature":signature}), previous).await?;
        let token = result["session_token"]
            .as_str()
            .filter(|token| {
                token.len() == 43
                    && URL_SAFE_NO_PAD
                        .decode(token)
                        .is_ok_and(|raw| raw.len() == 32)
            })
            .ok_or_else(failure)?
            .to_owned();
        let absolute = result["expires_in_seconds"]
            .as_u64()
            .filter(|value| (61..=900).contains(value))
            .ok_or_else(failure)?;
        let idle = result["idle_timeout_seconds"]
            .as_u64()
            .filter(|value| (31..=120).contains(value))
            .ok_or_else(failure)?;
        *cached = Some(CachedSession {
            token: Zeroizing::new(token.clone()),
            created: Instant::now(),
            last_use: Instant::now(),
            absolute: Duration::from_secs(absolute - 60),
            idle: Duration::from_secs(idle - 30),
        });
        Ok(token)
    }
    fn sign(keys: &InstallationKeys, message: &[u8]) -> Result<String> {
        Ok(URL_SAFE_NO_PAD.encode(
            InstallationKeys::key(&keys.gameplay_seed)?
                .sign(message)
                .as_ref(),
        ))
    }
}

pub(crate) async fn post(
    client: &reqwest::Client,
    host: &str,
    port: u16,
    path: &str,
    body: &Value,
) -> Result<Value> {
    post_authorized(client, host, port, path, body, None).await
}
async fn post_authorized(
    client: &reqwest::Client,
    host: &str,
    port: u16,
    path: &str,
    body: &Value,
    token: Option<&str>,
) -> Result<Value> {
    let mut request = client
        .post(format!("https://{host}:{port}{path}"))
        .json(body);
    if let Some(token) = token {
        let mut header = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| failure())?;
        header.set_sensitive(true);
        request = request.header(reqwest::header::AUTHORIZATION, header);
    }
    let mut response = request.send().await.map_err(|_| ConnectionError::new(
        "companion_unavailable",
        "The Companion did not answer securely. Keep your saved Pairing. Check Board power and USB; after a Board restart, start the installed Companion through official App Lab before retrying. This may compile/upload firmware and needs parent approval.",
    ))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED
        || response.status() == reqwest::StatusCode::FORBIDDEN
    {
        return Err(failure());
    }
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(ConnectionError::new(
            "authentication_busy",
            "The Board is limiting authentication attempts. Keep your saved Pairing, wait one minute, then reconnect.",
        ));
    }
    if !response.status().is_success() {
        return Err(ConnectionError::new(
            "companion_unavailable",
            "The Companion service could not complete authentication. Keep your saved Pairing and retry after checking the running app; no credentials were replaced.",
        ));
    }
    let mut bytes = Zeroizing::new(Vec::new());
    while let Some(chunk) = response.chunk().await.map_err(|_| failure())? {
        if bytes.len() + chunk.len() > 16384 {
            return Err(failure());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| failure())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};

    #[test]
    fn unreachable_companion_is_not_reported_as_rejected_installation_keys() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let client = reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_millis(100))
            .timeout(Duration::from_millis(200))
            .build()
            .unwrap();
        let error = tauri::async_runtime::block_on(post(
            &client,
            "127.0.0.1",
            port,
            "/v1/sessions/challenge",
            &json!({"fixture":"no credentials"}),
        ))
        .unwrap_err();
        assert_eq!(error.code, "companion_unavailable");
        assert!(error.message.contains("Pairing"));
    }

    #[test]
    fn authentication_transport_distinguishes_unavailable_busy_and_rejected_peers() {
        use std::io::Read;
        for (status, expected) in [
            (503, "companion_unavailable"),
            (429, "authentication_busy"),
            (401, "authentication_failed"),
        ] {
            let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
            let root = reqwest::Certificate::from_der(cert.cert.der()).unwrap();
            let mut acceptor = openssl::ssl::SslAcceptor::mozilla_intermediate(
                openssl::ssl::SslMethod::tls_server(),
            )
            .unwrap();
            acceptor
                .set_certificate(&openssl::x509::X509::from_der(cert.cert.der()).unwrap())
                .unwrap();
            acceptor
                .set_private_key(
                    &openssl::pkey::PKey::private_key_from_der(&cert.signing_key.serialize_der())
                        .unwrap(),
                )
                .unwrap();
            let acceptor = acceptor.build();
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let peer = std::thread::spawn(move || {
                let (socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut stream = acceptor.accept(socket).unwrap();
                let mut reader = BufReader::new(&mut stream);
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if line.to_lowercase().starts_with("content-length:") {
                        length = line
                            .split(':')
                            .nth(1)
                            .unwrap()
                            .trim()
                            .parse::<usize>()
                            .unwrap();
                    }
                }
                assert!(length < 4096);
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                drop(reader);
                write!(stream,"HTTP/1.1 {status} Fixture\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}").unwrap();
                stream.flush().unwrap();
            });
            let client = reqwest::Client::builder()
                .https_only(true)
                .no_proxy()
                .tls_certs_only([root])
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap();
            let result = tauri::async_runtime::block_on(post(
                &client,
                "127.0.0.1",
                port,
                "/v1/sessions/challenge",
                &json!({"fixture":"not credentials"}),
            ));
            peer.join().unwrap();
            assert_eq!(result.unwrap_err().code, expected);
        }
    }

    #[test]
    fn dedicated_ssh_memory_key_matches_the_registered_key_and_is_not_gameplay_authority() {
        let keys = InstallationKeys::generate().unwrap();
        let registration = keys.registration("Fixture Mac").unwrap();
        let encoded = keys.ssh_private().unwrap();
        let decoded = ssh_key::PrivateKey::from_openssh(encoded.as_str()).unwrap();
        assert!(!decoded.is_encrypted());
        assert_eq!(
            decoded.public_key().to_openssh().unwrap(),
            registration["ssh_public_key"].as_str().unwrap()
        );
        let gameplay = URL_SAFE_NO_PAD
            .decode(registration["public_key"].as_str().unwrap())
            .unwrap();
        assert_ne!(
            decoded.public_key().key_data().ed25519().unwrap().as_ref(),
            gameplay.as_slice()
        );
    }

    #[test]
    fn native_installation_proves_its_key_to_a_real_tls_companion_and_reuses_its_session() {
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
        let cert = directory.path().join("certificate.pem");
        let key = directory.path().join("key.pem");
        std::fs::write(&cert, certificate.cert.pem()).unwrap();
        std::fs::write(&key, certificate.signing_key.serialize_pem()).unwrap();
        let keys = InstallationKeys::generate().unwrap();
        let registration = keys.registration("Test Mac").unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut child = std::process::Command::new("python3")
            .arg(root.join("companion/tests/session_fixture.py"))
            .args([
                cert.as_os_str(),
                key.as_os_str(),
                directory.path().as_os_str(),
            ])
            .env("PYTHONPATH", root.join("companion/python"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        writeln!(child.stdin.take().unwrap(), "{}", registration).unwrap();
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let port: u16 = line.trim().parse().expect("TLS fixture started");
        let _fixture = Fixture(child);
        let client = reqwest::Client::builder()
            .https_only(true)
            .no_proxy()
            .tls_certs_only([
                reqwest::Certificate::from_pem(certificate.cert.pem().as_bytes()).unwrap(),
            ])
            .resolve("guardian.local", ([127, 0, 0, 1], port).into())
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let auth = GameplayAuth::paired(keys);
            let first = auth
                .token(
                    &client,
                    "guardian.local",
                    port,
                    "98f6411f-20ec-438a-a1de-6ce20fb0370e",
                )
                .await
                .unwrap();
            assert_eq!(
                auth.token(
                    &client,
                    "guardian.local",
                    port,
                    "98f6411f-20ec-438a-a1de-6ce20fb0370e"
                )
                .await
                .unwrap(),
                first
            );
            let response = client
                .get(format!("https://guardian.local:{port}/v1/hello"))
                .bearer_auth(&first)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            let wrong = GameplayAuth::paired(InstallationKeys::generate().unwrap());
            assert!(
                wrong
                    .token(
                        &client,
                        "guardian.local",
                        port,
                        "98f6411f-20ec-438a-a1de-6ce20fb0370e"
                    )
                    .await
                    .is_err()
            );
        });
    }
}
