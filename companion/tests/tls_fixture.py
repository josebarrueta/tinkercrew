"""Loopback-only real TLS Companion fixture used by native Rust transport tests."""
import ssl
import sys
from http.server import ThreadingHTTPServer

from tinkercrew_companion.core import Companion
from tinkercrew_companion.hardware import MemoryHardware
from tinkercrew_companion.server import make_handler

companion = Companion(MemoryHardware(), board_id="board-test-1234")
companion.record_distance(250)
companion.record_buttons([False, False, False])
server = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(companion, "test-only-credential-01234567890123456789"))
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(sys.argv[1], sys.argv[2])
server.socket = context.wrap_socket(server.socket, server_side=True)
print(server.server_port, flush=True)
server.serve_forever()
