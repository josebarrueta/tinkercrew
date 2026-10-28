# Navigation implementation, slice 1 — 2026-10-28

Tracking: #47. Implements the first, bounded part of the approved [navigation plan](../design/navigation-review-2026-10-27.md). This is not completion of the whole navigation redesign.

## Changes

- One draft-replacement decision shared by New, Open saved Creation and Mission starter: **Keep editing / Discard changes / Save and continue**.
- Failed saving cannot proceed with replacement. A successful write of an older version cannot discard edits made during that write. Existing version1 library/revision checks are unchanged.
- The decision stops the current program, prevents new preview/Run activation while pending, supports Escape/return focus, and does not trap the global Stop behind a modal.
- Creation title, save status, Run and Save now share a sticky toolbar below the measured header height. Copy/New remain secondary controls. This changes presentation, not the output execution path.
- Expanded real-protocol regression coverage for healthy and missing inputs across Try/Build round-trips and explicit module output tests. The healthy fixture sends fresh Distance samples plus controlled A/B/C transitions through the external SDK callback seam, not invented HTTP success responses.
- Corrected a pre-existing parent-network test's invalid `null` factory-status fixture to return idle, and assert that the renderer remains error-free. No production management behavior changed.

## Red → green evidence

1. New Creation test failed because the unsaved-work decision did not exist; it passed after the shared replacement guard.
2. Mission decision focus test exposed focusing a temporarily disabled button. Moving focus after the busy state clears resolved it.
3. At both 1180×760 and the native minimum 900×640, the primary-controls test failed with Run outside the viewport. The sticky toolbar passed full-button viewport assertions at one/eight rules and an actual Save click after scrolling to the document bottom.
4. Healthy-input protocol test failed to show B pressed before the external SDK fixture accepted controlled samples. With that test-only source implemented, A/B/C transitions, fresh 12.3cm display, navigation, explicit blue/chirp, Stop and final off state passed against the real Companion.

## Software validation

`bash scripts/check-mvp.sh`: **PASS**, local evidence `artifacts/qa/20261028T151244Z-12271.md`.

| Suite | Passed |
| --- | ---: |
| Frontend/unit integration | 39 |
| Native | 37 |
| Companion | 50 |
| Browser/protocol | 40 |
| Management helpers/release metadata | 38 |
| Release verifier | 1 |
| **Total** | **205** |

Four owner-only physical diagnostics remained ignored; none were invoked. Svelte reported zero errors/warnings. Formatting, Rust lint, dependency-notice freshness, Python compilation and whitespace checks passed. Production frontend build passed separately (`npm --prefix app run build`). No macOS bundle/DMG was rebuilt or installed during this slice.

## Trust / acceptance boundaries

- Only production file changed: `app/src/lib/creations/Workspace.svelte`.
- No actual Board operation or connection, native-app interaction, Keychain request, user save mutation, firmware action or runtime deployment occurred. Protocol tests use generated TLS, isolated storage and a fake external SDK; they execute the actual Companion and frontend runtime/adapter through the approved seams.
- The installed private app and deployed Board runtime remain untouched. The owner's earlier Buttons/Distance/Pixels/Buzzer success remains the physical baseline, not a new physical witness for this UI.
- No runtime, native transport/authentication, capability, freshness, lease, watchdog, schema or maintenance-guard change. No new permission on Connect and no automatic Run/rearm.
- Matrix, calibration, power-cycle/default startup, unplug-under-output and full Creation/Mission/family acceptance remain separate. Public release/legal/signing gates are unchanged.

## Still to implement under #47

Create/My creations destinations, visible Try/Build navigation and history/focus, contextual Board presentation/cancellation, correct Parent task/return targets, and protected legacy crossings remain later slices. Browser reload/app exit still need deliberate draft-lifecycle treatment; this slice does not claim crash recovery or cross-route draft restoration. Library capacity/deletion and persistence migrations are not included.
