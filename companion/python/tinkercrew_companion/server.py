from __future__ import annotations

import hmac
import json
import os
import ssl
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from collections import deque
from urllib.parse import parse_qs, urlparse

from .core import CommandError, Companion
from .hardware import BridgeHardware, MemoryHardware

MAX_BODY_BYTES = 65536


def load_config() -> dict:
    path = os.environ.get("TC_CONFIG", "data/companion.json")
    try:
        with open(path, "r", encoding="utf-8") as handle:
            config = json.load(handle)
    except FileNotFoundError:
        return {}
    if not isinstance(config, dict):
        raise SystemExit("Companion configuration must be a JSON object")
    return config


def setting(config: dict, environment_name: str, key: str, default=None):
    return os.environ.get(environment_name, config.get(key, default))


def build_companion(board_version: str = "unknown", board_id: str = "unprovisioned") -> Companion:
    if os.environ.get("TC_SIMULATE_HARDWARE") == "1":
        return Companion(MemoryHardware(), board_version=board_version, board_id=board_id)
    from arduino.app_utils.bridge import Bridge

    companion = Companion(BridgeHardware(Bridge), board_version=board_version, board_id=board_id)
    Bridge.provide("tc_buttons", lambda a, b, c: companion.record_buttons([bool(a), bool(b), bool(c)]))
    Bridge.provide("tc_distance", lambda millimeters: companion.record_distance(int(millimeters)))
    Bridge.provide("tc_disconnected", lambda capability: companion.mark_disconnected(str(capability)))
    Bridge.provide("tc_capability_health", lambda capability, health: companion.record_capability_health(str(capability), str(health)))
    return companion


def make_handler(companion: Companion, token: str):
    command_times = deque()
    rate_lock = threading.Lock()

    class Handler(BaseHTTPRequestHandler):
        server_version = "TinkerCrewCompanion/0.1"

        def log_message(self, format, *args):
            print(json.dumps({"event": "http", "client": self.client_address[0], "message": format % args}))

        def _send(self, status: int, body: dict | list) -> None:
            encoded = json.dumps(body, separators=(",", ":")).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(encoded)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(encoded)

        def _authorized(self) -> bool:
            expected = f"Bearer {token}"
            provided = self.headers.get("Authorization", "")
            return hmac.compare_digest(provided.encode(), expected.encode())

        def _require_auth(self) -> bool:
            if self._authorized():
                return True
            self._send(401, {"status": "error", "error": {"code": "authentication_required"}})
            return False

        def setup(self):
            super().setup()
            self.connection.settimeout(2.0)

        def _json_body(self) -> dict:
            if self.headers.get("Transfer-Encoding") or len(self.headers.get_all("Content-Length", [])) > 1:
                raise CommandError("invalid_message: unsupported or ambiguous body framing")
            try:
                length = int(self.headers.get("Content-Length", "0"))
            except ValueError as error:
                raise CommandError("invalid_message: invalid content length") from error
            if length < 0:
                raise CommandError("invalid_message: content length must be nonnegative")
            if length > MAX_BODY_BYTES:
                raise CommandError("message_too_large: maximum is 65536 bytes")
            try:
                raw = self.rfile.read(length)
                if len(raw) != length:
                    raise CommandError("invalid_message: incomplete request body")
                body = json.loads(raw or b"{}")
            except (json.JSONDecodeError, UnicodeDecodeError) as error:
                raise CommandError("invalid_message: body must be JSON") from error
            if not isinstance(body, dict):
                raise CommandError("invalid_message: body must be an object")
            return body

        def do_GET(self):
            parsed = urlparse(self.path)
            if parsed.path == "/v1/health/public":
                self._send(200, {"protocol_versions": ["1.0"], "state": companion.hello()["health"]["state"]})
                return
            if not self._require_auth():
                return
            if parsed.path == "/v1/hello":
                self._send(200, companion.hello())
            elif parsed.path == "/v1/state":
                self._send(200, companion.state())
            elif parsed.path == "/v1/events":
                try:
                    after = int(parse_qs(parsed.query).get("after", ["0"])[0])
                except ValueError:
                    after = 0
                self._send(200, {"events": companion.events(after)})
            else:
                self._send(404, {"status": "error", "error": {"code": "not_found"}})

        def do_POST(self):
            if not self._require_auth():
                return
            with rate_lock:
                now = time.monotonic()
                while command_times and command_times[0] <= now - 1.0:
                    command_times.popleft()
                if len(command_times) >= 20:
                    self._send(429, {"status": "error", "error": {"code": "rate_limited"}})
                    return
                command_times.append(now)
            try:
                body = self._json_body()
                if self.path == "/v1/commands":
                    result = companion.command(body.get("operation"), body.get("arguments", {}))
                elif self.path == "/v1/control-lease":
                    result = companion.renew_control_lease(body.get("lease_ms", 1500))
                elif self.path == "/v1/self-tests":
                    result = companion.start_self_test()
                elif self.path.startswith("/v1/self-tests/") and self.path.endswith("/confirmation"):
                    test_id = self.path.split("/")[3]
                    result = companion.confirm_self_test(test_id, body.get("lights"), body.get("sound"))
                else:
                    self._send(404, {"status": "error", "error": {"code": "not_found"}})
                    return
                self._send(200, result)
            except CommandError as error:
                code = str(error).split(":", 1)[0]
                self._send(400, {"status": "error", "error": {"code": code, "message": str(error)}})
            except Exception:
                companion.enter_safe_state("hardware_error")
                self._send(500, {"status": "error", "error": {"code": "internal_error", "message": "The Board could not complete the request."}})

    return Handler


def run() -> None:
    config = load_config()
    token = setting(config, "TC_API_TOKEN", "api_token")
    if not token or len(token) < 32:
        raise SystemExit("api_token/TC_API_TOKEN must contain at least 32 characters")
    host = setting(config, "TC_BIND", "bind", "0.0.0.0")
    port = int(setting(config, "TC_PORT", "port", 7443))
    board_id = str(setting(config, "TC_BOARD_ID", "board_id", "unprovisioned"))
    companion = build_companion(str(setting(config, "TC_BOARD_VERSION", "board_version", "unknown")), board_id=board_id)
    server = ThreadingHTTPServer((host, port), make_handler(companion, token))

    certificate = setting(config, "TC_TLS_CERT", "tls_cert")
    private_key = setting(config, "TC_TLS_KEY", "tls_key")
    development_http = os.environ.get("TC_DEVELOPMENT_HTTP") == "1" and host in {"127.0.0.1", "localhost"}
    if certificate and private_key:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.minimum_version = ssl.TLSVersion.TLSv1_2
        context.load_cert_chain(certificate, private_key)
        server.socket = context.wrap_socket(server.socket, server_side=True)
    elif not development_http:
        raise SystemExit("TLS certificate/key are required unless loopback development HTTP is explicitly enabled")

    def watchdog():
        while True:
            companion.check_watchdog()
            time.sleep(0.05)

    from .discovery import advertise
    advertisement = advertise(board_id, port, str(setting(config, "TC_ADVERTISE_HOST", "advertise_host", "")))
    threading.Thread(target=watchdog, daemon=True).start()
    try:
        server.serve_forever(poll_interval=0.1)
    finally:
        companion.enter_safe_state("server_stopped")
        server.server_close()
        if advertisement is not None:
            advertisement.close()
