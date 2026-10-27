# Mission entry and navigation diagnosis — 2026-10-26 (#35)

## Owner report and reproducible software fault

The owner supplied a native screenshot: Treasure Guardian's saved **stage 3 of 10** is visible, but an alert says `Could not open this Mission: [object Object]. Existing saves have not been replaced.` The owner also reports that setup and Mission navigation are confusing and requests connected-Board Mission verification.

A native-shaped browser fixture reproduces the exact failure pattern: saved progress loads, then Mission entry tries to obtain hardware control and a structured IPC rejection is converted with `String(cause)`. The UI loses its actionable message and presents an unrelated Board error as failure to open the whole Mission. The first regression failed with the exact `[object Object]` alert before the fix. This demonstrates the UI bug pattern, **not the historical Board-side cause of the supplied screenshot**.

## Software correction

- Readable bounded error messages for structured native failures across Mission, setup and management views; unknown objects are not dumped.
- Saved Mission/story progress opens independently of hardware control. Connected hardware receives no output-control lease merely because the Mission is opened. Explicit **Enable Mission controls** is required; navigation/reconnection does not silently reenable it.
- Primary story/build step before optional hardware controls; compact intro, direct Home/Board Setup links, and no misleading indefinitely `starting`/`sound enabled` panel before controls start. Stop resets transient live evidence and cancels queued/in-flight work before it can issue a later output effect; a delayed-read regression emitted a pixel command before this fix and none afterward. Simulator practice remains clearly labeled and is not physical proof.
- Board Setup begins with **Connect & play** and **Continue to Mission**. Optional connection parameters and parent maintenance are collapsed; adding/Pairing a Board has an explicit entry. Pairing does not require Linux-password sign-in. Existing native permission/identity/confirmation gates remain unchanged.
- Transport failure during session creation is `companion_unavailable`, not a claim that saved keys were rejected. HTTPS 503, throttling 429, and actual 401 denial are distinguished. Throttling stops route hunting/automatic reconnect until the parent retries. Expired/rejected sessions advise reconnecting existing Pairing before re-enrollment.
- No Mission save or Keychain credential was deleted/replaced to implement these changes. Firmware/Companion source files and baseline identity remain unchanged.

## Actual read-only Board observations

The owner had reported completing native Pairing. An explicitly opted-in native diagnostic reused existing Keychain Pairing with the real USB/TLS/signed-session code; it did not enroll new keys, obtain output authority or invoke self-tests. It failed during authentication transport. Subsequent pinned Board-local TLS inspection found **connection refused** on port 7443; this was not evidence that the installation's keys were wrong.

Read-only official App CLI/Docker lifecycle inspection then observed:

| Observation | Actual result |
| --- | --- |
| Official Companion status | `stopped` |
| Default app | `false` |
| Container | `tinkercrew-companion-main-1`, exited 255 |
| Container start | `2026-10-26T03:58:18.043463157Z` |
| Container finish | `2026-10-26T04:57:20.812545579Z` |
| Current Linux boot | `2026-10-26T04:59:08Z` |
| OOM killed / runtime error / restarts | false / none reported / 0 |

Thus the Board has booted since the earlier successful startup, and its non-default Companion is currently stopped. This explains the **current** inability to connect, not necessarily the earlier screenshot failure. No reboot, reset, container/app restart, firmware operation, credentials reset, control lease, or interactive output test was performed in this diagnosis. We did not assume that restarting an exited container alone would restore the correct MCU state.

Local sanitized lifecycle evidence: ignored `artifacts/qa/uno-q-mission-service-lifecycle.json`. The explicitly ignored `physical_authenticated_sensor_inspection_owner_only` diagnostic requires separate `read-only-sensors` scope, exact USB selection and a closed Tinkeroo UI; normal QA/CI never runs it or accesses live Keychain/Board data. Never run blanket `--ignored`.

## Remaining physical acceptance

The only implemented Mission is Treasure Guardian; other map stops are not completed Missions. The complete simulated Mission/resume/remix flow passes software tests, but **a full real-board Mission has not passed**. A separately approved official Companion startup may compile/upload STM32 firmware. Then verify fresh signed sensor state, real A/B/C presses, Distance movement, and explicit bounded lights/sound with the owner witnessing outputs. Do not mark these from simulator events, historical button events or a public health response. #21 broad physical/family acceptance and #25/#12 publication gates remain incomplete.
