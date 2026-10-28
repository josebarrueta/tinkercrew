# Integrated navigation — 2026-10-28

Tracking: #47 / #38; approved plan: [navigation review](../design/navigation-review-2026-10-27.md).

PR #49 (draft protection/sticky controls) had already merged before the owner's request to merge all slices together. This follow-up combines all remaining navigation slices; it does not rewrite that merge or pretend it never happened.

## Delivered together

- **Create / My creations** primary destinations, visible **Try modules / Build** selection and secondary Mission starters. One current draft survives ordinary navigation. The library shows actual titles/rule counts/current draft status, not invented timestamps.
- Whitelisted view-only URLs and SvelteKit shallow history. Heading focus/scroll on real transitions; selecting the current view does nothing. Back/Forward never restores output authority.
- One nonmodal Board region contains close/content, keeps global Stop accessible and restores focus on close. Connected inspection is read-only: no reconnect, credential lookup, lease or output clear just to inspect. Explicit Practice disconnect precedes selecting a different connection.
- Dismissing pending connection or leaving its view cancels intent and disposes the late specific USB handle. Saved-network retries are cancelled when leaving a failed/pending attempt. No automatic rearm.
- Parent groups: Board setup, Network & saved permission, Software updates, Troubleshooting. Contextual help opens its task; Return restores the originating Creation/view. Exact-target consent and native guards remain unchanged. Active maintenance prevents navigating away; discovered pending recovery stays visible even in another task group.
- Legacy adventure entry shares draft protection. A deliberate detour remembers only the current program/title/ID/dirty flag/view in frontend memory. Return restores it stopped, not a connection or credentials. Continue without saving explicitly does not promise survival after closing/reloading the app. Historical saves/checkpoints are untouched.
- One primary error instead of competing root/module warnings; Practice wording and one Add rule path. Block guidance is optional rather than another palette/action system.

## Tests and failure history

Added rendered UI/IPC regressions for Create/library/history retention, same-view no-op while running, Parent heading/return/task targeting, task-specific visibility, pending recovery visibility, exact-handle cancellation and legacy return. Existing tests were adapted to actual navigation, not removed.

Expanded real private-Unix TLS Companion regression covers fresh Distance and A/B/C transitions or missing inputs, Try/Build/library/Parent round-trips, non-disruptive connection inspection, delayed snapshots, explicit blue/chirp and Stop. Only generated fixture TLS, external SDK samples and the typed IPC carrier stand in for external systems. No unconditional-success gameplay replacement.

The first full run exposed two legacy recovery regressions: a one-way `open` attribute closed the recovery disclosure as input changed. A two-way disclosure state fixes this while auto-opening discovered recovery only in task-group mode. Original exact-target/restart/factory tests remain active.

Another run exposed an existing test race: Playwright click dispatch was being treated as asynchronous TLS-clear completion. Final-off verification now first waits for the runtime's **stopped** UI state, which occurs after successful clear; it does not poll until output TTL expiry, retry a Command or loosen watchdogs.

Final `bash scripts/check-mvp.sh`: **PASS**, evidence `artifacts/qa/20261028T171331Z-23626.md`:

| Suite | Passed |
| --- | ---: |
| Frontend | 39 |
| Native | 37 |
| Companion | 50 |
| Browser/protocol | 47 |
| Helpers/release metadata | 38 |
| Release verifier | 1 |
| **Total** | **212** |

Four owner-only physical diagnostics remained ignored. Formatting, zero-warning Svelte check, Rust lint, notices freshness, Python compilation and whitespace validation passed. Production frontend build passed separately. Browser screenshots were inspected; minimum supported 900×640 and default 1180×760 Run/Save/Stop checks remain active at one/eight rules.

## Boundaries

No native IPC/authentication/transport, Companion/runtime, firmware, lease, freshness, watchdog, schema, identity, key or consent-handler change. BoardManagement changes are presentation/task filters and disclosure state, not operations. No actual Board/native-app interaction, Keychain request, user-save mutation, firmware action or deployment occurred. The installed app/deployed runtime remain untouched; no new physical witness is claimed.

The owner's four-module success remains the physical baseline. Full family/Creation acceptance, Matrix, measured calibration, unplug-under-output and power-cycle/default-start remain separate. Library removal/archive and crash/reload draft recovery require separate scoped work; this does not migrate persistence or promise them. Public release/legal/signing blockers are unchanged.
