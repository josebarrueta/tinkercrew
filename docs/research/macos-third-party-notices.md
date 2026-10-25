# Mac app third-party notices: initial inventory and gaps

Date: 2026-10-25. Baseline: `add80c3` (merged PR #24). Engineering follow-up: **#25**, under #12 / #21 A08.

Status: **initial discovery, not a completed license audit or legal/publication approval**. No production signing credentials, Apple submission, publication, Board operation or acceptance result was changed. This note was researched directly from local locked package manifests and their packaged upstream license files; no separate research agent was available.

## Reproducible inputs

- `app/src-tauri/Cargo.lock` SHA-256: `c25dc71e8862dcf7f187e60b5ff562fd3da4fe7e466084472cba12941327e61c`.
- `app/package-lock.json` SHA-256: `64582d8c9a26e2adf77390aeb37e7671f80837ba123574e6614793fb0af893cb`.
- Cargo inspection: `cargo metadata --locked --offline --format-version 1 --filter-platform TARGET --manifest-path app/src-tauri/Cargo.toml`, for `aarch64-apple-darwin` and `x86_64-apple-darwin`. No build scripts were executed by this metadata inspection and no extra packages were fetched.
- npm inspection: non-root entries of the checked-in lockfile. These include dev dependencies and optional packages for platforms other than macOS.

| Inspection | Apple silicon | Intel |
| --- | ---: | ---: |
| Cargo package records, including workspace | 384 | 384 |
| Root-reachable normal dependency edges, including workspace | 346 | 347 |
| Root-reachable normal + build dependency edges, including workspace | 366 | 367 |

These are **graph upper bounds, not proof of code in the final executable**. Proc macros, generated code, build tools, feature configuration, linker elimination and vendored C inputs require separate classification. Build-dependency exclusion alone would miss some code compiled into static libraries. All 384 Cargo records have license metadata, but an SPDX string is not the required full notice evidence.

The npm lock has **118 non-root entries**, all with license metadata. Filtering only npm production dependencies would be misleading: Svelte is declared as a dev dependency while its compiled runtime contributes to the frontend. Conversely, optional foreign-platform compiler binaries in the lock do not establish redistribution in the Mac app. Sources: `app/package.json`, `app/package-lock.json`, and the actual build policy in `docs/release/macos.md`.

## Findings requiring explicit review

1. **MPL is present; the repository's MIT license is not a blanket license for every dependency.** Rust package metadata declares MPL-2.0 for `cssparser 0.37.0`, `cssparser-macros 0.7.1`, `dtoa-short 0.3.5`, `option-ext 0.2.0` and `selectors 0.38.0`. All five occur in the inspected normal-edge upper bound. The npm lock also has 12 MPL-2.0 entries for Lightning CSS and platform packages. Determine actual redistribution and provide the appropriate source-availability/notice plan; do not infer compliance from the inventory. Primary text: [`cssparser`'s packaged MPL, §§3.1–3.4](https://docs.rs/crate/cssparser/0.37.0/source/LICENSE); exact package metadata is in the locked crates and npm lock.
2. **Wrapper metadata misses upstream native notices.** `libssh2-sys 0.3.3` declares `MIT OR Apache-2.0`, while its vendored [`libssh2/COPYING`](https://docs.rs/crate/libssh2-sys/0.3.3/source/libssh2/COPYING) contains upstream copyright/binary-redistribution conditions. `openssl-src 300.6.1+3.6.3` has wrapper MIT/Apache files and a distinct [`openssl/LICENSE.txt`](https://docs.rs/crate/openssl-src/300.6.1+3.6.3/source/openssl/LICENSE.txt). `ring 0.17.14`'s [`LICENSE`](https://docs.rs/crate/ring/0.17.14/source/LICENSE) explicitly points to `LICENSE-other-bits`, `LICENSE-BoringSSL` and once_cell license files. An SPDX allowlist alone is not a notice collector. Compression and SQLite vendoring also need inspection; do not treat every directory shipped inside a crate (for example optional SQLCipher) as necessarily compiled.
3. **Missing packaged license text needs provenance, not a guessed substitute.** The cached `selectors 0.38.0` crate declares MPL-2.0 but has no top-level license file. Record the exact authoritative source/revision and evidence before filling the gap. Do not blindly follow arbitrary dependency URLs or scrape unrelated files into notices.
4. **No notice resource is configured today.** `app/src-tauri/tauri.conf.json` has no bundled third-party-notices resource. Source-tree notices alone would not prove that recipients of the DMG receive the required material. Verify the built app/DMG, not just the generator output. Existing `docs/release/macos.md` and #21 A08 already leave the third-party audit as a publication gate.
5. **Scope matters.** The Mac native executable statically incorporates native dependencies; the development artifact's `otool -L` showed system frameworks/libraries, not a Homebrew OpenSSL dylib. This is not a clean-Mac or licensing verdict. The compiled-in Companion source/requirements are separate from Python libraries downloaded on the Board and firmware libraries built by official tooling; a Mac inventory must not silently claim to audit those future installations. Official external Platform-Tools is not bundled; retain that distribution boundary.

## Proposed next vertical slices

Await confirmation of these public test seams before writing tests:

- Audit CLI: temporary lockfiles/package-source trees in; deterministic inventory/notices, diagnostics and exit status out.
- Packaged notice artifact: temporary/test bundle or built app in; evidence that required notices/source instructions are actually present and match inventory.

Then implement one behavior at a time: deterministic lock-bound inventory/drift rejection; bounded regular-file license evidence including vendored components; missing/conflicting evidence refusal; packaged notices and artifact checks; routine CI/release preflight before secrets. Store no private absolute cache paths or credentials. Keep inventory freshness, human compliance review and production-publication approval as separate states.

**Still pending:** those tests/implementation, complete license/source-availability review, actual bundled notices, Intel/Apple-silicon artifact comparison, owner approval and #21 acceptance. No checkbox has been marked passed by this research.
