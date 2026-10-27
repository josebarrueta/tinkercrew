# Throwaway creation-workspace prototype — #38

Question: **Which structure makes connect → try → invent obvious without making a family navigate engineering setup?** Three structurally different layouts share one small simulated behavior so the comparison is about the interface, not different capabilities.

Run from the repository root:

```sh
npm --prefix app run prototype:create
```

Opens `http://127.0.0.1:5182/prototype/create?variant=C`.

- `?variant=A`: **Studio** — rule card and invention preview, Board/module rail alongside.
- `?variant=B`: **Workbench** — click-to-add block palette, program canvas, separate input/preview column. Not drag-and-drop and not a general programming editor.
- `?variant=C`: **Playground** — try modules first, then a sentence-like rule workspace below.

Use the floating arrows, or left/right keyboard arrows outside controls. Switching layouts stops execution, retains the current draft and shows the prototype state. Each URL can be opened directly. The current Tinkeroo palette/type/branding are retained to judge layout in context; primary navigation deliberately replaces the five-screen production shell.

## Try this

1. Connect the **demo** Board. Try blue/purple, press A/B/C, move the pretend visitor, preview a chirp.
2. Click **Build with blocks** to move from C to B. Run the default rule; press A to see a one-second simulated blue light. Change its trigger/color or add a chirp; editing stops execution.
3. Open **Missions**. Read the visible physical goal, learning outcome and required modules. “Make my guardian” opens an editable distance-to-light starter, not a form/checklist. Run; move from 60 cm to below 20 cm.
4. Stop. Keep a draft in demo memory and reopen it; it stays stopped. Reloading clears everything.
5. In prototype scenarios, remove Distance; a button-to-light rule still works. A distance rule explains its missing requirement. Disconnect/reconnect never silently runs a program.

## Hard limits

- **All hardware and detection are pretend.** No native invocation, Keychain, pairing, USB, LAN discovery, Board API, audio playback, firmware, shell interface or real output.
- Buzzer preview is silent visual feedback for 250 ms. Pixel preview lasts one second. Stop cancels simulated outputs. C is reserved for stopping the demo, not offered as a programmable trigger; this is not a claim of an independent physical emergency stop.
- State and demo saves live only in browser memory. No localStorage, cookies, databases, actual Crew/Mission progress or credentials are read/written.
- The isolated route is intentional: embedding in production Home would mount its native connection effects. This is a replacement-workspace exploration, not a live management skin.
- Content/switcher are development-only, but **do not merge this branch into main or ship this route**. Prototype code lacks production safety/validation contracts. Implement a selected direction properly against the secure backend later.
- No general GPIO, power switching, independent Board execution, multi-rule compiler or actual saved-creation format is implemented.
- This does not solve actual first-time vendor provisioning or OS credential consent by pretending they do not exist. Those need a separately implemented state-driven readiness flow.

## Owner-selected direction / validation

The owner selected **C for starting, B for learning/composing with blocks**: “i like c for starting , I like b as block module to learn also some logic and connecting more things”. This is owner design feedback, not family usability or physical acceptance.

The prototype now has a direct path: C's module playground → **Build with blocks** → B's workbench, with **Modules & experiments** to go back. Both retain the same connected demo kit and draft, but navigating stops execution. Mission starters and kept drafts open in B. A remains on this archive branch for comparison, not part of the selected product path.

The next block-editor goal is **multiple inputs, conditions and outputs**, not only recoloring one supplied rule. The current prototype still supports one trigger and a Pixels action plus optional chirp; it is not a multi-rule engine or general hardware programming implementation. Define readable logic, dependencies, execution trace, output conflict policy, validation and independent Stop/watchdog behavior before adding that production engine.

No further layout choice is required. Validate the combined interaction and implement its smallest safe real vertical slice; do not promote throwaway code or claim the broader editor already exists.

Svelte/TypeScript check passed with zero warnings. Local browser smoke walkthrough exercised connection, A→light, Stop, memory-only keep, all three layout switches, and Guardian distance→light; no browser exceptions or localStorage keys appeared. Screens visually inspected at 1440 px. No automated test suite was added for throwaway code; no real hardware or family acceptance was performed.

Source capture: branch `prototype/creation-workspace`. Context: #38 and the product review in `docs/design/product-adversarial-review-2026-10-27.md`. Keep this branch as primary source; only validated decisions and rewritten production implementation should reach main.
