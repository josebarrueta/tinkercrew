# Tinkeroo UNO Q Companion

This directory is one reusable Arduino App for every Tinkeroo Mission. The STM32 sketch owns time-sensitive Modulino I/O and output expiry. The Linux Python process uses Arduino Router Bridge and exposes a bounded authenticated API; it never opens `/dev/ttyHS1`, `Serial1`, or a shell endpoint.

## Hardware

Connect Modulino Buttons, Pixels, Buzzer, and Distance through the UNO Q Qwiic connector. UNO Q maker I/O is **3.3 V only**. The LED matrix is onboard.

## Install and start on UNO Q

Copy this directory to `/home/arduino/ArduinoApps/tinkercrew-companion`, then use the version of `arduino-app-cli` installed on the Board:

```sh
arduino-app-cli app build tinkercrew-companion
arduino-app-cli app start tinkercrew-companion
```

Run `arduino-app-cli --help` on the Board if its installed release uses different app subcommand spelling. The checked-in `app.yaml`, `python/main.py`, and `sketch/sketch.yaml` follow the official Arduino App CLI directory format.

Before starting, provision owner-readable `data/companion.json` on the Board (do not commit or export it):

```json
{
  "api_token": "at-least-32-random-characters",
  "board_id": "stable-random-uuid-created-once-during-provisioning",
  "board_version": "uno-q-hardware-revision-from-setup",
  "tls_cert": "data/board.crt",
  "tls_key": "data/board.key"
}
```

For native Mac connections, provision a unique stable `board_id` (`TC_BOARD_ID`); do not regenerate it on each startup. The Mac verifies it after TLS/authentication. The certificate must include the fixed hostname used as `tlsName` in its DNS SAN. See [`docs/architecture/board-connections.md`](../docs/architecture/board-connections.md) for the legacy USB-verified record bridge, separate installed-TBP registration and hybrid guide, and remaining physical acceptance.

Optional Bonjour announcements use `python/requirements.txt` (`zeroconf`) and `_tinkercrew._tcp.local.`. Discovery failure never blocks USB or manual IP. `TC_ADVERTISE_IPV4` can choose a local IPv4 address when automatic interface selection is unsuitable; `advertise_host` / `TC_ADVERTISE_HOST` can supply the `.local` service hostname. An unprovisioned simulator does not advertise a fake Board identity.

Optional keys are `board_version` (defaults to the explicit value `unknown`), `bind` (default `0.0.0.0`), and `port` (default `7443`). Corresponding `TC_*` variables override the file for managed deployments.

The service refuses network startup without TLS. For local tests only, use loopback plus `TC_DEVELOPMENT_HTTP=1`. Existing deployments retain explicit provisioned-bearer mode. Updated deployments can opt into `auth_mode: "tbp"`, a stable UUID, `tls_name`, and the existing certificate/key paths; remove `api_token`/`TC_API_TOKEN` to avoid a rejected legacy bypass. Install `python/requirements.txt` in the runtime. The host `python/management.py` and app runtime must share the private `data` registry and kernel boot/monotonic identity; actual Arduino ownership/mount compatibility is still unverified. The host helper refuses root-account enrollment and does not install itself or generate TLS identity.

TBP mode adds USB-token-bound installation registration, restricted/tagged SSH keys, signed single-attempt Ed25519 session challenges, bounded opaque sessions, and physical revocation. Read the [implementation and limitations](../docs/architecture/installation-pairing.md) before attempting hardware bring-up. This is not completion of first-run provisioning or the full TBP proposal.

## Recoverable Linux-only updates

`python/main.py` now starts a stable local supervisor, which owns one server child. After a compatible baseline is explicitly installed, registered and running, Board Setup can transfer/resume only its compiled-in runtime, verify fresh authenticated disarmed health and commit a trial. An unacknowledged/crashing trial rolls back independently of the Mac; no Arduino CLI, sketch upload, dependency installation, TLS replacement or registration reset is performed. Authentication/registry code is pinned by the baseline contract. Unknown baseline/ownership compatibility fails closed.

