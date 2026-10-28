# Navigation review: one workspace, an obvious next action

Date: 2026-10-27. Reviewed baseline: `4f64b3f` (merged PR #46). Tracking: [#47](https://github.com/josebarrueta/tinkercrew/issues/47), related to #38. **Review and implementation proposal only; no production UI or hardware code changed.**

## Executive recommendation

Keep the owner's chosen journey: **try modules → build a behavior**. Make those two visible views of the **same Creation**, not places the maker has to rediscover through scrolling.

Use **Create** and **My creations** as the two primary destinations. Put **Try modules / Build** inside Create. Offer **Start from a Mission** as a secondary starting-point action; today there is only one implemented starter. Keep **Board / Practice context and Stop outputs** persistent, and move **Parent settings** to a consistently placed secondary entry.

The largest problem is not the number of top-level buttons. It is that the app mixes **destinations, editing actions, connection state and maintenance procedures** on long pages, without consistently protecting the current draft or returning the maker to the right place.

### Protect the part that now works

The owner confirmed functional success for **Buttons, Distance, Pixels and Buzzer** in #45/#21. Treat that as the verified product baseline, not another experiment to redesign. PR #46 also has 198 automated checks and targeted live native/Bridge evidence. Owner confirmation is not a claim about Matrix, measured calibration, power-cycle behavior or complete Creation/Mission acceptance.

This review does **not** authorize transport/authentication changes, firmware uploads, resets, new capabilities, credential migration, altered output limits, weakened guards, or changes to protected setup-completion evidence. Keep the password-free USB path, private Unix TLS ingress, shared ownership probes, bounded inbound relay, freshness accounting, one-controller ownership, explicit Run, cancellation and independent watchdogs intact.

## Method and confidence

- Read the current Create workspace, connection flow, Parent tools, setup guide, earlier shell, persistence/execution interfaces and applicable ADRs.
- Ran browser walkthroughs at 1280×800, 1180×800, 1024×800 and 900×800. Also inspected 390px browser behavior; this is **below** the native app's declared 900px minimum, not a claim that mobile is a supported product target.
- Exercised empty and populated libraries, an eight-rule Creation, Mission/new/open replacement, internal navigation, browser Back, Parent handoffs, legacy return and late connection completion.
- Used isolated browser storage. Native-looking connection scenarios used **fully intercepted, simulated IPC**; no actual ADB, Keychain, Board, output, user save or native app operation occurred.
- Local ignored evidence: `artifacts/qa/navigation-review-20261027/` contains screenshots and `audit.json`. These are review evidence, not physical or family acceptance.
- This is a source/browser self-review, **not** an observed child study, an independent accessibility audit or a competitive research study. Task/time targets below are proposed, not measured family outcomes.

The earlier [product review](product-adversarial-review-2026-10-27.md) reviewed `fe90fb1`. Several of its problems are now solved: real Create/Workbench, independent module tests, explicit USB intent and password-free play. Do not treat that older document's implementation inventory as the current app.

## Findings, ordered by impact

P0 = potential loss of the maker's work. P1 = significant flow/orientation problem. P2 = secondary simplification. These are product priorities, not security vulnerability ratings.

| ID | Priority | Evidence in the current app | Recommended change |
|---|---|---|---|
| N01 | **P0** | **Replacing a draft is unprotected.** Browser walkthroughs showed Make my guardian, New creation and opening a saved Creation replacing unsaved title/rules without a save/discard decision. `fresh`, `guardian`, `openCreation` overwrite the in-memory draft. | One replacement flow: **Save and continue / Discard changes / Keep editing**. If saving fails, stay with the original draft. No prompt for simply switching Try/Build because that is the same work. |
| N02 | **P1** | **The next action is below the fold.** At 1280×800, Build with blocks starts around document Y1041. In a one-rule editor, Run is Y883 and Save Y1196. With eight rules, Run is Y4077 and Save Y4389. | Stable Create subnavigation and an editor toolbar with name, save status, Save and Run. Keep global Stop visible. Essential controls must not move farther away as rules grow. |
| N03 | **P1** | **Screen changes do not reliably orient the user.** After opening Parent settings from the footer, scroll remained 861px and the new H1 was roughly 739px above the viewport. Keyboard focus stayed on the footer action. | Explicit transition behavior: focus the new view heading and position it below the sticky header; retain appropriate scroll only when returning to the same view. |
| N04 | **P1** | **Create does not identify the current view.** It highlights both module and block views; clicking it always goes to modules. There is no persistent way to select Build except an in-page action. | Create owns two labeled tabs: **Try modules / Build**. Create from another destination returns to the current Creation's last view; a fresh launch starts at Try modules. Clicking an already selected tab is a no-op, not an unexpected reset. |
| N05 | **P1** | **Saved work is a footer section, not a destination.** It is repeated below Try, Build and Missions, and is farther down as a program grows. Opening a saved item can silently replace the current draft. | **My creations** gets a stable destination. Show titles and useful existing information such as rule count; open stopped through the draft guard. Do not invent last-edited timestamps that version1 does not store. |
| N06 | **P1** | **Connection presentation is not one coherent sheet.** Close is in an otherwise empty sibling `connection-sheet`; the real `ConnectionPanel` is separate and pushes the workspace down. Browser found no dialog semantics and no close control inside the actual panel. | One compact Board sheet with its own title, close, focus/keyboard behavior and state-specific actions. Keep it contextual; do not navigate away from the working Creation just to see status. |
| N07 | **P1** | **Close does not cancel pending connection intent.** In intercepted-IPC replay, closing the panel and then resolving the pending connect still adopted the late handle and switched Practice to Real Board. No outputs were activated, but the context changed after dismissal. | Closing a pending attempt cancels its intent epoch and closes only its eventual specific handle. Closing already-connected details just dismisses presentation. Reuse existing native scoped-handle cleanup; do not create another credential path. |
| N08 | **P1** | **The only global Board action is Connect, even when connected.** Merely trying to inspect connection context routes through a stop/reconnect flow; the textual status is not an inspection action. | Disconnected: **Connect Board**. Connected: **Board: connected via USB**, opening read-only details. Reconnect, disconnect or change target are explicit separate actions. |
| N09 | **P1** | **Parent setup loses the intended task.** The shortcut from the compact connection panel changes to Parent settings but leaves the maintenance task closed. The user lands back at Connect & play and must choose again. | Carry a task and return target: `Open Parent setup → the relevant task → Return to my Creation`. Contextual repair links open the matching task, not a generic settings landing page. |
| N10 | **P1** | **A destination label is wrong.** Parent's Continue to Mission invokes `navigate('blocks')`; replay landed at Build a behavior, not the Mission brief. Connecting from Parent also forces the module screen instead of restoring the originating editor. | Replace with **Return to my Creation** and restore the recorded location. Starting a Mission is a different action that explicitly creates a starter draft. |
| N11 | **P1** | **Legacy entry crosses into a second app.** The earlier Mission link opens `/legacy` with five departments, including old World Map/Hardware Lab/setup. Returning produced a new default draft, not the previous working context. | Keep old saves accessible, but label the transition clearly and guard unsaved work. Restore current draft on return. Do not put legacy departments in normal navigation or silently reinterpret old completion. |
| N12 | **P1** | **Errors can dominate or move the route to success.** Source allows global problem, runtime error, generic connection warning and module-local error to coexist. The generic warning says “Reconnect your saved Board” although normal USB permission is temporary. | One actionable primary error at the affected task, a compact Board status indicator and optional details. Keep the original cause. Do not route every play failure to general maintenance. |
| N13 | **P1** | **Position is mostly visual.** Root navigation uses active CSS without `aria-current`; internal views have no route/history representation and no heading focus transition. Browser Back from repeated root view changes went to `about:blank`, not the prior view. | Explicit current-location semantics, keyboard/focus behavior and sensible Back/Forward. In the native app, provide visible return controls too; browser history alone is not a navigation UI. |
| N14 | **P2** | **Practice, simulator and reconnect language compete.** There is Use practice even in Practice, plus Use simulator / stop reconnecting inside connection UI, although ordinary USB does not automatically retry. | One vocabulary: **Practice / Real Board**. Put mode switching in the Board context. Remove redundant already-in-Practice actions and stale retry wording. |
| N15 | **P2** | **Parent settings mixes unrelated adult tasks.** It leads with Connect & play, exposes both a 12-checkpoint guide and maintenance procedures, and nests Mac upgrades under Board management. | Small Parent landing page with task groups: Board setup, Wi-Fi/network permission, Software updates, Troubleshooting. Isolate destructive recovery. Keep expert details only inside the applicable task. |
| N16 | **P2** | **A one-item Mission is presented as a primary destination.** Its brief is useful and the starter already uses the same editor, but it competes with the much more frequent task of finding saved work. | Make **Start from a Mission** a secondary action within Create/New. Retain the outcome/learning brief. Promote Missions to top-level later only if a real catalog justifies it. |
| N17 | **P2** | **The palette has two ways to add a rule.** Input buttons add a rule while conditions/outputs are explanatory text, and Add rule creates another default A rule. This adds navigation-like choices inside an already long editor. | Keep a single clear Add rule path with an input choice, or make palette behavior consistently actionable. Do not change the bounded language or introduce a block-editor library for this navigation pass. |
| N18 | **P2** | **Library capacity has no visible management flow.** The UI advertises 24 entries, but offers no remove/archive action; the save path rejects a 25th independent Creation. | Include a subsequent scoped library-management task. Preserve revisions and deliberate deletion/recovery; do not bundle an unreviewed storage migration into navigation work. |

### What already works and should remain

- C-style module cards → B-style readable composition matches the chosen product direction.
- Try/Build round-trip retained the in-memory title and rules in the walkthrough. Preserve this rather than resetting on each view change.
- Missions already open editable rules in the same editor; no extra engine or firmware is needed.
- Opening saved work does not Run it. Explicit Run, Stop and stopped state on edits/context changes are important invariants.
- The global Stop button is discoverable and stays in the header. Do not hide it in a menu or behind a dialog.
- Parent actions are separate from everyday USB play. Existing exact-target consent and recoverable maintenance remain necessary.

## Recommended information architecture

```text
Persistent header
  Tinkeroo | Create | My creations      Board / Practice      Stop outputs      ⚙

Create — one current Creation, not separate departments
  [Try modules] [Build]            Start from a Mission

  Try modules
    Buttons / Distance / Pixels / Buzzer
    same known safe controls and truthful readings
    clear next action: Build a behavior

  Build
    Creation title + saved/unsaved state       Save       Run
    current bounded rules
    compact live inputs / reported outputs / execution feedback

My creations
  saved Creations → Open stopped / Make a copy
  New creation → draft-replacement guard when necessary

Board sheet — contextual, not a primary destination
  status + current identity/context + applicable next action
  read-only details / deliberate connection actions / Practice switch
  contextual Parent help only if needed

Parent settings — secondary gear entry
  Board setup | Network & saved permission | Updates | Troubleshooting
  advanced/recovery only inside the applicable task
```

**This deliberately revises the earlier Create/Missions top-level proposal**, not either security ADR. Saved work now exists and is a frequent task; only one current Mission starter exists. The owner's **C→B** preference is retained. If a later catalog makes Missions a frequent browsing task, it can become a third destination then; do not build an empty catalog now.

Rejected alternatives:

- A new Home/dashboard before modules: adds a mandatory click without making anything easier.
- Five top-level departments (Home, Hardware, Build, Missions, Setup): repeats the legacy fragmentation.
- Three hidden modes called Playground/Workbench/Free Build: teaches implementation vocabulary rather than the maker's task.
- Only moving buttons upward: useful first patch, but does not solve draft loss, wrong return targets or late connection intent.
- A complete editor/Blockly/persistence rewrite: much larger regression surface than required for navigation.

## Journey contract

| Journey | Desired behavior |
|---|---|
| First launch, no Board | Try modules in clearly labeled Practice; no credential lookup, native discovery, real output or forced setup on mount. |
| Prepared single USB Board | One explicit Connect action; progress/cancel visible if needed. Success returns to the invoking view, stopped. No Keychain/network fallback or setup checklist. |
| Already connected | Inspect status without disconnecting/reconnecting. Never Run just because details closed or health recovered. |
| Try → Build → Try | Same draft, same Board context; different views. Stop active execution on a real view change, retain work and explain stopped state. |
| Same selected tab | No-op. Do not recreate the runtime or acquire/release authority merely because its current tab was clicked. |
| Save | Save the current draft in place, with success/failure beside a stable control. No navigation or automatic output change. |
| New / open another / use Mission starter | If dirty, offer Save and continue / Discard / Keep editing. Save failure cannot discard the draft. New/open/fork remains stopped. |
| Browse Missions | Browse without changing the current draft. Choose a starter through the same replacement flow, then open Build with the physical goal retained. |
| My creations → current work | Return to the current Creation/last view. Opening another item is explicit, not an implicit reset. |
| Connect from Build | Restore Build, title, rules and relevant scroll/focus after connection. It must not redirect to modules or an unrelated Mission. |
| Close while connecting | Cancel that attempt; late native handle is disposed specifically. Do not adopt it or silently switch context after dismissal. |
| Connection fails | Preserve work and show one local next action. Do not suggest reset/re-Pair as a generic remedy; ending maintenance/reconnecting never rearms. |
| Parent task | Open the relevant task, stop outputs, retain draft and originating location. Return explicitly to that place. Do not force full guide completion for USB play. |
| Legacy adventure | Explicitly leave the current workspace after protecting the draft. Preserve old saves and restore current context on return; never merge completion semantics silently. |
| Back/Forward / keyboard return | Restore logical location and draft, never hardware authority. Focus the correct heading/control; keep Stop reachable. |

## Layout and language changes

1. **Move navigation and action controls, not the working hardware behavior.** Try/Build tabs are visible before scrolling. Save and Run stay in the editor toolbar as rules grow. Stop remains globally available.
2. **Keep the robot preview subordinate.** The current preview occupies a large persistent card while progression, Save and the library move below it. Use a compact output/status area beside rules or beneath module feedback, not another destination.
3. **Use names consistently.** Create = activity; Creation = saved work; Try modules / Build = views; Mission = optional starting point; Board / Practice = context; Parent settings = adult tasks. “Continue to Mission” must not mean “open whatever rules happen to be in memory.”
4. **Make saving visible without adding a form gate.** Default untitled work can be edited immediately. Name/save status belongs in Build's stable toolbar, not beneath the last rule. This review does not require changing manual saving to autosave.
5. **Treat empty, loading and unreadable libraries differently.** Corruption must not look like “no Creations.” Keep the existing fail-closed write protection and preserve work when suggesting recovery.
6. **Use a proper contextual panel.** Choose either a nonmodal side panel with explicit region/focus behavior, or a dialog with appropriate focus containment/Escape/return focus. Do not apply `role=dialog` without implementing its interaction contract. Keep close and content together.
7. **Do not make safety text an extra navigation obstacle.** Contextual warnings stay at relevant actions, with precise firmware/reset consent retained. Long engineering detail belongs in expandable Parent diagnostics, not the default module-to-rule path.
8. **Supported-window acceptance matters more than a hypothetical phone UI.** At 900px (the declared native minimum), Run/Save were roughly Y1337/Y1675 even with one rule. Fix this. The 390px browser overflow comes from the explicit 900px body/native minimum; mobile support is a separate product decision, not an immediate hardware regression.

## Implementation sequence: small, reversible slices

### 0. Lock the verified baseline

No implementation in this review. Before navigation changes, attach the current owner witness and 198-check baseline to the implementation ticket. Add protocol-backed regression coverage for the complete four-module UI and navigation round-trips before refactoring; the current preview regression is valuable but is not by itself comprehensive Buttons/Distance journey coverage. Use a known prepared Board for later owner smoke tests; ordinary CI must not run owner-only physical diagnostics or activate hardware.

### 1. Protect work and correct obvious destinations

- One guarded draft-replacement operation used by New, Open and Mission starter.
- Stable Save/Run/title controls and visible Try/Build selection.
- Correct Return to my Creation behavior, scroll and focus.
- Avoid persistence/schema, runtime or transport changes in this slice.

### 2. Make saved work and workspace location explicit

- Create / My creations primary navigation, Mission starter as secondary action.
- One current draft retained through ordinary navigation.
- Route/history-aware location, or an equivalent explicit history model, without program data/credentials in URLs. Use existing SvelteKit routing rather than inventing a second router.
- One shared UI owner for Board context and current draft. Do not instantiate another connected client/poller on every route.

### 3. Consolidate Board presentation

- Same verified connection implementation behind one compact contextual panel.
- Status inspection separate from reconnect; pending dismissal cancels intent.
- Contextual failure/Parent handoff returns to the original view.
- No new backend credential or readiness protocol.

### 4. Group Parent tasks and fence legacy paths

- Task landing page with only relevant actions expanded.
- About/Mac updates separate from Board networking/firmware/recovery.
- Advanced technical and destructive actions remain explicit and guarded.
- Earlier adventure remains available with draft protection; no progress migration in this pass.

### 5. Validate navigation, then refine

One short maker session should cover connect → try → build → save → reopen → change → Stop without coaching. Separately check Parent recovery/return, keyboard use and smaller supported windows. Proposed targets: Try↔Build and reaching My creations take one visible action; primary Run/Save/Stop require no document-bottom hunt at 1 or 8 rules. These are acceptance goals, not claims of observed success.

## Regression gates: navigation must not break verified modules

### Existing baseline to keep

- `app/e2e/module-protocol.spec.ts`: actual rendered UI/runtime/connected adapter → real TLS/Unix Companion, including delayed responses and notification-before-ack SDK fixture; no unconditional-success gameplay mocks.
- `app/src/lib/board/connected.test.ts`: overlapping snapshot coalescing, genuine restart failure, preserved readable session after command rejection, input validity/freshness and event handling.
- `app/src/lib/creations/runtime.test.ts`: explicit Run, edit/Stop/cancellation, output TTLs, conditions and stale dependent inputs.
- `app/src-tauri/src/connection.rs`: native concurrent guard checks, genuine management exclusion, TLS identity/authentication and bounded typed Commands.
- Companion protocol/Core tests: one-use USB authority, TCP rejection, lease ownership, pre-lease safe clear, maintenance separation, timestamped stale data and watchdog behavior.
- Existing Creation persistence/revision, legacy Mission, setup/recovery and notice/package tests remain active. Do not delete old regressions merely because navigation changes labels.

### New navigation checks at the rendered UI / IPC seam

| Check | Required result |
|---|---|
| Switch Try/Build with edited rules | Identical draft remains; active execution stops; no new connection or lease. |
| Repeatedly select current view | No new runtime, transport, lease or lost work. |
| Open My creations and return | Current draft/view retained; no automatic Run. |
| New/Open/Mission with dirty draft | Correct save/discard/stay flow. Failed save stays on original work. |
| Save/Open/fork/restart | Existing work remains readable; fork never overwrites original; opened work stopped. |
| Return after Parent task | Correct Creation/view/title/rules and focus, not generic modules or an unrelated Mission. |
| Cancel pending connection, resolve late | Late handle cleaned specifically; no context switch, command, lease or saved credential lookup. |
| Inspect already-connected Board | No reconnect/disconnect and no authority changes just to view details. |
| All four modules after navigating away and back | Protocol-backed healthy input fixtures update physical-button indicators and Distance; missing/stale fixtures remain truthful. Pixels/chirp stay bounded and require explicit action. Fixture success is not a new physical witness. |
| Rendered module preview while background reads run | Blue/chirp/Stop path still succeeds through real protocol, with no false restart/maintenance block. |
| Active maintenance / missing module / true disconnect | Existing fail-closed behavior preserved; no reset/re-Pair prompt or automatic rearm. |
| Keyboard, Back/Forward, supported window sizes | Logical current location, visible heading/focus, reachable Stop and stable primary actions. |
| Earlier adventure round-trip | Saved legacy work untouched and current draft preserved; outputs do not resume. |

Keep assertions about **behavior and authority**, not only button names or screenshots. Updating a selector after an approved label change is fine; replacing actual protocol rejection/latency fixtures with success mocks is not.

A final **owner-controlled** smoke check should repeat the four known module tests after the navigation-only change and exercise a small Creation's Run/Stop. No unattended physical output test, and no automatic completion of old protected setup/Mission evidence. Owner witness and automated regression results remain distinct.

## Source map / scope exclusions

- Current view, draft replacement, save, header/footer, Mission starter, library placement, return behavior: `app/src/lib/creations/Workspace.svelte` (`navigate`, `fresh`, `guardian`, `openCreation`, `parentSettings`, ConnectionPanel callbacks and markup).
- Connection cancellation/presentation and Parent handoff: `app/src/lib/board/ConnectionPanel.svelte` (`connectUsb`, `openConnection`, `openParentSetup`, `useSimulator`, exported interface and section markup).
- Parent task inventory: `app/src/lib/board/BoardManagement.svelte`; guide/proofs: `app/src/lib/setup/{SetupWizard,ObservedSetup}.svelte`.
- Route/layout/history ownership: `app/src/routes/+layout.svelte`, `+page.svelte`, `/legacy/+page.svelte`.
- Current body/native window minimum: `app/src/lib/theme.css`, `app/src-tauri/tauri.conf.json`.
- Execution/persistence contracts: `app/src/lib/creations/{runtime,storage,definition}.ts`; verified trust separation: ADR-0001/0002 and `docs/architecture/usb-play.md`.

Out of scope: a new block language, autonomous deployment, GPIO/power controls, Matrix delivery, Mission completion rewrite, Crew model migration, new signing/release claims, runtime/provisioning changes or direct manipulation of Board/session credentials.
