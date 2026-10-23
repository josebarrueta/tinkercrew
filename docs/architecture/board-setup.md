# Board Setup: configuration, maintenance, and recovery (#19)

Status: **implemented software, physical UNO Q acceptance pending**. Parent Settings is now named **Board Setup**. This consolidates management without changing the bundle identifier, existing Keychain services, saved Mission progress, Crew vocabulary, or Board protocol/discovery identifiers. No Board was reset during development.

This is a single navigation hub, **not a claim that full one-click provisioning is complete**. Full TBP Pairing, dedicated installation keys/sessions, atomic Companion deployment/rollback, and integrated hardware/Crew acceptance remain blocked implementation work under #11. The existing official-tool checkpoint guide and provisioned-record development bridge remain visibly distinct.

## Existing-Board workflow

1. Install official external Android Platform-Tools, connect a direct USB data cable, and open Board Setup in the native macOS app. Browser previews cannot perform management.
2. Refresh USB Boards, explicitly select a serial, and inspect. The adapter requires physical `usb:` evidence from `adb devices -l`, UNO Q's `arduino,imola` compatible string, Debian identity, and the SoC serial from `/sys/devices/soc0/serial_number`. An ADB transport alone is not model/OS proof. Wireless ADB is not physical USB evidence.
3. Physically confirm the Board, then supply its existing Linux username/password. If SSH is disabled, explicitly enable its service using the current USB Linux account and password. This changes network exposure: establish a strong Linux password through official setup first. We never guess a default password or silently enable SSH.
4. Authenticate over SSH forwarded through that exact USB device. Host-key SHA-256 fingerprints are pinned in a new, separate macOS Keychain service after password authentication and matching USB/SSH SoC identity. An unexplained key change fails closed.
5. Use bounded operations to inspect status, change the hostname, scan/join Wi-Fi, or schedule a reboot. Actions require the authenticated session (five-minute authorization lifetime) and fresh USB identity. Hostname, SSH enablement, and reboot require re-entering the Linux password. Wi-Fi secrets go to `nmcli --ask` stdin, not command arguments.

Management ADB connections are fixed to this Mac's loopback server on port 5037. ADB server redirection environment variables are ignored. `TC_ADB_PATH` still selects the separately installed executable. The gameplay USB adapter and record import also require local physical USB evidence.

Linux passwords are not Companion API credentials. An authenticated Linux login does not imply a Companion is installed, a Mission is hardware-ready, or TBP Pairing has succeeded. Unsupported images/tools/permissions produce an error, not speculative fallback commands. NetworkManager/SSH/sudo behavior on real image versions still needs acceptance.

## Boundaries and secret handling

- UI invokes a closed, typed management API. `management.rs` owns USB inspection, transient SSH forwarding, fingerprint trust, and session authorization. `board_management.py` is a fixed, transient stdlib helper, not a deployed daemon or generic shell service.
- The helper checks UNO Q/OS/SoC identity before mutations and rejects unknown operations/fields. It cannot accept shell commands, arbitrary programs, downloads, or reset-by-deleting-files.
- Passwords travel in native IPC and protected SSH/USB request stdin. They are never placed in shell text, argv, environment variables, progress/checkpoints, logs, or persistent app credential storage. Owned Rust secret buffers are zeroized. Frontend fields clear after requests and when leaving Board Setup; this is not a claim to securely erase immutable JavaScript strings from RAM.
- Native subprocess input/output is bounded, failures sanitized, and read-only/bootstrap subprocess groups have deadlines. Destructive flashing is deliberately different: it is not killed on timeout or app closure.
- A native maintenance guard blocks new gameplay discovery/probes/Commands; requests recheck after acquiring the active-connection mutex. A pending factory marker blocks them across app instances as well. An OS file lock permits only one instance to prepare/start/accept recovery, preventing competing erases. Process exit releases that lock, but the durable marker still prevents a new erase until the old flasher's outcome is resolved. Existing leases/connections are closed before changes. The UI switches to Simulator and stops automatic reconnection. Ending management never rearms a Mission.
- `MissionSession`/`GuardianRuntime` still receive only `BoardClient`, not Linux credentials or management operations. One reusable Companion remains the gameplay boundary.

