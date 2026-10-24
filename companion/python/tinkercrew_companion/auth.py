"""TBP installation credentials and bounded sessions. Management methods are USB/SSH-only.

Only public keys and hashes of opaque tokens are persisted. No HTTP handler exposes
begin/activate/revoke. SSH activation is fail-closed across filesystem/DB interruption.
"""
from __future__ import annotations

import base64
from collections import deque
from contextlib import contextmanager
import datetime
import fcntl
import hashlib
import hmac
import json
import os
from pathlib import Path
import secrets
import shlex
import sqlite3
import struct
import subprocess
import threading
import time
import uuid


class AuthError(Exception):
    def __init__(self, code="authentication_failed"):
        self.code = code
        super().__init__(code)


def _uuid(value):
    if not isinstance(value, str) or str(uuid.UUID(value)) != value:
        raise ValueError("Invalid identity")
    return value


def _decode(value, size):
    if not isinstance(value, str) or len(value) > 256:
        raise ValueError("Invalid key/proof")
    raw = base64.b64decode(value + "=" * (-len(value) % 4), altchars=b"-_", validate=True)
    if len(raw) != size or base64.urlsafe_b64encode(raw).decode().rstrip("=") != value:
        raise ValueError("Invalid key/proof")
    return raw


def boot_identity():
    try:
        value = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
    except FileNotFoundError:
        value = subprocess.run(["/usr/sbin/sysctl", "-n", "kern.bootsessionuuid"], check=True, capture_output=True, timeout=2, text=True).stdout.strip().lower()
    return _uuid(value)


