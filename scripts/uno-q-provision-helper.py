#!/usr/bin/env python3
"""Board-side helper for the issue #1 physical provisioning spike.

Reads exactly one JSON request from stdin and writes one JSON response. Secret values
are never accepted as command-line arguments or included in responses.
"""

import json
import os
import pathlib
import re
import subprocess
import sys


def run(args, *, stdin=None, check=True):
    result = subprocess.run(
        args,
        input=stdin,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        env={**os.environ, "LC_ALL": "C"},
    )
    if check and result.returncode != 0:
        raise RuntimeError(f"{args[0]} failed with exit code {result.returncode}")
    return result


def sudo(password, args):
    return run(["sudo", "-S", "-p", "", *args], stdin=password + "\n")


def status():
    def output(args):
        result = run(args, check=False)
        return result.stdout.strip() if result.returncode == 0 else None

    network_enabled = all(
        run(["systemctl", check, unit], check=False).returncode == 0
        for unit in ("ssh.service", "avahi-daemon.service")
        for check in ("is-enabled", "is-active")
    )
    return {
        "user": output(["id", "-un"]),
        "uid": output(["id", "-u"]),
        "hostname": output(["hostnamectl", "--static"]),
        "keyboard": output(["localectl", "status"]),
        "password_status": output(["chage", "-l", "arduino"]),
        "wifi": output(["nmcli", "-t", "-f", "GENERAL.STATE,GENERAL.CONNECTION", "device", "show", "wlan0"]),
        "network_mode": network_enabled,
        "os_release": pathlib.Path("/etc/os-release").read_text(encoding="utf-8"),
        "app_cli_version": output(["arduino-app-cli", "version"]),
    }


def set_hostname(request):
    name = request.get("hostname", "")
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9-]{0,63}", name):
        raise ValueError("hostname must match [A-Za-z0-9][A-Za-z0-9-]{0,63}")
    run(["sudo", "hostnamectl", "set-hostname", name])
    if run(["systemctl", "is-active", "avahi-daemon"], check=False).returncode == 0:
        run(["sudo", "systemctl", "restart", "avahi-daemon"])
    return {"hostname": name}


def set_password(request):
    password = request.get("new_password", "")
    if not isinstance(password, str) or len(password) < 8:
        raise ValueError("new_password must contain at least eight characters")
    run(["sudo", "arduino-passwd"], stdin=password)
    return {"password_set": True}


def set_keyboard(request):
    layout = request.get("layout", "")
    if not re.fullmatch(r"[A-Za-z0-9_-]{1,32}", layout):
        raise ValueError("invalid keyboard layout")
    run(["sudo", "/usr/local/bin/arduino-set-keyboard-layout", layout])
    return {"layout": layout}


def wifi_scan():
    run(["nmcli", "radio", "wifi", "on"])
    run(["nmcli", "device", "wifi", "rescan"], check=False)
    result = run(["nmcli", "-t", "-f", "SSID", "device", "wifi", "list"])
    return {"ssids": sorted({line for line in result.stdout.splitlines() if line and line != "--"})}


def wifi_join(request):
    ssid = request.get("ssid", "")
    password = request.get("wifi_password", "")
    if not isinstance(ssid, str) or not ssid or "\n" in ssid:
        raise ValueError("invalid SSID")
    if not isinstance(password, str) or "\n" in password:
        raise ValueError("invalid Wi-Fi password")
    args = ["nmcli", "--ask", "--wait", "20", "device", "wifi", "connect", ssid]
    result = run(args, stdin=password + "\n", check=False)
    if result.returncode != 0:
        raise RuntimeError(f"nmcli failed with exit code {result.returncode}")
    return {"connected": True, "ssid": ssid}


def enable_network(request):
    password = request.get("linux_password", "")
    command = """set -e
systemctl unmask ssh.service
systemctl unmask avahi-daemon.service
dpkg-reconfigure openssh-server
systemctl enable --now avahi-daemon.service
systemctl enable --now ssh.service
"""
    sudo(password, ["sh", "-c", command])
    return {"network_mode": True}


def install_key(request):
    key = request.get("public_key", "").strip()
    if not re.fullmatch(r"ssh-(ed25519|rsa) [A-Za-z0-9+/=]+(?: [^\r\n]+)?", key):
        raise ValueError("unsupported or malformed SSH public key")
    ssh_dir = pathlib.Path.home() / ".ssh"
    authorized = ssh_dir / "authorized_keys"
    ssh_dir.mkdir(mode=0o700, exist_ok=True)
    existing = authorized.read_text(encoding="utf-8").splitlines() if authorized.exists() else []
    if key not in existing:
        with authorized.open("a", encoding="utf-8") as handle:
            handle.write(key + "\n")
    authorized.chmod(0o600)
    return {"installed": True, "already_present": key in existing}


def reboot(request):
    password = request.get("linux_password", "")
    subprocess.Popen(
        ["sudo", "-S", "-p", "", "reboot"],
        stdin=subprocess.PIPE,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        text=True,
    ).communicate(password + "\n", timeout=2)
    return {"reboot_requested": True}


OPERATIONS = {
    "status": lambda request: status(),
    "set_hostname": set_hostname,
    "set_password": set_password,
    "set_keyboard": set_keyboard,
    "wifi_scan": lambda request: wifi_scan(),
    "wifi_join": wifi_join,
    "enable_network": enable_network,
    "install_key": install_key,
    "reboot": reboot,
}


def main():
    try:
        request = json.load(sys.stdin)
        operation = request.get("operation")
        if operation not in OPERATIONS:
            raise ValueError("unknown operation")
        result = OPERATIONS[operation](request)
        json.dump({"status": "ok", "result": result}, sys.stdout)
        sys.stdout.write("\n")
    except Exception as error:
        json.dump({"status": "error", "error": str(error)}, sys.stdout)
        sys.stdout.write("\n")
        raise SystemExit(1)


if __name__ == "__main__":
    main()
