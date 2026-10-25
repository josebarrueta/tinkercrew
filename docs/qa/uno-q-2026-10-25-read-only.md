# Configured UNO Q — read-only verification, 2026-10-25

**Result: PASS for the narrow native read-only inspection; overall app/hardware acceptance NOT COMPLETE.** Owner connected an already configured Board and approved inspection. No reset, configuration, Pairing, deployment, output test, reboot or firmware/OS operation was requested. A willingness to reset was not used as destructive-run approval.

## Environment and evidence

- Report UTC: 2026-10-25T17:06:27Z.
- Mac: Mac16,6, arm64, macOS 27.0.1; external official Platform-Tools 37.0.1 (`adb` protocol 1.0.41).
- App base: `c35a4e9` (PR #26), with #27 transport fix working tree on `fix/physical-usb-request-eof`. This is a native backend inspection, **not a completed GUI wizard**.
- Selected device: one explicit ADB `device` entry with physical `usb:` evidence; shared evidence uses SoC/USB suffix **8022**, never a credential/Keychain export or full provisioned record.
- Native fixed helper confirmed `arduino,imola`, matching SoC, account `arduino`, Debian **13**, BUILD_ID **`20260407-523`**, and available valid kernel boot identity. Independent read-only `id`/probe confirmed **UID 1000**.
- Host Python **3.13.5**; OpenSSL **3.5.5**. App CLI reports **0.9.0**.
- `/home/arduino/ArduinoApps`: exists, not a symlink, owned by UID 1000 and writable by that account. Reserved `tinkercrew-companion` directory is **absent**.
- SSH service reports **active**. This is not authentication, host-key pinning or dedicated-key verification.
- Docker socket exists. No container was started/inspected/changed, and host/container mounts/data ownership are not proven.
- Host `cryptography` module is absent. The fixed host bootstrap is designed to use stdlib; this observation does **not** establish the future Companion container's dependencies or health.

No SSIDs, LAN addresses, passwords, tokens, private keys, complete serials, boot UUIDs or existing app data are included in this shared report.

## Real defect and fix validation (#27)

1. After Platform-Tools installation, no local ADB server was running. Explicit `-H 127.0.0.1` operations cannot auto-start a daemon; `-L tcp:127.0.0.1:5037 start-server` was also treated as remote by this version.
2. Explicit **`-L tcp:localhost:5037 start-server`** succeeded, loopback-only without `-a`. Subsequent operations pin `-H 127.0.0.1 -P 5037`; no remote ADB server was used.
3. Raw `exec-out id` and a no-input Python print completed, excluding absent Python/basic transport as the cause.
4. A tiny bounded Python stdin reader (`read(16385)`) with the nonsecret test payload `{}` **timed out under `exec-out`**, but returned length **2** under non-TTY shell transport. The original actual fixed helper similarly timed out under raw exec.
5. Peer `features` output is **newline-separated** (unlike comma-separated `host-features`). Tests were corrected to match the observed peer format before the native parser was fixed.
6. The fixed adapter requires `shell_v2`, uses **`shell -T -e none`** for EOF/remote-exit propagation with no PTY/escape processing, keeps private input off argv, and refuses unsupported legacy fallback.
7. The explicit ignored acceptance test **`management::tests::physical_usb_inspection_owner_only`** passed through the actual Rust management path after the fix. It printed only the sanitized account/OS/image/suffix summary above. Default automated checks skip this test.
8. External-process fixture regressions cover cold local startup, private stdin EOF, no argv/diagnostic leakage, unsupported-capability refusal and nonzero remote exit. Redirect-environment overrides were also supplied to the fixture suite and correctly ignored. These fixture results do not count as additional physical cases.

The test method and two explicit opt-in environment fields are documented in [recovery-safety.md](recovery-safety.md). No ADB `kill-server`, network scan, privilege escalation or arbitrary Mission command endpoint was introduced.

## What this does not pass

This is partial **#21 P01/M01** evidence only; keep the broad checkboxes unchecked. Actual Companion first startup/dependencies, host/container shared state, Router Bridge/sketch/Modulino compatibility, SSH/HTTPS Pairing, actual transfers/rollback, tracked reboot/LAN+USB, hardware self-test/Crew flow, physical safe-off timing, failure recovery and family acceptance are **NOT RUN**.

App CLI 0.9.0 is older than the 0.14.0 source used for earlier startup research. Do not infer current startup/firmware behavior or compatibility from that research; use the supported official-tool handoff and verify the actual contract before any start. Host Python/OpenSSL/account availability is not full baseline compatibility.

## Next owner-approved phase

Keep the configured Board intact. Confirm connected Modulinos/loads and the safe setup before permitting initial source/identity preparation or firmware-capable first startup. Linux credentials belong only in the native parent UI, not chat/logs. Ask explicitly before Pairing, deployment, output tests, reboot or any destructive operation. Source publication alone never proves firmware startup or readiness.

Separately, #25 license/source-availability review and #12 production signing/notarization remain pending. PR #26's disclosure tooling is merged, but the pending review gate still blocks public binary sharing/release preparation; this physical inspection does not bypass it.
