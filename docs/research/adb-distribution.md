# ADB distribution strategy for TinkerCrew

Status: recommendation for issue #2  
Researched: 2026-10-22  
Scope: macOS development and public distribution outside the Mac App Store

> This is an engineering and licensing assessment, not legal advice. Counsel should review the final third-party notices and distribution package before a public release.

## Decision

### MVP development

**Require a separately installed official Android Platform-Tools `adb`, with an in-app diagnostic and guided Homebrew installation. Do not copy Google's downloaded binary into the TinkerCrew app.**

Arduino's UNO Q documentation explicitly tells macOS users to install Android Platform-Tools with `brew install android-platform-tools`, then use `adb devices` and `adb shell` [1]. This keeps development moving with the implementation Arduino documents and Google tests, while avoiding a claim that TinkerCrew may redistribute Google's prebuilt archive.

The application should:

1. Look for `adb` in known absolute locations, including Apple-silicon and Intel Homebrew prefixes.
2. Run `adb version` and reject unsupported versions with a precise message.
3. Ask the parent to install or update Platform-Tools; never install Homebrew or accept Google's SDK terms silently.
4. Always select the UNO Q by validated serial (`adb -s SERIAL …`), never by “the only attached device.” Google documents `-s` as the safe selector when multiple devices are present [2].
5. Keep secrets out of command arguments and logs.
6. Treat this as a development/early-access dependency, not the final one-download experience.

### Public release

**Implement the narrow USB recovery/provisioning surface with a native Rust ADB client, contingent on a physical UNO Q compatibility spike. Retain externally installed official `adb` as a parent-facing fallback.**

The required TinkerCrew surface is much smaller than the full Platform-Tools CLI: enumerate devices, identify one serial, authenticate, execute bounded non-interactive shell commands, transfer files, observe disconnects, and reboot. A direct client avoids shipping a general-purpose background ADB server and avoids the Google prebuilt archive's redistribution ambiguity.

The leading existing candidate is the MIT-licensed `adb_client` crate. Its own repository describes direct USB and TCP connections, shell, push/pull, serial selection, and custom private keys [3]. It is not an official Android component and must pass the issue #1 physical-board spike before adoption. Pin an exact version and audit its full dependency/license graph.

If the Rust client cannot meet reliability requirements, the release fallback is **a reproducible source build of AOSP `adb`**, produced on controlled Linux CI, assembled for both macOS architectures, signed as nested code, and shipped with complete Apache-2.0 and third-party notices. Do not make this the first choice: current AOSP documentation says modern AOSP builds require 64-bit Linux and that macOS builds are unsupported [4], making a maintained universal macOS toolchain a substantial release-engineering burden.

**Do not bundle Google's prebuilt Platform-Tools archive unless Google grants redistribution permission in writing or legal review establishes a clear right to do so.**

## Why the downloaded binary and the source code are different

Google's Platform-Tools download page requires acceptance of the Android SDK License Agreement [5]. Section 3.4 says that, except where applicable third-party licenses require otherwise, a recipient may not copy other than for backup, modify, adapt, or redistribute the SDK. Section 3.5 separately says open-source components are governed by their open-source licenses [6].

The AOSP `adb` source repository marks the module Apache-2.0 and its `NOTICE` grants reproduction and distribution of source and object forms subject to the license and notice conditions [7]. That clearly supports distributing **our own compliant build of the covered source**. It does not establish that Google's downloadable archive contains only those bits or that Google's archive itself may be repackaged. The download and source therefore require separate treatment.

## Decision matrix

Ratings: **Low / Medium / High** burden or risk. “Legal clarity” is relative and still requires release review.

