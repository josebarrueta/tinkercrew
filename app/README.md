# Tinkeroo macOS app

Tauri 2 + Svelte/TypeScript foundation for the local-first family Crew experience.

## Prerequisites

- macOS 12 or newer
- Xcode Command Line Tools (`xcode-select --install`)
- current stable Rust toolchain
- Node.js 22.12+ recommended, or Node.js 20.19+ within the 20.x line
- Python 3.11+ for combined checks and release tooling

## Start

From the repository root:

```sh
cd app
npm ci
npm run dev:app
```

The initial app starts in Simulator mode and needs no UNO Q, cloud account, credentials, or internet after dependencies are installed.

## Checks and build

Install the pinned authentication test dependencies once from the repository root:

```sh
python3 -m venv .venv
.venv/bin/python -m pip install -r companion/python/requirements-auth.txt
```

Then, from `app/` (or use `bash scripts/check-mvp.sh --build` at the root):

```sh
source ../.venv/bin/activate
npm run quality
cargo test --manifest-path src-tauri/Cargo.toml
npx playwright install chromium # once per development machine
npm run test:e2e
npm run build:app
```

The app command creates `src-tauri/target/release/bundle/macos/Tinkeroo.app`. `npm run build:dmg` also creates a headless drag-to-Applications DMG under `src-tauri/target/release/bundle/dmg/`. Development artifacts use ad-hoc signing, not a public Developer ID identity. Hardened Runtime is enabled. The manual protected release workflow prepares Universal signed/notarized candidates only when owner-provided credentials exist; actual notarization and clean-Mac acceptance remain pending. See [`docs/release/macos.md`](../docs/release/macos.md).

## CI downloads and licensing

[macOS CI](../.github/workflows/ci-macos.yml) runs the checks and native-architecture app/DMG build without signing secrets. Download development bundles from the run's Artifacts section (14-day retention); QA reports and failed browser diagnostics are retained for 7 days. They are not production-signed/notarized installers. See the root [installation and CI guidance](../README.md#install-the-macos-app).

Original project code/documentation uses the root [MIT License](../LICENSE); upstream dependencies and tools retain their own licenses. Generated binaries are not committed to git.

## Theme and compatibility

The Tinkeroo theme uses shared tokens in `src/lib/theme.css`: calm surfaces, one purple next action, small playful accents, visible keyboard focus, and reduced-motion support. Crew editing is expanded during the roles stage and tucked away otherwise to keep the current goal prominent. See the [design guide](../docs/design/tinkeroo-theme.md).

The display/package/artifact brand is Tinkeroo; internal `com.tinkercrew.desktop`, Keychain/discovery/protocol names, Rust crate names, and progress storage keys stay unchanged to preserve existing state and Board connections.

## Architecture boundary

UI and Mission code depend on the `BoardClient` interface in `src/lib/board/types.ts`. `SimulatorBoardClient` and `ConnectedBoardClient` deliver inputs through the same event subscription. Domain/UI code must not execute shell commands.

Mission definitions are versioned, schema-validated local JSON in `src/lib/missions/`. The intentionally limited `MissionSession` supports human-confirmed creative stages, named live checks, three progressive hints, independent Crew roles/tracks, remix, reflection, and completion records. Other World Map adventures are explicitly marked as future content; Free Build invitations are available now.

The native app stores progress in `crew-progress.sqlite` inside its macOS application-data directory. Saves are atomic and revision-checked so a stale window cannot overwrite newer progress. Mission/version validation refuses incompatible or corrupt saves rather than silently resetting them. Browser-only development uses localStorage for **non-secret progress only**, separate from native SQLite saves. Live hardware evidence is never restored as if newly observed after a restart.

Connected Board records and credentials use macOS Keychain, never SQLite or frontend storage. The Mission engine never collects credentials. Board Setup retains the explicit USB-verified provisioned-record import for legacy development and adds installation-key registration/signed sessions for an already installed TBP-configured Companion. Global gameplay/SSH seeds live only in Keychain; temporary session tokens stay native-memory-only. Complete wizard/SSH/deployment integration remains unfinished. See [`installation-pairing.md`](../docs/architecture/installation-pairing.md). See [`docs/architecture/board-connections.md`](../docs/architecture/board-connections.md) for transport, trust, diagnostics, and pending physical acceptance.

## Board Setup

Board Setup adds serial-targeted physical USB/SSH authentication with request-only Linux credentials, hostname/Wi-Fi configuration, scheduled restart, explicit local credential/fingerprint cleanup, and guarded official-tool Linux factory restore. The destructive path binds the exact SoC serial, image version/SHA-256, flasher 0.5.4 contract, typed erase, and every safety acknowledgement; boot and human-reported external recovery are distinguished. Native management pauses/disconnects gameplay and never provides Mission code a shell endpoint. Physical acceptance remains pending; no reset was executed during development. See [`docs/architecture/board-setup.md`](../docs/architecture/board-setup.md).

Board Setup also includes a resumable USB identity/official-tool handoff guide, with checkpoint records in Keychain and explicit observed versus human confirmation. It stops honestly at unsupported full Pairing/deployment instead of allowing fake completion. Already provisioned connections remain separate. See [`docs/architecture/setup-checkpoints.md`](../docs/architecture/setup-checkpoints.md) for implemented behavior and missing integrations; factory-first provisioning is not complete.

## Treasure Guardian

Open **Mission** to play the ten-stage adventure with three named Crew members and independently selectable tracks. Rotate roles any time. Use the live simulator controls to move through sleeping/curious/alert ranges and A/B/C to arm, silence, and disarm. The self-test requires distance movement, all three buttons, and human confirmation of the test light/tone. The fresh behavior challenge cannot be passed using evidence from previous stages.

Inventor controls adjust thresholds, warning color, and alarm duration. The Hacker panel exposes the bounded six-field rule JSON and a rolling input event log; it never executes arbitrary code. A tested change from the default rules plus a description and reflection are required to complete the Mission. Saved configuration resumes on restart, but the guardian always restarts disarmed and live checks must collect new evidence.

`GuardianRuntime` accepts only `BoardClient`, so the same Mission behavior runs over simulator and connected adapters. Select a provisioned Board in Board Setup for connected play; otherwise the shell uses the simulator. Physical connected-mode acceptance remains pending until tested on UNO Q.

The runtime rejects stale/implausible distance readings, clears outputs on failure or exit, and requires explicit rearming after recovery. The simulator models the 1.5-second control lease and bounded buzzer duration; use **Freeze distance readings** to explore recovery. The Companion's Linux/STM32 watchdogs are the physical safety authority, not frontend timers.

Automated coverage includes a browser play-through (self-test, automatic checks, remix, reflection, reload/resume, and blocked external resources), deterministic runtime safety tests, definition/progress validation, and native SQLite reopen/conflicting-writer tests. These checks are not a family play test or physical hardware validation.
