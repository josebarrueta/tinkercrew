from __future__ import annotations

import hmac
import json
import os
import ssl
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
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


class CompanionHTTPServer(ThreadingHTTPServer):
    def serve_forever(self, poll_interval=0.1):
        stopped = threading.Event()
        def watchdog():
            while not stopped.wait(0.05):
                try:
                    self.RequestHandlerClass.tick()
                except Exception:
                    # Never log private state or abandon future ticks after a hardware/IO fault.
                    # The independent STM32 lease remains the physical backstop.
                    pass
        worker = threading.Thread(target=watchdog, daemon=True)
        worker.start()
        try:
            super().serve_forever(poll_interval=poll_interval)
        finally:
            stopped.set()
            worker.join(timeout=0.25)


def runtime_maintenance(directory):
    """Fixed private runtime metadata; errors deny gameplay, never trust an incomplete trial."""
    try:
        for name in ("job.json", "selection.json"):
            path = Path(directory) / name
            if not path.exists() and not path.is_symlink():
                continue
            if path.is_symlink() or not path.is_file() or path.stat().st_size > 4096:
                return True
            record = json.loads(path.read_bytes())
            if not isinstance(record, dict):
                return True
            if name == "job.json" and record.get("phase") not in {"aborted", "rolled_back", "committed"}:
                return True
            if name == "selection.json" and ("trial" not in record or record["trial"] is not None):
                return True
        return False
    except (OSError, ValueError, TypeError):
        return True


def make_handler(companion: Companion, token: str | None = None, *, auth=None, rate_clock=time.monotonic, maintenance=None):
    if (auth is None) == (token is None):
        raise ValueError("Select exactly one authentication mode")
    if auth is not None:
        auth.start_runtime()
    command_times = deque()
    authentication_times = deque()
    rate_lock = threading.RLock()
    control_token = None
    control_until = 0.0

    def tick():
        nonlocal control_token
        with rate_lock:
            if maintenance is not None and maintenance():
                if not companion.hello()["health"]["safe_state"]:
                    companion.enter_safe_state("maintenance_active")
                control_token = None
            if control_token and (time.monotonic() >= control_until or not auth.authenticate(control_token, touch=False)):
                companion.enter_safe_state("control_session_ended")
                control_token = None
        companion.check_watchdog()

    class Handler(BaseHTTPRequestHandler):
        server_version = "TinkerCrewCompanion/0.1"

        def log_message(self, format, *args):
            # BaseHTTPRequestHandler errors can contain arbitrary request text. Never reflect it.
            pass

        def log_request(self, code="-", size="-"):
            route = urlparse(getattr(self, "path", "")).path
            allowed = {"/v1/health/public", "/v1/hello", "/v1/state", "/v1/events", "/v1/commands", "/v1/control-lease", "/v1/self-tests", "/v1/pairings", "/v1/sessions/challenge", "/v1/sessions", "/v1/sessions/current"}
            print(json.dumps({"event": "http", "method": getattr(self, "command", "other") if getattr(self, "command", None) in {"GET", "POST", "DELETE"} else "other", "route": route if route in allowed else "other", "status": code}))

        def _send(self, status: int, body: dict | list) -> None:
            encoded = json.dumps(body, separators=(",", ":")).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(encoded)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(encoded)

        def _authorized(self) -> bool:
            provided = self.headers.get("Authorization", "")
            if len(self.headers.get_all("Authorization", [])) != 1 or len(provided) > 1024:
                return False
            if auth is not None:
                tick()
                return provided.startswith("Bearer ") and auth.authenticate(provided[7:]) is not None
            return hmac.compare_digest(provided.encode(), f"Bearer {token}".encode())

        def _require_auth(self) -> bool:
            if self._authorized():
                return True
            self._send(401, {"status": "error", "error": {"code": "authentication_required"}})
            return False

        def setup(self):
            super().setup()
            self.connection.settimeout(2.0)

        def _json_body(self, maximum=MAX_BODY_BYTES) -> dict:
            if self.headers.get("Transfer-Encoding") or len(self.headers.get_all("Content-Length", [])) > 1:
                raise CommandError("invalid_message: unsupported or ambiguous body framing")
            try:
                length = int(self.headers.get("Content-Length", "0"))
            except ValueError as error:
                raise CommandError("invalid_message: invalid content length") from error
            if length < 0:
                raise CommandError("invalid_message: content length must be nonnegative")
            if length > maximum:
                raise CommandError("message_too_large: request exceeds its allowed size")
            try:
                raw = self.rfile.read(length)
                if len(raw) != length:
                    raise CommandError("invalid_message: incomplete request body")
                body = json.loads(raw or b"{}")
            except (ValueError, UnicodeDecodeError, RecursionError) as error:
                raise CommandError("invalid_message: body must be JSON") from error
            if not isinstance(body, dict):
                raise CommandError("invalid_message: body must be an object")
            return body

        def do_GET(self):
            parsed = urlparse(self.path)
            if parsed.path == "/v1/health/public":
                self._send(200, {"board_id": companion.hello()["board"]["board_id"], "protocol_versions": ["1.0"], "state": companion.hello()["health"]["state"]})
                return
            if not self._require_auth():
                return
            if parsed.path == "/v1/hello":
                hello = companion.hello()
                hello["runtime"] = {"release": os.environ.get("TC_RUNTIME_RELEASE", "unmanaged"), "nonce": os.environ.get("TC_RUNTIME_NONCE", "")}
                self._send(200, hello)
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
            nonlocal control_token, control_until
            authentication = auth is not None and self.path in {"/v1/pairings", "/v1/sessions/challenge", "/v1/sessions"}
            if not authentication and not self._require_auth():
                return
            with rate_lock:
                now = rate_clock()
                times = authentication_times if authentication else command_times
                while times and times[0] <= now - 1.0:
                    times.popleft()
                if len(times) >= 20:
                    self._send(429, {"status": "error", "error": {"code": "rate_limited"}})
                    return
                times.append(now)
            from .auth import AuthError
            try:
                body = self._json_body(16384 if authentication else MAX_BODY_BYTES)
                if authentication:
                    if self.path == "/v1/sessions":
                        with rate_lock:
                            previous = self.headers.get("Authorization", "")
                            previous = previous[7:] if len(self.headers.get_all("Authorization", [])) == 1 and previous.startswith("Bearer ") and len(previous) <= 135 else None
                            owner = auth.authenticate(previous, touch=False) if previous else None
                            result = auth.create_session(body, replace_token=previous)
                            if result["replaced_session"] and owner == body["installation_id"] and control_token == previous:
                                control_token = result["session_token"]
                    else:
                        result = (auth.complete_pairing if self.path == "/v1/pairings" else auth.challenge)(body)
                    self._send(200, result)
                    return
                with rate_lock:
                    if maintenance is not None and maintenance():
                        tick()
                        self._send(503, {"status": "error", "error": {"code": "maintenance_active"}})
                        return
                    if auth is not None:
                        tick()
                        current = self.headers["Authorization"][7:]
                        # Recheck after taking the control lock, including revocation during body read.
                        if not auth.authenticate(current):
                            raise AuthError()
                        if self.path in {"/v1/commands", "/v1/self-tests", "/v1/control-lease"}:
                            if control_token is not None and control_token != current:
                                self._send(409, {"status": "error", "error": {"code": "board_busy"}})
                                return
                            if self.path == "/v1/control-lease":
                                result = companion.renew_control_lease(body.get("lease_ms", 1500))
                                control_token = current
                                control_until = time.monotonic() + min(body.get("lease_ms", 1500), 2000) / 1000
                                self._send(200, result)
                                return
                            if control_token != current:
                                raise CommandError("safe_state_active: acquire a control lease first")
                    if self.path == "/v1/commands":
                        result = companion.command(body.get("operation"), body.get("arguments", {}))
                    elif self.path == "/v1/control-lease":
                        result = companion.renew_control_lease(body.get("lease_ms", 1500))
                    elif self.path == "/v1/self-tests":
                        result = companion.start_self_test()
                    elif self.path.startswith("/v1/self-tests/") and self.path.endswith("/confirmation"):
                        if auth is not None and control_token != self.headers["Authorization"][7:]:
                            raise AuthError()
                        result = companion.confirm_self_test(self.path.split("/")[3], body.get("lights"), body.get("sound"))
                    else:
                        self._send(404, {"status": "error", "error": {"code": "not_found"}})
                        return
                self._send(200, result)
            except AuthError as error:
                self._send(429 if error.code in {"rate_limited", "session_limit"} else 400 if error.code == "invalid_message" else 401, {"status": "error", "error": {"code": error.code}})
            except CommandError as error:
                self._send(400, {"status": "error", "error": {"code": str(error).split(":", 1)[0]}})
            except Exception:
                if not authentication:
                    companion.enter_safe_state("hardware_error")
                self._send(500, {"status": "error", "error": {"code": "internal_error"}})

        def do_DELETE(self):
            if not self._require_auth():
                return
            if auth is not None and self.path == "/v1/sessions/current":
                auth.close_session(self.headers["Authorization"][7:])
                tick()
                self._send(200, {"status": "ok"})
            else:
                self._send(404, {"status": "error", "error": {"code": "not_found"}})

    Handler.tick = staticmethod(tick)
    return Handler


