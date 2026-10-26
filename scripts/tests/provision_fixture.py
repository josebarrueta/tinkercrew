"""Native fixed-package interoperability fixture. Never touches real apps/Board credentials."""
import base64
import hashlib
import json
from pathlib import Path
import sys
from test_companion_provision import provision

apps = Path(sys.argv[1]).resolve() / "apps"
apps.mkdir()
store = provision.CompanionProvisioning(apps, inspect=lambda: {"serial": "42", "uid": 1000, "username": "arduino", "os": "debian", "version": "13"})
raw = sys.stdin.buffer.read(2097153)
package = json.loads(raw)
def request(operation, **body):
    return store.dispatch(dict(operation=operation, expectedSerial="42", **body))
job = request("begin", release=package["release"], sha256=hashlib.sha256(raw).hexdigest(), size=len(raw))
for offset in range(0, len(raw), 8192):
    request("chunk", token=job["token"], offset=offset, data=base64.b64encode(raw[offset:offset + 8192]).decode())
request("publish", token=job["token"])
# Substitute only official App CLI metadata handling, never compilation/upload.
# v0.9.0 migrateRemoveRouterBridgeIfNeeded removes the explicit Bridge library
# on an unversioned zephyr profile when the installed core is newer than 0.54.1.
# Receipt integrity must still hold after the supported first-start handling.
profile = apps / "tinkercrew-companion/sketch/sketch.yaml"
text = profile.read_text()
if "platform: arduino:zephyr\n" in text:
    profile.write_text("\n".join(line for line in text.splitlines() if line.strip() != "- Arduino_RouterBridge") + "\n")
print(json.dumps(request("inspect")))
