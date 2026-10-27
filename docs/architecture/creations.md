# Creation workspace — C to start, B to compose (#38 / #40)

Owner-selected direction: module-first Playground, then a block Workbench for inputs, conditions and outputs. This is production implementation rewritten from the throwaway prototype, not a merged prototype route. The prototype remains on `prototype/creation-workspace` as a primary design source. Only owner design preference has been observed; physical/family acceptance remains #21.

## Normal flow

- `/` opens **Create**, explicitly in **Practice — no real Board outputs**. It reads only the separate local Creation library, not Board credentials. No discovery, Pairing, SSH, Keychain lookup or output runs implicitly.
- **Connect Board** is visible intent to establish [temporary USB play](usb-play.md) using pinned/authenticated native transport, without reading Keychain or saved Pairing. The compact connection sheet explains the memory-only permission. It never starts the Companion, flashes firmware, reboots, resets, changes networking, takes over default-app status or rekeys as a retry.
- The connection panel is created only on explicit Connect/Parent settings intent; it is deferred until Connect, rather than merely hidden while doing credential work. Explicit Practice invalidates pending lookup/probe intent. Late results cannot switch the maker back to hardware, start new route probes or silently acquire output control.
- The module Playground displays supported Buttons/Distance/Pixels/Buzzer health and live inputs. It does not claim universal I²C inventory or wiring/electrical certification. Missing/stale/invalid input states are distinct from a fresh released button or a numeric distance. Matrix drawing and general GPIO/power switching are not offered by this initial action interface.
- Each output preview is intentional and independent of unrelated input modules: Pixels at brightness 0.25 for up to 1.5 seconds; Buzzer at 880 Hz for 250 ms. Practice uses visual feedback, not Mac audio. No output on discovery/connection. Reported/accepted state is not human evidence of light or sound.
- **Build with blocks** opens the Workbench; **Modules & experiments** returns to Playground. Navigation, rule editing, reconnect/switch and open/fork stop execution. **Stop outputs** is always available, not gated on a save or slow request. C is reserved for software Stop, not a certified emergency power disconnect.

## Bounded block behavior

A Creation contains 1–8 rules:

- Input: physical/practice A or B press, Distance crossing nearer than a boundary, or crossing farther/equal to a boundary.
- Optional condition: only if current Distance is below an additional boundary (an AND condition, not arbitrary Boolean code).
- Output: Pixels color, a short Buzzer chirp, or both. At least one output per rule.
- Distance boundaries are 20–4000 mm (shown in cm). Colors are strict hex RGB; output brightness/frequency/durations are fixed safe parameters, not arbitrary expressions.

The input block palette adds rules; connected WHEN / ONLY IF / THEN blocks edit them. No drag/drop library, JavaScript evaluator, shell, arbitrary path/archive, GPIO register, firmware or Board-program deployment interface exists. Programs execute on the Mac through `BoardClient`; loss of the Mac/transport must stop Board outputs through the independent watchdogs.

`CreationRuntime` is the authority-owning interpreter at the existing BoardClient seam:

- Start is explicit; initial state, saved load, reconnect, recovered health and changed rules never auto-run.
- Validate the bounded program, required Capabilities and dependent input freshness before lease acquisition. A stale Distance reading does not block a button/light program; it does block a program whose rule/condition needs Distance. Missing dependent Capability stops the program, not the entire app.
- Capture baseline distance at Run; no queued/historical edge replay. A distance rule fires only on crossing after Run, not continuously while stationary. A/B presses coalesce until the next tick; C cancels immediately in the app.
- Evaluate up to eight rules in display order. Later matching rules win **per output**. Coalesce to at most one Pixels and one Buzzer Command per batch; a 500 ms repeat budget prevents rapid output spam. The UI ticks at 250 ms without accumulating overlapping ticks. Trace is bounded to 12 human-readable entries.
- Serialize authority changes and Commands. Generation checks after async I/O cancel late work; Stop invalidates intent before waiting for cleanup. Explicit global Stop attempts clear even if no Creation was running. Repeated cleanup coalesces; explicit retry remains possible. Pending starts/previews cannot emit new effects after Stop intent.
- Input/transport/control loss stops execution; cleanup is best effort. Board-reported health transitions conservatively stop a running program, including a newly disconnected module; after checking wiring/power, an explicit new Run can use only the still-available capabilities. Dependency-specific stale snapshot validation does not require unrelated sensors. An unavailable clear is reported, not certified as physically off. Existing Linux/MCU lease and per-output deadlines remain independent authorities.
- Native `GameplayRequest` and Companion validation still bound each actual Command. Program data never becomes executable native code, and a frontend schema is not trusted to grant arbitrary operations.

