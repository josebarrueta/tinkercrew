# TinkerCrew UNO Q Companion

This directory is one reusable Arduino App for every TinkerCrew Mission. The STM32 sketch owns time-sensitive Modulino I/O and output expiry. The Linux Python process uses Arduino Router Bridge and exposes a bounded authenticated API; it never opens `/dev/ttyHS1`, `Serial1`, or a shell endpoint.

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

For native Mac connections, provision a unique stable `board_id` (`TC_BOARD_ID`); do not regenerate it on each startup. The Mac verifies it after TLS/authentication. The certificate must include the fixed hostname used as `tlsName` in its DNS SAN. See [`docs/architecture/board-connections.md`](../docs/architecture/board-connections.md) for the current USB-verified record/Keychain development bridge and remaining full TBP integration.

Optional Bonjour announcements use `python/requirements.txt` (`zeroconf`) and `_tinkercrew._tcp.local.`. Discovery failure never blocks USB or manual IP. `TC_ADVERTISE_IPV4` can choose a local IPv4 address when automatic interface selection is unsuitable; `advertise_host` / `TC_ADVERTISE_HOST` can supply the `.local` service hostname. An unprovisioned simulator does not advertise a fake Board identity.

Optional keys are `board_version` (defaults to the explicit value `unknown`), `bind` (default `0.0.0.0`), and `port` (default `7443`). Corresponding `TC_*` variables override the file for managed deployments.

The service refuses network startup without TLS. For local tests only, use loopback plus `TC_DEVELOPMENT_HTTP=1`. TLS identity generation, USB trust bootstrap, and short-lived Ed25519 sessions are specified in `../docs/architecture/secure-board-protocol.md`; this minimum Companion currently consumes a provisioned bearer credential and certificate.

## API

All routes except `GET /v1/health/public` require `Authorization: Bearer …`.

- `GET /v1/hello` — versions, Board health, capabilities, limits.
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
cd companion
PYTHONPATH=python python3 -m unittest discover -s tests -v
```

Run the simulator and inspect it with a client:

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

Use `python3 tools/test_client.py https://<board>:7443 <token> --watch` after trusting the provisioned certificate. `--insecure` exists only for an attended bring-up test and must not be used by the TinkerCrew app.

These checks have not been performed in this development environment.
