# Installation registration and TBP sessions (#11, first implementation slice)

Status: **software implemented; real USB/SSH/UNO Q compatibility and acceptance NOT RUN**. This is the authentication subset of the [TBP proposal](secure-board-protocol.md), not completion of all-in-one setup. Outstanding acceptance belongs to #21; missing deployment/setup code remains #11.

## Available surface

Board Setup → **Installation Pairing and lost-Mac revocation** operates on an **already installed and explicitly TBP-configured Companion**. It does not create Linux credentials, generate the Board TLS identity, install/upgrade an app, or finish the twelve-stage setup guide. The guide's existing stage-8 completion gate remains closed.

1. Physically inspect/select an UNO Q over this Mac's local ADB server. Type `PAIR <SoC serial>` and choose a recognizable Mac label.
2. Native code creates independent Ed25519 gameplay and SSH seeds plus a random installation UUID once per Mac installation. They are kept in a new Keychain service (`com.tinkercrew.desktop.installation-keys`); an OS file lock serializes initial creation across app instances. Existing Board/progress/Keychain identifiers are unchanged. Keys are saved **before** remote mutations, so retries reuse the same installation identity rather than orphaning newly generated credentials.
3. A fixed host-side CLI receives JSON on USB stdin, checks UNO Q/Debian/current SoC, and returns its configured Board UUID, public TLS certificate/DNS identity, and one-time Pairing token bound to the exact installation/public-key registration. Native code never exposes those token/secret buffers to frontend storage.
4. Native code opens a serial-specific temporary ADB forward and pinned-certificate HTTPS connection. `POST /v1/pairings` atomically consumes the USB token and creates a `pending_ssh` record. This record **cannot** obtain gameplay sessions yet.
5. A separate physical USB `pair_activate` operation appends only the tagged restricted SSH key, then authorizes the registry record. File data and the SSH directory are synced before the DB activation commit. On interruption before commit, a leftover forced key still cannot operate because its registry entry is inactive. Retry is idempotent and preserves unrelated keys/comments.
6. The Mac proves its gameplay key through a real signed challenge and closes the temporary session before remembering the Board. The ordinary connection adapter thereafter obtains short-lived sessions using the Keychain key, never a persisted session token.

The native host command is deliberately fixed to `python3 /home/arduino/ArduinoApps/tinkercrew-companion/python/management.py`. There is no arbitrary path, program, or shell-string IPC. Unknown paths/images/permissions fail with a supported-tooling handoff rather than trying guessed helpers. Direct root-account setup is refused: we must not install a root forced key pointing into a user-writable app tree. Run the host helper as the supported unprivileged Companion owner. The app container and host helper must share the same private `data` directory, Board UUID, kernel boot identity, and monotonic clock; **this topology/ownership has not been verified on actual Arduino images**.

## Companion configuration and dependencies

The existing provisioned bearer mode remains the default for compatibility; it does not silently change existing credentials. Explicit TBP mode requires the updated files, a stable Board UUID, provisioned certificate/private key, and DNS SAN matching `tls_name`, for example this **non-secret schema illustration**:

```json
{
  "auth_mode": "tbp",
  "board_id": "98f6411f-20ec-438a-a1de-6ce20fb0370e",
  "tls_name": "dragon.local",
  "tls_cert": "data/board.crt",
  "tls_key": "data/board.key"
}
```

Do not retain `api_token`/`TC_API_TOKEN` in TBP mode; startup rejects a legacy bearer bypass. No real private key or token belongs in git. Identity/TLS generation and automatic mode migration are still provisioning work, not performed by the registration button.

Install `companion/python/requirements.txt` for the app runtime. Ed25519 verification uses pinned `cryptography`/CFFI dependencies, not custom cryptographic algorithms. The host-side bootstrap/inspection helper uses stdlib; verification happens in the Companion runtime. Dependency installation can need internet during initial provisioning; normal installed sessions need only local TLS.

## Authentication contract

