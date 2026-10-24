# macOS release procedure (#12)

Status: packaging/signing/notarization automation and signed-update design implemented; **no public signed/notarized release or clean-Mac acceptance has been performed**. Apple credentials and a production update-signing key have not been created by the agent. Do not bypass Gatekeeper to declare acceptance.

## Architecture and minimum OS

- Minimum macOS: **12.0**.
- Development builds use the current machine's architecture and are explicitly ad-hoc signed by the development-only config.
- Public release workflow builds **Universal 2** (`universal-apple-darwin`) with both `aarch64-apple-darwin` and `x86_64-apple-darwin` Rust targets. One DMG supports both Apple silicon and Intel.
- A successful cross-build is not Intel runtime validation. Test an actual supported Intel Mac and an Apple silicon Mac before claiming support/publication. No Rosetta requirement is intended.
- No ADB executable is embedded today; official external Platform-Tools remains separately installed. Any future embedded USB helper must pass the same signing-team/embedded-code checks and licensing review.

## Local development packaging

```sh
cd app
npm ci
npm run build:dmg
```

The `.app` is under `src-tauri/target/release/bundle/macos/`, and the drag-to-Applications `.dmg` is under `src-tauri/target/release/bundle/dmg/`. Tauri's DMG bundler creates the Applications link. The script sets exactly `CI=true` to skip interactive Finder layout automation; `CI=1` is not equivalent for the installed bundler and caused an AppleScript timeout during implementation. The Applications link is retained even when cosmetic Finder layout is skipped. These are **development artifacts**, not notarized public installers. `build:app`, `build:dmg`, and CI bundling explicitly use `src-tauri/tauri.development.conf.json` to seal/sign the executable **and bundle resources** ad-hoc. CI runs `codesign --verify --deep --strict`; local verification uses the same command on `src-tauri/target/release/bundle/macos/Tinkeroo.app`. The protected production workflow does not load this override.

## Owner-only prerequisites (do later, not silently)