## Three different maintenance actions

| Action | Meaning | Confirmation/evidence |
| --- | --- | --- |
| Restart | `shutdown -r +1`; retains Linux data | `RESTART <SoC serial>`, authenticated session/password. Reports **scheduled**, never infers completed reboot from disappearance. |
| Forget Tinkeroo record | Removes this Mac's gameplay Keychain record/index entry only | `FORGET <Board ID>`. Does not erase Board data, Mission progress, other installations, remote keys, or revoke a copied development bearer token. |
| Clear SSH trust | Removes only this app's fingerprint for an explicitly inspected USB Board | `FORGET SSH <USB serial>`. Use after an intentional recovery, not to bypass unexplained key changes. Does not edit `authorized_keys` or user's SSH configuration. |
| Factory restore | Official destructive **Linux OS reflash**, without preserving userdata | Exact approved image, USB/SoC-bound plan, typed erase and every safety acknowledgement; see below. |

The checkpoint guide's **Start new setup** still resets only local guide checkpoints, never the Board.

## Destructive factory restore

The adapter accepts a separately installed official **Arduino Flasher CLI 0.5.4** at an explicit absolute path. Its `version --format json` contract is checked; unknown versions fail closed. This verifies the adapter's version/format contract, **not vendor authentication of an arbitrary executable**. Obtain the executable from Arduino, respect its license, and do not bypass Gatekeeper. No Google or Arduino executable is bundled or silently downloaded.

1. While the Board is booted and still readable over physical USB, fetch the flasher's UNO Q image catalog. Choose an exact Debian image version; no implicit `latest`. Only HTTPS `downloads.arduino.cc` URLs with SHA-256 metadata are eligible. The original `20250807-136` image is excluded because it lacks observable `/etc/buildinfo` build identity.
2. Prepare a fresh plan. The native plan binds the inspected USB/SoC identity, exact image/version/OS/URL/SHA-256, and executable hash. It expires after ten minutes before starting. Preparing a plan performs no erase.
3. Back up/export needed files. Disconnect power and maker loads/accessories. Follow Arduino's physical JCTL GND/USB_BOOT EDL preparation; never move the jumper while powered. Unplug other Boards and use stable power/direct data USB.
4. Acknowledge backup/data loss, EDL preparation, isolated target, and safe power/disconnected loads. Type **`ERASE <SoC serial>`**, not the ADB identifier. Native validation independently requires every acknowledgement and exact typed serial.
5. The app checks at least 10 GiB free space, downloads the approved image over HTTPS without redirects (30-minute download deadline, 4 GiB archive cap), verifies SHA-256, and rechecks the executable hash. Image download/checksum failure starts no flasher.
6. The fixed invocation is `arduino-flasher-cli flash unoq <private-verified-archive> --serial <decimal-SoC-serial> --yes`. **`--preserve-user` is absent**; source-verified 0.5.4 defaults to destructive restore. It never substitutes whichever Board is first or falls back to another serial.
7. Once writing starts there is no Cancel/kill timeout. Keep the app open and Board powered. The child owns file-backed output and continues if the app closes. A durable private record is written before spawning, then records PID and Mac kernel boot-session UUID. Restarting the app with a pending record blocks new gameplay/erase attempts.
8. Successful exit means **flasher reported success**, not complete onboarding. After completion, unplug, remove the EDL jumper, reconnect, and verify fresh physical USB, matching SoC identity, and the exact `/etc/buildinfo` `BUILD_ID`. A changed USB identifier can be selected explicitly; hardware identity must still match. Reconfigure Linux/Wi-Fi, explicitly clear obsolete SSH trust, and establish new Companion Pairing/deployment.

This restores the Linux system and userdata, not a forensic secure erase. STM32 sketch/bootloader recovery is not independently verified. Keep loads disconnected until known Companion firmware and hardware self-test are installed and checked. There is no claim of physical factory acceptance yet.