| Option | Legal clarity | Universal macOS | Notarization | USB reliability | Maintenance | Binary/update impact | Recommendation |
| --- | --- | --- | --- | --- | --- | --- | --- |
| User installs official Platform-Tools, preferably through Homebrew | High for TinkerCrew distribution because no binary is redistributed; user accepts upstream terms | Google's macOS package has been universal since Platform-Tools 32 [5] | TinkerCrew does not sign the external binary; runtime discovery and launch still need testing | Highest initial confidence; this is Arduino's documented path [1] | Low for MVP, but installation support remains | No app-size cost; updates controlled externally | **MVP choice and permanent fallback** |
| Build AOSP `adb` from Apache-licensed source | High for covered source if all NOTICE and dependency duties are met | Possible in principle because AOSP contains Darwin host sources [8], but modern AOSP builds officially require Linux [4] | Embedded executable and libraries must be signed, hardened, and notarized with the app [9] | Expected high after qualification; closest to Google's implementation | High: large build graph, patches, security updates, two-architecture assembly | Larger app and CI; TinkerCrew owns update cadence | **Fallback public-release option** |
| Native Rust ADB client | High for an MIT implementation after dependency audit [3] | Normal Rust dual-target builds; USB backend must be validated on both architectures | Sign the resulting app/helper and any native USB library as nested code [9] | Unknown until tested on UNO Q; less field history than official `adb` | Medium: narrower code, but TinkerCrew owns protocol compatibility | Smallest tailored surface; normal app updates | **Preferred public-release direction after spike** |
| Bundle Google's prebuilt Platform-Tools | Low/uncertain: SDK terms restrict redistribution while separately preserving open-source rights [6] | Yes; Google notes a universal macOS binary from v32 [5] | Technically feasible if re-signed correctly, but signing does not cure licensing uncertainty | High | Low technically; track upstream releases | Adds the Platform-Tools archive contents unless pruned, which adds more licensing questions | **Do not use without written permission/legal approval** |
| Avoid ADB and use another Arduino interface | Unknown because no documented replacement covers initial USB shell/recovery | Depends on replacement | Depends on replacement | Unsupported for current requirements | Potentially high discovery cost | Unknown | **Monitor, but not currently viable** |

## License and notice obligations

### Externally installed official `adb`

- Do not place Platform-Tools binaries in the app, DMG, updater, cache, or repository.
- Link to Arduino's instructions and Google's download page rather than proxying the archive.
- State that Android and ADB are third-party technologies; avoid Google trademarks in the product name or iconography. Google's SDK agreement does not grant trademark rights [6].
- Do not automate acceptance of Google's license on the user's behalf.

### Source-built AOSP `adb`

At minimum, Apache-2.0 section 4 requires:

- a copy of the Apache License;
- prominent notices on modified files;
- retention of relevant copyright, patent, trademark, and attribution notices;
- a readable copy of applicable `NOTICE` attributions [7].

A release must also inventory every linked dependency and generated/prebuilt input; the module's top-level Apache marker is not a substitute for a complete software-bill-of-materials and notice audit.

Suggested bundle locations:

- `TinkerCrew.app/Contents/Resources/ThirdPartyNotices/ADB-NOTICE.txt`
- `TinkerCrew.app/Contents/Resources/ThirdPartyNotices/Apache-2.0.txt`
- Parent Settings → About → Third-Party Software

### Rust client

The current `adb_client` repository is MIT-licensed and requires retaining its copyright and permission notice [3]. Audit and retain notices for all transitive crates and any USB library. Generate a machine-readable SBOM and a human-readable third-party notices file during release builds.

## macOS packaging and notarization

Apple permits embedding an externally built command-line helper, but it must be placed in a standard nested-code location, built for the intended architectures, signed with hardened runtime, and included in the app's signing/notarization flow [9]. Apple specifically demonstrates building x86_64 and arm64 slices, combining them with `lipo`, and signing the helper before embedding it.

For either a source-built helper or native Rust implementation:

- build both `arm64-apple-darwin` and `x86_64-apple-darwin` unless the product explicitly drops Intel;
- combine or package architectures consistently with the Tauri app;
- inspect dynamic dependencies with `otool -L` and keep non-system libraries inside the signed bundle;
- sign nested executables before the outer `.app`;
- enable hardened runtime with the minimum entitlements;
- verify with `codesign --verify --deep --strict --verbose=2` and `spctl`;
- notarize the final distributable and staple its ticket.

## Security implications

Official `adb` is a powerful, general-purpose shell and file-transfer tool. Google describes it as a client/server program whose host server binds local TCP port 5037 [2]. Current AOSP help identifies the default listener as `tcp:localhost:5037`; `-a` changes it to all interfaces [10].

