# First public release: execution checklist

Working baseline: `ba2aa0a`. **Not release-ready.** GitHub #25 (dependency review), #42 (lifecycle), #21 (acceptance), and #12 (trusted installer) remain the authoritative trackers. This checklist orders the work; it does not replace their evidence or owner approval.

## Latest local verification

First evidence batch: full software check and native development build passed **212 tests** (4 additional native tests intentionally ignored), followed by strict ad-hoc bundle-seal and read-only DMG layout/disclosure checks. Report: ignored `artifacts/qa/20261109T022652Z-84204.md`. Local arm64/macOS 27.2 only; no production signature, clean-Mac/Intel or physical/family acceptance inferred.

This machine now defaults to OpenSSL 4, which the identity tooling correctly rejects. For the passing check, OpenSSL 3.6.3 was built locally from the checksum-verified `openssl-src` archive into ignored `artifacts/toolchains/openssl3-build/` without a system install. Run-scoped `PATH` selected that CLI; `OPENSSL_CONF=/dev/null` prevented its uninstalled default config path from breaking temporary test-certificate generation. Tests specifying their own identity config continued to use it. No production compatibility guard was relaxed and no credential configuration changed. Normal developer setup remains the documented OpenSSL 3 prerequisite.

Apple-silicon scope follow-up: **216 software tests passed** (4 native tests intentionally ignored), including real arm64/Intel/Universal Mach-O architecture fixtures. Report: ignored `artifacts/qa/20261109T030543Z-89841.md`. Existing local app and mounted DMG passed the new arm64-only check; no new production signing/notarization was attempted.

Latest Rust runtime notice batch: **216 tests and native build passed** after adding 96 included entries; report: ignored `artifacts/qa/20261109T045022Z-99735.md`. Rebuilt development DMG passed strict ad-hoc seal, app/mounted-DMG arm64-only checks and packaged notice/layout validation. Coverage is now **111 included / 556 pending**; release approval remains blocked.

## 1. Confirm the shipping contract — remaining owner decisions pending

Proposed first release: development-preview quality clearly disclosed, hardware-free Practice and bounded Creations on an already prepared compatible UNO Q. No one-click first-time setup promise.

- [x] Owner chose **Apple silicon only for now**; Intel/Universal 2 deferred. Production workflow, finalizer, metadata and app/mounted-DMG architecture checks now enforce `aarch64-apple-darwin` / `darwin-aarch64` / arm64-only code. This is a scope decision and software gate, not platform acceptance.
- [ ] Decide minimum macOS version based on real runtime evidence, not just the current configured 12.0 target.
- [ ] Decide which advanced setup/update/restore features ship. Exclusions must be explicit; unvalidated dangerous paths may need disabling, not merely a release-note disclaimer.
- [ ] Define the witnessed family/product acceptance required for this preview. Do not turn unrun original acceptance cases into passes.

## 2. Finish source and notice obligations — #25

- [x] Begin exact locked-source evidence collection: five MPL crates plus Svelte, with full texts and source-access instructions.
- [x] Add native/wrapper batch: full libssh2 source notices, SQLite wrapper/disclaimer, conservative zlib notice and six API/FFI/macro wrappers. Details: [native notice review](../research/native-release-notices.md).
- [x] Add [Rust runtime/generated-code batch](../research/rust-runtime-release-notices.md): 96 additional entries, including nested spin, Atomic Waker and Futures BSD queue notices. Crossbeam channel and Tower remain pending for identified scope/attribution gaps.
- [ ] Finish the **556 remaining** component scope/evidence decisions, including OpenSSL source, ring, native AWS-LC, crossbeam-channel, Tower and any justified build/platform exclusions.
- [ ] Confirm source-availability arrangements, including durable access to the exact unmodified MPL sources.
- [ ] Regenerate and inspect the complete disclosure; rebuild app/DMG and verify packaged bytes.
- [ ] Owner reviews the complete evidence and supplies the matching review record. Agent does not approve it on the owner's behalf.
- [ ] `dependency_notices.py release-check` passes without weakening or bypassing the gate.

See [batch evidence and remaining work](../research/release-dependency-evidence.md) and [audit procedure](third-party/README.md).

## 3. Product and physical acceptance — #21 / #42

Only under renewed explicit owner scope for each physical operation:

- [ ] Save a Creation: A input → brief blue light. Run, witness actual behavior, Stop, reopen; it must remain stopped. Retain existing module-test evidence rather than unnecessarily repeating it.
- [ ] Observe approved Board power-cycle/autostart behavior. The vendor default startup configuration was set previously, but power-cycle behavior is unproven. Vendor startup may compile/upload the existing sketch. Do not infer a Linux-only restart or perform this silently.
- [ ] Verify reconnect is disarmed, no repeated credential prompts on ordinary USB play, and no unrelated app/identity/progress changes.
- [ ] Measure physical safe-off on USB/control loss and Mac crash, with supervised loads and consent. A UI Stop label or loopback test is not this evidence.
- [ ] Verify native installed-app persistence, offline use, upgrade/reinstall and recovery from unreadable progress.
- [ ] Observe the family making/predicting/changing a rule; record confusion and scope exclusions honestly.
- [ ] Complete applicable platform/network/maintenance cases from #21 or explicitly approve exclusions. Close #42 only after its actual acceptance gaps are resolved.

## 4. Owner-only trust setup — #12

Do not paste private keys/passwords into chat or issues. No credential setup, upload, or Apple operation has been performed by this checklist.

- [ ] Apple Developer Program account holder creates/retains Developer ID Application certificate and encrypted private `.p12` export.
- [ ] Owner creates and safely retains the appropriate notarization API `.p8` key.
- [ ] Owner generates a separate production update-signing key and backup; test fixtures are never production keys.
- [ ] Protect GitHub `release` environment/version tags and populate the exact workflow secrets/variable privately.

Exact inputs and official references: [owner-only prerequisites](macos.md#owner-only-prerequisites-do-later-not-silently). An attended setup wizard can be prepared after its stages and destinations are confirmed; no unattended credential generation is authorized.

## 5. Candidate, clean-Mac acceptance, publication — #12 / #21

- [ ] Choose release version and synchronize package/Cargo/Tauri versions and lockfiles.
- [ ] Run full software QA and review the exact candidate commit.
- [ ] Tag the reviewed version and explicitly dispatch the protected release workflow.
- [ ] Produce Developer-ID-signed, notarized/stapled app and DMG plus independently verified update signatures. Routine ad-hoc builds are not substitutes.
- [ ] Download the candidate through a browser; verify Gatekeeper, offline launch, installed progress and upgrade on the supported Mac/OS matrix without bypasses.
- [ ] Review final release notes, known limitations, source/notice resources and signed artifacts.
- [ ] Owner explicitly approves publication of the draft release. No automatic public release.
