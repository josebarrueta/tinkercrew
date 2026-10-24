"""Content-addressed Linux runtime updates; never writes firmware, dependencies or identities."""
from contextlib import contextmanager
import ast
import base64
import fcntl
import hashlib
import hmac
import json
import math
import os
from pathlib import Path
import re
import secrets
import shutil
import time

from .auth import AuthError, boot_identity

RUNTIME_FILES = tuple("python/tinkercrew_companion/" + name + ".py" for name in ("__init__", "auth", "core", "discovery", "hardware", "server"))
CONTRACT_FILES = ("app.yaml", "sketch/sketch.ino", "sketch/sketch.yaml", "python/main.py", "python/management.py", "python/requirements.txt", "python/requirements-auth.txt", "python/tinkercrew_companion/management.py", "python/tinkercrew_companion/deployment.py", "python/tinkercrew_companion/supervisor.py", "python/tinkercrew_companion/auth.py")
MAX_BUNDLE = 2 * 1024 * 1024


class DeploymentError(AuthError):
    pass


def _digest(data):
    return hashlib.sha256(data).hexdigest()


def _hex(value):
    if not isinstance(value, str) or not re.fullmatch("[a-f0-9]{64}", value):
        raise DeploymentError("invalid_deployment")
    return value


def _links(path):
    if path.is_symlink() or any(parent.is_symlink() for parent in path.parents):
        raise DeploymentError("unsupported_storage")


def _file(path, maximum=262144):
    _links(path)
    if path.is_symlink() or not path.is_file() or path.stat().st_size > maximum:
        raise DeploymentError("unsupported_runtime")
    return path.read_bytes()


def _tree_digest(files):
    return _digest(b"".join(f"{name}\0{len(data)}\0{_digest(data)}\n".encode() for name, data in sorted(files.items())))


def _release(contract, files):
    return _digest(b"TC-RUNTIME-1\n" + contract.encode() + b"\n" + _tree_digest(files).encode() + b"\n")


def build_bundle(root):
    root = Path(root).resolve()
    contract = _tree_digest({name: _file(root / name) for name in CONTRACT_FILES})
    files = {name: _file(root / name) for name in RUNTIME_FILES}
    release = _release(contract, files)
    payload = json.dumps({"format": 1, "contract": contract, "release": release, "files": {name: base64.b64encode(data).decode() for name, data in files.items()}}, sort_keys=True, separators=(",", ":")).encode()
    if len(payload) > MAX_BUNDLE:
        raise DeploymentError("bundle_too_large")
    return {"release": release, "contract": contract, "bytes": payload}


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise DeploymentError("invalid_deployment")
        result[key] = value
    return result


def _read_json(path):
    _links(path)
    if path.is_symlink() or not path.is_file() or path.stat().st_size > MAX_BUNDLE:
        raise DeploymentError("invalid_deployment")
    return json.loads(path.read_bytes(), object_pairs_hook=_unique)


