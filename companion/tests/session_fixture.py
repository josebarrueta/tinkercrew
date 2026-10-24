"""Real TLS TBP peer for the native session adapter. No hardware or user Keychain."""
import json
from pathlib import Path
import ssl
import sys
from tinkercrew_companion.auth import PairingRegistry
from tinkercrew_companion.core import Companion
from tinkercrew_companion.hardware import MemoryHardware
from tinkercrew_companion.server import CompanionHTTPServer, make_handler

board_id = "98f6411f-20ec-438a-a1de-6ce20fb0370e"
registry = PairingRegistry(Path(sys.argv[3]) / "data", board_id, ssh_directory=Path(sys.argv[3]) / ".ssh")
registration = json.loads(sys.stdin.readline())
bootstrap = registry.begin_pairing(registration)
registry.complete_pairing(dict(registration, pairing_token=bootstrap["pairing_token"]))
registry.activate_pairing(registration["installation_id"])
server = CompanionHTTPServer(("127.0.0.1", 0), make_handler(Companion(MemoryHardware(), board_id=board_id), auth=registry))
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(sys.argv[1], sys.argv[2])
server.socket = context.wrap_socket(server.socket, server_side=True)
print(server.server_port, flush=True)
server.serve_forever()
