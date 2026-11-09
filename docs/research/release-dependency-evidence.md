# Release dependency evidence: first reviewed batch

Baseline: `ba2aa0a`; follow-up #25. **Partial engineering evidence, not legal or publication approval.** No background research agent was available; the primary sources below were inspected directly. No credentials, Apple submission, Board operation, or protected acceptance result changed.

## Method and reproducible provenance

The lock inventory remains unchanged: 549 Cargo and 118 npm records. `docs/release/third-party/provenance.json` records exact component descriptors, public archive locations, archive integrity, upstream archive member names, and copied text SHA-256 values. It is research provenance, not a new approval mechanism; the existing evidence/review gate remains authoritative.

For the eight Cargo packages in this batch, each cached `.crate` archive was checked against its **Cargo.lock SHA-256**, and license texts were read directly from named regular archive members without extracting paths or executing package code. Copied license bytes also matched the installed source. For the five MPL crates, every regular archive member was compared with the installed source; no source modifications were found. Public source archives were fetched over HTTPS and checked against the same lock checksums. No `[patch]` overrides are configured in the application manifest.

Svelte's published tarball was downloaded from its exact npm lock URL and checked against the lock's **SHA-512 integrity**. `package/LICENSE.md` matched the installed Svelte license byte-for-byte. The full upstream license is included even though Svelte is a devDependency: its generated runtime is used in the shipped frontend.

No source-URL or SPDX declaration was treated as approval. The owner must still review the complete disclosure, source-access arrangements, and distribution obligations. The review record remains `pending`.

## Six included engineering evidence entries

| Component | Primary source | Treatment |
| --- | --- | --- |
| cssparser 0.37.0 | [Locked published source](https://static.crates.io/crates/cssparser/cssparser-0.37.0.crate), `LICENSE` | Full MPL-2.0 text and source-access instructions. |
| cssparser-macros 0.7.1 | [Locked published source](https://static.crates.io/crates/cssparser-macros/cssparser-macros-0.7.1.crate), `LICENSE` | Conservatively include macro crate evidence; do not exclude solely because it is a compiler input. |
| dtoa-short 0.3.5 | [Locked published source](https://static.crates.io/crates/dtoa-short/dtoa-short-0.3.5.crate), `LICENSE` | Full MPL-2.0 text and source-access instructions. |
| option-ext 0.2.0 | [Locked published source](https://static.crates.io/crates/option-ext/option-ext-0.2.0.crate), `LICENSE.txt` | Full MPL-2.0 text and source-access instructions. |
| selectors 0.38.0 | [Locked published source](https://static.crates.io/crates/selectors/selectors-0.38.0.crate), `Cargo.toml.orig` and `lib.rs` | MPL declaration and per-file header verified. Crate has no top-level license file; full text obtained from the license steward, explicitly identified below. |
| Svelte 5.57.2 | [Locked published source](https://registry.npmjs.org/svelte/-/svelte-5.57.2.tgz), `package/LICENSE.md` | Full MIT notice, including Svelte Contributors attribution. |

MPL §§3.1–3.4 require source access and retention of applicable notices, not relicensing the entire app as MPL. Each MPL evidence entry now includes a Tinkeroo-authored `SOURCE.txt` with the exact free source archive URL, its checksum, an unmodified-source statement, unpacking instructions, and explicit preservation of the recipient's MPL source rights. These instructions are rendered into the packaged disclosure, not merely left in the repository. The full selectors license is copied from [Mozilla's official MPL-2.0 text](https://www.mozilla.org/media/MPL/2.0/index.txt), **not** misrepresented as a file present in the published crate. Its [exact upstream revision](https://github.com/servo/stylo/tree/572ecba2d1600e7c3d490586692a209faf703baa/selectors) also matches the crate's VCS metadata.

Public source availability was verified during collection, but cannot be guaranteed forever. Before distribution, the owner must accept this source-access method or arrange a durable matching source mirror; any local modifications require updated source disclosure. The original source archive retains its per-file notices. No generic MIT declaration overrides these rights.

## Native texts collected, but three entries deliberately remain pending

- **libssh2-sys 0.3.3:** [exact archive](https://static.crates.io/crates/libssh2-sys/libssh2-sys-0.3.3.crate). Copied wrapper MIT/Apache texts **and** `libssh2/COPYING`, whose binary-redistribution and non-endorsement terms are not represented by the Rust wrapper SPDX alone. Remaining work: inspect built vendored sources and file-specific/subcomponent attribution before deciding coverage is complete.
- **openssl-src 300.6.1+3.6.3:** [exact archive](https://static.crates.io/crates/openssl-src/openssl-src-300.6.1+3.6.3.crate). Copied wrapper MIT/Apache texts, `openssl/LICENSE.txt`, and the separately licensed bundled Perl Text-Template license. The Perl tool's presence does not establish runtime distribution. Remaining work: native build inputs, additional attribution and build-only scope decisions. Do not classify the package merely from its wrapper license.
- **ring 0.17.14:** [exact archive](https://static.crates.io/crates/ring/ring-0.17.14.crate). Copied `LICENSE`, `LICENSE-other-bits`, `LICENSE-BoringSSL`, both once_cell texts and `third_party/fiat/LICENSE`. `LICENSE` explicitly points to several of these. Fiat's license refers to an `AUTHORS` file absent from this published archive; attribution/source-header review remains open. The crate VCS record also says `dirty: true`, so the checksum-bound published crate, **not** an assumed clean Git revision, is the source of truth.

Collected-but-pending files are stored with provenance; they are **not** rendered as complete included evidence. The gate correctly distinguishes collection from a completed subcomponent audit.

## Coverage after this first batch (historical)

**6 / 667 included; 0 excluded; 661 pending (including the three native entries above).** The subsequent [native/wrapper batch](native-release-notices.md) brought coverage to 15 included; the later [Rust runtime batch](rust-runtime-release-notices.md) brings the current total to **111 included, 0 excluded, 556 pending**. See those notes for updated attribution findings. No mass exclusions were made from dependency type or metadata graphs. The other cached Cargo/npm records still need scope, exact license/NOTICE text and source-access decisions; do not call this batch completion of #25.

Next review batches:

1. Complete native crypto/SSH/compression/SQLite attribution, including aws-lc-sys and its vendored source (do not overlook the rustls provider).
2. Review the remaining Rust runtime and generated/proc-macro inputs using checksum-verified source archives.
3. Review frontend runtime/generated material and distinguish build tooling/platform binaries with actual packaging evidence; include or explicitly justify exclusions.
4. Rebuild and check actual app/DMG disclosures; only then request the human hash-bound review. Preserve pending review and Apple/physical acceptance gates throughout.