The simulator now models Pixels' 1.5-second TTL separately from the lease, so a running program cannot accidentally keep a one-shot light permanently on in practice. It reports frozen Distance as stale, consistently with the connected adapter.

## Creation library and old saves

`CreationRepository` stores a bounded, version-1 library (up to 24 named Creations, each at most eight rules) at the existing save/load seam:

- Native: new `creations-v1` row in the existing SQLite progress store, using the existing atomic revision-checked save operation. No table migration or rewrite of `treasure-guardian` data.
- Browser practice: separate `tinkercrew.creations.v1` localStorage entry. No credentials, tokens, keys, network configuration or live output authority.
- Strict version/schema/size/identity checks; failed/unreadable load disables saving rather than replacing the library. Pending saves serialize, stale windows reject conflicts, Save captures a snapshot and does not label later edits as saved. Fork creates a separate ID/title and preserves the original. Opening/copying never runs it.
- Local work is not autonomously uploaded to the Board. Progress and runtime authority remain different things.

The new **Missions** brief states the Guardian's physical outcome, learning goal and required modules before any forms. **Make my guardian** opens two editable distance-to-color rules in the same Workbench. Buttons/sound are optional extensions; no fixed Crew size, naming or written reflection is required for this starter. It does not claim completion of the historical adventure.

The original ten-stage Mission, exact stored progress contract, rotating roles, old evidence gates and maintenance views remain at `/legacy`, reachable through **Earlier guided adventure and saved progress**. Its regression tests now target that explicit route rather than assuming it is the default Home. The original Mission is neither reset nor silently reinterpreted under the new starter. The new normal flow doesn't require completing the old LAN/reboot/Crew wizard or forge its protected checkpoints.

## Trust and parent settings

Parent settings expose existing recovery/provisioning functions only when deliberately opened, separate from the play path. Their typed confirmations, exact-target checks, locks and separate authority are unchanged. First-time unsupported/account/firmware setup retains the honest official-tool handoff; this implementation does not promise one-click virgin-Board preparation.

Keychain remains app-owned secure credential storage for the separate optional parent network/maintenance path, not normal USB play. Missing item (`errSecItemNotFound`) is distinguished from OS access denial/locked storage; the latter is `keychain_unavailable`, preserves keys and stops route hunting/retry. There is no plaintext fallback, ACL modification, helper impersonation or requirement to grant a diagnostic executable broad access. Stable production signing/OS prompt acceptance remains #12 work.

## Validation and limits

Regression seams: BoardClient with real simulator/connected adapters, native public storage/protocol fixtures and browser UI/IPC. Tests cover explicit Run/C/Stop, delayed-start cancellation, one-shot TTL while leases continue, multiple matching rules/conditions, stale/invalid dependent inputs, independent Buzzer with missing Distance, no initial credential lookup, cancelled lookup intent, corrupt-library preservation, saving/forking and stopped reopen. Original guided/maintenance/recovery regressions remain active at `/legacy`.

Software QA/build/signature/disclosure checks are not physical or family acceptance. No Board start/upload/reset/rekey, witnessed GPIO timing or real Creation execution is claimed from this implementation. General-purpose GPIO, loops/timers, arbitrary code, matrix action editing, autonomous deployment, unlimited library size, and a full Scratch-like language are not delivered by this bounded first editor. Release gates and unrelated apps/keys remain intact.
