# Initial Companion preparation, dedicated SSH and certificate renewal (#11)

Status: **hybrid preparation/identity and wizard evidence software implemented; real UNO Q compatibility NOT RUN**. The [resumable guide](setup-checkpoints.md) now consumes observed Pairing/runtime/restart/hardware evidence and persisted Crew confirmation; preparation alone never completes it. Unsupported whole-baseline migrations remain separate #11 work. Physical/family verification remains #21. No live Board was configured, paired, deployed, rebooted, reset or flashed during development.

## Supported hybrid boundary

First-time account/password/image configuration stays in official Arduino App Lab. Source review of App CLI v0.14.0 establishes that app start can compile/upload the STM32 sketch and stop another app; it is not a Linux-only transaction. Tinkeroo therefore **prepares files and identity only**, then explicitly hands first firmware startup to the owner in official tooling. The helper has no start, stop, build, reboot, flash, shell or arbitrary-path operation.

Board Setup → **Prepare initial Companion files and identity** requires a freshly inspected physical USB UNO Q, matching SoC, Debian 13, account `arduino` UID 1000 and the fixed `/home/arduino/ArduinoApps` location. Unknown accounts/images/permissions fail closed; there is no root/sudo fallback or guessed credential. Host/container shared ownership, mounts, Python/dependency and Bridge compatibility still need P01/P02 evidence.

1. Type `PREPARE <SoC serial>` after physical USB confirmation.
2. The native app sends only its compiled-in sixteen-file baseline: app YAML, sketch and metadata, Python entry points/requirements and fixed Companion modules. No frontend archive, executable or destination path is accepted.
3. A private namespace holds boot-bound ten-minute capability hashes, bounded 8 KiB chunks (2 MiB total), exact payload SHA-256 and canonical per-file content identity. File fsync precedes durable offset updates; retry truncates an unacknowledged tail. Unknown/corrupt metadata is not silently reset.
4. Verify the exact file set, hashes and Python compilation before materializing/syncing source and receipt. Journal intent precedes atomic publication into the **absent** reserved app directory. Existing/modified/different app trees are never overwritten, stopped or removed. A publication interruption resumes from verified bytes/receipt.
5. Generate a Board UUID and fixed DNS identity once, journaling ownership before any key generation. OpenSSL 3 generates a private P-256 key on the Board and a one-year self-signed server certificate. Key/config/data permissions are private; the private key never enters a response, Mac storage, environment or command argument. The app receives public certificate/identity/fingerprint only.
6. Existing data/configuration without the owned journal, legacy bearer configuration, missing keys or changed identity fail with a handoff. Retries reuse the identity/key, never silently rekey or migrate credentials.
7. The owner reviews the selected app, backups, dependencies and disconnected unsafe loads, then starts it in official App Lab. **Preparation is not proof of an app running, firmware compatibility, Pairing, hardware readiness or setup completion.**

`ABORT PREPARE <SoC serial>` resolves only the inspected partial transfer and invalidates its capability. It never deletes already published app files or private identity. A different/unsupported whole-baseline upgrade still uses the explicit supported official-tool handoff; compatible Linux runtime updates remain the separate [runtime-only path](runtime-updates.md). Neither path may be described as an atomic Linux+MCU firmware rollback.

## Dedicated-key SSH verification

Pairing retains independent gameplay/SSH seeds in Mac Keychain. SSH uses an in-memory standard OpenSSH Ed25519 private-key encoding; there is no private key file, agent/password fallback, forwarding permission or SFTP/shell access.

After registration activation, native code reads the fixed **public** Ed25519 host key through an exact-SoC USB management operation, negotiates only that SSH host algorithm over a serial-specific temporary ADB forward, and compares the live peer before authenticating. It checks any existing installation SSH pin (`com.tinkercrew.desktop.installation-ssh`) and fails closed on changed pins or locked Keychain. A restricted forced-command `inspect` must prove the same account, SoC, Board UUID and TLS identity. Only then is its pin saved and the Board record remembered after signed HTTPS verification.

A missing/disabled/unsupported SSH service is a recoverable error, not proof that writing `authorized_keys` worked. Enable SSH explicitly through supported parent/official controls, then retry with the same installation keys. The dedicated key remains identity-inspection-only; it does not gain deployment, enrollment, revocation or OS-management privileges. Physical revocation still disables the registration before SSH-file cleanup. Explicit parent trust cleanup clears both this installation pin and the existing password-management pin; unrelated keys/configuration remain untouched.

## Same-key certificate renewal

**Renew prepared Board certificate (same key)** requires the selected registered profile, exact USB/SoC and `RENEW TLS <SoC serial>`. It supports only the owned prepared identity and compatible current supervised baseline; pending runtime transfers/trials are rejected under the shared Board operation lock.

The helper validates the current certificate/key/config, signs a replacement certificate with the **existing** private key, verifies its identity and publishes the leaf atomically. It does not rotate keys or registrations. The stable supervisor detects the public certificate hash change and restarts only its owned Linux child, assigning a fresh process nonce and invalidating transient sessions. No Arduino CLI, MCU or OS reboot occurs.

Native code compares the old/current/new public keys locally and retains the **old** TLS anchor for the post-renewal signed-session check. A fresh disarmed child must report the expected loaded-certificate hash and a different process nonce. Only after that verification is the saved certificate refreshed. Failure reports an unresolved outcome rather than accepting a replacement trust key. Both real loopback TLS and native Rust tests cover preserved-key renewal; physical restart/safe-off behavior remains unverified.

Certificate responses are normalized to X.509 PEM only, so an accidentally concatenated private PEM key is not exported by either provisioning or Pairing inspection.

## Tests and remaining integration

Public fixed-interface tests use temporary files, real OpenSSL, an injected physical identity/UID seam and no Board commands. Cases include transfer resume, interruption before publication, wrong target/root refusal, preserved unrelated/legacy data, cancellation, idempotent identity, same-key renewal and pending-runtime exclusion. Native compiled packages interoperate with the actual Python provisioner. Native TLS keeps the prior certificate pin against the renewed service; supervisor tests verify a new authenticated process after certificate replacement. Browser tests intercept every native call and exercise the complete UI/IPC flow without claiming a real initial start, hardware result or physical wizard acceptance. SSH encoding/management boundaries are tested; actual UNO Q dedicated-key login is still NOT RUN, not an independent security audit.

For local software checks, add OpenSSL 3 to PATH (for example `brew install openssl@3` and the formula's documented PATH), install the pinned Python auth dependencies and run `scripts/check-mvp.sh`. CI prepares this test dependency explicitly; normal installed Mac operation does not require local OpenSSL/Python for Board identity generation.

The [guide evidence contract](setup-checkpoints.md) documents the implemented protected transitions, tracked kernel/LAN/USB restart, bounded hardware witness and persisted Crew completion. Supported whole-baseline/schema/dependency migrations outside this narrow contract remain separate #11 engineering; account/OS setup and first firmware start retain official handoffs. No prepared/connected icon or checkbox substitutes for actual proofs. Real UNO Q compatibility, reset-Board and family acceptance remain NOT RUN.
