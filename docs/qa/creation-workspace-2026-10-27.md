# Creation workspace implementation QA — 2026-10-27

Scope: #40, implementing owner-selected C module Playground → B block Workbench from #38. Baseline `3f7f5bc`; prototype code remains on its throwaway branch. No Board operation or user credential access was performed during this implementation.

## Delivered software

- Default Create Playground, explicit Practice/real context, live supported module health and input display; intentional brief light/chirp tests over the existing BoardClient/native/HTTPS protocol.
- No credential lookup on default Home. Explicit Connect mounts/loads its deferred connection flow. Denied/locked secure storage is not missing Pairing; it stops probing/retry and retains records. Choosing Practice cancels late lookup/probe results rather than silently connecting later.
- Workbench input palette and editable WHEN / ONLY IF / THEN blocks: up to eight rules, A/B and distance crossings, optional distance condition, color/chirp/both outputs. C/global Stop, serialized authority, generation cancellation, deterministic later-rule-per-output conflict handling, coalescing and cooldown. Programs are bounded data running on the Mac, never code/firmware deployment.
- Local, versioned Creation library; native existing SQLite save interface under a new namespace, separate browser nonsecret storage. Save/open/fork, stale-window rejection, corrupt-library preservation, stopped reopen/copy, original Mission row untouched.
- Goal-first Guardian starter in the same editor; no fixed Crew size or required written answers. Earlier guided Mission and its saved contract are preserved at `/legacy`, with a return link. Prior maintenance/recovery/mission tests still run there; new default-route tests are additional, not replacements for the earlier guarantees.
- No dependencies/lockfiles, Companion source, firmware, native trust identifiers or credential format changed. Native changes distinguish credential-read errors and test storage namespace isolation.

## Automated results

Final full report: `artifacts/qa/20261027T141659Z-35063.md` (ignored local artifact; tested dirty workspace before commit).

**180 automated tests passed:**

- Frontend/runtime: 37.
- Native: 35; two exact-consent owner-only physical diagnostics ignored, never run wholesale.
- Companion: 39.
- Browser: 30 (including eight new Creation journeys plus all earlier-route regressions).
- Management/provisioning/release helper: 38.
- Detached release verifier: 1.

Also green: Svelte/TypeScript (zero errors/warnings), formatter, Rust lint, dependency inventory freshness, Python compilation, app build and packaged disclosure checks. Native/Python checks used the check script's pinned dependency environment.

Built private development app and DMG; strict code-signature seal check and actual read-only mounted DMG app/Applications-link/disclosure checks passed. Ad-hoc signing is not Developer ID/notarization or clean-Mac acceptance. No binary was published.

Meaningful red/green cases included missing initial UI, independent Pixels TTL while leases continue, invalid fresh-looking inputs being incorrectly advertised/accepted, unreadable-library raw parser errors, late credential lookup returning after choosing Practice, and explicit global Stop when no Creation was running. The initial UI journey also found and fixed cloning a Svelte proxy on reopen. Final navigation regression coverage ensures the brand/Create action preserves an unsaved draft rather than performing a full-page reload. Regression coverage includes pending-lease/read cancellation without later positive authority/output effects, dependent freshness, conditional multi-rule output resolution, preservation of the original when copying, other-window save conflicts and old Mission-data isolation. These tests use public approved seams, not private helper inspection or a physical Board.

Screens captured and visually inspected under `app/test-results/creation-the-new-workspace-*/`: Playground, Workbench and Mission brief. The final Workbench uses compact live inputs/outputs rather than repeating the whole module-test panel; Stop remains sticky and keyboard accessible. This is a narrow visual/interaction check, not a complete accessibility study.

## Self-review

### Standards

Reviewed trust/management separation, no arbitrary execution/IPC, fixed native operations, secret-free frontend saves, cancellation, schema bounds, file/storage compatibility and documented test seams. No hard documented-standard violation found. Deliberate compatibility cost: the earlier shell is retained as an explicit separate route rather than rewriting old progress/evidence semantics. The workspace is a relatively large UI module; its execution and persistence logic are separate deep modules, not embedded shell/network logic.

### Spec

Implemented the bounded #40 contract and selected C→B path. The previously rejected pattern of simulated Free Build prompt text is replaced by real bounded composition and save/reopen. No general-purpose language, GPIO/power switching, matrix block, autonomous deployment, unlimited library, virgin-Board auto-provisioning or actual new-game catalog is claimed. Official setup/startup remains explicit; this UI does not falsify protected LAN/reboot/hardware checkpoints. #38 remains the broader product direction, not a claim that every future invention tool is complete.

This is self-review only, not an independent review/security/legal audit.

## Not run / not proven

- Actual UNO Q authenticated connect → module previews → multi-rule execution and witnessed light/sound/input response in the new UI.
- Real Keychain prompt/reinstall behavior, new/unprepared Board handoffs, family comprehension or the proposed 60-second first-response target.
- Electrical safe-off/latency/current limits; software Stop is not a physical power cutoff.
- Clean-Mac Gatekeeper/notarization, Intel/Universal acceptance or public binary approval. Existing #12/#25 release obligations remain.

No firmware compile/upload/start, default-app takeover, reboot, reset, rekey, ACL change or raw-port test was requested or performed. Physical/family acceptance stays open in #21/#35. A previous official restart is historical evidence, not a new workspace pass.