def run() -> None:
    config = load_config()
    token = setting(config, "TC_API_TOKEN", "api_token")
    mode = setting(config, "TC_AUTH_MODE", "auth_mode", "legacy")
    if mode not in {"legacy", "tbp"} or (mode == "tbp" and token):
        raise SystemExit("Select an explicit authentication mode; TBP cannot retain a legacy bearer bypass")
    if mode == "legacy" and (not token or len(token) < 32):
        raise SystemExit("api_token/TC_API_TOKEN must contain at least 32 characters")
    host = setting(config, "TC_BIND", "bind", "0.0.0.0")
    port = int(setting(config, "TC_PORT", "port", 7443))
    board_id = str(setting(config, "TC_BOARD_ID", "board_id", "unprovisioned"))
    companion = build_companion(str(setting(config, "TC_BOARD_VERSION", "board_version", "unknown")), board_id=board_id)
    auth = None
    if mode == "tbp":
        from .auth import PairingRegistry
        directory = Path(os.environ.get("TC_CONFIG", "data/companion.json")).parent
        auth = PairingRegistry(directory, board_id)
    runtime_directory = os.environ.get("TC_RUNTIME_STATE")
    maintenance = (lambda: runtime_maintenance(runtime_directory)) if runtime_directory else None
    server = CompanionHTTPServer((host, port), make_handler(companion, token if auth is None else None, auth=auth, maintenance=maintenance))

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

    from .discovery import advertise
    advertisement = None if host in {"127.0.0.1", "localhost", "::1"} else advertise(board_id, port, str(setting(config, "TC_ADVERTISE_HOST", "advertise_host", "")))
    import signal
    for number in (signal.SIGTERM, signal.SIGINT):
        signal.signal(number, lambda *_: threading.Thread(target=server.shutdown, daemon=True).start())
    try:
        server.serve_forever(poll_interval=0.1)
    finally:
        companion.enter_safe_state("server_stopped")
        server.server_close()
        if advertisement is not None:
            advertisement.close()


if __name__ == "__main__":
    run()