The installation commands above are **not** this update primitive: official app start/stop/build can affect the MCU. Read the [runtime update contract, recovery and limitations](../docs/architecture/runtime-updates.md) before hardware use. Actual host/container and physical update acceptance is still NOT RUN; initial account/OS setup and first firmware startup stay official-tool handoffs. The separate hybrid guide now verifies dedicated-key SSH and protects later setup evidence; whole-baseline migrations beyond the prepared contract remain unsupported.

## Initial files, identity and renewal

On a supported configured Debian 13 / `arduino` UID-1000 Board, native Board Setup can prepare only its fixed initial source and private TBP identity into an absent app directory. Existing app/configuration/key data is not overwritten. The owner then explicitly starts the app in official App Lab; start may upload the sketch or stop another app, so preparation never claims that firmware/start succeeded. Pairing now verifies the restricted dedicated SSH key against a USB-read host identity before remembering the Board. Prepared certificates can renew with the same key; the supervisor detects the certificate change and restarts only its owned Linux child. Read the [exact preparation constraints](../docs/architecture/initial-provisioning.md) and [integrated guide evidence contract](../docs/architecture/setup-checkpoints.md). None of this has been validated on a real UNO Q.

## API

Gameplay routes except `GET /v1/health/public` require `Authorization: Bearer …`. In TBP mode this is a short-lived session token; legacy mode uses the provisioned token. Only TBP mode exposes token-protected `POST /v1/pairings` and signed authentication via `POST /v1/sessions/challenge` / `POST /v1/sessions`; it also supports `DELETE /v1/sessions/current`. Begin/activate/revoke remain fixed host-side management operations, not gameplay endpoints.

- `GET /v1/hello` — versions, Board health, capabilities, limits, and authenticated supervisor release/process nonce (not a physical self-test).
- `GET /v1/state` — latest input health/value/sample age and bounded requested output state (expired commands are reported off; this is not a physical light/sound measurement).
- `GET /v1/events?after=<sequence>` — ordered button edges and distance samples.
- `POST /v1/control-lease` — renew the maximum two-second control lease.
- `POST /v1/commands` — `pixels.set`, `buzzer.play`, `matrix.draw`, or `outputs.clear`.
- `POST /v1/self-tests` — run bounded light/sound output.
- `POST /v1/self-tests/<id>/confirmation` — record human `lights` and `sound` confirmation.

The STM32 and Linux watchdogs clear pixels and matrix state, stop the buzzer, and cancel output deadlines when control is lost. Inputs become `stale` after 750 ms without a report and `disconnected` when the sketch reports failed discovery.

## Local automated test

No Board is needed for the deterministic core and HTTP adapter tests:

```sh
# From the repository root; tests never touch hardware or live SSH/Keychain files.
python3 -m venv .venv
.venv/bin/python -m pip install -r companion/python/requirements-auth.txt
PYTHONPATH=companion/python .venv/bin/python -m unittest discover -s companion/tests -v
```

Run the simulator and inspect it with a client (from `companion/`; the token below is a public local-test fixture):

```sh
TC_SIMULATE_HARDWARE=1 \
TC_API_TOKEN=0123456789abcdef0123456789abcdef \
TC_BIND=127.0.0.1 TC_DEVELOPMENT_HTTP=1 \
PYTHONPATH=python python3 python/main.py

curl -H 'Authorization: Bearer 0123456789abcdef0123456789abcdef' \
  http://127.0.0.1:7443/v1/hello
```

## Physical acceptance test (pending)

On an UNO Q, confirm rather than infer:

1. The app builds and starts through `arduino-app-cli`.
2. All four Modulinos report `operational`; unplugging Buttons or Distance eventually reports `disconnected` or `stale` as appropriate.
3. Button edges and live distance samples appear through `/v1/events`.
4. In-range pixel, buzzer, and matrix commands work; out-of-range commands do not actuate hardware.
5. Stop lease renewal or disconnect the client and verify all outputs enter Safe state within two seconds.
6. Start self-test and have a human confirm visible light and audible sound before recording a pass.

Use `python3 tools/test_client.py https://<board>:7443 <token> --watch` after trusting the provisioned certificate. `--insecure` exists only for an attended bring-up test and must not be used by the Tinkeroo app.

These checks have not been performed in this development environment.
