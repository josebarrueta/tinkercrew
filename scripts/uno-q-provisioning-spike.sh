#!/usr/bin/env bash
set -euo pipefail
set +x

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
HELPER_LOCAL="$ROOT_DIR/scripts/uno-q-provision-helper.py"
HELPER_REMOTE="/home/arduino/.cache/tinkercrew/provision-helper.py"
ADB=${ADB:-adb}

usage() {
  cat <<'EOF'
Usage: uno-q-provisioning-spike.sh [--serial SERIAL] COMMAND [ARG]

Commands:
  detect                         Require exactly one online ADB Board and print its serial
  status                         Report current setup and exact software versions
  hostname NAME                  Set a validated hostname
  keyboard LAYOUT                Set a validated keyboard layout
  password                       Prompt for and set the initial Linux password
  wifi-scan                      List visible SSIDs
  wifi-join SSID                 Prompt for the Wi-Fi password and join SSID
  enable-network                 Prompt for Linux password; enable SSH and mDNS
  install-key PUBLIC_KEY_FILE    Append one SSH public key without replacing existing keys
  reboot                         Prompt for Linux password and reboot
  deploy APP_DIR APP_NAME        Transfer and start a minimal Arduino App

Secrets are read without echo and sent over stdin. This spike must only be used in
an attended physical-board experiment. It does not flash the OS or bootloader.
EOF
}

SERIAL=""
if [[ ${1:-} == "--serial" ]]; then
  SERIAL=${2:-}
  shift 2
fi
COMMAND=${1:-}
shift || true

online_serials() {
  "$ADB" devices | awk 'NR > 1 && $2 == "device" {print $1}'
}

select_board() {
  if [[ -n "$SERIAL" ]]; then
    [[ $("$ADB" -s "$SERIAL" get-state 2>/dev/null) == "device" ]] || {
      echo "Board $SERIAL is not online." >&2
      exit 1
    }
    return
  fi
  boards=$(online_serials)
  count=$(printf '%s\n' "$boards" | awk 'NF {count++} END {print count+0}')
  if [[ $count -ne 1 ]]; then
    echo "Expected exactly one online ADB Board; found $count. Use --serial." >&2
    exit 1
  fi
  SERIAL=$boards
}

install_helper() {
  "$ADB" -s "$SERIAL" shell install -d -m 700 /home/arduino/.cache/tinkercrew >/dev/null
  "$ADB" -s "$SERIAL" push "$HELPER_LOCAL" "$HELPER_REMOTE" >/dev/null
  "$ADB" -s "$SERIAL" shell chmod 700 "$HELPER_REMOTE"
}

call_helper() {
  install_helper
  "$ADB" -s "$SERIAL" shell python3 "$HELPER_REMOTE"
}

json_request() {
  local operation=$1
  local key=${2:-}
  local value=${3:-}
  python3 -c 'import json,sys; d={"operation":sys.argv[1]}; d.update({sys.argv[2]:sys.stdin.read().rstrip("\n")}) if sys.argv[2] else None; print(json.dumps(d))' \
    "$operation" "$key" <<<"$value"
}

prompt_secret() {
  local prompt=$1
  local first second
  read -r -s -p "$prompt: " first </dev/tty
  printf '\n' >/dev/tty
  read -r -s -p "Confirm $prompt: " second </dev/tty
  printf '\n' >/dev/tty
  [[ "$first" == "$second" ]] || { echo "Values do not match." >&2; exit 1; }
  printf '%s' "$first"
}

case "$COMMAND" in
  detect)
    select_board
    printf '%s\n' "$SERIAL"
    ;;
  status)
    select_board
    json_request status | call_helper
    ;;
  hostname)
    select_board
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    json_request set_hostname hostname "$1" | call_helper
    ;;
  keyboard)
    select_board
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    json_request set_keyboard layout "$1" | call_helper
    ;;
  password)
    select_board
    secret=$(prompt_secret "new Linux password")
    json_request set_password new_password "$secret" | call_helper
    unset secret
    ;;
  wifi-scan)
    select_board
    json_request wifi_scan | call_helper
    ;;
  wifi-join)
    select_board
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    secret=$(prompt_secret "Wi-Fi password")
    python3 -c 'import json,sys; print(json.dumps({"operation":"wifi_join","ssid":sys.argv[1],"wifi_password":sys.stdin.read().rstrip("\n")}))' "$1" <<<"$secret" | call_helper
    unset secret
    ;;
  enable-network)
    select_board
    secret=$(prompt_secret "Linux password")
    json_request enable_network linux_password "$secret" | call_helper
    unset secret
    ;;
  install-key)
    select_board
    [[ $# -eq 1 && -f $1 ]] || { usage >&2; exit 2; }
    key=$(<"$1")
    json_request install_key public_key "$key" | call_helper
    unset key
    ;;
  reboot)
    select_board
    secret=$(prompt_secret "Linux password")
    json_request reboot linux_password "$secret" | call_helper || true
    unset secret
    ;;
  deploy)
    select_board
    [[ $# -eq 2 && -d $1 && $2 =~ ^[A-Za-z0-9_-]+$ ]] || { usage >&2; exit 2; }
    remote="/home/arduino/ArduinoApps/$2"
    "$ADB" -s "$SERIAL" shell install -d -m 755 "$remote"
    "$ADB" -s "$SERIAL" push "$1/." "$remote/" >/dev/null
    "$ADB" -s "$SERIAL" shell arduino-app-cli app start "$remote"
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