### Interruption and externally completed recovery

- Interrupted **download**, before any app flasher started: fresh same-Board inspection can discard the plan; the result explicitly says **no app flash started**, not “factory reset complete.”
- App-observed successful flasher exit: matching Board/build verification is labeled `tinkeroo_flasher` evidence.
- Failed/unknown flasher outcome: an old image booting is not reset evidence. Follow official recovery for the originally approved exact image, then explicitly attest successful destructive restore with userdata preservation disabled and no active flasher. Type **`RECOVERED <SoC serial>`**. The app still requires matching physical Board/build and records `human_official_recovery`, not app-observed success.
- An existing/reused PID on the same Mac boot conservatively blocks acceptance. If a spawn-time crash lost the PID, same-boot ownership remains uncertain even with a checkbox. **Do not kill a flasher or reboot the Mac during active writing.** After all flashing has stopped, a Mac restart removes the old process ambiguity; successful official recovery plus explicit attestation/fresh identity/build can then close the pending record.
- The app never automatically retries an erase or accepts an unsupported image/tool contract. A failed/unbootable Board can require official Arduino recovery rather than an in-app retry.

Private files live under the unchanged application-data directory's `board-recovery/` (directory 0700; JSON/log files 0600). Active markers contain identifiers/build/PID/boot UUID, never credentials. The last verified outcome is retained in `factory-restore-last-outcome.json`, including source and observed time/identity. Private image/extraction directories are cleaned after verified completion/discard; failed attempts may retain artifacts until recovery is resolved. Nothing is uploaded automatically.

## Evidence and pending acceptance

Automated coverage includes real local subprocess stdin/deadline/output limits; sanitized failures; helper identity/model mismatch, hostname/SSH/Wi-Fi/reboot allowlists and secret separation; native restore acknowledgement/image/hash/outcome policy and cross-instance locks/markers; physical-versus-wireless ADB evidence; maintenance fallback stopping; and browser UI/IPC contract tests with **every native call mocked**. Browser mocks never run SSH or a flasher. Existing loopback HTTPS Companion tests are not USB/SSH/flashing tests.

Still **PENDING**: actual USB descriptors across official images; USB-forwarded SSH/password/Keychain pin behavior; sudo/NetworkManager configuration; reboot completion; real EDL exact-serial routing; checksum/disk/interruptions on real restore tooling; Linux userdata loss and `/etc/buildinfo` after reflash; obsolete credential cleanup; STM32 safety/self-test; full #11 provisioning; and family acceptance. Use the physical matrix in [recovery-safety.md](../qa/recovery-safety.md), with backups and deliberate owner approval. No automated script should reset connected hardware.

## Primary sources for the adapter contract

- [Arduino UNO Q Linux OS flashing guide](https://docs.arduino.cc/software/app-lab/configure/flash/): EDL preparation, App Lab restore, preservation distinction, disk/power/backup prerequisites.
- [Arduino Flasher CLI 0.5.4 flash command](https://github.com/arduino/arduino-flasher-cli/blob/v0.5.4/cmd/arduino-flasher-cli/flash/flash.go): `flash unoq <image>`, exact decimal/hex serial normalization, `--yes`, default-false `--preserve-user`, local archive support.
- [0.5.4 image catalog](https://github.com/arduino/arduino-flasher-cli/blob/v0.5.4/cmd/arduino-flasher-cli/list/list.go) and [version JSON](https://github.com/arduino/arduino-flasher-cli/blob/v0.5.4/cmd/arduino-flasher-cli/version/version.go).
- [Official Debian image index](https://downloads.arduino.cc/debian-im/Stable/info.json): versioned image URLs/checksums.
- [Arduino App CLI OS identity](https://github.com/arduino/arduino-app-cli/blob/main/pkg/board/os_image.go): `/etc/buildinfo`/`BUILD_ID`; missing legacy identity must not be invented as a fresh observation.
