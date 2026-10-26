"""Native fixed package -> real owned-unstarted correction. Temporary files only."""
import base64
import hashlib
import json
from pathlib import Path
import shutil
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "app/src-tauri/src"))
from companion_provision import CompanionProvisioning, release_id

package = json.loads(sys.stdin.buffer.read(2 * 1024 * 1024 + 1))
wanted = package["release"]
files = {name: base64.b64decode(value, validate=True) for name, value in package["files"].items()}
assert release_id(files) == wanted == "d3c762374833ecce711fdd4a0b3c2727a846c443a34c2ba426de457610c6b6fe"
files["sketch/sketch.yaml"] = b"profiles:\n  default:\n    platforms:\n      - platform: arduino:zephyr\n    libraries:\n      - Arduino_Modulino\n      - Arduino_RouterBridge\ndefault_profile: default\n"
old = release_id(files)
assert old == "63e90d374bc27c69b2fd810f851fed9a51d00abaaa82ce6369ba4b7cac7feef3"
raw = json.dumps({"format": 1, "release": old, "files": {name: base64.b64encode(value).decode() for name, value in files.items()}}, sort_keys=True, separators=(",", ":")).encode()
store = CompanionProvisioning(Path(sys.argv[1]).resolve(), inspect=lambda: {"serial": "42", "uid": 1000, "username": "arduino", "os": "debian", "version": "13"}, openssl=shutil.which("openssl"), app_state=lambda: "uninitialized")
def request(operation, **body):
    return store.dispatch(dict(operation=operation, expectedSerial="42", **body))
job = request("begin", release=old, sha256=hashlib.sha256(raw).hexdigest(), size=len(raw))
for offset in range(0, len(raw), 8192):
    request("chunk", token=job["token"], offset=offset, data=base64.b64encode(raw[offset:offset + 8192]).decode())
request("publish", token=job["token"])
before = request("identity", release=old)
result = request("correct_prepared_profile", release=wanted)
after = request("identity", release=wanted)
assert result["correctionPhase"] == "corrected" and result["release"] == wanted
for field in ("board_id", "tls_name", "certificate_pem", "spki_sha256"):
    assert before[field] == after[field]
print(json.dumps({"release": result["release"], "correctionPhase": result["correctionPhase"], "identityPreserved": True, "started": result["started"]}))
