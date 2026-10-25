"""External ADB contract fixture only; temporary files, no real server or Board."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

root = Path(__file__).resolve().parent
args = sys.argv[1:]
with (root / "calls.jsonl").open("a") as log:
    log.write(json.dumps(args) + "\n")
if any(name in os.environ for name in ("ADB_SERVER_SOCKET", "ANDROID_ADB_SERVER_ADDRESS", "ANDROID_ADB_SERVER_PORT", "ADB_SERVER_PORT", "ADB_TRACE", "ADB_TRACE_LOG_PATH")):
    raise SystemExit("Redirect environment was not cleared")
if args == ["-L", "tcp:localhost:5037", "start-server"]:
    (root / "server-ready").touch()
    raise SystemExit(0)
if args[:4] != ["-H", "127.0.0.1", "-P", "5037"] or not (root / "server-ready").exists():
    raise SystemExit("cannot start server on remote host")
args = args[4:]
if args[:2] == ["-s", "USB_FIXTURE"]:
    args = args[2:]
if args == ["features"]:
    print("cmd" if (root / "legacy-peer").exists() else "shell_v2\ncmd")
elif args[:1] == ["exec-out"]:
    # Raw exec never forwards stdin EOF to this class of peer. Match the real
    # failure without launching a Board process or recording private input.
    time.sleep(10)
elif args[:4] == ["shell", "-T", "-e", "none"] and len(args) == 5:
    if (root / "legacy-peer").exists():
        raise SystemExit("input must not be sent to an unsupported peer")
    payload = sys.stdin.buffer.read(16385)
    result = subprocess.run(["/bin/sh", "-c", args[4]], input=payload, capture_output=True, timeout=5)
    sys.stdout.buffer.write(result.stdout)
    sys.stderr.buffer.write(result.stderr)
    raise SystemExit(result.returncode)
elif args == ["devices", "-l"]:
    print("List of devices attached\nUSB_FIXTURE device usb:1 transport_id:1")
else:
    raise SystemExit("Unexpected external fixture request")
