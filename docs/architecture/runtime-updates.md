# Recoverable Companion Linux runtime updates (#11)

Status: **local software tests only; actual UNO Q/container/USB compatibility NOT RUN**. This is an incremental installed-baseline updater, not first installation, firmware delivery, full TBP deployment, or setup completion. Physical acceptance remains #21. No real Board was configured, deployed, rebooted, reset, or flashed during implementation.

## Deliberately narrow scope

Board Setup → **Recoverable Linux runtime update** accepts only the runtime compiled into the Mac app. There is no archive picker, arbitrary path/program/shell IPC, HTTP deployment route, package installer, or Arduino CLI invocation. Dedicated-key SSH remains inspection-only; this update protocol is **physical USB only**.

The six packaged Python modules are `__init__`, `auth`, `core`, `discovery`, `hardware`, and `server`. `auth.py` is additionally pinned in the baseline contract: changing registry/authentication implementation requires a separate supported baseline upgrade, not a rollback across unknown database migrations. The contract also hashes the STM32 sketch/metadata, app YAML, dependency requirements, fixed entry points, management helper, supervisor, and deployment implementation. A mismatch fails before upload/promotion. Credentials, certificate/private key, Board UUID, registrations, and dependency/firmware files are not replacement targets. Normal server restart invalidates transient sessions, not registrations.

