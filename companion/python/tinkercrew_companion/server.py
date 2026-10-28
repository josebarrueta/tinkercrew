from __future__ import annotations

import hmac
import json
import os
import ssl
import socket
import stat
import errno
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from socketserver import TCPServer
from collections import deque
from urllib.parse import parse_qs, urlparse

from .core import CommandError, Companion
from .hardware import BridgeHardware, MemoryHardware

MAX_BODY_BYTES = 65536


class UsbPlayRegistry:
    """Physical-USB-issued, loopback-only capabilities. Board stores hashes, never Mac keys.

    Separate DB/namespace: no installation, SSH or existing Pairing mutation.
    begin/refresh are host-bootstrap operations, never HTTP routes.
    """
    def __init__(self, directory, board_id, *, clock=time.monotonic, boot_id=None):
        from .auth import AuthError, boot_identity
        import uuid
        if str(uuid.UUID(board_id)) != board_id:
            raise AuthError('invalid_message')
        self.directory = Path(directory)
        self.board_id, self.clock, self.boot_id = board_id, clock, boot_id or boot_identity()
        self.lock = threading.RLock()
        if self.directory.is_symlink() or any(p.is_symlink() for p in self.directory.parents):
            raise AuthError('unsupported_storage')
        self.directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        if self.directory.stat().st_uid != os.geteuid() or self.directory.stat().st_mode & 0o077:
            raise AuthError('unsupported_storage')
        self.path = self.directory / 'usb-play.sqlite'
        import stat
        fd = os.open(self.path, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
        try:
            if not stat.S_ISREG(os.fstat(fd).st_mode):
                raise AuthError('unsupported_storage')
        finally:
            os.close(fd)
        with self._db() as db:
            if db.execute('PRAGMA user_version').fetchone()[0] not in (0, 1):
                raise AuthError('unsupported_storage')
            db.executescript('CREATE TABLE IF NOT EXISTS identity (singleton INTEGER PRIMARY KEY CHECK(singleton=1),board TEXT,epoch TEXT); CREATE TABLE IF NOT EXISTS grants (id TEXT PRIMARY KEY,digest TEXT,board TEXT,boot TEXT,epoch TEXT,expires REAL); CREATE TABLE IF NOT EXISTS sessions (digest TEXT PRIMARY KEY,id TEXT,board TEXT,boot TEXT,epoch TEXT,expires REAL,last_use REAL); PRAGMA user_version=1;')
            db.execute('INSERT OR IGNORE INTO identity VALUES (1,?,?)', (self.board_id,''))
            if db.execute('SELECT board FROM identity WHERE singleton=1').fetchone()[0] != self.board_id:
                raise AuthError('identity_mismatch')

    def _db(self):
        from contextlib import contextmanager
        import sqlite3
        from .auth import AuthError
        @contextmanager
        def opened():
            with self.lock:
                import stat
                info = self.path.lstat()
                if not stat.S_ISREG(info.st_mode) or self.path.is_symlink() or info.st_uid != os.geteuid() or info.st_nlink != 1 or info.st_mode & 0o077:
                    raise AuthError('unsupported_storage')
                db = sqlite3.connect(self.path, timeout=0.2)
                db.row_factory = sqlite3.Row
                try:
                    with db:
                        yield db
                finally:
                    db.close()
        return opened()

    @staticmethod
    def _digest(token, *, session=False):
        import hashlib
        import re
        from .auth import AuthError
        pattern = r'usb1\.[A-Za-z0-9_-]{43}' if session else r'[A-Za-z0-9_-]{43}'
        if not isinstance(token, str) or not re.fullmatch(pattern, token):
            raise AuthError()
        return hashlib.sha256(token.encode()).hexdigest()

    def _epoch(self, db):
        from .auth import AuthError
        epoch = db.execute('SELECT epoch FROM identity WHERE singleton=1 AND board=?', (self.board_id,)).fetchone()
        if not epoch or not epoch[0]:
            raise AuthError('usb_play_unavailable')
        return epoch[0]

    def _prune(self, db):
        now, epoch = self.clock(), self._epoch(db)
        db.execute('DELETE FROM grants WHERE board!=? OR boot!=? OR epoch!=? OR expires<=?', (self.board_id, self.boot_id, epoch, now))
        db.execute('DELETE FROM sessions WHERE board!=? OR boot!=? OR epoch!=? OR expires<=? OR last_use<=?', (self.board_id, self.boot_id, epoch, now, now-60))

    def start_runtime(self):
        import uuid
        with self._db() as db:
            db.execute('BEGIN IMMEDIATE')
            db.execute('UPDATE identity SET epoch=? WHERE singleton=1', (str(uuid.uuid4()),))
            db.execute('DELETE FROM grants')
            db.execute('DELETE FROM sessions')

    def begin(self):
        import secrets
        import uuid
        from .auth import AuthError
        token, identity = secrets.token_urlsafe(32), str(uuid.uuid4())
        with self._db() as db:
            db.execute('BEGIN IMMEDIATE')
            self._prune(db)
            if db.execute('SELECT count(*) FROM grants').fetchone()[0] >= 4:
                raise AuthError('session_limit')
            db.execute('INSERT INTO grants VALUES (?,?,?,?,?,?)', (identity, self._digest(token), self.board_id, self.boot_id, self._epoch(db), self.clock()+30))
        return {'board_id':self.board_id,'grant_id':identity,'grant_token':token}

    def exchange(self, body, *, loopback=False):
        import secrets
        import uuid
        from .auth import AuthError
        if not loopback:
            raise AuthError('usb_transport_required')
        try:
            if set(body) != {'board_id','grant_id','grant_token'} or body['board_id'] != self.board_id or str(uuid.UUID(body['grant_id'])) != body['grant_id']:
                raise ValueError()
            supplied = self._digest(body['grant_token'])
        except (ValueError, TypeError, KeyError):
            raise AuthError() from None
        with self._db() as db:
            db.execute('BEGIN IMMEDIATE')
            self._prune(db)
            row = db.execute('SELECT * FROM grants WHERE id=?', (body['grant_id'],)).fetchone()
            db.execute('DELETE FROM grants WHERE id=?', (body['grant_id'],))
        if not row or not hmac.compare_digest(row['digest'], supplied):
            raise AuthError()
        token, identity = 'usb1.'+secrets.token_urlsafe(32), str(uuid.uuid4())
        with self._db() as db:
            db.execute('BEGIN IMMEDIATE')
            self._prune(db)
            if db.execute('SELECT count(*) FROM sessions').fetchone()[0] >= 2:
                raise AuthError('session_limit')
            if row['epoch'] != self._epoch(db):
                raise AuthError()
            db.execute('INSERT INTO sessions VALUES (?,?,?,?,?,?,?)', (self._digest(token,session=True),identity,self.board_id,self.boot_id,self._epoch(db),self.clock()+150,self.clock()))
        return {'board_id':self.board_id,'boot_id':self.boot_id,'session_id':identity,'session_token':token,'expires_in_seconds':150,'idle_timeout_seconds':60,'usb_play_version':1}

    def authenticate(self, token, *, loopback=False, touch=True):
        from .auth import AuthError
        import sqlite3
        if not loopback:
            return None
        try:
            digest = self._digest(token,session=True)
            with self._db() as db:
                self._prune(db)
                row = db.execute('SELECT id FROM sessions WHERE digest=?', (digest,)).fetchone()
                if not row:
                    return None
                if touch:
                    db.execute('UPDATE sessions SET last_use=? WHERE digest=? AND last_use<=?', (self.clock(),digest,self.clock()-1))
                return 'usb:'+row['id']
        except (AuthError, sqlite3.Error, OSError):
            return None

    def refresh(self, token):
        from .auth import AuthError
        digest = self._digest(token,session=True)
        with self._db() as db:
            db.execute('BEGIN IMMEDIATE')
            self._prune(db)
            if db.execute('UPDATE sessions SET expires=? WHERE digest=?', (self.clock()+150,digest)).rowcount != 1:
                raise AuthError('usb_session_expired')
        return {'board_id':self.board_id,'boot_id':self.boot_id,'usb_play_version':1}

    def close_session(self, token):
        with self._db() as db:
            db.execute('DELETE FROM sessions WHERE digest=?', (self._digest(token,session=True),))


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
    # The vendor SDK invokes notifications on its single RPC receiver. Never
    # acquire Companion's lock there: an outgoing RPC may hold it while waiting
    # for the acknowledgement that only this receiver can deliver.
    inputs = BridgeInputRelay(companion)
    Bridge.provide("tc_buttons", lambda a, b, c: inputs.push('buttons', [bool(a), bool(b), bool(c)]))
    Bridge.provide("tc_distance", lambda millimeters: inputs.push('distance', int(millimeters)))
    Bridge.provide("tc_disconnected", lambda capability: inputs.push('disconnected', str(capability)))
    Bridge.provide("tc_capability_health", lambda capability, health: inputs.push('health', str(capability), str(health)))
    return companion


class BridgeInputRelay:
    """Bounded inbound-only handoff; never wait for Core or hardware in the SDK receiver.

    Exactly one daemon belongs to the Companion child process. Original receive
    times follow input samples through the queue; delayed data never looks fresh.
    Overflow drops queued data and disarms rather than growing memory or rearming.
    """
    def __init__(self, companion):
        import queue
        self.companion = companion
        self.queue = queue.Queue(maxsize=64)
        self.overflow = threading.Event()
        threading.Thread(target=self.run, name='tinkeroo-bridge-inputs', daemon=True).start()

    def push(self, kind, *values):
        import queue
        try:
            self.queue.put_nowait((time.monotonic(), kind, values))
        except queue.Full:
            self.overflow.set()

    def run(self):
        import queue
        while True:
            try:
                if self.overflow.is_set():
                    self.overflow.clear()
                    for _ in range(64):
                        try:
                            self.queue.get_nowait()
                        except queue.Empty:
                            break
                    self.companion.mark_disconnected('buttons')
                    self.companion.mark_disconnected('distance')
                    self.companion.enter_safe_state('bridge_input_overflow')
                received, kind, values = self.queue.get(timeout=0.1)
                if kind == 'buttons':
                    self.companion.record_buttons(values[0], observed_at=received)
                elif kind == 'distance':
                    self.companion.record_distance(values[0], observed_at=received)
                elif kind == 'disconnected':
                    self.companion.mark_disconnected(values[0])
                elif kind == 'health':
                    self.companion.record_capability_health(*values)
            except queue.Empty:
                pass
            except Exception:
                # No values/exception reflection. A processing fault invalidates
                # queued authority; independent MCU/output deadlines still apply.
                self.overflow.set()
                time.sleep(0.05)


class CompanionHTTPServer(ThreadingHTTPServer):
    def server_bind(self):
        # HTTPServer's diagnostic server_name lookup performs blocking reverse DNS.
        # The configured TLS identity is independent; bind/listen must work offline.
        TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address[:2]

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


class CompanionUSBServer(CompanionHTTPServer):
    """Private filesystem listener: ADB can forward it, LAN TCP cannot reach it.

    The app directory is bind-mounted into the official Docker container. Unix
    socket identity survives that boundary; source-IP assumptions do not.
    """
    address_family = socket.AF_UNIX

    def server_bind(self):
        path = Path(self.server_address)
        if path.parent.is_symlink() or path.parent.stat().st_uid != os.geteuid() or path.parent.stat().st_mode & 0o077:
            raise ValueError('Unsafe USB socket directory')
        if path.exists() or path.is_symlink():
            previous = path.lstat()
            if not stat.S_ISSOCK(previous.st_mode) or previous.st_uid != os.geteuid() or previous.st_nlink != 1:
                raise ValueError('Unknown USB socket target')
            with socket.socket(socket.AF_UNIX) as probe:
                try:
                    probe.connect(str(path))
                except OSError as error:
                    if error.errno != errno.ECONNREFUSED:
                        raise
                else:
                    raise ValueError('USB socket is already owned by a live listener')
            if path.lstat().st_ino != previous.st_ino:
                raise ValueError('USB socket ownership changed')
            path.unlink()
        TCPServer.server_bind(self)
        path.chmod(0o600)
        self.owned_socket_inode = path.lstat().st_ino
        self.server_name, self.server_port = 'usb-play', 0

    def serve_forever(self, poll_interval=0.1):
        # The shared main TCP server owns the single control watchdog.
        TCPServer.serve_forever(self, poll_interval=poll_interval)

    def server_close(self):
        super().server_close()
        path = Path(self.server_address)
        try:
            if path.lstat().st_ino == getattr(self, 'owned_socket_inode', None) and stat.S_ISSOCK(path.lstat().st_mode):
                path.unlink()
        except FileNotFoundError:
            pass


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


def make_handler(companion: Companion, token: str | None = None, *, auth=None, usb_play=None, rate_clock=time.monotonic, maintenance=None, control_clock=time.monotonic):
    if (auth is None) == (token is None):
        raise ValueError("Select exactly one authentication mode")
    if auth is not None:
        auth.start_runtime()
    if usb_play is not None:
        if auth is None:
            raise ValueError('USB play requires the prepared TBP identity')
        usb_play.start_runtime()
    def authenticate(value, *, touch=True, loopback=False):
        if usb_play is not None and isinstance(value, str) and value.startswith('usb1.'):
            return usb_play.authenticate(value, touch=touch, loopback=loopback)
        return auth.authenticate(value, touch=touch) if auth is not None else None
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
            if control_token and (control_clock() >= control_until or not authenticate(control_token, touch=False, loopback=True)):
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
            allowed = {"/v1/health/public", "/v1/hello", "/v1/state", "/v1/events", "/v1/commands", "/v1/control-lease", "/v1/self-tests", "/v1/pairings", "/v1/sessions/challenge", "/v1/sessions", "/v1/sessions/current", "/v1/usb-sessions"}
            print(json.dumps({"event": "http", "method": getattr(self, "command", "other") if getattr(self, "command", None) in {"GET", "POST", "DELETE"} else "other", "route": route if route in allowed else "other", "status": code}))

        def _send(self, status: int, body: dict | list) -> None:
            encoded = json.dumps(body, separators=(",", ":")).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(encoded)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(encoded)

        def _loopback(self):
            # Only the protected ADB-forwarded Unix listener, never TCP, even
            # localhost or spoofed forwarded headers. Docker NAT is irrelevant.
            return self.server.address_family == socket.AF_UNIX

        def _authorized(self) -> bool:
            provided = self.headers.get("Authorization", "")
            if len(self.headers.get_all("Authorization", [])) != 1 or len(provided) > 1024:
                return False
            if auth is not None:
                tick()
                return provided.startswith("Bearer ") and authenticate(provided[7:], loopback=self._loopback()) is not None
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
                hello["runtime"] = {"release": os.environ.get("TC_RUNTIME_RELEASE", "unmanaged"), "nonce": os.environ.get("TC_RUNTIME_NONCE", ""), "certificate_sha256": os.environ.get("TC_RUNTIME_CERTIFICATE_SHA256", ""), "boot_id": auth.boot_id if auth is not None else ""}
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
            authentication = auth is not None and self.path in {"/v1/pairings", "/v1/sessions/challenge", "/v1/sessions", "/v1/usb-sessions"}
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
                    if self.path == '/v1/usb-sessions':
                        if usb_play is None:
                            self._send(404, {'status':'error','error':{'code':'usb_play_unavailable'}})
                            return
                        result = usb_play.exchange(body, loopback=self._loopback())
                    elif self.path == "/v1/sessions":
                        with rate_lock:
                            previous = self.headers.get("Authorization", "")
                            previous = previous[7:] if len(self.headers.get_all("Authorization", [])) == 1 and previous.startswith("Bearer ") and len(previous) <= 135 else None
                            owner = authenticate(previous, touch=False, loopback=self._loopback()) if previous else None
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
                    clearing = self.path == '/v1/commands' and body.get('operation') == 'outputs.clear'
                    if auth is not None:
                        tick()
                        current = self.headers["Authorization"][7:]
                        # Recheck after taking the control lock, including revocation during body read.
                        if not authenticate(current, loopback=self._loopback()):
                            raise AuthError()
                        if self.path in {"/v1/commands", "/v1/self-tests", "/v1/control-lease"}:
                            if control_token is not None and control_token != current:
                                self._send(409, {"status": "error", "error": {"code": "board_busy"}})
                                return
                            if self.path == "/v1/control-lease":
                                result = companion.renew_control_lease(body.get("lease_ms", 1500))
                                control_token = current
                                control_until = control_clock() + min(body.get("lease_ms", 1500), 2000) / 1000
                                self._send(200, result)
                                return
                            if control_token != current and not clearing:
                                raise CommandError("safe_state_active: acquire a control lease first")
                    if self.path == "/v1/commands":
                        result = companion.command(body.get("operation"), body.get("arguments", {}))
                        if clearing:
                            # Clearing never arms. It is safe before the first lease
                            # or after Stop, but the foreign-owner check above still
                            # prevents another session interrupting a live controller.
                            control_token, control_until = None, 0.0
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
                value = self.headers['Authorization'][7:]
                (usb_play if usb_play is not None and value.startswith('usb1.') else auth).close_session(value)
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
    usb_play = UsbPlayRegistry(directory, board_id) if mode == 'tbp' else None
    runtime_directory = os.environ.get("TC_RUNTIME_STATE")
    maintenance = (lambda: runtime_maintenance(runtime_directory)) if runtime_directory else None
    handler = make_handler(companion, token if auth is None else None, auth=auth, usb_play=usb_play, maintenance=maintenance)
    server = CompanionHTTPServer((host, port), handler)
    usb_server = CompanionUSBServer(str(directory / 'usb-play.sock'), handler) if usb_play is not None else None

    certificate = setting(config, "TC_TLS_CERT", "tls_cert")
    private_key = setting(config, "TC_TLS_KEY", "tls_key")
    development_http = os.environ.get("TC_DEVELOPMENT_HTTP") == "1" and host in {"127.0.0.1", "localhost"}
    if certificate and private_key:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.minimum_version = ssl.TLSVersion.TLSv1_2
        context.load_cert_chain(certificate, private_key)
        server.socket = context.wrap_socket(server.socket, server_side=True)
        if usb_server is not None:
            usb_server.socket = context.wrap_socket(usb_server.socket, server_side=True)
    elif usb_server is not None or not development_http:
        raise SystemExit("TLS certificate/key are required unless loopback development HTTP is explicitly enabled")

    if usb_server is not None:
        threading.Thread(target=usb_server.serve_forever, daemon=True).start()

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
        if usb_server is not None:
            usb_server.shutdown()
            usb_server.server_close()
        if advertisement is not None:
            advertisement.close()


if __name__ == "__main__":
    run()
