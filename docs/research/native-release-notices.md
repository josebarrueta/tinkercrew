# Native dependency notice batch

Follow-up to #25 on `release/dependency-evidence`, based on `ba2aa0a`. **Engineering evidence only; owner/legal and publication approval remain pending.** No background researcher was available; primary sources were inspected directly. No credentials or Board operations were used.

## Result

Nine additional included evidence entries: `aws-lc-rs 1.18.2`, `openssl 0.10.81`, `openssl-sys 0.9.117`, `openssl-macros 0.1.1`, `ssh2 0.9.6`, `rusqlite 0.38.0`, `libsqlite3-sys 0.36.0`, `libz-sys 1.1.30`, and `libssh2-sys 0.3.3`.

Total after this batch (historical): **15 included, no exclusions, 652 pending** out of 667 inventory records. The subsequent [Rust runtime batch](rust-runtime-release-notices.md) brings current coverage to **111 included, 556 pending**. Inclusion is conservative notice coverage, not a claim that every file or alternate backend is in the executable. Wrapper inclusion does not approve separately versioned native dependencies. No missing/foreign-platform/build-only component was mass-excluded.

`evidence.json` is the gate input; `provenance.json` records exact archive checksums, member names, excerpt byte offsets/lengths/hashes and observed resolved features. All copied upstream texts are unchanged; labels, scope notes and source-access instructions are explicitly Tinkeroo-authored. The generated disclosure embeds the notices for included entries. Pending collected texts are not represented as completed evidence.

## Primary sources and decisions

### libssh2: wrapper license is not sufficient