1. An Apple Developer Program account holder creates a **Developer ID Application** certificate, with its private key held in Keychain and an encrypted `.p12` export stored outside this repository. See [Apple CSR instructions](https://developer.apple.com/help/account/create-certificates/create-a-certificate-signing-request) and [Tauri's macOS signing guide](https://v2.tauri.app/distribute/sign/macos/).
2. Create an App Store Connect API key with appropriate notarization access, retaining its `.p8` privately. The issuer/key IDs are metadata; the `.p8` contents are secret. Never use a personal account password in process arguments.
3. Generate a **separate Tauri/Minisign update signing key** on the owner's trusted machine, store its private key/password in a secure backup, and retain the public key. Use `npm run tauri signer generate -- --help` for the installed CLI. This is not the Apple signing certificate or a Board credential. Never use the checked-in test fixture key for production (its private key was discarded).
4. Configure the GitHub `release` environment with required owner approval and protected version tags. The owner must do the account/dashboard work; automation does not pretend these resources or approvals already exist.
5. Populate exactly the workflow inputs below through GitHub encrypted secrets/variables. A guided credential-setup wizard can be authored after the owner confirms this manual procedure; do not run credential generation or upload commands unattended.

| GitHub environment input | Kind | Value/source |
| --- | --- | --- |
| `APPLE_CERTIFICATE` | Secret | Base64-encoded encrypted Developer ID `.p12` export |
| `APPLE_CERTIFICATE_PASSWORD` | Secret | Password protecting that export |
| `APPLE_SIGNING_IDENTITY` | Secret | Exact `Developer ID Application: …` identity from the certificate |
| `APPLE_API_KEY` | Secret | App Store Connect key ID |
| `APPLE_API_ISSUER` | Secret | App Store Connect issuer ID |
| `APPLE_API_PRIVATE_KEY` | Secret | Full downloaded `.p8` contents |
| `TAURI_SIGNING_PRIVATE_KEY` | Secret | Private Tauri update signing key contents |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Secret | Its password (empty only if deliberately unencrypted) |
| `TC_UPDATE_PUBLIC_KEY` | Variable | Exact Tauri public-key file contents; public, not a private key |

No release inputs belong in source, `.env` commits, issue comments, build logs, or Mission progress. The workflow creates an owner-readable temporary `.p8`, passes only public IDs/file paths to `notarytool`, uses the official pinned Tauri action to import the signing identity, and deletes the temporary file on completion/failure. Hosted runners are ephemeral. Do not enable shell tracing for secret-bearing steps.

## Release candidate pipeline

`.github/workflows/release-macos.yml` is **manual-dispatch only**; it does not publish on pushes or pull requests. All external actions are pinned to reviewed commit SHAs. Before executing it:

1. Complete/triage #4/#10 and the remaining TBP/provisioning integration gates. The workflow cannot replace this decision.
2. Update `app/package.json`, `app/src-tauri/Cargo.toml`, and `app/src-tauri/tauri.conf.json` to the same version; refresh lockfiles. Commit reviewed code and create the matching immutable `vVERSION` tag.
3. Dispatch the workflow **on that tag** and approve the protected environment. It fails before signing if the ref/version or required inputs are missing.
4. It runs software checks without signing inputs, builds a Developer-ID-signed/notarized Universal app and DMG, and finalizes the DMG with `scripts/finalize-macos-release.sh`.
5. The finalizer submits the **final DMG** using `notarytool --wait`, requires `Accepted`, staples app and DMG, and runs strict release verification. It never passes a private key/password's contents as a CLI argument.
6. `scripts/verify-macos-release.sh` requires Developer ID, Hardened Runtime, valid signatures on all embedded Mach-O code with the same signing team, valid app/DMG stapled tickets, and Gatekeeper assessments. It fails on ad-hoc development builds. No extra JIT, unsigned executable memory, disabled library validation, App Sandbox, or debug entitlements are configured.
7. Only after stapling/verification, it signs both the immutable DMG and `release-manifest.json` with the update key, then independently verifies both signatures against the configured public key using `tools/release-verify`.
8. The workflow creates a **draft** GitHub Release containing DMG, DMG signature, manifest, and manifest signature. It does not publish automatically. Existing outputs/releases are not silently overwritten.

For an attended local finalization, first build with an installed Developer ID identity and Tauri's documented notarization environment. Then supply absolute key-file paths, `TAURI_SIGNING_PRIVATE_KEY` or `TAURI_SIGNING_PRIVATE_KEY_PATH`, `TC_UPDATE_PUBLIC_KEY_FILE`, and either `TC_NOTARY_PROFILE` (an existing local Keychain notary profile) or the App Store Connect API environment. Run:

```sh
bash scripts/finalize-macos-release.sh /absolute/Tinkeroo.app /absolute/input.dmg VERSION darwin-universal
```

This performs real Apple submissions and is intentionally **not** part of routine checks. Production credentials/notarization remain untested here. The release verifier's signed public fixture tests prove Tauri/Minisign compatibility and tamper rejection, not Apple approval or custody of a production key.

## Signed application update design

MVP updates are **parent-approved manual installs**, not an automatic background installer. The app never checks/downloads/installs updates during a Mission or silently updates the UNO Q. A parent obtains a release candidate from the official GitHub Release, verifies signatures, and installs the DMG. There is currently no in-app updater plugin or permission capable of installing arbitrary downloads. **Board Setup → Upgrade the Mac app** displays the installed version and opens only the fixed official Releases URL, after stopping local gameplay and refusing unresolved maintenance. It never compares/downloads/installs artifacts automatically. Follow the [user replacement/source-build instructions](../../README.md#upgrade-the-mac-app); keep app data/Keychain and explicitly reconnect after replacement. Mac app replacement never automatically updates the Board's Companion or firmware.

The publication format protects **both** metadata and artifact:

- `release-manifest.json.sig`: detached Tauri/Minisign signature covering the entire manifest, including version, target, URL, minimum OS, approval requirement, and artifact digest/signature.
- DMG `.sig`: detached artifact signature, bound to the app version by the CLI's trusted comment.
- Apple Developer ID + notarization: macOS platform trust and Gatekeeper, independently of update signatures.

The manifest uses a fixed official repository URL and versioned immutable asset name, not a caller-controlled update endpoint. Consumers must verify the manifest **before parsing/trusting its URL**, validate increasing SemVer/minimum OS/architecture, require parent confirmation, verify the downloaded artifact signature and SHA-256, and retain progress/Keychain during installation. A future Tauri updater integration must pin the production public key in the installed app, preserve this signed-manifest check (ordinary HTTPS JSON alone is insufficient), reject downgrades/replays, and quiesce hardware before replacing the app. It must not expose arbitrary endpoint overrides to Mission code.

The independent verifier accepts the Tauri-base64 public key/signature files:

```sh
cargo run --locked --manifest-path tools/release-verify/Cargo.toml -- \
  /absolute/update-public.txt /absolute/release-manifest.json.sig /absolute/release-manifest.json
```

Verify against an already trusted public key, not a new key downloaded alongside a potentially tampered release. Key rotation requires an explicit old-key-authorized transition/release; loss of the signing key is not solved by disabling verification.

## Clean-Mac acceptance — pending

Before publishing the draft, record software versions and verify on Apple silicon and Intel:

- Download through a browser so quarantine/Gatekeeper behavior is exercised.
- Open the DMG and drag to Applications; launch without bypassing Gatekeeper.
- Verify `codesign`, `spctl`, and both stapled tickets on the downloaded artifacts.
- Repeat launch offline with stapled tickets; complete simulator play with no external network.
- Test an upgrade retains local progress and Keychain Pairing; the guardian starts disarmed.
- Test corrupted manifest/artifact and wrong signing key rejection. Confirm no background or child-triggered install.
- Complete actual USB/LAN/hardware interruption tests from `docs/qa/recovery-safety.md`.

These acceptance checks are not yet performed. Keep #12 open until they pass with a real production-signed/notarized release.
