# Board connections — implementation status

Implemented for #7 before physical acceptance, at the owner's request. The native transport is macOS-only. Simulator play remains independent of credentials, internet, and Board availability.

## Connection ladder

`app/src/lib/board/connection.ts` exposes observable searching/connected/failed states and attempts, in order:

1. Last validated LAN address for the selected Board, stored with its Keychain record.
2. Bonjour `_tinkercrew._tcp.local.` announcements, bounded to 1.8 seconds and eight candidates. TXT identity is only a discovery hint, never authorization.
3. With explicit parent consent, up to eight private addresses from the Mac's existing ARP neighbor cache. There is no subnet or port sweep.
4. USB/ADB forward to port 7443, after exact-match selection of the provisioned ADB serial. Offline and unauthorized USB states have separate next actions. Other ADB devices are never selected implicitly.
5. Optional manual local IP or hostname in Parent Settings.

Discovery and probes run asynchronously; DNS lookup, HTTP, mDNS, and subprocess waits are bounded. Only private, loopback, or link-local resolved IP addresses are eligible. Redirects and HTTP proxies are disabled, and no internet endpoint is required. VPN/local-network permission failures suggest allowing local routes or using USB; the app does not claim to detect the cause of an unreachable route with certainty.

ADB comes from official external Platform-Tools: `TC_ADB_PATH` (absolute executable path), `~/Library/Android/sdk/platform-tools/adb`, `/opt/homebrew/bin/adb`, or `/usr/local/bin/adb`. The app does not bundle Google binaries. The adapter runs only fixed local `devices`/serial-targeted `forward` operations, never remote shell commands. It removes only forwards created by this app when disconnecting/retrying. Ephemeral USB loopback addresses do not replace a last-known LAN address.

## Trust and the current development bridge

The current #6 Companion uses a provisioned bearer credential, not the full proposed TBP Ed25519 challenge/session flow. To exercise connected play now, Parent Settings can import an **already provisioned, USB-verified** record:

```json
{
  "boardId": "the-stable-Board-ID-provisioned-on-the-Companion",
  "name": "Dragon den",
  "adbSerial": "exact-physically-verified-adb-serial",
  "tlsName": "dragon.local",
  "certificatePem": "-----BEGIN CERTIFICATE-----\n...USB-provisioned self-signed certificate...\n-----END CERTIFICATE-----\n",
  "apiToken": "a-unique-random-credential-at-least-32-characters-long",
  "lastAddress": null
}
```

Do not commit a real record, paste it into an issue, or accept it solely from Wi-Fi discovery. The parent must obtain/verify the serial, ID, certificate, and credential directly through physical USB. The Companion's `board_id` / `TC_BOARD_ID` must match `boardId`. Its certificate must include the fixed `tlsName` as a DNS SAN. A fixed DNS identity lets the native client validate the certificate even when routing to a changed LAN IP or ADB loopback port.

The entire sensitive record is stored in macOS Keychain; a separate ordinary file contains only opaque Board lookup handles. Certificate roots are restricted to the USB-provisioned certificate, normal TLS hostname/validity checks remain enabled, and HTTPS is required on **all** paths. The authenticated hello must match the selected stable Board ID and protocol before a connection can accept gameplay Commands. Native connection handles prevent stale frontend clients from sending Commands through a replacement connection.

This is a deliberately explicit development bridge, **not completion of full TBP Pairing**. Automated physical USB bootstrap, Ed25519 installation keys/short-lived sessions, SPKI-preserving certificate renewal, revocation UX, and WSS remain integration work. The current certificate-scoped trust bridge requires re-import after certificate renewal; it does not silently tolerate clock/certificate errors. Do not present the import form as production one-click first-time setup.

## Connected Mission behavior

`ConnectedBoardClient` maps real Companion snapshots/events into the same `BoardClient` interface as the simulator. Inputs include sample age plus a conservative bound for request/IPC/transit time; monotonic client timing prevents wall-clock changes from making old readings appear fresh. Stale/implausible readings fail closed. It primes its event cursor on connect so historical arm-button presses are never replayed. Event gaps, delayed button edges, restart detection, and transport/authentication failures disconnect the adapter and expose a next action. Reconnection is retried at five-second intervals, except for trust/authentication/protocol problems that require parent intervention. Retrying remains bound to the same Board identity even if the parent changes the dropdown without pressing Connect.

A new connection recreates the Mission runtime from saved progress, discards live validation evidence, and starts disarmed. Only bounded native gameplay operations are exposed to the UI; Mission/domain code cannot choose request URLs, invoke a shell, or manage Linux. Independent Companion/STM32 watchdogs remain the physical safety authority.

## Evidence and remaining acceptance

Automated tests cover fallback order/consent/manual entry, authentication failure, serial ambiguity, input validation, event-history suppression/gaps, freshness parsing, and a **real loopback HTTPS Python Companion** exercised by the Rust client. That TLS integration test checks valid operation, a wrong identity, wrong credential, and wrong certificate without touching user Keychain records.

Physical UNO Q power-cycle reconnection, live Bonjour on macOS, guest/VPN routing, multiple attached real Boards, real ADB forwarding, and the macOS Keychain permission/signing experience remain **pending**. So do the full TBP integration items above. Automated checks must not be described as physical acceptance.
