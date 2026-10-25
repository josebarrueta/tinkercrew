//! Fixed local ADB transport; never exposed as an arbitrary command IPC.
use std::{path::Path, time::Duration};

pub(crate) fn ensure_server(adb: &Path) -> Result<(), String> {
    // ADB treats an explicit -H (and even 127.0.0.1 in -L) as a remote
    // daemon for auto-start purposes. Its documented localhost spelling starts
    // a loopback-only daemon. Never use -a or an environment-supplied socket.
    crate::admin_process::run(
        adb,
        &["-L", "tcp:localhost:5037", "start-server"],
        &[],
        Duration::from_secs(5),
    )?;
    Ok(())
}
pub(crate) fn local(adb: &Path, args: &[&str], timeout: Duration) -> Result<Vec<u8>, String> {
    ensure_server(adb)?;
    let mut pinned = vec!["-H", "127.0.0.1", "-P", "5037"];
    pinned.extend_from_slice(args);
    crate::admin_process::run(adb, &pinned, &[], timeout)
}

pub(crate) fn request(
    adb: &Path,
    serial: &str,
    command: &str,
    input: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    if !crate::management::valid_serial(serial) || input.len() > 16384 {
        return Err("Invalid fixed USB request.".into());
    }
    let features = local(adb, &["-s", serial, "features"], Duration::from_secs(4))?;
    if !std::str::from_utf8(&features)
        .map_err(|_| "Unsupported USB transport features.")?
        .trim()
        .split(|character: char| character == ',' || character.is_ascii_whitespace())
        .any(|feature| feature == "shell_v2")
    {
        return Err("This Board/Platform-Tools combination lacks shell-v2 EOF/exit-status support. Use supported official setup; no private input was sent.".into());
    }
    // Raw exec-out does not propagate stdin EOF on the UNO Q. Shell-v2 does,
    // without a PTY/echo or local escape processing; private bytes stay on stdin.
    crate::admin_process::run(
        adb,
        &[
            "-H",
            "127.0.0.1",
            "-P",
            "5037",
            "-s",
            serial,
            "shell",
            "-T",
            "-e",
            "none",
            command,
        ],
        input,
        timeout,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};
    fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("adb");
        fs::write(
            &path,
            format!(
                "#!/usr/bin/env python3\n{}",
                include_str!("../../../scripts/tests/adb_transport_fixture.py")
            ),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        (directory, path)
    }
    const HELPER: &str = "python3 -c 'import json,sys; body=json.loads(sys.stdin.buffer.read(16385)); print(json.dumps({\"received\":body[\"operation\"]==\"inspect\" and len(body[\"fixture\"])>0,\"argv\":len(sys.argv)}))'";
    #[test]
    fn fixed_request_starts_only_a_local_server_and_delivers_private_stdin_eof() {
        let (directory, adb) = fixture();
        let output = request(
            &adb,
            "USB_FIXTURE",
            HELPER,
            br#"{"operation":"inspect","fixture":"fixture-only-not-a-real-secret"}"#,
            Duration::from_secs(2),
        )
        .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output).unwrap(),
            serde_json::json!({"received":true,"argv":1})
        );
        let calls = fs::read_to_string(directory.path().join("calls.jsonl")).unwrap();
        assert!(!calls.contains("fixture-only-not-a-real-secret"));
        assert!(!calls.contains("exec-out"));
        assert!(calls.contains("tcp:localhost:5037"));
        assert!(!calls.contains("\"-a\""));
    }
    #[test]
    fn legacy_peer_is_refused_before_transmitting_any_private_input() {
        let (directory, adb) = fixture();
        fs::write(directory.path().join("legacy-peer"), b"").unwrap();
        let error = request(
            &adb,
            "USB_FIXTURE",
            HELPER,
            b"fixture-only-private",
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert!(error.contains("shell-v2"));
        let calls = fs::read_to_string(directory.path().join("calls.jsonl")).unwrap();
        assert!(!calls.contains("\"shell\""));
        assert!(!calls.contains("exec-out"));
        assert!(!calls.contains("fixture-only-private"));
    }
    #[test]
    fn shell_exit_failure_does_not_leak_private_input_or_become_success() {
        let (directory, adb) = fixture();
        let command = "python3 -c 'import sys; data=sys.stdin.buffer.read(16385); sys.stdout.buffer.write(data); sys.exit(7)'";
        let error = request(
            &adb,
            "USB_FIXTURE",
            command,
            b"fixture-only-private",
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert!(!error.contains("fixture-only-private"));
        assert!(
            !fs::read_to_string(directory.path().join("calls.jsonl"))
                .unwrap()
                .contains("fixture-only-private")
        );
    }
}
