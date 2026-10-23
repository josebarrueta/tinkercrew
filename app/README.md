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
npm run build:app
```

The production command creates `src-tauri/target/release/bundle/macos/TinkerCrew.app`. Development bundles are unsigned. Tauri's hardened runtime is enabled and the signing identity remains unset until issue #12 configures Developer ID signing and notarization.

## Architecture boundary

UI and Mission code depend on the `BoardClient` interface in `src/lib/board/types.ts`. `SimulatorBoardClient` is the first adapter and sends inputs through the same event subscription used by a future connected-Board adapter. Domain/UI code must not execute shell commands.

Mission definitions are versioned, schema-validated local JSON in `src/lib/missions/`. The intentionally limited `MissionSession` supports human-confirmed creative stages, named live checks, three progressive hints, independent Crew roles/tracks, remix, reflection, and completion records. Other World Map adventures are explicitly marked as future content; Free Build invitations are available now.

The native app stores progress in `crew-progress.sqlite` inside its macOS application-data directory. Saves are atomic and revision-checked so a stale window cannot overwrite newer progress. Mission/version validation refuses incompatible or corrupt saves rather than silently resetting them. Browser-only development uses localStorage for **non-secret progress only**, separate from native SQLite saves. Live hardware evidence is never restored as if newly observed after a restart.

Credentials will use macOS Keychain and must never be stored in SQLite or frontend storage. No credentials are collected by the Mission engine.