- USB-only management operations: `pair_begin`, `pair_activate`, `list_pairings`, `revoke`, and identity `inspect`. Every request includes the expected SoC serial. These operations have **no HTTP management endpoint**.
- `POST /v1/pairings`: registration plus its USB token, maximum 16 KiB. Token lifetime is five monotonic minutes, same kernel boot, one successful consumption, at most five incorrect token attempts. Tokens bind installation ID, label, gameplay public key, and a distinct SSH public key.
- `POST /v1/sessions/challenge`: exact Board/installation IDs; random challenge UUID/256-bit nonce, 30-second monotonic lifetime. Five challenges/minute/installation and a stricter global twenty/minute budget bound state and abuse.
- `POST /v1/sessions`: Ed25519 proof of the proposal's exact UTF-8 `TBP1\n<board>\n<installation>\n<challenge>\n<nonce>\n<expires_at>\n` message. Every attempt consumes its challenge, including invalid signatures. Revocation/generation are rechecked transactionally before issuance.
- Sessions: opaque 256-bit bearer tokens, 15-minute absolute lifetime, two-minute idle timeout, maximum two active sessions. Native tokens remain memory-only. Restarting the Companion invalidates sessions/challenges; Linux reboot also invalidates boot-bound tokens. Installation registrations persist.
- Session rotation: a fresh signed proof may carry the previous session in a sensitive `Authorization` header. A valid same-installation replacement invalidates the old token and transfers only an existing unexpired control lease under the server control lock, without extending it or rearming outputs. Another installation/session cannot silently take a live lease.
- `DELETE /v1/sessions/current`: terminates the current session and its control authority. Native disconnect closes it after best-effort output clearing.
- Detailed GETs, bounded Commands, leases, self-tests and confirmations require active authentication. Only the minimal public health resource is anonymous. Existing polling remains; WSS is not implemented in this slice.

The private SQLite registry stores public keys, registration metadata, and **hashes**, never plaintext Pairing/session tokens or Mac private keys. Its directory is 0700 and DB is 0600; boot/monotonic checks prevent wall-clock rollback from reviving credentials. There are at most sixteen non-revoked registrations; recent revoked metadata is bounded. SQLite/content/version/identity errors fail closed, never reset the store silently.

## Revocation and SSH boundaries

To revoke a lost Mac: select the physical Board, list installation labels/IDs, select one, and type `REVOKE <installation UUID> ON <SoC serial>`. The native adapter rechecks the current SoC, preventing an old list/selection from revoking the same installation ID on a swapped Board.

Revocation first disables the installation/generation and deletes its sessions/challenges/tokens. Only then does it remove that installation's `tinkercrew:<UUID>` SSH line. If filesystem cleanup fails, the response explicitly reports `ssh_key_removed=false`: authorization remains revoked, and a leftover forced key checks registry authorization before doing anything. Unrelated SSH keys/configuration and Mac Mission progress are untouched.

The independent server watchdog checks controlling-session validity every 50 ms and clears output authority after revocation/expiry even if no further request arrives. The STM32 output lease remains the physical backstop; measured physical safe-off is still pending. Database errors deny authentication rather than leaving it permissive.

Dedicated SSH keys are currently **forced-command, restricted identity-inspection credentials only**. They do not grant interactive shells, forwarding, SFTP, or permission to enroll/revoke installations over SSH. Password-based existing-Board management remains separate. Dedicated-key SSH login verification/maintenance remains #11 work; merely writing an authorized-key line does not prove SSH works. A separate [bounded USB Linux runtime updater](runtime-updates.md) now supports compatible installed baselines without granting SSH deployment or firmware privileges.

**Forget local Board record** is still not remote revocation. It removes only this Mac's saved connection record. TBP revocation uses the physical operation above; legacy bearer credentials require explicit Board-side rotation/migration.

## Tests and remaining frontier

Agreed seams: Companion HTTPS API, fixed management/deployment interface, and Board Setup user flow. Implemented in vertical slices with real loopback TLS and temporary registries/SSH files: token replay/expiry/attempt budgets, pending activation, invalid/single-use/expired proofs, absolute/idle session expiry, lease exclusion/rotation, immediate revocation and failed SSH cleanup, preserved unrelated keys, mode separation, and secret-free logs/storage. Native Rust generates actual keys and interoperates with the Python TLS peer; tests never touch live user Keychain or actual ADB/SSH. Browser checks preserve the completion frontier.

Still unfinished: supported first-boot account/image mapping and initial credentials; Board TLS/identity provisioning and SPKI-preserving certificate renewal; verified dedicated-key SSH use; initial/whole-app Companion installation and compatible baseline upgrades (the separate [Linux-only updater](runtime-updates.md) now covers integrity-checked runtime transfer, resume and rollback); observed reboot/LAN+USB validation; wiring actual hardware self-test/Crew creation into completion; WSS and the rest of the broader TBP proposal. No real Board GPIO, Linux configuration, reboot, deployment, or destructive reset was executed during this work. #11 and every applicable physical checklist item in #21 remain open.
