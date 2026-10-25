"""Fixed, transient UNO Q management helper. No general shell or reset-by-rm endpoint."""
import json
import pathlib
import pwd
import re
import shutil
import subprocess
import sys


def run(args, secret=None):
    result = subprocess.run(args, input=secret, text=True, capture_output=True, timeout=30)
    if result.returncode != 0:
        raise RuntimeError("The Board rejected the operation. Check permissions/password and retry; no success is assumed.")
    if len(result.stdout) > 65536:
        raise RuntimeError("Board response is too large.")
    return result


def handle(request, reader=lambda path: pathlib.Path(path).read_bytes(), runner=run, username=None):
    allowed = {
        "inspect": {"operation"},
        "enable_ssh": {"operation", "expectedSerial", "linuxPassword", "username"},
        "hostname": {"operation", "expectedSerial", "linuxPassword", "hostname"},
        "wifi_scan": {"operation", "expectedSerial"},
        "wifi_join": {"operation", "expectedSerial", "ssid", "wifiPassword"},
        "reboot": {"operation", "expectedSerial", "linuxPassword"},
    }
    operation = request.get("operation")
    if operation not in allowed or set(request) != allowed[operation]:
        raise ValueError("Unsupported management request.")
    compatible = reader("/proc/device-tree/compatible").split(b"\0")
    if b"arduino,imola" not in compatible:
        raise ValueError("This is not a supported Arduino UNO Q. No configuration changed.")
    raw_serial = reader("/sys/devices/soc0/serial_number").decode().strip()
    serial = int(raw_serial, 16 if raw_serial.lower().startswith("0x") else 10)
    if not 0 < serial < 2**63:
        raise ValueError("Unsupported hardware serial; recovery must use official Arduino tooling.")
    os_info = dict(line.split("=", 1) for line in reader("/etc/os-release").decode().splitlines() if "=" in line)
    if os_info.get("ID", "").strip('"') != "debian":
        raise ValueError("Unsupported Board OS. Use official Arduino setup instead.")
    actual_user = username or pwd.getpwuid(__import__("os").geteuid()).pw_name
    if operation == "inspect":
        try:
            build = dict(line.split("=", 1) for line in reader("/etc/buildinfo").decode().splitlines() if "=" in line).get("BUILD_ID", "unknown").strip().strip('"')[:64]
        except (OSError, KeyError):
            build = "unknown"
        try:
            boot_id = reader("/proc/sys/kernel/random/boot_id").decode().strip().lower()
            if not re.fullmatch(r"[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}", boot_id):
                boot_id = None
        except (OSError, KeyError):
            boot_id = None
        return {"bootId": boot_id, "serial": str(serial), "username": actual_user, "hostname": reader("/etc/hostname").decode().strip()[:64], "os": "debian", "osVersion": os_info.get("VERSION_ID", "unknown").strip('"')[:32], "imageVersion": build}
    if request["expectedSerial"] != str(serial):
        raise ValueError("Board identity changed. Reconnect the exact selected UNO Q.")

    password = request.get("linuxPassword", "")
    if not isinstance(password, str) or len(password) > 512 or any(c in password for c in "\r\n\0"):
        raise ValueError("Use a single-line Linux password (not logged or stored).")
    def sudo(args):
        if not password:
            raise ValueError("Enter the Linux password for this explicit change.")
        return runner(["sudo", "-k", "-S", "-p", "", "--", *args], password + "\n")

    if operation == "enable_ssh":
        if request["username"] != actual_user:
            raise ValueError("USB SSH enablement must use the current USB Linux account; use official setup for another account.")
        sudo(["systemctl", "enable", "--now", "ssh.service"])
        return {"state": "ssh_enabled"}
    if operation == "hostname":
        name = request["hostname"]
        if not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9-]{0,62}", name):
            raise ValueError("Use a hostname of 1–63 letters, digits, or hyphens, beginning with a letter/digit.")
        sudo(["hostnamectl", "set-hostname", name])
        return {"hostname": name, "state": "hostname_set"}
    if operation == "wifi_scan":
        if runner is run and shutil.which("nmcli") is None:
            raise ValueError("NetworkManager is unavailable on this image. Use official Arduino setup.")
        result = runner(["nmcli", "-t", "--escape", "no", "-f", "SSID", "device", "wifi", "list", "--rescan", "yes"])
        return {"ssids": sorted(set(line for line in result.stdout.splitlines() if line and len(line.encode()) <= 32))[:64]}
    if operation == "wifi_join":
        ssid, wifi_password = request["ssid"], request["wifiPassword"]
        if not isinstance(ssid, str) or not ssid or len(ssid.encode()) > 32 or ssid.startswith("-") or any(c in ssid for c in "\r\n\0"):
            raise ValueError("Enter a supported Wi-Fi name of 1–32 bytes.")
        if not isinstance(wifi_password, str) or len(wifi_password) > 128 or any(c in wifi_password for c in "\r\n\0"):
            raise ValueError("Enter a single-line Wi-Fi password.")
        runner(["nmcli", "--ask", "--wait", "20", "device", "wifi", "connect", ssid], wifi_password + "\n")
        return {"state": "connected", "ssid": ssid}
    if operation == "reboot":
        sudo(["shutdown", "-r", "+1"])
        return {"state": "reboot_scheduled", "delaySeconds": 60, "note": "Reconnect after boot; a scheduled request is not verified restart completion."}
    raise ValueError("Unsupported operation.")


def main():
    try:
        raw = sys.stdin.buffer.read(16385)
        if len(raw) > 16384:
            raise ValueError("Management request too large.")
        request = json.loads(raw)
        if not isinstance(request, dict):
            raise ValueError("Invalid management request.")
        result = handle(request)
        print(json.dumps({"status": "ok", "result": result}))
    except Exception:
        # Never reflect credentials, arbitrary command output, or exception text.
        print(json.dumps({"status": "error", "message": "Board operation failed or is unsupported. Check the selected identity, password, image support, and connections."}))
        raise SystemExit(1)


if __name__ == "__main__":
    main()
