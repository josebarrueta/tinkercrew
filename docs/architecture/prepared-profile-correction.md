# Owned, unstarted sketch-profile correction (#29)

Status: software implements a **single allowlisted source correction**, not a general baseline updater, app starter, or Linux+MCU transaction. Initial preparation remains absent-target-only. Read [the actual compile-only evidence](../qa/uno-q-2026-10-26-compile-only.md).

## Exact supported case

- Physical USB and SoC match; `arduino,imola`, Debian 13, account `arduino`, UID 1000; reserved `/home/arduino/ArduinoApps/tinkercrew-companion` only.
- Original owned sixteen-file release **`63e90d374bc27c69b2fd810f851fed9a51d00abaaa82ce6369ba4b7cac7feef3`**, with a completed initial publication journal and valid owned TBP identity.
- Corrected release **`d3c762374833ecce711fdd4a0b3c2727a846c443a34c2ba426de457610c6b6fe`**: only `sketch/sketch.yaml` changes, to the exact compile-validated Zephyr 0.55.0 / fourteen-library profile. All other source bytes must still match the original release. A future different compiled package is **not** implicitly authorized.
- The complete source/receipt/data layout is checked, not merely an app name. Data contains exactly the original identity journal, TBP configuration, private key and public certificate. Unknown files, `.cache`, registries/runtime state, symlinks, nonprivate/foreign storage, hardlinked files or modified sources fail closed.
- Bounded, fixed read-only App CLI `version` and `app list` calls require CLI **and daemon 0.9.0**, exactly one matching app ID and status **`uninitialized`** before staging, immediately before exchange, and before acknowledging completion. Calls use an absolute executable and a minimal fixed environment, including the expected home/apps directory; inherited Docker/CLI redirect variables are not accepted. This also depends on the exact no-cache/no-runtime layout, not a daemon status alone.

## Owner controls

Board Setup → Prepare initial Companion files and identity offers separate controls:

- `CORRECT PREPARED <SoC serial>`: apply or resume the supported correction.
- `RESTORE PREPARED <SoC serial>`: restore the exact original unstarted source, only while still uninitialized. **The original unversioned profile is not suitable for first startup**; correct it again before starting.

No frontend file, archive, destination, executable, dependency/version selection or shell text is accepted. Native code supplies its fixed package release and the USB-only helper operations `correct_prepared_profile` / `restore_prepared_profile`. The normal app maintenance permit disconnects gameplay and excludes competing Tinkeroo management work. Native code compares the same UUID, TLS name and normalized certificate before/after. Neither operation advances setup or starts/uploads firmware.

**Close official App Lab and do not issue other app start/build/stop commands during correction/recovery.** The Tinkeroo lock does not coordinate third-party tools or other machines. Fresh checks detect observed state changes and fail without stopping anything; they are not an assertion that arbitrary concurrent owner/root actions can be fenced. A concurrent official start can upload firmware independently and invalidate this unstarted-only procedure. Do not automatically roll back a running app.

## Durable publication and private recovery copy

1. Verify the complete old source and identity, including the existing key/certificate correspondence and expiry. Missing/legacy/unknown identity is refused; no key generation or certificate renewal is performed.
2. Durably record correction intent in private `.tinkercrew-provision/profile-correction.json` before staging. It binds the SoC and existing public identity/configuration digests, never private key contents.
3. Materialize a private complete sibling at `.tinkercrew-provision/prepared-profile-backup`. Source is the known old tree plus the fixed corrected profile. The four identity files are copied **only on the Board**, with the exact UUID/key/certificate/configuration preserved; only the identity journal's source-release binding changes. The staged source receipt binds the corrected full release.
4. Verify and fsync every staged file/directory, including identity consistency. Owner-only directory/file modes are 0700/0600. Staging recovery accepts only expected files containing exact known bytes or interrupted prefixes; unknown content is retained and refused, never deleted.
5. Persist `exchange_pending` before one **same-filesystem atomic directory exchange** (`renameat2(RENAME_EXCHANGE)` on Debian). There is no two-rename fallback and no missing-target gap. The app root receives a complete source/identity tree together; the private sibling receives the complete original. Mac temporary-tree tests use the equivalent native atomic filesystem exchange.
6. Sync both parents, independently verify actual current/backup trees and identity, and invalidate old transfer authority when recording the completed initial publication metadata. Only then persist `corrected`. An interruption before or after exchange resumes by verifying the actual directory contents, never trusting a guessed swap outcome. Pending correction blocks ordinary preparation/identity mutations.
7. Explicit restore similarly journals `restore_pending`, verifies both trees and identities, exchanges them atomically, and records `restored`. The newer copy remains private. Repeating an already completed operation verifies both current and retained trees before reporting success.

The recovery copy intentionally contains a duplicate **private Board key**, never a Mac export. Keep the private namespace intact; do not copy it into logs, backups shared publicly, git or frontend storage. There is no automatic cleanup, rekey, arbitrary rollback path or removal of unrelated apps/SSH keys/registrations. No Arduino compile/upload, app start/stop, flasher, OS/network change, reset, reboot or rearm is invoked.

## Evidence and remaining gates

Public fixed-interface tests cover exact legacy-to-current package compatibility, preserved key/certificate/UUID/configuration, unrelated files, interruption around exchange and partial staging, explicit restore, started/state-changing app refusal, unknown CLI versions, redirected environment exclusion, caches/registries, changed source, foreign staging, symlinks and hardlinks. Native compiled packages exercise the actual helper and real temporary filesystem/OpenSSL. UI tests intercept native IPC and verify separate typed confirmations; they are not physical acceptance.

Physical application must be reported separately from software tests. Even a successful file correction proves neither a running container nor MCU/Bridge/TLS/SSH/Pairing/GPIO behavior. First firmware-capable official startup needs its own owner approval. Keep the Board bare during correction; later connect Buttons/Distance/Pixels/Buzzer only with USB and all other power disconnected. #21 hardware/family acceptance and #25/#12 release gates remain separate.
