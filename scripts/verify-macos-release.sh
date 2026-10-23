#!/usr/bin/env bash
# Strict public-release verification: never use this to bless an ad-hoc development build.
set -euo pipefail
if [[ "$#" != 2 ]]; then
  printf 'Usage: %s APP_BUNDLE DMG\n' "$0" >&2
  exit 2
fi
app="$1"
dmg="$2"
[[ -d "$app/Contents" && -f "$dmg" ]] || { printf 'Missing app bundle or DMG.\n' >&2; exit 1; }
metadata="$(codesign -dv --verbose=4 "$app" 2>&1)"
[[ "$metadata" == *'Authority=Developer ID Application:'* ]] || { printf 'A Developer ID Application signature is required.\n' >&2; exit 1; }
[[ "$metadata" == *'(runtime)'* ]] || { printf 'Hardened Runtime is required.\n' >&2; exit 1; }
team="$(printf '%s\n' "$metadata" | awk -F= '/^TeamIdentifier=/{print $2}')"
[[ -n "$team" && "$team" != 'not set' ]] || { printf 'A signing team is required.\n' >&2; exit 1; }
codesign --verify --deep --strict --verbose=2 "$app"
while IFS= read -r -d '' binary; do
  if file -b "$binary" | grep -q 'Mach-O'; then
    codesign --verify --strict --verbose=2 "$binary"
    binary_team="$(codesign -dv --verbose=4 "$binary" 2>&1 | awk -F= '/^TeamIdentifier=/{print $2}')"
    [[ "$binary_team" == "$team" ]] || { printf 'Embedded code is not signed by the release team.\n' >&2; exit 1; }
  fi
done < <(find "$app/Contents" -type f -print0)
codesign --verify --strict --verbose=2 "$dmg"
dmg_team="$(codesign -dv --verbose=4 "$dmg" 2>&1 | awk -F= '/^TeamIdentifier=/{print $2}')"
[[ "$dmg_team" == "$team" ]] || { printf 'The DMG signing team does not match the app.\n' >&2; exit 1; }
xcrun stapler validate "$app"
xcrun stapler validate "$dmg"
spctl --assess --type execute --verbose=4 "$app"
spctl --assess --type open --context context:primary-signature --verbose=4 "$dmg"
printf 'Developer ID, embedded code, Hardened Runtime, tickets, and Gatekeeper checks passed.\n'
