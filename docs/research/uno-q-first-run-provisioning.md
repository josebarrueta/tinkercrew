# UNO Q first-run provisioning from macOS

Status: **source research and physical-test harness complete; factory-reset Board experiment pending**

Issue: #1

Research date: 2026-08-19

## Recommendation

Use a **hybrid setup** for the MVP:

1. TinkerCrew may safely own USB detection, exact-Board selection, setup-state inspection, Companion transfer/start, dedicated SSH-key installation, and post-setup recovery.
2. Keep an **Arduino App Lab handoff for factory-first configuration** until the physical matrix below passes on a genuinely reset 4 GB / 32 GB UNO Q across the supported image range.
3. If the matrix passes, integrate only the source-backed operations implemented by `scripts/uno-q-provisioning-spike.sh`. Treat Arduino-installed helpers and sudoers rules as compatibility-sensitive, probe them before use, and retain the App Lab fallback.

The source evidence is encouraging but does not satisfy the issue's physical acceptance criteria. No claim is made that an unconfigured Board was tested.

## What primary sources establish

### USB and identity

- Arduino officially documents USB shell access using ADB on macOS, installed with `brew install android-platform-tools`. `adb devices` lists the Board and the Board may take up to one minute to appear. Before App Lab setup, `adb shell` may request the default password `arduino`. [Arduino ADB guide](https://docs.arduino.cc/tutorials/uno-q/adb/)
- Arduino's App CLI source selects an ADB connection by serial and checks `adb -s <serial> get-state`; its file transfer and port-forwarding APIs also keep the serial selector on each invocation. This supports treating the ADB serial as the session's stable USB selector rather than choosing the first device. [arduino-app-cli `pkg/board/remote/adb/adb.go`](https://github.com/arduino/arduino-app-cli/blob/3bbf5fd757fec6a216d885ff50be61402ca3b0dd/pkg/board/remote/adb/adb.go)
- The same source defaults the Board user to `arduino` for ADB/SSH operations. It does not prove what UID or supplemental groups every factory image returns; the physical test must record `id`. [arduino-app-cli `pkg/board/board.go`](https://github.com/arduino/arduino-app-cli/blob/3bbf5fd757fec6a216d885ff50be61402ca3b0dd/pkg/board/board.go)

### Official first-run behavior

- Arduino App Lab's supported first-run UI asks for a Board name, keyboard layout, Linux password, and optionally Wi-Fi. USB is required for initial network configuration. The password is then used for SSH/network mode and SBC login. [Connect, Configure, and Update](https://docs.arduino.cc/software/app-lab/configure/config/)
- Updates require internet, but recent App Lab versions allow non-mandatory updates and network setup to be skipped. Some updates may remain mandatory. [Arduino automatic updates](https://support.arduino.cc/hc/en-us/articles/24360072322972-About-automatic-updates-in-Arduino-App-Lab), [offline issue resolution](https://github.com/arduino/arduino-app-lab/issues/11)
- Arduino's USB troubleshooting guidance requires a data cable, direct connection where possible, a completed Board boot, and permission approval if macOS asks. It explicitly warns that Apple USB-C hubs are incompatible and that some USB-A hub paths fail. [Arduino Help Center](https://support.arduino.cc/hc/en-us/articles/23170726082332-If-your-Arduino-UNO-Q-is-not-detected-by-Arduino-App-Lab)

### Configuration mechanisms used by App Lab

The current App Lab implementation delegates Board setup to `arduino-app-cli` library functions rather than scraping localized terminal prompts:

| Operation | Current implementation | Secret handling in current source | Stability assessment |
| --- | --- | --- | --- |
| Inspect password state | `chage -l arduino`, parsed structurally | No secret | Linux command is stable; parser/output assumptions need image tests. |
| Set Board name | Validate `^[a-zA-Z0-9][a-zA-Z0-9-]{0,63}$`, then `sudo hostnamectl set-hostname`; restart Avahi | No secret | Standard Linux command plus image-specific passwordless sudo policy. |
| Get/set keyboard | `localectl status`; `sudo /usr/local/bin/arduino-set-keyboard-layout <layout>` | No secret | The `/usr/local/bin` helper is Arduino image-specific and not a documented third-party API. |
| Set initial password | `sudo arduino-passwd`, new password written to stdin and terminated with EOF | Password stays out of argv | `arduino-passwd` is image-specific and requires physical compatibility testing. |
| Scan Wi-Fi | `nmcli radio wifi on`, rescan, then terse SSID listing | No secret | Standard NetworkManager interface. |
| Join Wi-Fi | `nmcli --ask ... device wifi connect <ssid>`; password written to stdin | Password stays out of argv | Strong safe-subset candidate; incorrect-password classification is App Lab logic, not an nmcli stable code. |
| Enable network mode | Password sent to `sudo -S -p ''` over stdin; unmask/enable SSH and Avahi and reconfigure OpenSSH | Password stays out of argv | Unit names and image policy are implementation details. |
| Reboot | `sudo -S -p '' reboot`, password over stdin | Password stays out of argv | Standard command; expected transport loss must be handled as success only after rediscovery. |

Sources: [board setup implementation](https://github.com/arduino/arduino-app-cli/blob/3bbf5fd757fec6a216d885ff50be61402ca3b0dd/pkg/board/board.go), [keyboard implementation](https://github.com/arduino/arduino-app-cli/blob/3bbf5fd757fec6a216d885ff50be61402ca3b0dd/pkg/board/board_system.go), [sudo stdin implementation](https://github.com/arduino/arduino-app-cli/blob/3bbf5fd757fec6a216d885ff50be61402ca3b0dd/pkg/board/asroot.go), [Wi-Fi connection](https://github.com/arduino/arduino-app-lab/blob/8947b90ec81c4455ad28ef14d215bacb89837623/standalone-apps/app-lab-desktop/internal/network/wifi/connect.go), and [nmcli stdin adapter](https://github.com/arduino/arduino-app-lab/blob/8947b90ec81c4455ad28ef14d215bacb89837623/standalone-apps/app-lab-desktop/internal/network/network.go).

### App transfer and startup

- Arduino App format is a directory containing `app.yaml`, `python/main.py`, and optionally `sketch/sketch.ino` plus `sketch/sketch.yaml`. The default user-app directory is `/home/arduino/ArduinoApps`. [App specification](https://github.com/arduino/arduino-app-cli/blob/3bbf5fd757fec6a216d885ff50be61402ca3b0dd/docs/app-specification.md), [App CLI user documentation](https://github.com/arduino/arduino-app-cli/blob/3bbf5fd757fec6a216d885ff50be61402ca3b0dd/docs/user-documentation.md)
- `arduino-app-cli app start app_path` is a real CLI command in the current source. The harness transfers an App to a validated name under `/home/arduino/ArduinoApps` and invokes that command. [start command](https://github.com/arduino/arduino-app-cli/blob/3bbf5fd757fec6a216d885ff50be61402ca3b0dd/cmd/arduino-app-cli/app/start.go)

## Supported, fragile, and unsupported conclusions

### Officially supported

- Install ADB on macOS and reach UNO Q over USB.
- Use Arduino App Lab to perform first-run Board configuration.
- Use the Board's `arduino-app-cli` to manage Arduino Apps.
- Use SSH after network mode is configured.
- Configure Wi-Fi manually with NetworkManager tools on the Board.

### Source-backed but fragile until physically verified

- Treating ADB serial as stable across reboot and factory-reset states.
- Detecting initial-password state from `chage` output.
- Calling `arduino-passwd` directly.
- Calling `/usr/local/bin/arduino-set-keyboard-layout` directly.
- Depending on passwordless sudo rules for hostname and Arduino helpers.
- Enabling network mode through exact `ssh.service` and `avahi-daemon.service` unit names.
- Assuming all images classify a failed password-protected Wi-Fi join as “incorrect password”.
- Assuming reboot preserves the same ADB serial and returns within a bounded time.

These are implementations visible in Arduino's GPL source, not a documented stable third-party provisioning API.

### Unsupported or not established

- Claiming that App Lab is unnecessary for a factory-reset Board.
- Automating OS flashing, partitioning, EDL recovery, or bootloader changes; these are explicitly out of scope.
- Scraping interactive/localized first-login prompts.
- Passing Linux or Wi-Fi passwords in command arguments, URLs, logs, or shell history.
- Replacing `authorized_keys`; only append-if-absent behavior is acceptable.
- Assuming network discovery works on guest/corporate/VPN networks; Arduino documents mDNS restrictions.

## Repeatable spike

The attended harness consists of:

- `scripts/uno-q-provisioning-spike.sh` — macOS driver that requires one exact ADB serial, prompts for secrets without echo, and sends structured requests over stdin.
- `scripts/uno-q-provision-helper.py` — fixed Board-side helper that validates input and invokes commands as argument arrays. Passwords are passed only through stdin to `arduino-passwd`, `nmcli --ask`, or `sudo -S`.

The harness intentionally has no “run everything” command. Each operation is individually inspectable and repeatable so a disconnection or power loss resumes by running `status` and retrying only the incomplete idempotent step.

Example:

```sh
scripts/uno-q-provisioning-spike.sh detect
scripts/uno-q-provisioning-spike.sh --serial <adb-serial> status
scripts/uno-q-provisioning-spike.sh --serial <adb-serial> hostname dragon-den
scripts/uno-q-provisioning-spike.sh --serial <adb-serial> password
scripts/uno-q-provisioning-spike.sh --serial <adb-serial> wifi-scan
scripts/uno-q-provisioning-spike.sh --serial <adb-serial> wifi-join 'Family WiFi'
scripts/uno-q-provisioning-spike.sh --serial <adb-serial> enable-network
scripts/uno-q-provisioning-spike.sh --serial <adb-serial> install-key ~/.ssh/tinkercrew_board.pub
scripts/uno-q-provisioning-spike.sh --serial <adb-serial> reboot
scripts/uno-q-provisioning-spike.sh --serial <adb-serial> deploy companion tinkercrew-companion
```

The script disables shell tracing and never prints request JSON. Review the parent shell/terminal capture settings before entering real credentials.

## Physical test matrix — not yet performed

Record exact values rather than replacing “pending” with an inference.

| Check | Required evidence | Result |
| --- | --- | --- |
| Board baseline | 4 GB / 32 GB model, reset method, OS build, Debian version, kernel, App CLI version, App Lab version | Pending |
| USB detection | macOS version, ADB version, USB topology, time-to-detect, stable serial before/after reboot | Pending |
| Shell privilege | `id -un`, UID, groups, sudo policy before and after password setup | Pending |
| Setup-state inspection | Output of the redacted `status` operation on a genuinely unconfigured Board | Pending |
| Hostname | Valid and invalid names; `/etc/hostname`; mDNS reannouncement | Pending |
| Password | Initial state and one successful stdin-only setup; verify no secret in `ps`, shell history, harness logs, or Board journal | Pending |
| Keyboard | At least two listed layouts and one applied layout | Pending |
| Wi-Fi | Scan, successful join, deliberately incorrect password, retry, reboot persistence; inspect process listings during join | Pending |
| Network mode | SSH/Avahi enabled, dedicated key appended, unrelated keys preserved, password login policy recorded | Pending |
| Interruption | Disconnect during at least one major step, reconnect by exact serial, inspect state, and resume successfully | Pending |
| Reboot/rediscovery | Expected transport loss, bounded rediscovery, serial continuity, hostname/IP discovery | Pending |
| App deployment | Transfer, `arduino-app-cli app start`, process/status evidence, reboot behavior | Pending |
| Safety scope | Confirm no flash, partition, EDL, or bootloader operation occurred | Pending |

## Decision gate

Move from hybrid to integrated provisioning only if the physical run demonstrates all of the following on the supported image baseline:

- the reset Board exposes a stable exact ADB identity;
- helper availability and privilege rules are probeable before mutation;
- every step has a truthful state check and can be retried;
- passwords remain absent from arguments, logs, history, and process inspection;
- wrong Wi-Fi credentials leave a recoverable state;
- at least one forced interruption resumes successfully;
- the Companion app installs and starts after reboot.

Otherwise, retain App Lab for first-run configuration and start TinkerCrew Pairing only after the Board reports the required configured state.
