# Tinkeroo

**Build. Code. Imagine. Together.**

A local-first cooperative maker game for one family Crew: build LEGO creations, give them behavior with an Arduino UNO Q, and explore Missions together. The macOS app uses Tauri 2, Rust, and Svelte/TypeScript.

> **Development preview:** the current implementation is on `main`. Simulator play is available without hardware. Physical UNO Q/family acceptance, full first-time Pairing/deployment, and public release signing/notarization are not complete. No production installer has been validated yet.

## Try it without installing anything

Clone/download the development branch and open [`prototypes/treasure-guardian.html`](prototypes/treasure-guardian.html) directly in a browser. This throwaway prototype is one self-contained file: no account, backend, installation, or Board is required.

For family feedback, use the [play-test guide](prototypes/treasure-guardian-playtest.md) and [findings template](prototypes/treasure-guardian-findings.md). Automated checks are not a substitute for a real family play test.

## Install the macOS app

### Public releases — not available/validated yet

When an owner-approved signed/notarized release is published, download the appropriate `.dmg` from [GitHub Releases](https://github.com/josebarrueta/tinkercrew/releases), open it, and drag **Tinkeroo** to **Applications**. Public release preparation targets Universal 2 (Apple silicon and Intel), macOS 12+; actual Intel/clean-Mac acceptance remains pending.

Do not disable Gatekeeper or remove quarantine to declare an installation successful. Current development/CI artifacts are **ad-hoc signed, not Developer-ID-signed or notarized**, and may be blocked when downloaded. They are for developer testing, not a production installer. Build locally using the instructions below if you need to try the native app now.

### CI development downloads

After a successful run of [macOS CI](https://github.com/josebarrueta/tinkercrew/actions/workflows/ci-macos.yml), open the run's **Artifacts** section and download `tinkeroo-macos-ARCH-COMMIT` (GitHub sign-in may be required). Extract the Actions archive; it contains a DMG, an app ZIP that preserves bundle metadata/symlinks, and `SHA256SUMS.txt`.

CI builds the runner's **native architecture**, identified in the artifact name and DMG filename—not a validated Universal release. Check the workflow/ref/commit before downloading; PR artifacts are untrusted development outputs. Hashes detect corruption, not publisher authenticity. Production signature verification is a separate release gate.

Build downloads expire after **14 days**; QA reports and failed browser-test diagnostics after **7 days**. Re-run CI or build locally after expiry. GitHub Actions artifacts are temporary storage; approved public installers belong in GitHub Releases, not git or Git LFS.

## Build and run from source

### Prerequisites

- macOS **12 or newer**; development uses your Mac's native architecture.
- Xcode Command Line Tools: `xcode-select --install` (complete Apple's installation prompt).
- Current stable Rust with `cargo`, `rustfmt`, and `clippy`. With [rustup](https://rustup.rs/): `rustup component add rustfmt clippy`.
- Node.js **22.12+** recommended (Node **20.19+ within the 20.x line** is also supported).
- Python **3.11+** for the combined software checks and release tooling.
- Git. Internet is needed to obtain dependencies/browser binaries initially, not for installed simulator gameplay.

```sh
git clone https://github.com/josebarrueta/tinkercrew.git
cd tinkercrew/app
npm ci
npm run dev:app
```

The app starts in Simulator mode: no UNO Q, cloud account, or credentials are required. `npm run dev` launches the browser-only frontend instead; its non-secret localStorage progress is separate from the native app's SQLite progress.

### Create the app and installer

From `app/`:

```sh
npm run build:app  # native-architecture release-mode .app
npm run build:dmg  # .app plus headless drag-to-Applications DMG
```

Outputs:

- `app/src-tauri/target/release/bundle/macos/Tinkeroo.app`
- `app/src-tauri/target/release/bundle/dmg/Tinkeroo_VERSION_ARCH.dmg`

Open your local `.app`, or copy it to Applications for local testing. “Release mode” means an optimized build, **not** a production-signed/notarized release. The DMG script sets `CI=true` to avoid interactive Finder layout automation while preserving the Applications link.

For Universal builds, production signing/notarization, owner-provided secrets, detached update signatures, and clean-Mac gates, see the [release procedure](docs/release/macos.md). Routine CI never receives signing credentials, publishes releases, or updates a Board.

## Tests and CI

After `npm ci`, install the test browser once and run from the repository root:

```sh
cd app
npx playwright install chromium
cd ..
bash scripts/check-mvp.sh --build
```

This runs frontend/type/format/lint checks, Rust persistence and real loopback TLS tests, Python Companion tests, browser Mission/recovery tests, release metadata/signature checks, and an app build. Reports go to ignored `artifacts/qa/` and record toolchain versions and pass/fail results. To skip the app build, omit `--build`.

[macOS CI](.github/workflows/ci-macos.yml) runs on pushes, pull requests, and manual dispatch. It runs these software checks, builds the development DMG, verifies its read-only app/Applications-link layout, and retains the outputs described above. The separate [production release workflow](.github/workflows/release-macos.yml) is manual, protected, credential-dependent, and creates drafts only.

Physical Board, actual ADB/Bonjour/VPN, electrical safe-off timing, family play-testing, and notarization acceptance remain separate. See the [QA/recovery matrix](docs/qa/recovery-safety.md).

## Connect an Arduino UNO Q

Simulator mode is the easiest first step. For hardware play:

1. Complete supported first-time configuration in official Arduino App Lab; Tinkeroo's all-in-one factory provisioning is not complete.
2. Install/start the reusable [UNO Q Companion](companion/README.md) on the Board and provision its TLS identity and credential outside source control.
3. Follow the [USB-verified connection guide](docs/architecture/board-connections.md) and select the exact Board in Parent Settings. The current provisioned-record import is a development bridge, not full TBP Pairing.
4. For USB fallback, separately install official [Android Platform-Tools](https://developer.android.com/tools/releases/platform-tools) (Arduino documents `brew install android-platform-tools`). Tinkeroo does **not** bundle Google's binaries or accept their license for you. `TC_ADB_PATH` may point to an absolute `adb` executable.

UNO Q maker GPIO is **3.3 V**; never apply 5 V to analog inputs. Disconnect power before changing wiring. Follow the in-app safety panel and pending [physical acceptance plan](docs/qa/recovery-safety.md).

## Tinkeroo look and feel

![Tinkeroo Home Base with a calm lilac workspace and one clear Mission action](docs/design/tinkeroo-home.png)

Tinkeroo shares the playful “roo” family naming theme with Rallyroo. Its calm lilac/white workspace uses purple for the next action, teal for supportive cues, and small sunshine/coral accents. Hardware green/yellow/red signals and amber safety warnings keep their meaning. No flashing, autoplay, confetti, or external font downloads compete with building. See the [brand and accessibility guide](docs/design/tinkeroo-theme.md).

The GitHub repository remains `josebarrueta/tinkercrew`. Existing bundle identity, Keychain entries, saved progress, Board discovery names, and protocol identifiers intentionally retain their legacy names so this visual rename does not reset a Crew or break a paired Board.

## Project guides

- [App architecture and playable Mission](app/README.md)
- [Companion installation, API, and tests](companion/README.md)
- [Resumable setup handoffs and remaining implementation](docs/architecture/setup-checkpoints.md)
- [Secure protocol proposal](docs/architecture/secure-board-protocol.md)
- [MVP roadmap and open work](https://github.com/josebarrueta/tinkercrew/issues/13)

## License

Original Tinkeroo code and documentation are licensed under the **[MIT License](LICENSE)**, consistent with the existing app package metadata. Third-party dependencies, libraries, tools, and assets retain their own licenses; this is not a claim that every distributed component is MIT-licensed. Preserve required upstream notices and complete the dependency/notice audit before public binary distribution.

Google's Platform-Tools are separately installed, not redistributed; see the [ADB licensing assessment](docs/research/adb-distribution.md). LEGO, Arduino, Android, and other third-party names/trademarks are not licensed by this repository's MIT license; no affiliation or endorsement is implied.