Primary source: [checksum-locked libssh2-sys 0.3.3 archive](https://static.crates.io/crates/libssh2-sys/libssh2-sys-0.3.3.crate), especially `build.rs`, `libssh2/COPYING`, `libssh2/src/` and `libssh2/include/`.

The build script enumerates vendored C compilation inputs and selects OpenSSL on non-Windows. The observed arm64 release output reports `static=ssh2`; resolved features include `vendored-openssl`. The source tree has per-file notices beyond `COPYING`, including OpenBSD-derived bcrypt/blowfish, chacha and poly1305 material. Some headers put include guards or an OpenBSD revision comment before their license, so reading only the first comment would miss obligations.

The disclosure now includes both wrapper licenses, full upstream `COPYING`, and **44 distinct verbatim notice comment blocks** from vendored C files, headers and the generated-config template. Each occurrence has an exact checksum-verified archive-member/byte-range reference. Every `.file(...)` input named by `build.rs` was accounted for; additional platform/backend notices were retained conservatively without claiming those backends were compiled. Build inputs and all scanned members are listed in provenance.

Some upstream SPDX labels do not match the wording of their full grant: `bcrypt_pbkdf.c` says MIT but has ISC-style text; `cipher-chachapoly.c` says BSD-2-Clause but likewise includes an ISC-style grant. No upstream wording was silently corrected. Full grants are preserved, and the engineering expression conservatively includes ISC along with the declared BSD/MIT/public-domain terms. This is not a legal determination resolving those upstream labels; the owner must review the full texts.

### SQLite: actual bundled source, not optional SQLCipher

Primary source: [libsqlite3-sys 0.36.0 archive](https://static.crates.io/crates/libsqlite3-sys/libsqlite3-sys-0.36.0.crate), `build.rs`, `LICENSE`, and `sqlite3/sqlite3.c`; API wrapper [rusqlite 0.38.0 archive](https://static.crates.io/crates/rusqlite/rusqlite-0.38.0.crate), `LICENSE`.

The selected features include `bundled`/`bundled_bindings`, not SQLCipher. `build.rs` selects the SQLite amalgamation; current local release output reports `static=sqlite3`. The published source identifies SQLite **3.51.1** and retains its copyright-disclaimer/blessing comment. The wrapper MIT license and that exact source comment are both included. `LicenseRef-SQLite-Public-Domain` refers to the included source-disclaimer text; it is not an invented SPDX license or a blanket assertion about public-domain treatment in every jurisdiction.

SQLCipher's optional license directory is not evidence it was compiled. This entry's scope note explicitly limits the native assessment to the selected SQLite path; no SQLCipher feature has been enabled or source removed.

### zlib: system link observed; fallback notice retained

Primary source: [libz-sys 1.1.30 archive](https://static.crates.io/crates/libz-sys/libz-sys-1.1.30.crate), `build.rs`, wrapper licenses and `src/zlib/LICENSE`.

The observed arm64 release build emits `rustc-link-lib=z`, not a bundled static archive. zlib-ng features are absent. The disclosure conservatively retains stock zlib's full notice, alongside wrapper MIT/Apache texts, rather than making an exclusion dependent on one machine's system-library probe. This does not claim the optional zlib-ng/contrib utilities are shipped. The source/scope note describes the distinction.

### AWS Rust wrapper: retain original contributors too

Primary source: [aws-lc-rs 1.18.2 archive](https://static.crates.io/crates/aws-lc-rs/aws-lc-rs-1.18.2.crate), `LICENSE`, `src/cipher/chacha.rs`, `src/debug.rs`, `src/digest.rs` and other source headers.

The root license contains Apache/ISC terms and Amazon's attribution, but Rust files also retain ISC-covered work from **Brian Smith, Google and Trent Clarke**. The disclosure now includes **12 distinct verbatim leading source notice blocks**, with each archive location recorded, in addition to the complete root license. All source copyright occurrences in scanned Rust members were checked to lie within those captured headers. Native `aws-lc-sys` coverage remains separately pending.

### Other wrappers

- [openssl 0.10.81](https://static.crates.io/crates/openssl/openssl-0.10.81.crate): both Apache text and the separate Google/Jack Lloyd/Steven Fackler attribution in `LICENSE` are retained.
- [openssl-sys 0.9.117](https://static.crates.io/crates/openssl-sys/openssl-sys-0.9.117.crate): full Alex Crichton MIT license retained. This is not the OpenSSL native-source license.
- [openssl-macros 0.1.1](https://static.crates.io/crates/openssl-macros/openssl-macros-0.1.1.crate): both MIT/Apache texts retained, including Steven Fackler's MIT attribution. No exclusion merely because it is a proc macro.
- [ssh2 0.9.6](https://static.crates.io/crates/ssh2/ssh2-0.9.6.crate): both wrapper MIT/Apache texts retained; vendored libssh2 has its own entry above.

## Native blockers still pending

- **openssl-src 300.6.1+3.6.3:** wrapper/OpenSSL/Perl Text-Template texts collected previously. Native attribution and generated/build-only input review remains open; wrapper entries above do not close it.
- **ring 0.17.14:** multiple full license texts collected previously. File-level attribution/fiat AUTHORS provenance remains open; its published crate has dirty VCS metadata, so a guessed clean Git revision is not a substitute.
- **aws-lc-sys 0.46.0:** now collected full [published aggregate](https://static.crates.io/crates/aws-lc-sys/aws-lc-sys-0.46.0.crate) from root `LICENSE`, `aws-lc/LICENSE` and `aws-lc/third_party/fiat/LICENSE`. Native static crypto is present in release output. The aggregate names s2n-bignum, Jitter Entropy, Fiat and per-file public-domain sources; several referenced subcomponent license paths are absent from the published crate. Complete source/build attribution review remains necessary. Its Jitter Entropy section expressly elects BSD rather than GPL; this was not silently reduced to the Rust wrapper license. No approval inferred from the aggregate alone.

## Verification

Read-only source/provenance verification checked **17 checksum-locked Cargo archives** across both batches and **160 exact source-notice locations**, plus all collected file hashes. Named copied texts and excerpt bytes matched their locked archives. Inventory/notice freshness passed. `release-check` still rejects incomplete coverage; `review.json` remains untouched and pending.

Full local software checks plus rebuilt native app passed **216 tests** (4 native tests intentionally ignored); report: ignored `artifacts/qa/20261109T031806Z-91951.md`. Rebuilt development DMG, strict ad-hoc seal, arm64-only app/mounted-DMG architecture and packaged notice/layout checks all passed. No public installer was produced or uploaded.

Local build-output observations are scoped to the arm64 development build. They are neither a substitute for reviewing the eventual protected release build nor proof of production signing, macOS-version support, physical safety or family acceptance.
