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
print(json.dumps(request("publish", token=job["token"])))
