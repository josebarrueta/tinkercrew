---
status: proposed
---

# Separate Board management from authenticated gameplay

TinkerCrew will use direct USB/ADB and dedicated-key SSH only for parent-approved Board setup, maintenance, and recovery. Normal play will use a bounded HTTPS/WSS Companion API authenticated by a revocable credential established through physical USB Pairing, with the Board's self-signed TLS identity pinned during that Pairing. This prevents mission logic from gaining general shell access, works without cloud infrastructure or internet access, and protects Commands and Observables on untrusted local networks; it adds local certificate and credential lifecycle work that must remain recoverable through USB.
