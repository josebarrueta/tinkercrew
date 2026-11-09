# Rust runtime and generated-code notice batch

Follow-up #25 on `release/dependency-evidence`, working baseline `ba2aa0a`. **Partial engineering evidence, not owner/legal or publication approval.** No background researcher was available; local primary package sources were inspected directly. No dependency versions, credentials, Board state or acceptance results changed.

## Result and method

Inspected **98 exact Cargo package/version records** from a fixed, attended list covering serialization, asynchronous execution, networking, data structures, time, errors and generated Rust code. Added **96 included entries**; two attribution/scope cases remain pending. Current overall coverage: **111 included, 0 excluded, 556 pending** out of 667 records.

Each source archive was checked against its exact Cargo.lock SHA-256 before reading any notice. Collected **175 root/nested license/notice files** and **29 distinct supplemental Rust comment blocks** across the candidate batch. Every copied text matched both its archive member and the installed source. Full license texts are retained, not replaced with SPDX strings or summaries. Legacy slash-style MIT/Apache metadata was normalized only for the reviewed fixed-list entries, not as a general approval policy.

All published Rust source members were checked for copyright/SPDX occurrences; each occurrence had to lie in a retained verbatim comment block. Supplemental notices include test/alternate-feature material conservatively and do not assert every referenced member reached the executable. The collector refused native C/assembly/Objective-C source inputs in this batch; native packages use the separate review method. This is a conservative evidence collection/check, not a legal parser or proof that every possible obligation is discoverable from comments.

Complete source references are recorded per exact component in [`provenance.json`](../release/third-party/provenance.json): public archive URL, lock checksum, archive member names, full file hashes, excerpt offsets/lengths/hashes and number of Rust source members checked. [`evidence.json`](../release/third-party/evidence.json) supplies the actual included/pending decisions to the existing gate. Missing evidence stays pending; no new approval mechanism or mass exclusion was introduced.

## Covered groups

Exact versions and source URLs are in provenance. The included batch covers:

- **Serialization/errors:** Serde/core/derive/JSON/repr/spanned/untagged, erased-serde, anyhow, thiserror and its implementation.
- **Async/networking:** Tokio/util, Mio, socket2, bytes, HTTP/body/body-util/httparse, Hyper/util, Reqwest, Futures channel/core/io/macro/sink/task/util, tower-http/layer/service, tracing/core, try-lock, want and sync_wrapper. Tower itself remains pending below.
- **Containers/synchronization:** both selected bitflags and indexmap versions, three selected hashbrown versions, hashlink, equivalent, smallvec, slab, once_cell, lazy_static, lock_api, parking_lot/core, scopeguard, crossbeam-utils, atomic-waker and cfg-if. crossbeam-channel remains pending below.
- **Syntax/generated Rust:** proc-macro2, quote, both selected Syn and synstructure versions, displaydoc, derive_more/implementation, ref-cast/implementation, typeid, dyn-clone, heck and strsim. These were not excluded merely because they generate code or run at build time.
- **Supporting runtime:** both selected base64 versions, URL/form-urlencoding/percent-encoding, MIME, ipnet, log, itoa, dtoa, UUID, semver, camino, dunce, fs2, wait-timeout, zeroize and time/core/macros/num-conv/powerfmt/deranged.

The list is deliberately not the entire Rust lockfile and does not establish frontend, macOS binding, native-provider or every build-tool obligation. Ryu was absent from the inspected target metadata and was not given an invented evidence entry.

## Source-specific obligations found and retained

### Futures channel: a BSD queue inside a MIT/Apache crate

