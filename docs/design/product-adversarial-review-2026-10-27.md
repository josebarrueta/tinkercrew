# Tinkeroo product reset: adversarial review

Date: 2026-10-27. Status: **review and proposed design, not implemented or family-validated**.

Reviewed baseline: `fe90fb1` (PR #36). Review method: source-led walkthrough of every shipped screen, setup/maintenance path, the one implemented Mission and its ten stages, plus the owner's reported failures. This is a self-review, not an independent security/accessibility audit or an observed child usability study. No new Board operation, output test, reset, credential change or app replacement was performed for this review. The previous official Companion restart succeeded; physical Mission acceptance remains incomplete (#21/#35).

## Verdict

**The product does not yet deliver its basic promise: connect hardware, make something happen, understand it, invent something new.** It exposes an engineering acceptance procedure with a playful visual layer. Passing software tests did not establish that a family can use it successfully.

The owner's frustration is a product failure, not a failure to follow instructions. More disclosures, disclaimers and repositioned buttons will not solve it. Preserve the secure hardware foundation; replace the task model and first-use experience.

### Desired first experience

> Plug in my Board. See which modules are available. Change a light. Hear a short sound. Press a button and see the app respond. Make a small rule of my own. Save my invention.

On a compatible, prepared, running Board, target the first **intentional physical response within 60 seconds** of choosing to connect. This is a proposed usability target, not a measured result. A new/unprepared Board may need vendor setup, downloads and firmware startup; show that honestly rather than promising instant readiness or silently uploading firmware.

## 1. Evidence and priority

Priority meanings: **P0** blocks the central product promise; **P1** substantially harms understanding, trust or safe control; **P2** improvement after the main flow works. These are product priorities, not vulnerability ratings. “Observed” below means owner report or source behavior, not necessarily a reproduced physical outcome.

| ID | Priority | Finding and evidence | Adversarial question / required change |
|---|---|---|---|
| R01 | P0 | **No reliable first-success path.** Owner reports no useful play yet. Board was stopped after reboot; an approved restart restored it, but independent signed sensor inspection then hit Mac credential access. | Can a nontechnical adult get a visible response without an agent, terminal, Keychain troubleshooting or guessing which setup path applies? Model exact readiness and offer one next action. |
| R02 | P0 | **Free creation is not implemented.** `catalog.ts` maps `free-build` to a prompt disclosure; `+page.svelte` has no program workspace, creation save/open or behavioral composition. | Can I make a new button-to-light behavior rather than tune the supplied guardian? Build a genuine creation loop; do not call a prompt list a mode. |
| R03 | P1 | **Credential access can precede visible intent.** `ConnectionPanel.svelte` connects a single saved Board in `onMount`; the root always mounts it, merely hiding its section. | Why is macOS asking for access while I am on Home? Tie initial credential access to a visible connection decision. Later auto-connect preferences must be explicit; never auto-rearm. |
| R04 | P1 | **Setup has competing task models.** Connect & play, 12-checkpoint guide, adult maintenance, initial preparation, Pairing and legacy import coexist. | Which one do I need, and how do I know I am done? Derive the path from verified Board state, not a family choosing technical procedures. |
| R05 | P1 | **The shortest hardware feedback loop is fragmented/incomplete.** Hardware Lab offers distance text, raw last-event JSON and three guardian-colored pixel buttons, but no buzzer test, per-module health cards or ordinary Stop control. | Why must I enter a Mission to try the buzzer? Put simple, independent module tests next to detected modules. |
| R06 | P1 | **Available capability count is not an inventory.** `ConnectedBoardClient.getSnapshot()` filters out unavailable capabilities, maps absent distance to NaN and unavailable buttons to false values. | Is a module absent, unsupported, stale, or simply not pressed? Preserve explicit states/reasons through the interface; never display NaN or imply stale=false is a fresh release. |
| R07 | P1 | **Mission purpose is weakly surfaced.** Stage-specific `goal` text exists, but the title has no visible overall outcome/learning summary; narrative is collapsed. | Before filling anything, can I explain what I will build and what I will learn? Put both above the first action and retain a compact goal while making. |
| R08 | P1 | **Participation is hard-coded.** `MissionProgress` and `setCrew()` require exactly three members; setup also saves three. The roles stage instructs naming three. Defaults allow advancing without custom names, but do not solve the model. | Can one maker, two siblings or four family members participate without invented people? Optional, flexible Crew metadata; no membership gate to the first experiment. |
| R09 | P1 | **Typing becomes a completion gate.** Remix and reflection strings must be nonempty; the last stage needs two written responses. | Can a seven-year-old demonstrate understanding without typing an essay? Keep explanation invitations, but accept oral/shared discussion and meaningful observed changes without mandatory prose. |
| R10 | P1 | **Opaque evidence encourages button hunting.** UI reports `self-test`, `zones`, `signals`, `buttons`, `behavior` booleans; missing evidence is largely “keep experimenting.” | What exactly has not happened? Show individual pending actions and live progress, not one unexplained failed check. |
| R11 | P1 | **Command state is not physical proof.** Guardian marks signals after accepted Commands and renders reported outputs; actual light/sound need human witnessing. Completion has a long acceptance disclaimer. | Did the output actually happen, or did software accept the request? Separate “sent,” “reported,” “you observed”; do not burden every child-facing screen with QA terminology. |
| R12 | P1 | **Stop is not consistently primary.** Mission Stop is inside a disclosure and disabled while busy; Hardware Lab has no always-visible Stop. `GuardianRuntime.receive()` collects C as test input during self-test rather than disarming. Watchdogs bound outputs, but they do not replace an understandable immediate stop interaction. | Can I stop during a slow request without finding a section or waiting for a form? A visible, high-priority Stop; cancel pending execution, attempt clear, retain independent watchdogs. |
| R13 | P1 | **Keychain errors conflate states.** `Connections::profile()` maps every credential-read failure to `pairing_required` and suggests import/unlock. | Is my Board unpaired, or did macOS deny access? Distinguish missing record, locked/denied storage, malformed record, rejected registration and offline Companion. Never prescribe deleting/re-Pairing for all of them. |
| R14 | P1 | **Mandatory LAN/reboot verification overshoots USB play.** Full guide completion requires new-kernel LAN+USB proof and later a fresh hardware witness before Crew completion. | Why does a USB-only family need LAN and a reboot test to start inventing? Separate play readiness from maintenance/restart certification without forging old checkpoint completion. |
| R15 | P1 | **The world advertises a breadth it does not deliver.** Four named stops are disabled; only Treasure Guardian is playable. | Is this a useful choice or a wall of unavailable content? Remove empty destinations from primary navigation until implemented. |
| R16 | P1 | **Creation is bounded guardian configuration, not expressive hardware logic.** “Hacker” edits six guardian JSON fields; it cannot compose a new behavior or follow a program execution trace. | Can a curious older maker create something the author did not anticipate? Add safe event/condition/action composition, not arbitrary shell/firmware execution. |
| R17 | P2 | **Interface speaks engineering.** BoardClient, cached neighbors, TBP, SoC, runtime digests and raw events appear in ordinary paths or nearby panels. | Does this help the maker's next action? Use concrete hardware language; keep technical details accessible in diagnostics. |
| R18 | P2 | **Saving is ambiguous.** Mission autosaves many actions yet always shows “Save again”; no library exists for independent inventions. | What is saved: rules, progress, hardware or my creation? Autosave status with retry only on failure; clearly separate stored work from current output execution. |

### Security and trust, specifically Keychain

The app requests **specific items it owns**: installation private keys, saved Board records, dedicated SSH trust/key material, and setup checkpoint data. It does not request a list of unrelated passwords. Native code keeps gameplay credentials out of the frontend and progress storage. This is materially safer than putting private keys/tokens in localStorage or an ordinary JSON file.

However, “uses Keychain” is not a complete security argument: a compromised authorized app could misuse its own Board credentials, and app/signature/access policies still matter. The owner must understand what is authorized and why.

The recent isolated test executable could not access an existing record noninteractively. That is an operating-system access-control outcome, **not evidence that the Board needs re-Pairing**. Development/ad-hoc rebuilds can also cause new prompts; stable production signing is a separate release gate, not a promise of zero OS prompts.

Proposed visible explanation, only when needed:

> This Mac will remember permission to control this Board. Its private key stays in macOS secure storage. Tinkeroo does not request your other saved passwords. You can revoke this Mac's Board permission later.

Requirements:

- No Keychain setup section, manual secret-record paste, terminal credential command, broad Keychain grant or diagnostic-binary approval in normal family onboarding.
- Initial credential access follows a visible Connect/Pair decision. Do not hide prompts behind Home or a collapsed panel.
- Handle denied/locked access without deleting records, replacing keys or repeated background prompts. Offer a clear retry through the real app.
- Preserve cryptographic Pairing, TLS pinning, revocation, scoped Control sessions and independent watchdogs. USB transport is not a reason to make control unauthenticated.
- Whether integrity-protected nonsecret setup progress belongs in Keychain should be reviewed separately. Moving it is not a prerequisite for improving UX and must not weaken evidence integrity.
- No “fix” by impersonating the app's identity with a helper, changing ACLs, exporting keys or falling back to plaintext credentials.

## 2. Every screen and setup section

| Current surface | Assessment | Proposed disposition |
|---|---|---|
| Home Base | Friendly but abstract; main action opens one Mission, not a hardware success or own invention. Capability count doesn't explain what is connected. | Workspace empty state: **Make something**, **Try a guided Mission**, reopen saved work. Board dock visible. Avoid a separate mandatory dashboard. |
| Board Setup / Connect & play | Improved labels, still mixes saved identity selection, transport options, simulator switch and several onboarding paths. | One state-driven connection sheet. For verified ready hardware, lead directly to module cards. Multiple Boards require explicit selection. |
| Optional connection settings | Useful recovery mechanisms, wrong concepts for first use. | Keep manual endpoint/neighbor consent in troubleshooting. USB-first does not require blanket LAN permission or a subnet scan. |
| Resumable setup guide | Honest evidence handling, but 12 steps make the family conduct an acceptance test. | Replace ordinary entry with task-based readiness. Preserve old records and official handoffs; maintain an expert verification view separately. |
| Observed Pairing/runtime/reboot proofs | Valuable engineering protections; duplicate maintenance controls and require repeated navigation/serial confirmations. | Deep module owns ordering, identity, resume and error classification; family sees one understandable task at a time. Changes affecting firmware/reboot retain explicit scoped approval. |
| Setup hardware self-test | Useful witness machinery, reached too late; all required modules/steps bundle basic play. | Independent module tests available as soon as authenticated readiness allows them. A missing Distance module must not block a healthy Buzzer test. Full Mission readiness remains a separate check. |
| Setup Crew | Three names/tracks plus fresh-witness deadline make administration compete with hardware success. | Optional participant metadata later; editing names must not require repeating an output test. Never turn expired evidence into new output authority. |
| Linux sign-in / SSH / Wi-Fi / hostname | Adult operations are visually numbered like prerequisites even when Pairing doesn't need them. | Parent settings, task-specific authentication only when needed. Clearly separate from gameplay trust. |
| Initial preparation / profile correction / restore / certificate renewal | Precise recovery tools, but implementation history and release digests overwhelm normal setup. | Expert/maintenance route, contextually offered only for a verified applicable state. Never auto-restore a never-started backup onto a started app. |
| Pairing and revocation | Necessary security operations; labels expose protocol details and a manual bridge. | Plain “Allow this Mac to control this Board”; parent trust management for revocation. Keep full native checks behind it. |
| Linux runtime updater | Strong trial/rollback model, not a play destination. | Parent settings when compatibility requires it; no automatic firmware update. |
| Mac upgrade instructions | Installer/development instructions mixed into Board management. | About/Updates/help. Keep publication/notarization/legal gates intact. |
| Restart / local Forget / SSH Forget / factory restore | Distinct effects grouped together. Dangerous remedies are near connection frustration. | Separate troubleshooting, trust management and clearly isolated destructive recovery. Preserve typed/exact-target safeguards for destructive actions; never make reset a routine fix. |
| Legacy secret-record import | A development bridge with a textarea containing a secret. Not a family workflow. | Remove from ordinary product UI; if still needed, an explicit development-only entry with no plaintext persistence. Preserve existing legacy data during migration. |
| Hardware Lab | Closest to the desired experience, but has only three mission-colored light actions and developer telemetry. | Incorporate into the Board dock/setup sheet: identifiable module cards, live inputs, color chooser, bounded chirp and Stop. Do not create another destination to find. |
| Mission | Works as a prescribed guardian checklist, not a clear invitation to learn and invent. | Outcome-first guided recipe inside the creation workspace; starter behavior is editable and forkable. |
| World Map | Four unavailable games plus a prompt-list “mode.” | Remove from primary navigation for now. A future library can show only meaningful playable choices. |
| Simulator | Valuable real adapter, but default practice can be mistaken for connected progress; auto-connect can change context. | Explicit **Practice / Real Board** context, persistent indicator, no silent switching while running. Practice never certifies physical hardware. |
| Save/completion/free-build invitations | Saved progress is useful; finishing leads to suggestions, not an editable new invention. | Save/fork/reopen creations; Mission completion remains distinct from physical verification. |

## 3. Every game and every Treasure Guardian stage

### Inventory: do not pretend unbuilt games were reviewed as playable

- **Treasure Guardian:** the only implemented Mission; reviewed below.
- **Wake the Machine, LEGO Signal Tower, Curious Robot, Secret Control Panel:** titles/placeholders only. No playable logic or learning journey exists to validate. Remove disabled world stops from the main experience. A future Signal Tower could be a small button-to-light recipe, but that is a proposal, not delivered content.
- **Free Build Mode:** currently a list of three invitations, not a game/editor. Creature greeting, secret doorway and space traffic signal prompts have no dedicated composition/run/save journey. Do not promise working cooperation, doorway actuation or matrix programming from the prompt text alone.

### Overall Treasure Guardian assessment

The hardware theme is good: sensing an approach, making a decision and communicating a response are concrete. The problem is the packaging: ten progression gates, three fixed roles, four mandatory module types, duplicate checks, abstract live evidence and mandatory writing before an actual free-creation experience exists.

**Proposed first visible brief**:

> **Build a guardian that notices someone approaching your treasure.**
> Make its light change when a visitor gets close.
> **You'll learn:** how a sensor becomes an input, how a distance rule makes a decision, and how a light becomes an output.
> Start with Distance + Pixels. Add Buttons and Buzzer when you want more control and sound.

A visible animated/simulated cause-and-effect illustration should be silent by default and never operate real hardware. Keep the sensor path clear, but do not grade a physical build or prescribe LEGO parts.

| Stage | Adversarial review | Redesign |
|---|---|---|
| 1. Story | A valuable invitation, but asking for a story before showing what the electronics do gives a novice little to imagine. No saved story field exists here; “confirm” may look like a missing form. | Show the outcome first. Offer “What will yours protect?” as optional conversation/title, not a gate to trying hardware. |
| 2. Roles | Three people and three tracks are assumed. It front-loads administrative choices and may make a solo maker think play is impossible. | Solo works immediately; optional flexible Crew later. Roles are suggestions, not a prerequisite or skill ranking. |
| 3. Build | Open-ended materials and clear sensor entrance are strengths. Safety text dominates before a successful experiment; required module list repeats on every stage. | Small illustrated sensor-placement guidance beside the work. Specific wiring instructions at the moment needed; disconnect power before wiring changes. Support cardboard/other materials as well as LEGO. |
| 4. Hardware | Electronics checks belong here, but a bundled test requires all three buttons, Distance movement and visual/audible confirmation while the UI mostly exposes one pass/fail bit. | Show each module individually from setup onward. Track “A seen / B pending / Distance moved,” with one action at a time. Keep authentic fresh evidence and human output witnessing distinct. |
| 5. Zones | Three boundaries and millimeters arrive before an explicit input→condition→output representation. | Begin with one visible rule and a live distance marker: **when closer than 20 cm → red**. Add a warning zone as an extension; units clearly convertible. |
| 6. Signals | Considerable overlap with zones; software acknowledgment does not demonstrate that the family perceived green/yellow/red/tone. | Teach outputs in the same live rule diagram. Optional short sound action, preview warning, text/pattern alternatives to color-only cues. Ask what actually happened. |
| 7. Buttons | Useful physical control, but A/B/C meanings are prescribed and an earlier self-test uses A/B/C with different semantics. | Teach button events after the first distance rule. Show exact current meanings; stop/disarm stays invariant. In open creation, user rules cannot override emergency stop. |
| 8. Predictions | Repeats much of stages 5–7 and resets transient challenge evidence. Failure can feel like losing previously demonstrated work. | One optional “Can you predict it?” challenge with explicit pending observations. Preserve saved work; fresh safety evidence is not the same as task credit. |
| 9. Remix | The strongest learning intention—change/predict/test—is hidden late. Requiring text plus an armed nondefault rule equates configuration changes with invention. | Remix from the first rule: visibly change one behavior and try it. Offer optional “tell someone what changed”; save a fork. Do not force unsafe arming just to prove writing. |
| 10. Reflection | Mandatory two-field prose gates completion and asks every maker to report failure, even if their surprise was something else. | Optional oral/shared reflection: “What surprised you?” / “What next?” Completion summary shows what was built and changed; no compulsory typing. |

The proposed lightweight version **changes the old #8 spec**, which intentionally required three rotating roles, a tested remix and written explanation. Several current frictions follow that spec; they are not simply implementation mistakes. Approve a revised Mission contract/version and preserve old saves instead of silently reinterpreting them as completed under new rules.

## 4. Proposed product model

### Navigation: creation first, not departments

Two primary activities:

1. **Create** — start, run, change, save or reopen an invention.
2. **Missions** — optional guided starting points that open editable work in Create.

A persistent **Board dock** shows the selected Board, real/practice context, connection health, detected modules and **Stop**. Clicking it opens connection/module tests. Parent settings are a separate secondary route, not a primary play screen.

This deliberately revisits the earlier request to put Board Setup first in the old sidebar: the Board remains immediately discoverable, but is context for making rather than the first of five departments. It is a proposal, not a navigation change made during this review.

### First-use state flow

```text
Choose Connect Board or Practice
  → identify the actual Board (choose explicitly if ambiguous)
  → assess support, Companion readiness and trust
      new/unprepared → guided official setup, only required steps
      prepared but stopped → explain explicit official startup scope
      ready but unpaired → one understandable physical Pairing decision
      locked/denied credential access → expected app OS prompt/retry
      unsupported/busy → exact problem, preserve data, no guessed repair
  → module cards with live health
  → press “Try blue” / “Play short chirp” intentionally
  → “Make a rule” or “Try a Mission”
```

Do not automatically start the official Board app, upload firmware, reboot, set it as default, stop unrelated apps, enable SSH or reconfigure networks merely to meet the target time. Companion availability after power cycles is part of product readiness, but its startup policy needs explicit engineering and owner-approved behavior. “Retry” must not silently escalate to firmware operations.

### Module cards

| Module | Immediate feedback | Safe test |
|---|---|---|
| Buttons | A/B/C visibly change on fresh press/release; distinguish stale/unknown. | Press the physical buttons; no fake input injected into real mode. |
| Distance | Large value, chosen units, live range graphic, freshness/health. | Move a target; show actual changes, not a cached default. |
| Pixels | Color chooser and labeled output state. | **Try color** for a short disclosed duration, bounded brightness; Stop always available. |
| Buzzer | Clear sound warning; requested/finished state. | One short chirp on an intentional click; never autoplay on discovery. |
| Matrix | Show it only when supported and healthy; label onboard vs attached hardware accurately. | A bounded icon preview only after its Command is actually supported end-to-end. |
| Unsupported/unknown | “Not supported yet” / “Not responding” with appropriate help. | No controls that pretend an unsupported module works. |

Discovery is not electrical certification. Report what the Companion can actually identify and operate; do not claim automatic detection of every physical module, wiring correctness, or safe arbitrary circuits. Missing one module should disable only dependent tests/recipes, not all creation.

### Creation loop: connect → express → run → notice → change → keep

Start with a small, readable **event / condition / action** representation. Example:

```text
WHEN button A is pressed
THEN Pixels → blue, briefly

WHEN Distance crosses closer than 20 cm
THEN Pixels → red, briefly
     Buzzer → short chirp
```

Show the live input and highlight the rule that fired. State whether the program is running; make Run intentional and Stop persistent. A software Stop is not a certified emergency power disconnect. No auto-run after opening, reconnecting or resuming a save. A disconnected Mac stops execution and leaves the Board in Safe state; do not imply autonomous Board deployment exists.

Blocks, rule cards and a constrained text view are alternative representations to validate, not three editors to build immediately. A Scratch-like event/action interaction is useful inspiration, but this review is not a researched competitive evaluation of Scratch, MakeCode or other tools. Do not choose a block-editor library before proving that the interaction works for physical creations.

Starter Missions must produce the same kind of editable saved work as free creation. No separate special guardian editor that cannot become a different invention.

**“Creation” is a proposed domain concept not yet defined in `CONTEXT.md`.** Agree its relationship to Mission, Crew, program and saved work before adding persistence contracts. No domain schema is changed by this review.

### “Turn on a port” is a separate safety/engineering scope

Current `BoardClient` Commands expose Pixels, Buzzer and clear; no general GPIO output or power-rail switching exists. A port is not automatically a safe actuator or an on/off power supply.

Do not add raw port-number toggles as a cosmetic UI feature. Future supported digital outputs need an explicit pin/Capability allowlist, 3.3 V logic/current/load constraints, wiring guidance, known startup/Safe states, duration limits, backend+MCU validation and actual hardware acceptance. Motors/relays/unknown loads need appropriate drivers and separate safeguards. Router Bridge pins and unrelated systems remain unavailable. No shell, arbitrary firmware or unrestricted register writes through the creation editor.

## 5. Architecture: preserve depth, stop exposing the machinery

Keep:

- One reusable Companion; no firmware/app swap for every creation.
- Authenticated bounded Commands and fresh Observables, Router Bridge and independent watchdogs.
- Separate gameplay/management authority, exact-target verification and recoverable maintenance.
- Local-first persistence and real simulator/connected adapters at the `BoardClient` seam.

Deepen the **Board readiness module**: callers should ask for the selected Board's current readiness, allowed next action and module health, not orchestrate several overlapping guides. The implementation owns identity, credentials, transport, lifecycle and safe resume. A small interface must not hide firmware effects from consent.

Deepen the **creation execution module**: validated event/rule/action data, bounded scheduling, cancellation, run/stop, trace and safe reconnection behavior behind one interface. Both simulator and real Board adapters already justify a real seam; tests should exercise meaningful programs through that interface.

Needed interface improvements include explicit per-Capability health/freshness/reason and supported actions, rather than only a filtered list. A constrained program must validate allowed operations and resource limits natively as well as in the UI; a browser validator alone is not authority. Define loop/rate/time budgets, single-controller ownership and output limits before adding repetitions or concurrent rules. Keep input-dependent shutdown independent of a stalled frontend.

This preserves the intent of ADR-0001 (management separate from gameplay). It changes the **product's setup-completion/readiness contract**, not the security policy. Do not bypass the current protected checkpoint writer or falsely complete old LAN/reboot checks; design separate play-readiness evidence and explicit data migration. Do not erase existing Board identity, credentials, progress or unrelated apps.

## 6. Adversarial acceptance before declaring the redesign successful

| Scenario | Pass condition |
|---|---|
| Adult, prepared USB Board, no prior app knowledge | Without coaching, explains the next action and triggers one intentional physical response within the proposed target. No terminal, Linux sign-in or manual Keychain setup. |
| Brand-new Board | Sees honest vendor/setup requirements and firmware scope; no surprise upload, reset, credential guessing or false “ready.” |
| Solo maker / two / four participants | All can start; invented names/extra people and mandatory typing are unnecessary. |
| Seven-year-old / older sibling | Each can say what the Mission makes and identify an input, a rule and an output; older maker can compose a different behavior, not just recolor it. These require observed sessions, not checkbox declarations. |
| Only Pixels + Buttons present | Tests both and makes a button-to-light creation; unrelated missing Distance/Buzzer do not block it. Original full guardian variant remains explicitly unmet. |
| Module missing / stale / degraded / spoofed discovery | No NaN/default-as-live value or false physical detection claim. Only verified compatible capabilities are operable; specific dependent behaviors stop safely. |
| Keychain denied or locked | Clear storage-access explanation and explicit retry through the real app. No re-Pair prompt as a universal remedy, background prompt storm, key export or plaintext fallback. |
| Two Boards / wrong UUID / changed certificate / another controller | Explicit target selection and fail-closed trust/ownership. Never control whichever device answers first. |
| Stop during slow request / navigate / unplug / app crash | Pending execution cancels without late rearming; clear is attempted; independent watchdogs enforce Safe state. Verify timing physically, not just with mocks. |
| Reconnect / reopen saved creation | Work restored; execution and outputs remain stopped until explicit action. |
| Quiet or color-insensitive participant | No autoplay sound, output labels and non-color-only signals, usable keyboard/focus and Stop; assess actual accessibility rather than infer it from colors. |
| Save / edit / fork / restart app | Rules and names preserved locally; fork doesn't overwrite the original. A saved run is not a hardware verification record. |
| Guardian first screen | Maker can point to the physical goal, learning outcome, required modules and one next action before entering a name or explanation. |
| Guided → free creation | Fork the guided behavior, change its purpose, run it and reopen it without a new firmware upload. |

Automated checks remain essential for protocol, safety, persistence and regressions. They cannot establish fast first success, comprehension, enjoyable play, actual light/sound, electrical limits or clean-Mac trust UX.

## Source map

All references describe the reviewed baseline, not future implementations:

- Shell, navigation, Home, World Map, Hardware Lab and hidden connection panel: `app/src/routes/+page.svelte`.
- Initial auto-connect, retries, credential import and overlapping setup entries: `app/src/lib/board/ConnectionPanel.svelte` (`onMount`, `connect`, section markup).
- Twelve checkpoint instructions: `app/src/lib/setup/session.ts`; guide rendering: `SetupWizard.svelte`; native-proof/Crew UI: `ObservedSetup.svelte` in that directory.
- Every maintenance section and its consent/visibility behavior: `app/src/lib/board/BoardManagement.svelte`.
- All game availability: `app/src/lib/missions/catalog.ts`; all ten stage goals/conditions/hints: `treasure-guardian.v1.json` in that directory.
- Mission layout, control buttons, forms and save feedback: `app/src/lib/missions/MissionPanel.svelte`.
- Three-member persistence/validation and required written notes: `app/src/lib/missions/session.ts` (`progressSchema`, `setCrew`, `recordRemix`, `reflect`, `advance`).
- Evidence, self-test button semantics and command-derived signal state: `app/src/lib/missions/guardian.ts` (`receive`, `getStatus`, `beginSelfTest`, `tick`).
- Available action surface: `app/src/lib/board/types.ts`; discarded health reasons/null mapping: `connected.ts` (`getSnapshot`) in that directory.
- Credential-read error classification: `app/src-tauri/src/connection.rs` (`Connections::profile`); installation private-key storage: `app/src-tauri/src/session.rs`; Pairing/SSH trust: `pairing.rs`.
- Existing complete setup contract: `docs/architecture/setup-checkpoints.md`; trust separation: `docs/adr/0001-separate-management-and-gameplay-channels.md`; historical Mission requirements: GitHub #8.

## 7. Recommended sequence and decision gates

1. **Agree the product contract in this review.** No more incremental maintenance-panel polish as the main UX solution. Replace the current completion-focused brief with first success and invention.
2. **Throwaway clickable prototype:** one Board/module sheet, one creation workspace, one outcome-first guardian brief. Use clearly labeled simulated hardware; no credential access or real output. Compare rule cards versus blocks on the same simple behavior. Do not reuse production progress or call this hardware acceptance.
3. **Observe the owner and makers:** can they identify the next action, make one rule, predict it and change it? Record confusion and revise before wiring the prototype into the secure backend. No claim of validation until observed.
4. **Deliver real connect-and-try:** readiness/error classification, explicit secure trust, independent module tests and persistent Stop. Prove the prepared-Board path on actual UNO Q; retain the honest unsupported/new-board handoff.
5. **Deliver the smallest real creation loop:** button → light; save/open/fork; same simulator/real adapters; strict safe execution contract. No giant new platform or arbitrary code runner.
6. **Rebuild Treasure Guardian as an editable guided creation:** outcome-first, flexible participation, optional prose, extensions for warning zones/buttons/sound. Version old progress safely. Add other games only after each has a clear purpose and a successful small loop.
7. **Family/hardware/clean-Mac acceptance and release gates:** keep #21 honest; #25 dependency/legal evidence and #12 production signing/notarization/publication approval remain prerequisites to public binaries.

### Alternatives deliberately rejected

- **Only simplify today's menus:** fastest cosmetic change, but leaves free creation absent and families guessing about the guardian form.
- **Build a full generic Scratch clone immediately:** huge scope before proving the hardware interaction; risks another tool with no reliable first success.
- **Remove authentication/Keychain to make setup shorter:** moves friction into a credential and hardware-control vulnerability. Improve consent, storage errors and signing instead.
- **Auto-flash/reboot/reset until connection works:** conceals high-impact side effects and threatens data/other apps; not an acceptable onboarding strategy.

### Open decisions, not blockers to the audit

- Prefer rule cards or draggable blocks after a small observed comparison, not a large implementation debate.
- Confirm the smallest initial output set: existing module capabilities first; general GPIO is separate scope.
- Agree a compatible new Mission/creation-save model and how old three-member/progress records are preserved.
- Define explicit Companion startup policy after power cycles without silently claiming/overwriting another Board app's default role.

**Immediate next deliverable should be the clickable product prototype, not another hardware troubleshooting checklist.**
