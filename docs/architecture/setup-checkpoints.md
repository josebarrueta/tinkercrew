# Resumable parent setup guide (#11)

Status: **guarded checkpoint/handoff foundation implemented, full provisioning still blocked**. This is not completed factory-reset UNO Q acceptance or a substitute for the unsupported integrations below.

## Available now

Open Board Setup → **Set up a UNO Q — resumable official-tool handoff guide** in the native Mac app.

- Enumerate USB/ADB identities through a fixed, bounded local `adb devices -l` operation.
- Explicitly select the physically identified UNO Q serial; never assume the first ADB device is the Board.
- Record observed USB availability separately from human identity/boot/official-tool confirmations.
- Open the fixed official Arduino App Lab configuration guide for naming, Linux credentials, Wi-Fi, and network/SSH mode. Compatibility-sensitive Linux helpers are not automated speculatively.
- Correct Wi-Fi credentials in official tooling and return to the same checkpoint; this checkpoint guide never collects Linux/Wi-Fi passwords. The separate [Board Setup management controls](board-setup.md) can use request-only passwords for existing-Board configuration.
- Resume checkpoint progress from macOS Keychain after app restart. Reopening requires fresh USB inspection, and continuing refreshes USB evidence again.
- Changing Board identity requires an explicit new-setup action. It clears only guide checkpoints, not keys, apps, networks, Pairings, or Board configuration. Repeated confirmations do not append duplicate completed checkpoints.
- Native and frontend validators reject unknown/secret fields, incompatible state, incorrect proof-source labels, and impossible success beyond the implemented handoff frontier.
- Browser previews show the guide but cannot record actual Board setup success.

These claims describe the checkpoint guide, not the separate #19 management/recovery controls. Those controls add existing-Board configuration, scheduled restart, and guarded destructive restore, but do not complete the Pairing/deployment frontier below.

The guide performs no remote shell commands, OS updates, flashing, bootloader changes, password operations, SSH-key modification, or deployment. An `adb`-ready transport is **not** represented as proof of Linux boot completion, power stability, image compatibility, or UNO Q model; those steps are explicitly human/official-tool confirmations.

## Where it deliberately stops

Checkpoint 8, **Establish full USB Pairing**, is a hard implementation gate. A checkbox cannot claim this succeeded. The remaining steps stay visibly pending:

1. Physical USB trust bootstrap and dedicated SSH/gameplay key registration; revocation/removal and USB recovery.
2. Supported-version probing of image-specific setup helpers and safe password/Wi-Fi stdin operations before considering direct first-run automation.
3. Atomic Companion install/upgrade, integrity/version verification, interrupted-transfer rollback, and preservation of a known-good deployment.
4. Reboot followed by actual stable-identity LAN **and** USB verification, not treating transport disappearance as success.
5. Wiring the Mission's real hardware self-test result and Crew creation into setup completion, after the preceding trust/deployment gates exist.

The provisioned bearer-record import from #7 is a separate **development bridge**, not proof of full TBP Pairing or factory setup. Already configured Boards may use that connection flow and the playable Mission without marking the all-in-one wizard complete.

## Storage and safety

Checkpoint records are strict versioned JSON held in a dedicated Keychain item, separate from Board credentials and Mission SQLite progress. They contain the selected serial, ordered stage IDs, evidence source (`observed` or `human`), and current index—never passwords, private keys, tokens, command strings, or request URLs. Writes are revision-checked within the running app; no Board mutation depends solely on a saved checkpoint. A locked/unreadable Keychain is an error, not an empty record to silently overwrite.

Official documentation opens only through a fixed native URL; Mission code receives no arbitrary browser/shell/program interface. Image-specific operations remain disabled for unknown/unverified support rather than guessing a password or sudo policy. Existing unrelated SSH keys/configuration are untouched.

## Evidence and remaining validation

Automated tests exercise explicit identity selection, unrelated-device rejection, fresh evidence on resume, distinct human/observed records, blocked fake Pairing/deployment success, secret-field rejection, checkpoint reset behavior, native payload validation, and the browser preview's inability to claim completion.

Actual Keychain/reinstall behavior, USB disconnection and boot behavior on reset/configured UNO Q images, and the complete physical setup matrix remain untested. The full reset-Board flow cannot be accepted until the missing integrations above are implemented and the matrix in `docs/qa/recovery-safety.md` passes. #11 must remain open and blocked, not merely labeled as awaiting validation.