Primary source: [futures-channel 0.3.34 archive](https://static.crates.io/crates/futures-channel/futures-channel-0.3.34.crate), `src/mpsc/queue.rs`.

Its Dmitry Vyukov copyright and full two-condition BSD grant are distinct from the package's MIT/Apache texts. Binary redistribution must reproduce its notice/conditions/disclaimer. The entire original comment, including the additional views/conclusions statement, is now rendered into the disclosure. The evidence expression is `(MIT OR Apache-2.0) AND BSD-2-Clause`, not simply the Cargo declaration.

### tracing-core: nested spin attribution

Primary source: [tracing-core 0.1.36 archive](https://static.crates.io/crates/tracing-core/tracing-core-0.1.36.crate), `src/spin/LICENSE`.

Mathijs van de Nes's full MIT notice is nested under the source tree. Both that text and the root Tokio contributors' license are included. A root-only file collector would miss it.

### Atomic Waker: third-party notices are separate

Primary source: [atomic-waker 1.1.2 archive](https://static.crates.io/crates/atomic-waker/atomic-waker-1.1.2.crate), `LICENSE-THIRD-PARTY`.

Retained Alex Crichton/Tokio third-party attributions and their full MIT/Apache grant texts in addition to both root licenses. No assumption that a permissive SPDX label eliminates attribution requirements.

### Adapted float parsing, random generation and synchronization

- [serde_json 1.0.154](https://static.crates.io/crates/serde_json/serde_json-1.0.154.crate), `src/lexical/mod.rs`: retain Alexander Huszagh's attribution and explicit MIT/Apache licensing statement.
- [dtoa 1.0.11](https://static.crates.io/crates/dtoa/dtoa-1.0.11.crate), `src/dtoa.rs`: retain the Tencent/THL A29/Milo Yip RapidJSON-related notice. Elect the available **MIT** root alternative for this evidence; do not erase the MIT-only attribution inside comments.
- [uuid 1.28.0](https://static.crates.io/crates/uuid/uuid-1.28.0.crate), `src/rng.rs`: retain rust-random/Rust Project copyright and full MIT source notice. Elect the **MIT** root alternative for this evidence.
- [lock_api 0.4.14](https://static.crates.io/crates/lock_api/lock_api-0.4.14.crate), [parking_lot 0.12.5](https://static.crates.io/crates/parking_lot/parking_lot-0.12.5.crate), [parking_lot_core 0.9.12](https://static.crates.io/crates/parking_lot_core/parking_lot_core-0.9.12.crate): retain Amanieu d'Antras source notices alongside the root Rust Project license texts.
- [dunce 1.0.5](https://static.crates.io/crates/dunce/dunce-1.0.5.crate): the published full text is **CC0-1.0**. That is the elected alternative for this evidence; alternate MIT-0/Apache texts were not invented or substituted.

## Two additional cases deliberately remain pending

### crossbeam-channel 0.5.17

Primary source: [exact published archive](https://static.crates.io/crates/crossbeam-channel/crossbeam-channel-0.5.17.crate), `LICENSE-THIRD-PARTY`, `tests/mpsc.rs` and `tests/golang.rs`.

The root MIT/Apache declaration does not describe all packaged material. The full third-party file includes **CC-BY-3.0 matching.go material**, Go BSD terms and Rust notices. Collected all texts/source comments, but did not silently claim full attribution fulfillment or exclude material because a filename looks like a test. Remaining review: exact fixture-versus-shipped/generated-source scope, original attribution and any applicable change/credit requirements. Entry stays pending; collected texts are not rendered as a completed included entry.

### tower 0.5.3

Primary source: [exact published archive](https://static.crates.io/crates/tower/tower-0.5.3.crate), `src/load/peak_ewma.rs`.

PeakEWMA explicitly identifies **Apache-2.0 Finagle-derived code, Copyright 2017 Twitter Inc.**, with an [exact source revision](https://github.com/twitter/finagle/blob/9cc08d15216497bb03a1cafda96b7266cfbbcff1/finagle-core/src/main/scala/com/twitter/finagle/loadbalancer/PeakEwma.scala). Root MIT is not enough. Retained the original source attribution for review; matching upstream Apache/NOTICE coverage and actual feature scope remain open. No incomplete MIT-only inclusion was recorded.

These join the existing OpenSSL source, ring and native AWS-LC attribution cases; none has been approved by this batch.

## Verification

Read-only verification across all batches now covers **115 checksum-locked Cargo archives and 239 exact source-notice locations**, plus every collected file hash. No private absolute cache paths were recorded in provenance. Source notice regeneration succeeded and remains distinct from legal/publication approval. `review.json` is still pending and must not be mechanically renewed.

Full local QA and native development build passed **216 tests** (4 native tests intentionally ignored); report: ignored `artifacts/qa/20261109T045022Z-99735.md`. Rebuilt app/DMG passed strict ad-hoc bundle sealing, arm64-only app/mounted-DMG checks and exact packaged disclosure/layout checks. These are local development outputs, not a Developer-ID-signed/notarized public installer. No signing credentials, Apple submissions, Board operations or physical/family acceptance were involved.
