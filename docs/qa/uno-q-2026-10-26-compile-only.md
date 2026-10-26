# UNO Q compile-only verification — 2026-10-26 (#29)

**PASS: the candidate pinned sketch compiles and emits ARM artifacts. NOT TESTED: upload, execution, GPIO, Router Bridge at runtime, first App Lab start, authentication or full setup.** The owner explicitly approved compile-only validation in an isolated temporary folder, including required downloads. No port/programmer/upload/verify flag, App CLI start/stop, reset, reboot or flasher command was invoked.

## Scope and target

- Configured physical UNO Q, shared serial suffix `8022`; only the bare Board attached, no Modulinos/loads.
- Debian 13, image `20260407-523`, UID 1000; installed Arduino CLI **1.4.1**, App CLI **0.9.0**, App Lab **0.7.0**.
- Fixed explicit physical USB selection; loopback ADB server; shell-v2 non-TTY transport; host shared management lock and unresolved-operation exclusions.
- Copied only `sketch.ino` to a new private temporary sketch directory and wrote a **candidate** version-pinned `sketch.yaml` there. Build output was in that temporary tree. No app data, certificate or private key was copied into the build.
- Command shape: `arduino-cli compile --fqbn arduino:zephyr:unoq --profile default --jobs 1 --build-path <owned temporary build> --no-color <owned temporary sketch>`. Explicit deny guard rejected upload, port, programmer and verify options. Only the owned compiler process group could be terminated on its bounded timeout; no ADB server or flasher was killed.
- The compiler installed required versioned SDK/tool/library cache entries, including platform tools supplied by the core. Downloading a flasher/tool dependency is not invoking it. No MCU firmware or OS-image write was requested.

## First feedback loop: missing explicit transitive dependencies

A temporary profile pinning core **0.55.0**, Arduino_Modulino **0.8.0** and Arduino_RouterBridge **0.4.1** failed compilation with:

```text
fatal error: Arduino_RPClite.h: No such file or directory
```

This was a compiler/dependency failure, not a flash or hardware failure. The profile-based compile did not supply the library dependency closure automatically. Read-only downloaded `library.properties` and the existing official library index established the required dependency chain, including RPClite → MsgPack → ArxContainer/ArxTypeTraits/DebugLog and Modulino's sensor/graphics dependencies. No arbitrary or unversioned package substitution was used for the successful candidate.

## Successful candidate

Core: **arduino:zephyr 0.55.0**, target `arduino:zephyr:unoq`.

| Library | Version |
| --- | --- |
| ArduinoGraphics | 1.1.5 |
| Arduino_HS300x | 1.0.0 |
| Arduino_LPS22HB | 1.0.2 |
| Arduino_LSM6DSOX | 1.1.2 |
| Arduino_LTR381RGB | 1.0.1 |
| Arduino_Modulino | 0.8.0 |
| Arduino_RPClite | 0.2.1 |
| Arduino_RouterBridge | 0.4.1 |
| ArxContainer | 0.7.0 |
| ArxTypeTraits | 0.3.2 |
| DebugLog | 0.8.4 |
| MsgPack | 0.4.2 |
| STM32duino VL53L4CD | 1.0.5 |
| STM32duino VL53L4ED | 1.0.1 |

The successful run exited **0** after **296.4 seconds**, including remaining dependency setup. The failed first run was preserved separately. Final evidence is in ignored local `artifacts/qa/uno-q-20261026T011959Z-compile-only.json`, dependency archive URL/checksum metadata in `artifacts/qa/uno-q-compile-dependency-candidates.json`, and artifact headers/digests in `artifacts/qa/uno-q-pinned-compile-artifacts.json`.

Measured source SHA-256: `4e587a4772876fa67a4bee0d8f07b691b2cd5643dfbc08843a87aed632b0e457`.
Measured candidate profile SHA-256: `4819667d3cc834ddf3cc5d72a2431717ab7e027865fc8ed8304df20319877cd2`.

Read-only artifact inspection found, among other temporary outputs:

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `sketch.ino.bin` | 54088 | `99c0ac634d5546e895b2547a9dfd966aeb7a925ad8e1444d6bd43309736defbe` |
| `sketch.ino.elf` | 117164 | `17e5054befe025977f17c756988da0c6cca552a9ab7b5bb363a347ffa7c721f6` |

ELF metadata: class **1 (32-bit)**, machine **40 (ARM)**. These digests identify this observed compile, not a signed vendor release, a promise of bit-reproducibility across paths/tool invocations, or firmware that has executed. App CLI may apply additional supported board options during actual startup; this direct compiler check does not validate that full path.

## Integrity preserved and scope limits

Before/after checks confirmed that the **prepared** sketch and its original profile bytes did not change. Private identity file metadata (inode, mode, size and mtime) also did not change; key contents were neither read for this check nor exported. The prepared Board app remains the original unstarted release `63e90d374bc27c69b2fd810f851fed9a51d00abaaa82ce6369ba4b7cac7feef3`; the candidate profile has **not** been copied into it.

The software profile now uses these exact compile-validated pins. Its native fixed-package test binds the measured source/profile hashes. An external-tool fixture models the measured App CLI 0.9.0 / Zephyr 0.55.0 profile migration and rechecks the real provisioner's receipt: the original unversioned profile failed this regression before the pinned profile passed. That fixture does not run Arduino CLI or claim a second hardware compile.

## First-start blocker still open

Official App CLI v0.9.0 commit `3e5e8040308a0c5b16ef0746e0904f2ebb84e399` calls `migrateRemoveRouterBridgeIfNeeded` before compile/upload. For an unversioned platform newer than 0.54.1, it removes the explicit RouterBridge profile dependency; a version-pinned platform skips this migration. Primary source: [migration call](https://github.com/arduino/arduino-app-cli/blob/3e5e8040308a0c5b16ef0746e0904f2ebb84e399/internal/orchestrator/orchestrator.go#L176-L192), [migration implementation](https://github.com/arduino/arduino-app-cli/blob/3e5e8040308a0c5b16ef0746e0904f2ebb84e399/internal/orchestrator/orchestrator.go#L1199-L1279). The candidate solves that source-backed mutation risk for the measured contract; an actual App CLI startup has not been performed.

The existing prepared Board copy still has the old profile. Updating this **exact owned, unstarted** preparation needs an explicitly approved, recoverable path preserving UUID/private key/certificate. Do not ad-hoc edit its receipt/journal, weaken integrity checks, delete/rekey, or start the old copy just to get past the gate. #29 stays open for that remaining path and actual first-start verification.

With no attached Modulinos, actual Buttons/Distance/Pixels/Buzzer acceptance cannot pass; missing components must be reported honestly. #21 broad acceptance remains incomplete, and #25/#12 license/source-availability/production-publication gates remain in force.
