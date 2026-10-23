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

No database is included yet: this scaffold has no durable domain state to store. SQLite should be introduced with the first resumable Mission/progress repository rather than as an unused dependency. Credentials will use macOS Keychain and must never be stored in SQLite or frontend storage.