class PairingRegistry:
    def __init__(self, directory, board_id, *, ssh_directory=None, management_script=None, clock=time.monotonic, boot_id=None):
        self.directory = Path(directory)
        self.board_id = _uuid(board_id)
        self.ssh_directory = Path(ssh_directory) if ssh_directory is not None else None
        self.management_script = Path(management_script or Path(__file__).resolve().parents[1] / "management.py")
        self.clock = clock
        self.boot_id = boot_id or boot_identity()
        self.lock = threading.RLock()
        self.attempts = {}
        self.global_attempts = deque()
        if self.directory.is_symlink():
            raise AuthError("unsupported_storage")
        self.directory.mkdir(parents=True, exist_ok=True, mode=0o700)
        self.directory.chmod(0o700)
        self.path = self.directory / "pairings.sqlite"
        descriptor = os.open(self.path, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
        os.close(descriptor)
        self.path.chmod(0o600)
        with self._db() as db:
            if db.execute("PRAGMA user_version").fetchone()[0] not in (0, 1):
                raise AuthError("unsupported_storage")
            db.executescript("""
                CREATE TABLE IF NOT EXISTS identity (singleton INTEGER PRIMARY KEY CHECK(singleton=1), board TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS pairings (id TEXT PRIMARY KEY, label TEXT NOT NULL, public_key TEXT NOT NULL, ssh_key TEXT NOT NULL, state TEXT NOT NULL, generation INTEGER NOT NULL, created REAL NOT NULL, last_use REAL);
                CREATE TABLE IF NOT EXISTS tokens (id TEXT PRIMARY KEY, digest TEXT NOT NULL, registration TEXT NOT NULL, expires REAL NOT NULL, boot TEXT NOT NULL, attempts INTEGER NOT NULL);
                CREATE TABLE IF NOT EXISTS challenges (id TEXT PRIMARY KEY, installation TEXT NOT NULL, generation INTEGER NOT NULL, message BLOB NOT NULL, expires REAL NOT NULL, boot TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS sessions (digest TEXT PRIMARY KEY, installation TEXT NOT NULL, generation INTEGER NOT NULL, expires REAL NOT NULL, last_use REAL NOT NULL, boot TEXT NOT NULL);
                PRAGMA user_version=1;
            """)
            db.execute("INSERT OR IGNORE INTO identity VALUES (1, ?)", (self.board_id,))
            if db.execute("SELECT board FROM identity WHERE singleton=1").fetchone()[0] != self.board_id:
                raise AuthError("identity_mismatch")

    @contextmanager
    def _db(self):
        with self.lock:
            db = sqlite3.connect(self.path, timeout=0.2)
            db.row_factory = sqlite3.Row
            try:
                with db:
                    yield db
            finally:
                db.close()

    def _registration(self, body):
        try:
            if set(body) != {"installation_id", "label", "public_key", "ssh_public_key"}:
                raise ValueError()
            _uuid(body["installation_id"])
            if not isinstance(body["label"], str) or not 1 <= len(body["label"]) <= 60 or any(ord(c) < 32 or ord(c) == 127 for c in body["label"]):
                raise ValueError()
            public = _decode(body["public_key"], 32)
            parts = body["ssh_public_key"].split(" ")
            if len(parts) != 2 or parts[0] != "ssh-ed25519" or len(parts[1]) > 128:
                raise ValueError()
            blob = base64.b64decode(parts[1], validate=True)
            prefix = struct.pack(">I", 11) + b"ssh-ed25519" + struct.pack(">I", 32)
            if len(blob) != 51 or not blob.startswith(prefix) or blob[-32:] == public:
                raise ValueError()
            return dict(body)
        except (ValueError, TypeError, KeyError, AttributeError):
            raise AuthError("invalid_message") from None

    @staticmethod
    def _hash(token):
        if not isinstance(token, str) or not 32 <= len(token) <= 128 or not token.isascii():
            raise AuthError()
        return hashlib.sha256(token.encode()).hexdigest()

    def begin_pairing(self, registration):
        registration = self._registration(registration)
        token = secrets.token_urlsafe(32)
        with self._db() as db:
            db.execute("BEGIN IMMEDIATE")
            existing = db.execute("SELECT * FROM pairings WHERE id=?", (registration["installation_id"],)).fetchone()
            if existing and (existing["public_key"] != registration["public_key"] or existing["ssh_key"] != registration["ssh_public_key"]):
                raise AuthError("pairing_conflict")
            if not existing and db.execute("SELECT count(*) FROM pairings WHERE state!='revoked'").fetchone()[0] >= 16:
                raise AuthError("pairing_limit")
            now = self.clock()
            db.execute("DELETE FROM tokens WHERE boot!=? OR expires<=?", (self.boot_id, now))
            if db.execute("SELECT count(*) FROM tokens WHERE id!=?", (registration["installation_id"],)).fetchone()[0] >= 16:
                raise AuthError("pairing_limit")
            db.execute("INSERT OR REPLACE INTO tokens VALUES (?, ?, ?, ?, ?, 0)", (registration["installation_id"], self._hash(token), json.dumps(registration, sort_keys=True), now + 300, self.boot_id))
        return {"board_id": self.board_id, "pairing_token": token, "expires_in_seconds": 300, "protocol_versions": ["1.0"]}

    def complete_pairing(self, body):
        registration = self._registration({k: v for k, v in body.items() if k != "pairing_token"})
        supplied = self._hash(body.get("pairing_token"))
        accepted = False
        with self._db() as db:
            db.execute("BEGIN IMMEDIATE")
            row = db.execute("SELECT * FROM tokens WHERE id=?", (registration["installation_id"],)).fetchone()
            if row:
                valid = row["boot"] == self.boot_id and row["expires"] > self.clock() and row["attempts"] < 5
                accepted = valid and hmac.compare_digest(row["digest"], supplied) and row["registration"] == json.dumps(registration, sort_keys=True)
                if accepted or not valid or row["attempts"] >= 4:
                    db.execute("DELETE FROM tokens WHERE id=?", (registration["installation_id"],))
                else:
                    db.execute("UPDATE tokens SET attempts=attempts+1 WHERE id=?", (registration["installation_id"],))
            if accepted:
                existing = db.execute("SELECT * FROM pairings WHERE id=?", (registration["installation_id"],)).fetchone()
                if existing and existing["state"] == "active":
                    state = "active"
                    db.execute("UPDATE pairings SET label=? WHERE id=?", (registration["label"], registration["installation_id"]))
                else:
                    if (not existing or existing["state"] == "revoked") and db.execute("SELECT count(*) FROM pairings WHERE state!='revoked'").fetchone()[0] >= 16:
                        raise AuthError("pairing_limit")
                    state = "pending_ssh"
                    generation = existing["generation"] + 1 if existing else 1
                    db.execute("INSERT OR REPLACE INTO pairings VALUES (?, ?, ?, ?, ?, ?, ?, NULL)", (registration["installation_id"], registration["label"], registration["public_key"], registration["ssh_public_key"], state, generation, time.time()))
        if not accepted:
            raise AuthError()
        return {"board_id": self.board_id, "installation_id": registration["installation_id"], "state": state}

    def _ssh_key(self, installation, key=None):
        if self.ssh_directory is None or self.ssh_directory.is_symlink():
            raise AuthError("ssh_management_unavailable")
        if key is None and not self.ssh_directory.exists():
            return
        self.ssh_directory.mkdir(mode=0o700, exist_ok=True)
        if self.ssh_directory.stat().st_uid != os.geteuid() or self.ssh_directory.stat().st_mode & 0o022:
            raise AuthError("ssh_management_unavailable")
        lock_fd = os.open(self.ssh_directory / ".tinkercrew.lock", os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
        with os.fdopen(lock_fd, "r+") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            path = self.ssh_directory / "authorized_keys"
            if path.is_symlink():
                raise AuthError("ssh_management_unavailable")
            try:
                stat = path.stat()
                if stat.st_uid != os.geteuid() or stat.st_mode & 0o022 or stat.st_size > 262144:
                    raise AuthError("ssh_management_unavailable")
                old = path.read_bytes()
            except FileNotFoundError:
                old = b""
            tag = (" tinkercrew:" + installation).encode()
            new = b"".join(line for line in old.splitlines(keepends=True) if not line.rstrip(b"\r\n").endswith(tag))
            if key:
                command = shlex.join(["python3", str(self.management_script), "--installation", installation]).replace("\\", "\\\\").replace('"', '\\"')
                new += (b"\n" if new and not new.endswith(b"\n") else b"") + f'restrict,command="{command}" {key}'.encode() + tag + b"\n"
            if new == old:
                return
            temporary = self.ssh_directory / ".tinkercrew-authorized.tmp"
            descriptor = os.open(temporary, os.O_CREAT | os.O_TRUNC | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
            with os.fdopen(descriptor, "wb") as handle:
                handle.write(new)
                handle.flush()
                os.fsync(handle.fileno())
            os.replace(temporary, path)
            directory_fd = os.open(self.ssh_directory, os.O_RDONLY | os.O_DIRECTORY)
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)

    def activate_pairing(self, installation):
        _uuid(installation)
        with self._db() as db:
            db.execute("BEGIN IMMEDIATE")
            row = db.execute("SELECT * FROM pairings WHERE id=?", (installation,)).fetchone()
            if not row or row["state"] not in {"pending_ssh", "active"}:
                raise AuthError()
            # Key first, DB authorization second. An interrupted key write never grants gameplay.
            self._ssh_key(installation, row["ssh_key"])
            db.execute("UPDATE pairings SET state='active' WHERE id=?", (installation,))
        return {"installation_id": installation, "state": "active"}

    def list_pairings(self):
        with self._db() as db:
            return [dict(row) for row in db.execute("SELECT id AS installation_id, label, state, created, last_use FROM pairings ORDER BY created")]

    def is_active(self, installation):
        with self._db() as db:
            return db.execute("SELECT 1 FROM pairings WHERE id=? AND state='active'", (installation,)).fetchone() is not None

    def revoke(self, installation):
        _uuid(installation)
        with self._db() as db:
            db.execute("BEGIN IMMEDIATE")
            db.execute("UPDATE pairings SET state='revoked', generation=generation+1 WHERE id=?", (installation,))
            db.execute("DELETE FROM sessions WHERE installation=?", (installation,))
            db.execute("DELETE FROM challenges WHERE installation=?", (installation,))
            db.execute("DELETE FROM tokens WHERE id=?", (installation,))
            db.execute("DELETE FROM pairings WHERE state='revoked' AND id NOT IN (SELECT id FROM pairings WHERE state='revoked' ORDER BY created DESC LIMIT 64)")
        # Revoke BEFORE filesystem cleanup. Any leftover forced SSH key still checks is_active.
        removed = True
        try:
            self._ssh_key(installation)
        except (OSError, AuthError):
            removed = False
        return {"installation_id": installation, "revoked": True, "ssh_key_removed": removed}

    def _rate(self, installation):
        now = self.clock()
        while self.global_attempts and self.global_attempts[0] <= now - 60:
            self.global_attempts.popleft()
        self.attempts = {key: values for key, values in self.attempts.items() if values[-1] > now - 60}
        values = self.attempts.setdefault(installation, deque())
        while values and values[0] <= now - 60:
            values.popleft()
        if len(self.global_attempts) >= 20 or len(values) >= 5:
            if not values:
                self.attempts.pop(installation, None)
            raise AuthError("rate_limited")
        self.global_attempts.append(now)
        values.append(now)

    def challenge(self, body):
        try:
            if set(body) != {"board_id", "installation_id"} or body["board_id"] != self.board_id:
                raise ValueError()
            installation = _uuid(body["installation_id"])
        except (ValueError, TypeError, KeyError):
            raise AuthError() from None
        with self._db() as db:
            self._rate(installation)
            db.execute("BEGIN IMMEDIATE")
            row = db.execute("SELECT * FROM pairings WHERE id=? AND state='active'", (installation,)).fetchone()
            if not row:
                raise AuthError()
            challenge = str(uuid.uuid4())
            nonce = secrets.token_urlsafe(32)
            expires = (datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(seconds=30)).isoformat(timespec="seconds").replace("+00:00", "Z")
            message = f"TBP1\n{self.board_id}\n{installation}\n{challenge}\n{nonce}\n{expires}\n".encode()
            db.execute("DELETE FROM challenges WHERE boot!=? OR expires<=?", (self.boot_id, self.clock()))
            db.execute("DELETE FROM challenges WHERE installation=?", (installation,))
            db.execute("INSERT INTO challenges VALUES (?, ?, ?, ?, ?, ?)", (challenge, installation, row["generation"], message, self.clock() + 30, self.boot_id))
        return {"board_id": self.board_id, "installation_id": installation, "challenge_id": challenge, "nonce": nonce, "expires_at": expires}

    def create_session(self, body, *, replace_token=None):
        try:
            if set(body) != {"board_id", "installation_id", "challenge_id", "signature"} or body["board_id"] != self.board_id:
                raise ValueError()
            installation = _uuid(body["installation_id"])
            challenge = _uuid(body["challenge_id"])
        except (ValueError, TypeError, KeyError):
            raise AuthError() from None
        with self._db() as db:
            db.execute("BEGIN IMMEDIATE")
            row = db.execute("SELECT c.*, p.public_key FROM challenges c JOIN pairings p ON p.id=c.installation WHERE c.id=? AND c.installation=? AND p.state='active' AND p.generation=c.generation", (challenge, installation)).fetchone()
            # Commit consumption even for an invalid signature: one attempt, not just one success.
            db.execute("DELETE FROM challenges WHERE id=? AND installation=?", (challenge, installation))
        if not row or row["boot"] != self.boot_id or row["expires"] <= self.clock():
            raise AuthError()
        try:
            from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
            Ed25519PublicKey.from_public_bytes(_decode(row["public_key"], 32)).verify(_decode(body["signature"], 64), row["message"])
        except Exception:
            raise AuthError() from None
        token = secrets.token_urlsafe(32)
        now = self.clock()
        with self._db() as db:
            db.execute("BEGIN IMMEDIATE")
            if not db.execute("SELECT 1 FROM pairings WHERE id=? AND state='active' AND generation=?", (installation, row["generation"])).fetchone():
                raise AuthError()
            db.execute("DELETE FROM sessions WHERE boot!=? OR expires<=? OR last_use<=?", (self.boot_id, now, now - 120))
            replaced = False
            if replace_token is not None:
                previous = db.execute("SELECT 1 FROM sessions WHERE digest=? AND installation=? AND generation=?", (self._hash(replace_token), installation, row["generation"])).fetchone()
                if previous:
                    db.execute("DELETE FROM sessions WHERE digest=?", (self._hash(replace_token),))
                    replaced = True
            if db.execute("SELECT count(*) FROM sessions").fetchone()[0] >= 2:
                raise AuthError("session_limit")
            db.execute("INSERT INTO sessions VALUES (?, ?, ?, ?, ?, ?)", (self._hash(token), installation, row["generation"], now + 900, now, self.boot_id))
        return {"session_token": token, "expires_in_seconds": 900, "idle_timeout_seconds": 120, "replaced_session": replaced}

    def authenticate(self, token, *, touch=True):
        try:
            digest = self._hash(token)
            now = self.clock()
            with self._db() as db:
                row = db.execute("SELECT s.installation FROM sessions s JOIN pairings p ON p.id=s.installation WHERE s.digest=? AND s.boot=? AND s.expires>? AND s.last_use>? AND p.state='active' AND p.generation=s.generation", (digest, self.boot_id, now, now - 120)).fetchone()
                if not row:
                    return None
                if touch:
                    db.execute("UPDATE sessions SET last_use=? WHERE digest=? AND last_use<=?", (now, digest, now - 1))
                    db.execute("UPDATE pairings SET last_use=? WHERE id=? AND (last_use IS NULL OR last_use<=?)", (time.time(), row["installation"], time.time() - 60))
                return row["installation"]
        except (AuthError, sqlite3.Error):
            return None

    def close_session(self, token):
        with self._db() as db:
            db.execute("DELETE FROM sessions WHERE digest=?", (self._hash(token),))

    def start_runtime(self):
        """Server-only: restart invalidates all old sessions/challenges, not USB tokens or Pairings."""
        with self._db() as db:
            db.execute("DELETE FROM sessions")
            db.execute("DELETE FROM challenges")
