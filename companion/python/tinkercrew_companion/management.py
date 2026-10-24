"""Fixed host-side USB Pairing operations; SSH keys permit identity inspection only.

Run through python/management.py on the host, not a network endpoint. Physical USB
selection/consent is the Mac adapter's responsibility. No shell/OS/deployment RPC here.
"""
import argparse
import json
import os
from pathlib import Path
import pwd
import sys
import time

from .auth import AuthError, PairingRegistry


def inspect_host():
    if os.geteuid() == 0:
        raise AuthError("unsupported_account")  # Never install a root forced key pointing into a user's app tree.
    if b"arduino,imola" not in Path("/proc/device-tree/compatible").read_bytes().split(b"\0"):
        raise AuthError("unsupported_board")
    os_info = dict(line.split("=", 1) for line in Path("/etc/os-release").read_text().splitlines() if "=" in line)
    if os_info.get("ID", "").strip('"') != "debian":
        raise AuthError("unsupported_board")
    raw = Path("/sys/devices/soc0/serial_number").read_text().strip()
    serial = int(raw, 16 if raw.lower().startswith("0x") else 10)
    if not 0 < serial < 2**63:
        raise AuthError("unsupported_board")
    return {"serial": str(serial), "username": pwd.getpwuid(os.geteuid()).pw_name}


class PairingManagement:
    def __init__(self, app_directory, *, ssh_directory=None, inspect=inspect_host, clock=time.monotonic, boot_id=None):
        self.root = Path(app_directory).resolve()
        self.inspect = inspect
        config_path = self.root / "data/companion.json"
        if config_path.stat().st_size > 16384:
            raise AuthError("unsupported_configuration")
        self.config = json.loads(config_path.read_text())
        if self.config.get("auth_mode") != "tbp" or self.config.get("api_token"):
            raise AuthError("unsupported_configuration")
        self.registry_options = {"ssh_directory": ssh_directory or Path.home() / ".ssh", "management_script": self.root / "python/management.py", "clock": clock, "boot_id": boot_id}

    def dispatch(self, request, *, ssh_installation=None):
        fields = {"inspect": set(), "pair_begin": {"registration"}, "pair_activate": {"installation_id"}, "list_pairings": set(), "revoke": {"installation_id"}}
        operation = request.get("operation")
        if operation not in fields or set(request) != {"operation", "expected_serial"} | fields[operation]:
            raise AuthError("invalid_message")
        if ssh_installation is not None and operation != "inspect":
            raise AuthError()
        identity = self.inspect()
        if request["expected_serial"] != identity["serial"]:
            raise AuthError("identity_mismatch")
        if ssh_installation is not None and not (self.root / "data/pairings.sqlite").is_file():
            raise AuthError()
        self.registry = PairingRegistry(self.root / "data", self.config["board_id"], **self.registry_options)
        if ssh_installation is not None and not self.registry.is_active(ssh_installation):
            raise AuthError()
        if operation == "pair_activate":
            return self.registry.activate_pairing(request["installation_id"])
        if operation == "list_pairings":
            return {"board_id": self.registry.board_id, "pairings": self.registry.list_pairings()}
        if operation == "revoke":
            return self.registry.revoke(request["installation_id"])
        certificate_path = Path(self.config["tls_cert"])
        if not certificate_path.is_absolute():
            certificate_path = self.root / certificate_path
        if certificate_path.stat().st_size > 8192:
            raise AuthError("unsupported_configuration")
        metadata = {"board_id": self.registry.board_id, "serial": identity["serial"], "username": identity["username"], "tls_name": self.config["tls_name"], "certificate_pem": certificate_path.read_text(), "protocol_versions": ["1.0"]}
        if operation == "pair_begin":
            metadata.update(self.registry.begin_pairing(request["registration"]))
        return metadata


def main():
    try:
        parser = argparse.ArgumentParser(add_help=False)
        parser.add_argument("--installation")
        args = parser.parse_args()
        raw = sys.stdin.buffer.read(16385)
        if len(raw) > 16384:
            raise AuthError("message_too_large")
        request = json.loads(raw)
        if not isinstance(request, dict):
            raise AuthError("invalid_message")
        root = Path(__file__).resolve().parents[2]
        result = PairingManagement(root).dispatch(request, ssh_installation=args.installation)
        print(json.dumps({"status": "ok", "result": result}))
    except Exception:
        # No supplied body, credential, filesystem path, or exception text is reflected.
        print(json.dumps({"status": "error", "error": {"code": "management_failed"}}))
        raise SystemExit(1)
