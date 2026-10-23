# TinkerCrew macOS app

Tauri 2 + Svelte/TypeScript foundation for the local-first family Crew experience.

## Prerequisites

- macOS 12 or newer
- Xcode Command Line Tools (`xcode-select --install`)
- current stable Rust toolchain
- Node.js 20.19 or newer (or Node.js 22.12+)

## Start

From the repository root:

```sh
cd app
npm install
npm run dev:app
```

The initial app starts in Simulator mode and needs no UNO Q, cloud account, credentials, or internet after dependencies are installed.

## Checks and build

```sh
npm run quality
cargo test --manifest-path src-tauri/Cargo.toml
npx playwright install chromium # once per development machine
npm run test:e2e
npm run build:app
```

The production command creates `src-tauri/target/release/bundle/macos/TinkerCrew.app`. Development bundles are unsigned. Tauri's hardened runtime is enabled and the signing identity remains unset until issue #12 configures Developer ID signing and notarization.

## Architecture boundary

UI and Mission code depend on the `BoardClient` interface in `src/lib/board/types.ts`. `SimulatorBoardClient` is the first adapter and sends inputs through the same event subscription used by a future connected-Board adapter. Domain/UI code must not execute shell commands.

Mission definitions are versioned, schema-validated local JSON in `src/lib/missions/`. The intentionally limited `MissionSession` supports human-confirmed creative stages, named live checks, three progressive hints, independent Crew roles/tracks, remix, reflection, and completion records. Other World Map adventures are explicitly marked as future content; Free Build invitations are available now.

The native app stores progress in `crew-progress.sqlite` inside its macOS application-data directory. Saves are atomic and revision-checked so a stale window cannot overwrite newer progress. Mission/version validation refuses incompatible or corrupt saves rather than silently resetting them. Browser-only development uses localStorage for **non-secret progress only**, separate from native SQLite saves. Live hardware evidence is never restored as if newly observed after a restart.

Connected Board records and credentials use macOS Keychain, never SQLite or frontend storage. The Mission engine never collects credentials. Parent Settings provides an explicit USB-verified provisioned-record import for development; full TBP Pairing remains integration work. See [`docs/architecture/board-connections.md`](../docs/architecture/board-connections.md) for transport, trust, diagnostics, and pending physical acceptance.

## Treasure Guardian

Open **Mission** to play the ten-stage adventure with three named Crew members and independently selectable tracks. Rotate roles any time. Use the live simulator controls to move through sleeping/curious/alert ranges and A/B/C to arm, silence, and disarm. The self-test requires distance movement, all three buttons, and human confirmation of the test light/tone. The fresh behavior challenge cannot be passed using evidence from previous stages.

Inventor controls adjust thresholds, warning color, and alarm duration. The Hacker panel exposes the bounded six-field rule JSON and a rolling input event log; it never executes arbitrary code. A tested change from the default rules plus a description and reflection are required to complete the Mission. Saved configuration resumes on restart, but the guardian always restarts disarmed and live checks must collect new evidence.

`GuardianRuntime` accepts only `BoardClient`, so the same Mission behavior runs over simulator and connected adapters. Select a provisioned Board in Parent Settings for connected play; otherwise the shell uses the simulator. Physical connected-mode acceptance remains pending until tested on UNO Q.

The runtime rejects stale/implausible distance readings, clears outputs on failure or exit, and requires explicit rearming after recovery. The simulator models the 1.5-second control lease and bounded buzzer duration; use **Freeze distance readings** to explore recovery. The Companion's Linux/STM32 watchdogs are the physical safety authority, not frontend timers.

Automated coverage includes a browser play-through (self-test, automatic checks, remix, reflection, reload/resume, and blocked external resources), deterministic runtime safety tests, definition/progress validation, and native SQLite reopen/conflicting-writer tests. These checks are not a family play test or physical hardware validation.
