#!/usr/bin/env bash
# Read-only regression check: headless DMGs must still support drag-to-Applications.
set -euo pipefail
if [[ "$#" != 1 || ! -f "$1" ]]; then
  printf 'Usage: %s DMG\n' "$0" >&2
  exit 2
fi
mount="$(mktemp -d "${TMPDIR:-/tmp}/tinkercrew-dmg-check.XXXXXX")"
attached=0
cleanup() {
  if [[ "$attached" == 1 ]]; then
    if hdiutil detach "$mount" >/dev/null 2>&1; then rmdir "$mount"; else printf 'Could not detach temporary test image: %s\n' "$mount" >&2; fi
  else
    rmdir "$mount"
  fi
}
trap cleanup EXIT
hdiutil attach -nobrowse -readonly -mountpoint "$mount" "$1" >/dev/null
attached=1
[[ -d "$mount/TinkerCrew.app/Contents" ]] || { printf 'DMG lacks TinkerCrew.app.\n' >&2; exit 1; }
[[ -L "$mount/Applications" && "$(readlink "$mount/Applications")" == /Applications ]] || { printf 'DMG lacks the drag-to-Applications link.\n' >&2; exit 1; }
printf 'Read-only DMG app and Applications-link checks passed.\n'