Controls for an external/source-built CLI:

- Never pass `-a` or bind the server beyond localhost.
- Always use a validated serial or transport ID.
- Refuse ambiguous multi-device state.
- Use a TinkerCrew-specific ADB key location rather than silently reusing unrelated developer keys where UNO Q permits it.
- Prefer `--one-device SERIAL` for a private server where supported [10].
- Run only an allowlisted command model; game/domain code must not submit arbitrary shell text.
- Stop a private server when the operation ends, without killing a user's unrelated developer server.
- Redact command output and environment values before logging.

A direct Rust client reduces the local-server exposure and avoids conflicts with another `adb` version, but it still holds a key capable of board shell access. Store that key with restrictive filesystem permissions or Keychain protection, scope commands, and support revocation/re-pairing.

## Validation gates before changing the decision

### Native Rust go/no-go spike

On the physical 4 GB / 32 GB UNO Q and both Apple silicon and Intel Macs (or representative CI/test hardware):

1. Enumerate the UNO Q over USB and obtain a stable identity.
2. Distinguish zero, one, and multiple ADB devices.
3. Authenticate without borrowing the user's default ADB identity.
4. Run a bounded command and capture separate exit status/output.
5. Push and atomically replace a companion artifact.
6. Survive unplug/replug, reboot, and interrupted transfer.
7. Confirm no secret appears in arguments, logs, or process listings.
8. Verify behavior when official `adb` is already running.
9. Verify signing, hardened runtime, and notarization with the selected USB backend.

If any gate fails, keep official external `adb` for the MVP and investigate a source-built helper.

### Legal release gate

Before public distribution, have counsel or an authorized license reviewer confirm:

- the chosen implementation and every transitive dependency;
- generated notices and attribution UI;
- trademark wording;
- whether any Google prebuilt input entered the shipped artifact;
- export-control and cryptography declarations, if applicable.

## Consequences for related issues

- **#1 provisioning research:** use externally installed official `adb` for the physical experiment; record exact Platform-Tools version and board serial behavior.
- **#3 secure protocol:** ADB is a setup/recovery transport, not the gameplay authorization protocol.
- **#7 connectivity:** target every ADB command by serial/transport identity and model multi-device ambiguity explicitly.
- **#11 setup wizard:** the one-download promise remains blocked until the native Rust spike or source-build fallback passes.
- **#12 packaging:** include helper signing/notarization and third-party notices if the final app embeds any ADB implementation.

## Primary sources

1. Arduino, **Connect to UNO Q via ADB** — Homebrew installation and UNO Q shell flow: https://docs.arduino.cc/tutorials/uno-q/adb/
2. Android Developers, **Android Debug Bridge (adb)** — architecture, local port 5037, serial selection, commands: https://developer.android.com/tools/adb
3. `cocool97/adb_client` repository and MIT license — direct USB/TCP feature claims and license: https://github.com/cocool97/adb_client and https://raw.githubusercontent.com/cocool97/adb_client/main/LICENSE
4. Android Open Source Project, **Set up for AOSP development** — Linux requirement and unsupported macOS builds: https://source.android.com/docs/setup/start/requirements
5. Android Developers, **SDK Platform Tools release notes** — downloads and universal macOS binary note: https://developer.android.com/tools/releases/platform-tools
6. Google, **Android SDK License Agreement**, especially §§3.1–3.8: https://developer.android.com/studio/terms
7. AOSP `platform/packages/modules/adb`, **NOTICE** — Apache-2.0 grant and redistribution conditions: https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/main/NOTICE
8. AOSP `adb`, **Android.bp** — Darwin host sources and dependencies: https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/main/Android.bp
9. Apple Developer Documentation, **Embedding a command-line tool in a sandboxed app**: https://developer.apple.com/documentation/xcode/embedding-a-helper-tool-in-a-sandboxed-app
10. AOSP `adb`, **client/commandline.cpp** — localhost default, `-a`, serial/transport and `--one-device`: https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/main/client/commandline.cpp
11. Homebrew Cask, **android-platform-tools** — package source, checksum, and installed binaries: https://formulae.brew.sh/cask/android-platform-tools