def _sync_dir(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def _atomic(path, value):
    _links(path.parent)
    temporary = path.with_name(path.name + ".tmp")
    fd = os.open(temporary, os.O_CREAT | os.O_TRUNC | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as handle:
        handle.write(json.dumps(value, sort_keys=True, separators=(",", ":")).encode())
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)
    _sync_dir(path.parent)


class RuntimeDeployment:
    def __init__(self, root, *, boot_id=None, clock=time.monotonic, trial_seconds=45):
        self.root = Path(root).resolve()
        self.baseline = build_bundle(self.root)
        self.boot = boot_id or boot_identity()
        self.clock = clock
        if not 0 < trial_seconds <= 45:
            raise DeploymentError("invalid_trial_window")
        self.trial_seconds = trial_seconds
        self.directory = self.root / "data/runtime"
        _links(self.directory)
        if self.directory.is_symlink():
            raise DeploymentError("unsupported_storage")
        self.directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        self.directory.chmod(0o700)
        self.job_path = self.directory / "job.json"
        self.selection_path = self.directory / "selection.json"

    @contextmanager
    def _locked(self):
        fd = os.open(self.directory / "operation.lock", os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, "r+") as handle:
            deadline = time.monotonic() + 0.5
            while True:
                try:
                    fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    break
                except BlockingIOError:
                    if time.monotonic() >= deadline:
                        raise DeploymentError("deployment_busy") from None
                    time.sleep(0.005)
            yield

    def _job(self):
        if not self.job_path.exists() and not self.job_path.is_symlink():
            return None
        job = _read_json(self.job_path)
        required = {"release", "sha256", "size", "offset", "phase", "token_hash", "expires", "boot"}
        if not isinstance(job, dict) or not required <= set(job) or not isinstance(job["phase"], str) or job["phase"] not in {"uploading", "staged", "publishing", "awaiting_health", "acknowledging", "committed", "rolled_back", "aborted"}:
            raise DeploymentError("invalid_deployment")
        for key in ("release", "sha256", "token_hash"):
            _hex(job[key])
        if type(job["size"]) is not int or not 1 <= job["size"] <= MAX_BUNDLE or type(job["offset"]) is not int or not 0 <= job["offset"] <= job["size"] or not isinstance(job["boot"], str) or not job["boot"] or type(job["expires"]) not in {int, float} or not math.isfinite(job["expires"]):
            raise DeploymentError("invalid_deployment")
        return job

    def _selection(self):
        if not self.selection_path.exists():
            return {"current": self.baseline["release"], "previous": None, "trial": None}
        value = _read_json(self.selection_path)
        if not isinstance(value, dict) or set(value) != {"current", "previous", "trial"}:
            raise DeploymentError("invalid_deployment")
        _hex(value["current"])
        if value["previous"] is not None:
            _hex(value["previous"])
        trial = value["trial"]
        if trial is not None:
            if not isinstance(trial, dict) or set(trial) != {"id", "boot", "deadline"} or not isinstance(trial["id"], str) or not re.fullmatch("[A-Za-z0-9_-]{22}", trial["id"]) or not isinstance(trial["boot"], str) or not trial["boot"] or type(trial["deadline"]) not in {int, float} or not math.isfinite(trial["deadline"]):
                raise DeploymentError("invalid_deployment")
        return value

    def _authorized(self, job, token):
        if not job or not isinstance(token, str) or len(token) != 43 or not hmac.compare_digest(job["token_hash"], _digest(token.encode())):
            raise DeploymentError("deployment_authorization_failed")
        if job["boot"] != self.boot or job["expires"] <= self.clock():
            raise DeploymentError("deployment_authorization_expired")

    def _status(self, job):
        selected = self._selection()
        result = {"protocol": 1, "current": selected["current"], "contract": self.baseline["contract"], "phase": "idle", "offset": 0, "supervisor_ready": self._observed() is not None, "observed": self._observed()}
        if job:
            result.update({key: job[key] for key in ("release", "sha256", "size", "phase", "offset")})
            if "reason" in job:
                result["reason"] = job["reason"]
        return result

    def _observed(self):
        try:
            value = _read_json(self.directory / "observed.json")
            if value["boot"] != self.boot or not 0 <= self.clock() - value["seen"] < 2:
                return None
            return value
        except (DeploymentError, FileNotFoundError, KeyError, TypeError, ValueError):
            return None

    def launch_spec(self, release):
        """Supervisor interface; validates bytes again immediately before executing a release."""
        _hex(release)
        if release == self.baseline["release"]:
            location = self.root
        else:
            location = self.directory / "releases" / release
            try:
                manifest = _read_json(location / "manifest.json")
            except (OSError, ValueError, TypeError):
                raise DeploymentError("runtime_integrity_failed") from None
            if not isinstance(manifest, dict) or manifest.get("contract") != self.baseline["contract"] or manifest.get("release") != release or not isinstance(manifest.get("files"), dict) or set(manifest["files"]) != set(RUNTIME_FILES):
                raise DeploymentError("runtime_integrity_failed")
        files = {name: _file(location / name) for name in RUNTIME_FILES}
        if _release(self.baseline["contract"], files) != release:
            raise DeploymentError("runtime_integrity_failed")
        return {"release": release, "python_path": str(location / "python")}

    def _rollback(self, selected, job, reason):
        previous = None
        for candidate in ((job or {}).get("prior"), selected["previous"], self.baseline["release"]):
            if not candidate or (candidate == selected["current"] and reason != "interrupted_commit"):
                continue
            try:
                self.launch_spec(candidate)
            except DeploymentError:
                continue
            previous = candidate
            break
        if previous is None:
            raise DeploymentError("no_verified_previous_runtime")
        _atomic(self.selection_path, {"current": previous, "previous": None, "trial": None})
        if job:
            job["phase"] = "rolled_back"
            job["reason"] = reason
            _atomic(self.job_path, job)
        return self._selection()

    def supervise(self, *, failed_release=None):
        """Reconcile interrupted promotion and expired/unhealthy trials without a Mac connection."""
        with self._locked():
            selected = self._selection()
            job = self._job()
            if job and job["phase"] in {"publishing", "acknowledging"}:
                if job["phase"] == "acknowledging" and selected["current"] == job["release"] and not selected["trial"]:
                    job["phase"] = "committed"
                    _atomic(self.job_path, job)
                else:
                    selected = self._rollback(selected, job, "interrupted_commit")
            trial = selected["trial"]
            if trial:
                reason = "child_exited" if failed_release == selected["current"] else "board_rebooted" if trial["boot"] != self.boot else "health_timeout" if self.clock() >= trial["deadline"] else None
                if not job or job["phase"] != "awaiting_health" or job.get("trial") != trial["id"]:
                    reason = "interrupted_commit"
                if reason:
                    selected = self._rollback(selected, job, reason)
            elif failed_release == selected["current"] and (selected["previous"] or selected["current"] != self.baseline["release"]):
                selected = self._rollback(selected, job, "child_exited_after_commit")
            elif job and job["phase"] == "awaiting_health" and selected["current"] != job["release"]:
                job.update(phase="rolled_back", reason="interrupted_recovery")
                _atomic(self.job_path, job)
            try:
                spec = self.launch_spec(selected["current"])
            except DeploymentError:
                if not selected["trial"] and not selected["previous"] and selected["current"] == self.baseline["release"]:
                    raise
                selected = self._rollback(selected, job, "runtime_integrity_failed")
                spec = self.launch_spec(selected["current"])
            return dict(spec, trial=selected["trial"]["id"] if selected["trial"] else None, boot=self.boot)

    def dispatch(self, operation, body):
        allowed = {"status": set(), "begin": {"release", "sha256", "size", "contract"}, "chunk": {"token", "offset", "data"}, "stage": {"token"}, "commit": {"token"}, "finish": {"token", "nonce"}, "abort": {"token"}, "cancel": {"release"}}
        if operation not in allowed or set(body) != allowed[operation]:
            raise DeploymentError("invalid_deployment")
        if operation == "status":
            return self._status(self._job())
        with self._locked():
            job = self._job()
            if operation == "begin":
                release = _hex(body["release"])
                digest = _hex(body["sha256"])
                if body["contract"] != self.baseline["contract"] or type(body["size"]) is not int or not 1 <= body["size"] <= MAX_BUNDLE:
                    raise DeploymentError("unsupported_runtime")
                selected = self._selection()
                if selected["current"] == release and not selected["trial"]:
                    return dict(self._status(job), phase="current", release=release)
                if selected["trial"] and (not job or job["release"] != release or job["phase"] != "awaiting_health"):
                    raise DeploymentError("deployment_busy")
                if selected["trial"]:
                    token = secrets.token_urlsafe(32)
                    job.update(token_hash=_digest(token.encode()), expires=self.clock() + 600, boot=self.boot)
                    _atomic(self.job_path, job)
                    return dict(self._status(job), token=token)
                if job and job["phase"] in {"uploading", "staged"}:
                    if any(job[key] != body[key] for key in ("release", "sha256", "size")):
                        raise DeploymentError("deployment_pending")
                else:
                    if shutil.disk_usage(self.directory).free < body["size"] * 3 + 2 * 1024 * 1024:
                        raise DeploymentError("insufficient_disk_space")
                    job = {"release": release, "sha256": digest, "size": body["size"], "offset": 0, "phase": "uploading"}
                    fd = os.open(self.directory / "incoming.bundle", os.O_CREAT | os.O_TRUNC | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
                    os.close(fd)
                token = secrets.token_urlsafe(32)
                job.update(token_hash=_digest(token.encode()), expires=self.clock() + 600, boot=self.boot)
                _atomic(self.job_path, job)
                return dict(self._status(job), token=token)
            if operation == "cancel":
                if not job or job["release"] != _hex(body["release"]):
                    raise DeploymentError("deployment_changed")
            else:
                self._authorized(job, body.get("token"))
            if operation in {"abort", "cancel"}:
                selected = self._selection()
                if selected["trial"]:
                    self._rollback(selected, job, "parent_aborted")
                elif job["phase"] not in {"committed", "rolled_back"}:
                    job["phase"] = "aborted"
                    _atomic(self.job_path, job)
                return self._status(self._job())
            if operation == "commit":
                if job["phase"] == "awaiting_health":
                    return self._status(job)
                if job["phase"] != "staged" or self._selection()["trial"]:
                    raise DeploymentError("deployment_not_staged")
                selected = self._selection()
                observed = self._observed()
                if not observed or observed["release"] != selected["current"] or observed["state"] != "launched":
                    raise DeploymentError("supervisor_unavailable")
                self.launch_spec(job["release"])
                self.launch_spec(selected["current"])
                trial = {"id": secrets.token_urlsafe(16), "boot": self.boot, "deadline": self.clock() + self.trial_seconds}
                job.update(phase="publishing", trial=trial["id"], prior=selected["current"])
                _atomic(self.job_path, job)
                _atomic(self.selection_path, {"current": job["release"], "previous": selected["current"], "trial": trial})
                job["phase"] = "awaiting_health"
                _atomic(self.job_path, job)
                return self._status(job)
            if operation == "finish":
                if job["phase"] == "committed":
                    return self._status(job)
                selected = self._selection()
                observed = self._observed()
                trial = selected["trial"]
                if job["phase"] != "awaiting_health" or not trial or trial["boot"] != self.boot or self.clock() >= trial["deadline"] or not observed or observed["release"] != job["release"] or observed["trial"] != trial["id"] or observed["nonce"] != body["nonce"] or observed["state"] != "launched":
                    raise DeploymentError("health_evidence_stale")
                job.update(phase="acknowledging", observed_nonce=body["nonce"])
                _atomic(self.job_path, job)
                _atomic(self.selection_path, {"current": job["release"], "previous": selected["previous"], "trial": None})
                job["phase"] = "committed"
                _atomic(self.job_path, job)
                return self._status(job)
            if operation == "chunk":
                if job["phase"] != "uploading" or type(body["offset"]) is not int or body["offset"] != job["offset"] or not isinstance(body["data"], str) or len(body["data"]) > 10924:
                    raise DeploymentError("invalid_chunk")
                try:
                    chunk = base64.b64decode(body["data"], validate=True)
                except ValueError:
                    raise DeploymentError("invalid_chunk") from None
                if not chunk or len(chunk) > 8192 or job["offset"] + len(chunk) > job["size"]:
                    raise DeploymentError("invalid_chunk")
                fd = os.open(self.directory / "incoming.bundle", os.O_WRONLY | os.O_NOFOLLOW)
                with os.fdopen(fd, "r+b") as output:
                    # An interrupted append may be ahead of the durable offset: truncate and resume.
                    if os.fstat(output.fileno()).st_size < job["offset"]:
                        raise DeploymentError("invalid_deployment")
                    output.truncate(job["offset"])
                    output.seek(job["offset"])
                    output.write(chunk)
                    output.flush()
                    os.fsync(output.fileno())
                job["offset"] += len(chunk)
                _atomic(self.job_path, job)
                return self._status(job)
            if operation == "stage":
                if job["phase"] == "staged":
                    return self._status(job)
                if job["phase"] != "uploading" or job["offset"] != job["size"]:
                    raise DeploymentError("incomplete_transfer")
                raw = _file(self.directory / "incoming.bundle", MAX_BUNDLE)
                if _digest(raw) != job["sha256"]:
                    raise DeploymentError("bundle_hash_mismatch")
                bundle = json.loads(raw, object_pairs_hook=_unique)
                if set(bundle) != {"format", "contract", "release", "files"} or type(bundle["format"]) is not int or bundle["format"] != 1 or bundle["contract"] != self.baseline["contract"] or set(bundle["files"]) != set(RUNTIME_FILES):
                    raise DeploymentError("unsupported_runtime")
                files = {}
                for name, encoded in bundle["files"].items():
                    if not isinstance(encoded, str) or len(encoded) > 349528:
                        raise DeploymentError("invalid_deployment")
                    data = base64.b64decode(encoded, validate=True)
                    if len(data) > 262144:
                        raise DeploymentError("bundle_too_large")
                    ast.parse(data.decode("utf-8"), filename=name)
                    files[name] = data
                candidate_contract = _tree_digest({name: files[name] if name in files else _file(self.root / name) for name in CONTRACT_FILES})
                if candidate_contract != self.baseline["contract"]:
                    raise DeploymentError("unsupported_runtime")
                if _release(bundle["contract"], files) != job["release"] or bundle["release"] != job["release"]:
                    raise DeploymentError("bundle_hash_mismatch")
                release_dir = self.directory / "releases" / job["release"]
                _links(release_dir)
                release_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
                if release_dir.is_symlink():
                    raise DeploymentError("unsupported_storage")
                for name, data in files.items():
                    path = release_dir / name
                    _links(path)
                    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                    fd = os.open(path, os.O_CREAT | os.O_TRUNC | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
                    with os.fdopen(fd, "wb") as handle:
                        handle.write(data)
                        handle.flush()
                        os.fsync(handle.fileno())
                _sync_dir(release_dir / "python")
                _atomic(release_dir / "manifest.json", {"format": 1, "contract": bundle["contract"], "release": job["release"], "files": {name: _digest(data) for name, data in files.items()}})
                _sync_dir(release_dir / "python/tinkercrew_companion")
                _sync_dir(release_dir.parent)
                job["phase"] = "staged"
                _atomic(self.job_path, job)
                return self._status(job)
