# TinkerCrew

Build. Code. Imagine.

## Product prototype

Open [`prototypes/treasure-guardian.html`](prototypes/treasure-guardian.html) directly in a browser to try the throwaway Treasure Guardian family mission prototype.

Use [`prototypes/treasure-guardian-playtest.md`](prototypes/treasure-guardian-playtest.md) to facilitate a family session and record observations in [`prototypes/treasure-guardian-findings.md`](prototypes/treasure-guardian-findings.md).

## macOS app

Launch the Tauri/Svelte app with `cd app && npm install && npm run dev:app`. See [`app/README.md`](app/README.md) for checks and production builds.

## Checks and safety

Run `bash scripts/check-mvp.sh --build` for repeatable software checks and a versioned local report. See [`docs/qa/recovery-safety.md`](docs/qa/recovery-safety.md) for the pending physical acceptance matrix. Hardware wiring limits are also shown in the app's build stage and Parent Settings.

## UNO Q Companion

The reusable Board hardware service lives in [`companion/`](companion/README.md). It includes the Arduino App manifest, STM32 sketch, authenticated Linux API, local simulator, tests, and the pending physical acceptance checklist.
