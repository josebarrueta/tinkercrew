# Offline, recovery, and hardware safety acceptance (#10)

Status: automated checks and repeatable acceptance plan implemented; **physical acceptance pending**. Do not infer electrical safety or family usability from the software suite.

## Repeatable software checks

On a Mac with the prerequisites in `app/README.md`, install dependencies once:

```sh
cd app
npm ci
npx playwright install chromium
cd ..
bash scripts/check-mvp.sh --build
```

The script collects the commit/workspace state, OS and toolchain versions, pass/fail per software check, and explicit not-run hardware status in an ignored `artifacts/qa/<UTC>-<pid>.md` report. It never installs/deploys to a Board, scans a network, touches real Keychain records, signs a release, or exports credentials. Omit `--build` to skip the production bundle check. Native TLS tests launch only a loopback Python Companion with temporary test certificates/credentials and terminate it afterwards.

Covered behavior includes:

- Full ten-stage Mission, independent roles/tracks, progressive hints, failed check/retry, tested remix, reflection, and reload/resume.
- Browser replay with external-resource requests blocked (local development server retained); this is not an installed-app/airplane-mode claim.
- Atomic native SQLite reopen and stale-writer rejection; invalid definitions/progress and corrupt browser saves are not silently reset.
- Bounded simulated outputs, 1.5-second lease expiry, stale input → Safe state → disarmed recovery, and no historical button replay.
- Ordered connection fallback/consent/manual entry, exact ADB serial matching, local-address validation, event gaps, malformed freshness, and real TLS credential/certificate/identity rejection.
- Companion input health, bounded Commands, lease loss, expiring output state, negative/ambiguous body framing protection, request read timeouts, and authenticated POST rate limiting.

## Physical test prerequisites

- An adult supervises powered tests and follows the in-app hardware safety panel.
- Record Mac model/architecture/macOS/app commit; Board serial/ID/model/revision; OS image; App CLI, Companion, sketch, Bridge, and Modulino library versions. Record network type, VPN product/settings, cable/power supply, and connected components without logging passwords, keys, tokens, or private Wi-Fi details.
- Have official Arduino App Lab/Platform-Tools and a known-good USB-C data cable available for recovery. Never modify the bootloader or flash the OS as part of these tests.
- Back up progress/Board configuration through an owner-approved method. Never commit a live provisioned record or Keychain export.
- Measure output-off timing using a video/timer or instrumentation; a UI label alone is not evidence that physical outputs went off.

## Matrix

Every row starts **PENDING** until a person records a run and evidence. Software analogues above do not change these statuses.

| ID | Scenario / action | Expected result and next action | Physical result |
| --- | --- | --- | --- |
| H01 | Already configured UNO Q; normal power cycle | Selected ID reconnects; starts disarmed; new live checks required | PENDING |
| H02 | Factory/reset Board | Supported official first-run handoff; no guessed password or destructive action | PENDING |
| H03 | Correct then incorrect Wi-Fi password during official setup | Credentials can be corrected; resume supported steps without duplicating state | PENDING |
| H04 | Hidden Wi-Fi network | Supported explicit network entry or precise official-setup fallback | PENDING |
| H05 | No internet after app/dependencies/Companion installation | Installed simulator and local USB/LAN Mission operate without external resources | PENDING |
| H06 | Block Bonjour/mDNS | Bounded discovery failure; USB/manual fallback remains responsive | PENDING |
| H07 | VPN enabled; local routing blocked then allowed | Honest local-route diagnostic; USB works; recover without disabling TLS | PENDING |
| H08 | USB unplugged during transfer | Partial setup is explained; current Companion retained; safe retry, no automatic flashing | PENDING |
| H09 | Board power removed during setup | Resume only confirmed durable checkpoints; unsupported steps hand off | PENDING |
| H10 | Board power/USB/Wi-Fi lost during active alert | Pixels/matrix/buzzer physically off within 2 seconds; no silent rearm | PENDING |
| H11 | Mac app closed/killed during active alert | Board/STM32 watchdog independently clears outputs within 2 seconds | PENDING |
| H12 | Mac app reinstalled | Keychain/progress behavior documented; missing trust requires USB recovery, not trust-on-first-Wi-Fi | PENDING |
| H13 | Board renamed or LAN address changed | Stable ID retained; Bonjour/USB/manual succeeds; stale address is not mistaken for another Board | PENDING |
| H14 | Companion upgrade interrupted | Old verified deployment remains usable or explicit official recovery is available | PENDING |
| H15 | Two UNO Q Boards plus an unrelated ADB device | Explicit identity selection; all ADB mutations include exact paired `-s` serial; no output on other Board | PENDING |
| H16 | Weak supply / poor cable / repeated boot | Booting/not-detected diagnostics suggest power/data cable; no repeated destructive setup | PENDING |
| H17 | Buttons or Distance Modulino unplugged while running | Stale/disconnected state; safe output clearing; power off before reconnecting | PENDING |
| H18 | Button held / rapid presses / missed event range | Ordered edges; no replayed historical arm after reconnection; bounded output | PENDING |
| H19 | Stale, zero, or implausible distance | Live checks invalidated; Safe state; fresh valid input plus explicit arm required | PENDING |
| H20 | Wrong credential, changed certificate, wrong Board ID | Fail closed with USB recovery action; no gameplay command to the wrong Board | PENDING |
| H21 | Light/sound self-test and unavailable component | Human confirms actual outputs; missing component is not a judgment of LEGO design | PENDING |
| H22 | macOS local-network/Keychain permission denied | Safe diagnostic and simulator/USB path; no plaintext credential fallback | PENDING |

## Per-run evidence template

Copy into a dated local report, then publish only a sanitized summary:

```text
Run ID / UTC:
Tester:
Test ID:
Mac / architecture / macOS / app commit:
Board ID / serial suffix / hardware revision:
OS image / App CLI / Companion / firmware / Bridge / Modulino versions:
Network / VPN / cable / supply:
Initial state and exact steps:
Observed result / measured safe-off latency:
PASS / FAIL / BLOCKED / NOT RUN:
Recovery action and outcome:
Evidence reference (no credentials):
Follow-up issue:
```

## Release gate

Do not close #10 while physical rows remain untested. A failed or unexplained interruption is a release blocker, not a row to omit. Full TBP Pairing and supported provisioning recovery are still integration work; scenarios depending on those capabilities must be recorded BLOCKED or NOT RUN until implemented, not reported as passes. Family usability belongs to #4; signing/Gatekeeper acceptance belongs to #12.
