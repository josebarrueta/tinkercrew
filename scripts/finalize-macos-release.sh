#!/usr/bin/env bash
# Requires an already Developer-ID-signed/notarized app and signed DMG from Tauri.
# This submits the final DMG, staples both artifacts, verifies, then signs immutable metadata.
set -euo pipefail
umask 077
if [[ "$#" != 4 ]]; then
  printf 'Usage: %s APP_BUNDLE DMG VERSION TARGET\n' "$0" >&2
  exit 2
fi
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
app="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
source_dmg="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
version="$3"
target="$4"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.-]+)?$ ]] || { printf 'Invalid release version.\n' >&2; exit 2; }
case "$target" in darwin-aarch64|darwin-x86_64|darwin-universal) ;; *) printf 'Invalid release target.\n' >&2; exit 2 ;; esac
python3 "$root/scripts/dependency_notices.py" release-check --root "$root" --inventory "$root/docs/release/third-party/DEPENDENCY_INVENTORY.json" --evidence "$root/docs/release/third-party/evidence.json" --notices "$root/docs/release/third-party/DEPENDENCY_NOTICES.txt" --review "$root/docs/release/third-party/review.json"
python3 "$root/scripts/dependency_notices.py" bundle-check --root "$root" --inventory "$root/docs/release/third-party/DEPENDENCY_INVENTORY.json" --evidence "$root/docs/release/third-party/evidence.json" --notices "$root/docs/release/third-party/DEPENDENCY_NOTICES.txt" --app "$app"
: "${TC_UPDATE_PUBLIC_KEY_FILE:?Set the release verification public-key file}"
[[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ]] || { printf 'A release signing key is required in the environment.\n' >&2; exit 1; }
[[ -f "$TC_UPDATE_PUBLIC_KEY_FILE" && -f "$source_dmg" && -d "$app/Contents" ]] || { printf 'Missing release input files.\n' >&2; exit 1; }
public_key="$(cd "$(dirname "$TC_UPDATE_PUBLIC_KEY_FILE")" && pwd)/$(basename "$TC_UPDATE_PUBLIC_KEY_FILE")"
app_version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Contents/Info.plist")"
[[ "$app_version" == "$version" ]] || { printf 'App and release versions differ.\n' >&2; exit 1; }
if [[ -z "${TC_NOTARY_PROFILE:-}" ]]; then
  : "${APPLE_API_KEY_PATH:?Set the App Store Connect private-key file or TC_NOTARY_PROFILE}"
  : "${APPLE_API_KEY:?Set the App Store Connect key ID}"
  : "${APPLE_API_ISSUER:?Set the App Store Connect issuer ID}"
fi
out="$root/artifacts/release/$version/$target"
mkdir -p "$out"
dmg="$out/Tinkeroo_${version}_${target#darwin-}.dmg"
[[ ! -e "$dmg" ]] || { printf 'Release output already exists; do not overwrite immutable artifacts.\n' >&2; exit 1; }
cp "$source_dmg" "$dmg"
if [[ -n "${TC_NOTARY_PROFILE:-}" ]]; then
  xcrun notarytool submit "$dmg" --keychain-profile "$TC_NOTARY_PROFILE" --wait --output-format json > "$out/notarization.json"
else
  xcrun notarytool submit "$dmg" --key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY" --issuer "$APPLE_API_ISSUER" --wait --output-format json > "$out/notarization.json"
fi
python3 -c 'import json,sys; assert json.load(open(sys.argv[1]))["status"] == "Accepted", "Notarization was not accepted"' "$out/notarization.json"
xcrun stapler staple "$app"
xcrun stapler staple "$dmg"
bash "$root/scripts/check-dmg-layout.sh" "$dmg"
bash "$root/scripts/verify-macos-release.sh" "$app" "$dmg"
cd "$root/app"
npm exec --offline -- tauri signer sign --app-version "$version" "$dmg"
cargo run --quiet --locked --manifest-path "$root/tools/release-verify/Cargo.toml" -- "$public_key" "$dmg.sig" "$dmg"
python3 "$root/scripts/release_manifest.py" --version "$version" --target "$target" --artifact "$dmg" --signature "$dmg.sig" --output "$out/release-manifest.json"
npm exec --offline -- tauri signer sign --app-version "$version" "$out/release-manifest.json"
cargo run --quiet --locked --manifest-path "$root/tools/release-verify/Cargo.toml" -- "$public_key" "$out/release-manifest.json.sig" "$out/release-manifest.json"
printf 'Verified release files (not yet published): %s\n' "$out"
