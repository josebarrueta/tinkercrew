# Dependency disclosure and review gate (#25)

**Status: tooling/inventory foundation; compliance review PENDING.** The checked-in inventory is conservative and the evidence manifest is deliberately empty. Every component therefore remains pending. The disclosure is **not** a substitute for full upstream notices, source-availability obligations or owner/legal approval. No production release or physical acceptance is authorized.

## Files and scope

- `DEPENDENCY_INVENTORY.json`: deterministic records for **all** Cargo lock packages (including the workspace) and non-root npm lock entries. Includes build tools and optional/foreign-platform packages; this is not a binary-inclusion graph or SPDX SBOM. Cargo license expressions intentionally remain unknown until supplied by reviewed evidence; npm expressions are declarations, not compliance verdicts.
- `evidence.json`: explicit classifications and verified upstream license/source evidence. Missing entries remain pending; unknown/duplicate components fail. Source texts must be copied unchanged into `licenses/`, with exact SHA-256/provenance reviewed. No package scripts, network downloads or automatic license-policy decisions run.
- `DEPENDENCY_NOTICES.txt`: deterministic disclosure including full verified source texts for classified included components, explicit reasons for exclusions, and conspicuous pending components. It is labelled **NOT RELEASE APPROVAL**, even if coverage later becomes complete.
- `review.json`: currently `pending`. The tool **never creates or renews approval**. A matching human review record is required for binary-download sharing and release preparation, independently of Apple/update signing and protected release-environment approval.

Inventory/source freshness is checked in routine software QA. Both inventory and disclosure are packaged at `Tinkeroo.app/Contents/Resources/third-party/`; Finder → Show Package Contents exposes these readable files. The app and read-only DMG checks verify exact resource bytes. Changing notices after signing invalidates the seal: complete evidence before building/signing.

## Commands

From the repository root, Python 3.11+:

```sh
# Explicit regeneration after dependency/evidence changes; does not approve anything.
python3 scripts/dependency_notices.py inventory --root . --output docs/release/third-party/DEPENDENCY_INVENTORY.json
python3 scripts/dependency_notices.py notices --root . --inventory docs/release/third-party/DEPENDENCY_INVENTORY.json --evidence docs/release/third-party/evidence.json --output docs/release/third-party/DEPENDENCY_NOTICES.txt

# Read-only source freshness, also run by scripts/check-mvp.sh.
python3 scripts/dependency_notices.py check --root . --inventory docs/release/third-party/DEPENDENCY_INVENTORY.json --evidence docs/release/third-party/evidence.json --notices docs/release/third-party/DEPENDENCY_NOTICES.txt

# Read-only actual app resource validation, also run after --build and in DMG checks.
python3 scripts/dependency_notices.py bundle-check --root . --inventory docs/release/third-party/DEPENDENCY_INVENTORY.json --evidence docs/release/third-party/evidence.json --notices docs/release/third-party/DEPENDENCY_NOTICES.txt --app app/src-tauri/target/release/bundle/macos/Tinkeroo.app

# EXPECTED TO FAIL while coverage/review is pending. Never bypass this for publication.
python3 scripts/dependency_notices.py release-check --root . --inventory docs/release/third-party/DEPENDENCY_INVENTORY.json --evidence docs/release/third-party/evidence.json --notices docs/release/third-party/DEPENDENCY_NOTICES.txt --review docs/release/third-party/review.json
```

Generation uses atomic/fsynced output replacement; checks never regenerate files. Lock/JSON input bounds, duplicate-key/identity rejection, symlink refusal, public source-URL validation and managed license paths prevent arbitrary file reads or credential-bearing source URLs from being copied into disclosures. These checks are not a claim to detect every possible secret or authenticate upstream content. Human provenance review remains mandatory.

## Evidence entry contract

Use the **exact component object** from the current inventory as `component`; invented or stale descriptors fail. One entry per classified component:

- `status: included`: exactly `component`, `status`, `licenseExpression`, `sourceAvailability` and nonempty `files`. Each file has exactly `path` and `sha256`. Paths must stay under `docs/release/third-party/licenses/`, be bounded regular UTF-8 files, and not traverse/follow symlinks. References use public credential-free HTTPS and are not fetched by the tool. `sourceAvailability` needs a reviewed exact source/revision and fulfillment plan, especially for MPL; a syntactically valid URL does not establish legal compliance or availability.
- `status: excluded`: exactly `component`, `status`, `reason`. A human must document why no relevant code/material is distributed; `devDependency` or build-dependency status alone is insufficient. Do not mass-exclude the dependency graph to make the gate green.
- `status: pending`: `component`, `status`, optionally `reason`; cannot pass the release gate. Missing entries are likewise pending.

Retain all applicable upstream copyright/license/NOTICE texts, including vendored native subcomponents rather than just the Rust wrapper license. Multiple evidence files may belong to one component. The initial [source-backed findings](../../research/macos-third-party-notices.md) identify MPL, libssh2, OpenSSL, ring, compression/SQLite and missing-text/provenance gaps that still need work. No upstream license texts have been replaced by generated summaries.

## Owner review and publishing

Once **every** component has a defensible included/excluded classification and complete required notices/source availability, regenerate and inspect the disclosure. Only the owner/reviewer may change `review.json` to `decision: approved-for-release`, with a nonempty `reviewer`, UTC `reviewedAt` (`YYYY-MM-DDTHH:MM:SSZ`) and exact SHA-256 values for the inventory, evidence manifest and disclosure. No command does this for them. Lock, evidence or notice changes invalidate the record; do not mechanically refresh hashes to avoid review.

The CLI verifies record consistency, **not reviewer identity, legal correctness, source reachability or authority**. Review this record through the repository's owner-controlled code/release process. The protected GitHub release-environment approval remains separate and required. Do not treat a forged/editable JSON field as cryptographic authorization.

- Routine CI may pass source/software/build checks while review is pending, but withholds new binary download artifacts unless the review gate matches. QA reports remain available. Earlier CI artifacts are not retroactively reviewed or revoked by this gate and remain subject to their existing retention policy.
- The production workflow checks coverage/review **before passing signing inputs**. The attended finalizer independently repeats review and actual bundle checks before Apple submission/signing operations. Existing signature, notarization, Gatekeeper and acceptance checks still apply.
- This initial inventory does not cover Python/Arduino libraries later fetched/compiled on the Board, externally installed Platform-Tools, or all dependencies of build tooling shipped separately. Those distributions require their own applicable review. The independent release-verifier tool is not embedded in the app.

Keep #25/#12 and #21 A08 unresolved for the outstanding human/source-availability review and actual public-release acceptance. Tool success is not a passing legal, security, clean-Mac, Intel or family verdict.