This boundary follows official [Arduino App CLI v0.14.0 orchestration](https://github.com/arduino/arduino-app-cli/blob/v0.14.0/internal/orchestrator/orchestrator.go): starting/restarting an app can compile/upload its sketch, and stopping can reset the MCU. It is not a safe Linux-only update primitive. A separate Python supervisor owns exactly one server child instead. The source review is not hardware compatibility evidence or a claim that every installed CLI has that version/behavior.

## Prerequisites and consent

- An already installed, compatible baseline whose `python/main.py` starts the supervisor; a stable, explicitly TBP-configured TLS identity; a registered Mac profile and available Keychain keys.
- The fixed host helper at `/home/arduino/ArduinoApps/tinkercrew-companion/python/management.py`, running as the supported **unprivileged** owner, and the app child must see the same private `data` tree, kernel boot ID, monotonic clock, and code bytes. Actual Arduino host/container mounts, UID ownership, Python/dependency and Router Bridge behavior are **unverified**. Unknown topology is an unsupported-baseline handoff, never a guessed privileged command.
- Explicit local physical USB inspection/confirmation, exact saved ADB serial and Board UUID, and typed `UPDATE <SoC serial>`. Existing bearer profiles do not silently migrate. The native adapter also requires authenticated, disarmed baseline health before a new update.
- First installation, account/OS mapping, TLS provisioning/renewal, firmware/dependency/schema changes, missing credentials, and a baseline that cannot serve authenticated health still require separate supported setup/recovery. The wizard's stage-8 completion gate stays closed.

## Fixed management protocol

Every operation rechecks UNO Q/Debian and the expected hardware serial. Forced SSH rejects every `runtime_*` operation, including status and cancellation. Native code verifies the returned Board UUID, SoC and protocol on each update step.

| Operation | Purpose |
| --- | --- |
| `runtime_status` | Report current selection, job phase/durable offset and supervisor observation; not a hardware-health assertion. |
| `runtime_begin` | Bind release, baseline contract, exact payload SHA-256 and size; resume matching partial bytes/stage/trial with a fresh boot-bound, ten-minute capability. Already-current is idempotent. |
| `runtime_chunk` | Append at the durable offset; maximum 8 KiB decoded bytes, 2 MiB total payload. Data fsync precedes offset publication; an interrupted unacknowledged tail is truncated before retry. |
| `runtime_stage` | Verify exact payload hash, fixed file set, canonical content-derived release ID, baseline contract and Python syntax; materialize/sync private immutable release files and manifest. No activation. |
| `runtime_commit` | Record prior selection and a 45-second trial before atomic pointer publication. Never invoke build/upload/restart tooling. |
| `runtime_finish` | Accept only the currently observed trial's fresh supervisor nonce, matching boot/release/trial and unexpired deadline. |
| `runtime_abort` | Capability-bound transfer/trial abort. |
| `runtime_cancel` | Physical-owner recovery bound to the inspected job's exact release; permits cancellation after lost tokens or an app upgrade. It does **not** undo a committed update. |

SHA-256 is an integrity/content-identity check, **not vendor authentication or a signed release channel**. The code comes from the installed Mac application; production app/update signing remains #12. Physical management capability and pinned TLS authentication are separate boundaries.

## Supervision and acceptance

The stable supervisor polls/reconciles without a Mac connection. It owns only its `Popen` child, never kills a PID read from disk, and switches only validated runtime paths. Python is launched with `-B -X pycache_prefix=/dev/null`, so an unverified existing bytecode cache cannot bypass source verification. Child stdout/stderr are discarded; no secret-bearing trace is forwarded.

While private runtime metadata reports an upload, stage, unaccepted trial or unreadable state, the server denies gameplay mutations with `maintenance_active`. Its independent 50 ms watchdog clears existing control authority without another request, including authority held by a different paired Mac. Authenticated observation/session establishment remains available for parent health verification; resolving maintenance never renews a lease or rearms outputs.

A candidate receives a random process nonce. Authenticated `/v1/hello` reports its release and nonce. Native code uses the existing pinned-certificate, signed-TBP HTTPS adapter over a serial-specific USB forward and verifies Board UUID, protocol, expected release, reported Safe state, and agreement with the current supervisor observation. Only then does it acknowledge promotion over USB. This proves the tested software boundary, **not physical safe-off, working Modulinos, firmware compatibility, or a completed self-test**.

Crash, missing acknowledgement, deadline expiry, kernel reboot, interrupted pointer publication or failed runtime integrity verification selects a verified prior runtime. A committed child that subsequently exits also falls back to verified prior/baseline bytes. No verified fallback means **blocked**, never execution of unverified bytes. The publication journal records the actual prior current release, so interruption before publishing a second update does not accidentally select an older predecessor. The supervisor must itself be running; after supervisor/container loss, reconciliation occurs when it starts again. There is no promise that a dead supervisor can repair itself.

## Local ownership and interruption

`data/runtime/` is private (0700; files 0600), with bounded payloads, atomic JSON/fsync, file locks, no symlinked storage traversal, and a free-space check before a new upload. The Board stores only update capability hashes. Partial transfer/staged files are retained for explicit resume/recovery; this is not a general artifact garbage collector or protection against an unrelated process filling the disk.

The Mac keeps identifiers only in `runtime-update-active.json`, never capability/session tokens. An unresolved marker blocks gameplay and other parent mutations across app instances. A shared OS `board-management.lock` serializes parent operations (including restore/update), with existing recovery locks/markers retained. Native locks explicitly unlock at scope end: close-on-exec alone can briefly retain a lock through a child descriptor inherited between fork and exec. A deterministic duplicate-descriptor regression and twenty parallel native-suite reruns cover that lifecycle. Resume/abort uses its specific guarded entry point. An update for another Board or app build cannot silently overwrite pending ownership. Simultaneous older app versions that predate these guards are unsupported: quit other versions before maintenance.

Closing the UI/Mac does not claim success. Reconnect the same Board, inspect status, and explicitly resume the same bundle or type `ABORT UPDATE <SoC serial>` to cancel/recover. Cancellation clears local pending ownership only after authenticated disarmed active-runtime health is observed. Already committed trials are not undone. Corrupted ownership metadata is not silently reset. If credentials, the baseline, storage, or supervisor are broken, retain the unresolved status and use the explicit supported/official recovery handoff.

## Evidence and remaining work

Tests use temporary app/data/SSH trees, a real local supervisor/child, actual loopback TLS and Ed25519 sessions, plus native-package → Python-stager interoperability. Cases cover resume/offset ordering, incompatible firmware contract, corrupt payload hash, candidate crash, missing/stale acknowledgement, process-bound health, reboot identity change (injected boot seam, **not a real reboot**), interrupted second publication, successful promotion/restart, post-commit child failure, and server-side cross-client maintenance exclusion/safe-state entry. Browser tests intercept every native call and prove typed consent/IPC boundaries, not ADB or hardware behavior.

Remaining #11 engineering includes initial/whole-app installation and compatible baseline upgrades, verified dedicated-key SSH maintenance, identity/TLS provisioning and renewal, observed reboot plus LAN/USB validation, hardware self-test/Crew completion integration and broader TBP work. #21 remains unchecked until the owner supplies real evidence.
