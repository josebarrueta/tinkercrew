"""Real TLS peer using a renewed Board certificate, with the prior leaf as the Mac pin."""
import base64
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import ssl
import sys
from tinkercrew_companion.core import Companion
from tinkercrew_companion.hardware import MemoryHardware
from tinkercrew_companion.server import CompanionHTTPServer, make_handler

repository = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("provision", repository / "app/src-tauri/src/companion_provision.py")
provision = importlib.util.module_from_spec(spec)
spec.loader.exec_module(provision)
apps = Path(sys.argv[1]).resolve() / "apps"
apps.mkdir()
store = provision.CompanionProvisioning(apps, inspect=lambda: {"serial": "42", "uid": 1000, "username": "arduino", "os": "debian", "version": "13"}, openssl=shutil.which("openssl"))
package = provision.build_package(repository / "companion")
def request(operation, **body):
    return store.dispatch(dict(operation=operation, expectedSerial="42", **body))
job = request("begin", release=package["release"], sha256=hashlib.sha256(package["bytes"]).hexdigest(), size=len(package["bytes"]))
for at in range(0, len(package["bytes"]), 8192):
    request("chunk", token=job["token"], offset=at, data=base64.b64encode(package["bytes"][at:at + 8192]).decode())
request("publish", token=job["token"])
previous = request("identity", release=package["release"])
renewed = request("renew_identity", release=package["release"])
if previous["certificate_pem"] == renewed["certificate_pem"] or previous["spki_sha256"] != renewed["spki_sha256"]:
    raise RuntimeError("fixture renewal did not preserve identity")
server = CompanionHTTPServer(("127.0.0.1", 0), make_handler(Companion(MemoryHardware(), board_id=previous["board_id"]), "fixture-only-renewal-token-32-characters"))
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(store.root / "data/board.crt", store.root / "data/board.key")
server.socket = context.wrap_socket(server.socket, server_side=True)
print(json.dumps({"port": server.server_port, "boardId": previous["board_id"], "tlsName": previous["tls_name"], "previousCertificate": previous["certificate_pem"]}), flush=True)
server.serve_forever(poll_interval=0.1)
