use std::{
    io::{Read, Write},
    os::unix::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};
use wait_timeout::ChildExt;
use zeroize::Zeroizing;

/// Only fixed native management adapters call this; it is never exposed over IPC.
pub(crate) fn run(
    program: &Path,
    args: &[&str],
    input: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    if input.len() > 16384 {
        return Err("Management input exceeds its safe limit.".into());
    }
    let mut child = Command::new(program)
        .process_group(0)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("ARDUINO_FLASHER_ADDITIONAL_URLS")
        .env_remove("ARDUINO_FLASHER_ADDITIONAL_HEADERS")
        .env_remove("ADB_SERVER_SOCKET")
        .env_remove("ANDROID_ADB_SERVER_ADDRESS")
        .env_remove("ANDROID_ADB_SERVER_PORT")
        .env_remove("ADB_SERVER_PORT")
        .env_remove("ADB_TRACE")
        .env_remove("ADB_TRACE_LOG_PATH")
        .env("LC_ALL", "C")
        .spawn()
        .map_err(|_| "The required local management tool could not start.")?;
    let stdout = child.stdout.take().ok_or("No tool output stream.")?;
    let stderr = child.stderr.take().ok_or("No tool diagnostic stream.")?;
    let out = std::thread::spawn(move || {
        let mut bytes = Zeroizing::new(Vec::new());
        stdout.take(65537).read_to_end(&mut bytes).map(|_| bytes)
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Zeroizing::new(Vec::new());
        stderr.take(65537).read_to_end(&mut bytes).map(|_| bytes)
    });
    let mut stdin = child.stdin.take().ok_or("No tool input stream.")?;
    let input = Zeroizing::new(input.to_vec());
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let status = match child.wait_timeout(timeout) {
        Ok(Some(status)) => Some(status),
        _ => {
            // This read-only/bootstrap tool has its own process group; never kill an unrelated ADB server.
            unsafe {
                libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
            }
            let _ = child.wait();
            None
        }
    };
    let write = writer.join().map_err(|_| "Management input stopped.")?;
    let output = out
        .join()
        .map_err(|_| "Management output stopped.")?
        .map_err(|_| "Management output could not be read.")?;
    let diagnostic = err
        .join()
        .map_err(|_| "Management diagnostics stopped.")?
        .map_err(|_| "Management diagnostics could not be read.")?;
    if output.len() > 65536 || diagnostic.len() > 65536 {
        return Err("Management response exceeded its safe limit.".into());
    }
    if write.is_err() || !status.is_some_and(|status| status.success()) {
        return Err("The operation failed or timed out. Check the selected Board, password, tool/image support, and connection; success is not assumed.".into());
    }
    Ok(output.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_use_stdin_and_subprocess_failures_are_bounded_and_sanitized() {
        // Use the activated test toolchain, not Apple's CLT launcher, whose first
        // invocation can run developer-tool setup outside this fixture's deadline.
        let python = Path::new("python3");
        let code = "import sys,json; print(json.dumps({'stdin':sys.stdin.read()=='fixture-only-secret','argv':len(sys.argv)}))";
        let output = run(
            python,
            &["-c", code],
            b"fixture-only-secret",
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output).unwrap(),
            serde_json::json!({"stdin":true,"argv":1})
        );
        let error = run(
            python,
            &[
                "-c",
                "import sys; print('fixture-only-secret'); sys.exit(1)",
            ],
            &[],
            Duration::from_secs(5),
        )
        .unwrap_err();
        assert!(!error.contains("fixture-only-secret"));
        let error = run(
            python,
            &["-c", "import time; time.sleep(2)"],
            &[b'x'; 16384],
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert!(error.contains("timed out"));
        assert!(
            run(
                python,
                &["-c", "print('x'*70000)"],
                &[],
                Duration::from_secs(5)
            )
            .is_err()
        );
    }
}
