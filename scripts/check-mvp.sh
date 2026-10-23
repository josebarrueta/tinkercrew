#!/usr/bin/env bash
# Repeatable software-only checks. Never records physical acceptance as passed.
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
build=0
case "${1:-}" in
  "") ;;
  --build) build=1 ;;
  *) printf 'Usage: %s [--build]\n' "$0" >&2; exit 2 ;;
esac
mkdir -p artifacts/qa
report="artifacts/qa/$(date -u +%Y%m%dT%H%M%SZ)-$$.md"
failed=0
workspace=clean
[[ -z "$(git status --porcelain)" ]] || workspace=dirty
{
  printf '# Automated MVP check\n\n'
  printf -- '- UTC: %s\n' "$(date -u +%FT%TZ)"
  printf -- '- Commit: `%s` (%s workspace)\n' "$(git rev-parse HEAD)" "$workspace"
  printf -- '- OS: %s\n' "$(sw_vers -productVersion 2>/dev/null || uname -s)"
  printf -- '- Node: %s\n' "$(node --version 2>/dev/null || printf unavailable)"
  printf -- '- npm: %s\n' "$(npm --version 2>/dev/null || printf unavailable)"
  printf -- '- Rust: %s\n' "$(rustc --version 2>/dev/null || printf unavailable)"
  printf -- '- Python: %s\n' "$(python3 --version 2>/dev/null || printf unavailable)"
  printf '\n| Software check | Result |\n| --- | --- |\n'
} > "$report"

run_check() {
  local label="$1"
  shift
  printf '\n== %s ==\n' "$label"
  if "$@"; then
    printf '| %s | PASS |\n' "$label" >> "$report"
  else
    printf '| %s | FAIL |\n' "$label" >> "$report"
    failed=$((failed + 1))
  fi
}
run_check 'Frontend quality and Rust lint' bash -c 'cd app && npm run quality'
run_check 'Native persistence and real loopback TLS Companion' cargo test --manifest-path app/src-tauri/Cargo.toml
run_check 'Companion core and HTTP' env PYTHONPATH=companion/python python3 -m unittest discover -s companion/tests
run_check 'Browser Mission, offline resources, and recovery' bash -c 'cd app && npm run test:e2e'
run_check 'Release metadata tests' python3 -m unittest discover -s scripts/tests
run_check 'Detached release signature and tamper checks' cargo test --locked --manifest-path tools/release-verify/Cargo.toml
run_check 'Release verifier formatting' cargo fmt --manifest-path tools/release-verify/Cargo.toml --check
run_check 'Release verifier lint' cargo clippy --locked --manifest-path tools/release-verify/Cargo.toml --all-targets -- -D warnings
run_check 'Python compilation' python3 -m compileall -q companion/python companion/tests scripts
run_check 'Whitespace validation' git diff --check
if [[ "$build" == 1 ]]; then
  run_check 'Production macOS app bundle' bash -c 'cd app && npm run build:app'
else
  printf '| Production macOS app bundle | NOT RUN (use --build) |\n' >> "$report"
fi
{
  printf '\nPhysical UNO Q, actual ADB/Bonjour/VPN behavior, family play-testing, and clean-Mac Gatekeeper/notarization acceptance: **PENDING — not run by this script**.\n'
  printf '\nSee docs/qa/recovery-safety.md for the physical matrix and evidence template.\n'
} >> "$report"
printf '\nReport: %s\n' "$report"
[[ "$failed" == 0 ]]
