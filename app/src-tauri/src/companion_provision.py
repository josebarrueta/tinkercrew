"""Fixed initial Companion source/identity provisioning. Never runs Arduino CLI or firmware writers."""
import base64
from contextlib import contextmanager
import fcntl
import hashlib
import hmac
import json
import os
from pathlib import Path
import pwd
import re
import secrets
import shutil
import subprocess
import sys
import time
import uuid

SOURCE_FILES = ("app.yaml", "sketch/sketch.ino", "sketch/sketch.yaml", "python/main.py", "python/management.py", "python/requirements.txt", "python/requirements-auth.txt", *("python/tinkercrew_companion/" + name + ".py" for name in ("__init__", "auth", "core", "discovery", "hardware", "server", "management", "deployment", "supervisor")))
MAX_PACKAGE = 2 * 1024 * 1024


class ProvisionError(Exception):
    pass


def digest(data):
    return hashlib.sha256(data).hexdigest()


def hex_id(value):
    if not isinstance(value, str) or not re.fullmatch("[a-f0-9]{64}", value):
        raise ProvisionError("invalid_request")
    return value


def no_links(path):
    if path.is_symlink() or any(parent.is_symlink() for parent in path.parents):
        raise ProvisionError("unsupported_storage")


def read_file(path, maximum=262144):
    no_links(path)
    if not path.is_file() or path.stat().st_size > maximum:
        raise ProvisionError("unsupported_storage")
    return path.read_bytes()


def unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ProvisionError("invalid_metadata")
        result[key] = value
    return result


def read_json(path):
    return json.loads(read_file(path, MAX_PACKAGE), object_pairs_hook=unique)


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def atomic(path, value):
    no_links(path.parent)
    temporary = path.with_name(path.name + ".tmp")
    fd = os.open(temporary, os.O_CREAT | os.O_TRUNC | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as handle:
        handle.write(json.dumps(value, sort_keys=True, separators=(",", ":")).encode())
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)
    sync_directory(path.parent)


def release_id(files):
    return digest(b"".join(f"{name}\0{len(data)}\0{digest(data)}\n".encode() for name, data in sorted(files.items())))


def build_package(root):
    root = Path(root).resolve()
    files = {name: read_file(root / name) for name in SOURCE_FILES}
    release = release_id(files)
    raw = json.dumps({"format": 1, "release": release, "files": {name: base64.b64encode(data).decode() for name, data in files.items()}}, sort_keys=True, separators=(",", ":")).encode()
    if len(raw) > MAX_PACKAGE:
        raise ProvisionError("package_too_large")
    return {"release": release, "bytes": raw}


def inspect_host():
    if b"arduino,imola" not in Path("/proc/device-tree/compatible").read_bytes().split(b"\0"):
        raise ProvisionError("unsupported_board")
    raw = Path("/sys/devices/soc0/serial_number").read_text().strip()
    serial = int(raw, 16 if raw.lower().startswith("0x") else 10)
    if not 0 < serial < 2**63:
        raise ProvisionError("unsupported_board")
    os_info = dict(line.split("=", 1) for line in Path("/etc/os-release").read_text().splitlines() if "=" in line)
    return {"serial": str(serial), "uid": os.geteuid(), "username": pwd.getpwuid(os.geteuid()).pw_name, "os": os_info.get("ID", "").strip('"'), "version": os_info.get("VERSION_ID", "").strip('"')}


def boot_identity():
    try:
        return Path("/proc/sys/kernel/random/boot_id").read_text().strip()
    except FileNotFoundError:
        return subprocess.run(["/usr/sbin/sysctl", "-n", "kern.bootsessionuuid"], check=True, capture_output=True, text=True, timeout=2).stdout.strip()


class CompanionProvisioning:
    def __init__(self, apps, *, inspect=inspect_host, openssl="/usr/bin/openssl", clock=time.monotonic, boot=None):
        no_links(Path(apps))
        self.apps = Path(apps).resolve()
        self.root = self.apps / "tinkercrew-companion"
        self.directory = self.apps / ".tinkercrew-provision"
        self.inspect = inspect
        self.openssl = openssl
        self.clock = clock
        self.boot = boot or boot_identity()

    @contextmanager
    def locked(self):
        no_links(self.directory)
        self.directory.mkdir(mode=0o700, exist_ok=True)
        self.directory.chmod(0o700)
        fd = os.open(self.directory / "operation.lock", os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, "r+") as handle:
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise ProvisionError("provisioning_busy") from None
            yield

    def job(self):
        path = self.directory / "job.json"
        if not path.exists() and not path.is_symlink():
            return None
        job = read_json(path)
        required = {"phase", "release", "sha256", "size", "offset", "boot", "expires", "token_hash"}
        if not isinstance(job, dict) or set(job) != required or job["phase"] not in {"receiving", "publishing", "published", "aborted"} or type(job["size"]) is not int or not 0 < job["size"] <= MAX_PACKAGE or type(job["offset"]) is not int or not 0 <= job["offset"] <= job["size"]:
            raise ProvisionError("corrupt_provisioning_state")
        for key in ("release", "sha256", "token_hash"):
            hex_id(job[key])
        return job

    def installed(self):
        no_links(self.root)
        if not self.root.exists():
            return None
        receipt = read_json(self.root / ".tinkercrew-install.json")
        files = {name: read_file(self.root / name) for name in SOURCE_FILES}
        if receipt.get("format") != 1 or receipt.get("release") != release_id(files):
            raise ProvisionError("different_or_modified_baseline")
        return receipt

    def status(self):
        installed = self.installed()
        job = self.job()
        result = {"phase": "awaiting_identity" if installed else job["phase"] if job else "absent", "release": installed["release"] if installed else job["release"] if job else None, "offset": job["offset"] if job else 0, "started": False}
        config_path = self.root / "data/companion.json"
        certificate = self.root / "data/board.crt"
        if installed and config_path.exists() and certificate.exists():
            config = read_json(config_path)
            if isinstance(config, dict) and config.get("auth_mode") == "tbp" and config.get("tls_cert") == "data/board.crt":
                read_file(certificate, 8192)
                public_pem = self.tool(["x509", "-in", str(certificate), "-outform", "PEM"]).decode()
                result["identity"] = {"board_id": config.get("board_id"), "tls_name": config.get("tls_name"), "certificate_pem": public_pem}
        return result

    def authorized(self, job, token):
        if not job or job["phase"] == "aborted" or not isinstance(token, str) or len(token) != 43 or not hmac.compare_digest(job["token_hash"], digest(token.encode())) or job["boot"] != self.boot or type(job["expires"]) not in {float, int} or not self.clock() < job["expires"]:
            raise ProvisionError("provisioning_authorization_expired")

    def tool(self, arguments, *, input=None):
        environment = {key: value for key, value in os.environ.items() if not key.startswith(("OPENSSL_", "DYLD_")) and key != "RANDFILE"}
        environment["LC_ALL"] = "C"
        result = subprocess.run([self.openssl, *arguments], input=input, capture_output=True, timeout=8, env=environment)
        if result.returncode or len(result.stdout) > 16384 or len(result.stderr) > 16384:
            raise ProvisionError("identity_tool_failed")
        return result.stdout

    @contextmanager
    def renewal_lock(self):
        runtime = self.root / "data/runtime"
        if not runtime.exists():
            yield
            return
        no_links(runtime)
        descriptor = os.open(runtime / "operation.lock", os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
        with os.fdopen(descriptor, "r+") as handle:
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise ProvisionError("runtime_maintenance_pending") from None
            job = runtime / "job.json"
            if job.exists():
                state = read_json(job)
                if not isinstance(state, dict) or state.get("phase") not in {"committed", "rolled_back", "aborted"}:
                    raise ProvisionError("runtime_maintenance_pending")
            selection = runtime / "selection.json"
            if selection.exists() and read_json(selection).get("trial") is not None:
                raise ProvisionError("runtime_maintenance_pending")
            yield

    def identity_metadata(self, release, serial, *, renew=False):
        if renew:
            with self.renewal_lock():
                return self.create_identity_metadata(release, serial, renew=True)
        return self.create_identity_metadata(release, serial)

    def create_identity_metadata(self, release, serial, *, renew=False):
        if not self.tool(["version"]).startswith(b"OpenSSL 3."):
            raise ProvisionError("unsupported_identity_tool")
        data = self.root / "data"
        no_links(data)
        data.mkdir(mode=0o700, exist_ok=True)
        if data.stat().st_mode & 0o077:
            raise ProvisionError("insecure_identity_permissions")
        journal = data / "identity-provision.json"
        config_path = data / "companion.json"
        key_path, cert_path = data / "board.key", data / "board.crt"
        if not journal.exists():
            if any(data.iterdir()):
                raise ProvisionError("existing_identity_not_overwritten")
            board_id = str(uuid.uuid4())
            identity = {"format": 1, "serial": serial, "release": release, "board_id": board_id, "tls_name": "tc-" + board_id.replace("-", "") + ".local"}
            atomic(journal, identity)  # Save identity BEFORE key generation or any remotely usable configuration.
        else:
            identity = read_json(journal)
        if not isinstance(identity, dict) or set(identity) != {"format", "serial", "release", "board_id", "tls_name"} or identity["format"] != 1 or identity["serial"] != serial or identity["release"] != release or str(uuid.UUID(identity["board_id"])) != identity["board_id"] or identity["tls_name"] != "tc-" + identity["board_id"].replace("-", "") + ".local":
            raise ProvisionError("identity_journal_mismatch")
        expected = {"auth_mode": "tbp", "board_id": identity["board_id"], "tls_name": identity["tls_name"], "tls_cert": "data/board.crt", "tls_key": "data/board.key", "bind": "0.0.0.0", "port": 7443}
        configured = config_path.exists() or config_path.is_symlink()
        if configured and read_json(config_path) != expected:
            raise ProvisionError("existing_configuration_not_overwritten")
        if not key_path.exists():
            if configured or renew:
                raise ProvisionError("missing_identity_key")
            part = data / ".board-key.part"
            no_links(part)
            fd = os.open(part, os.O_CREAT | os.O_TRUNC | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
            os.close(fd)
            self.tool(["ecparam", "-name", "prime256v1", "-genkey", "-noout", "-out", str(part)])
            self.tool(["pkey", "-in", str(part), "-noout", "-check"])
            with part.open("rb") as handle:
                os.fsync(handle.fileno())
            os.replace(part, key_path)
            sync_directory(data)
        read_file(key_path, 8192)  # Validate private regular storage without returning its bytes.
        if key_path.stat().st_mode & 0o077:
            raise ProvisionError("insecure_identity_permissions")
        key_der = self.tool(["pkey", "-in", str(key_path), "-pubout", "-outform", "DER"])
        def certificate_key(path):
            public = self.tool(["x509", "-in", str(path), "-pubkey", "-noout"])
            return self.tool(["pkey", "-pubin", "-outform", "DER"], input=public)
        if cert_path.exists() or cert_path.is_symlink():
            read_file(cert_path, 8192)
            if certificate_key(cert_path) != key_der:
                raise ProvisionError("identity_key_mismatch")
            self.tool(["x509", "-in", str(cert_path), "-noout", "-checkhost", identity["tls_name"]])
        elif configured or renew:
            raise ProvisionError("missing_identity_certificate")
        if not cert_path.exists() or renew:
            config = data / ".certificate.conf"
            no_links(config)
            descriptor = os.open(config, os.O_CREAT | os.O_TRUNC | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
            with os.fdopen(descriptor, "w") as handle:
                handle.write("[req]\nprompt=no\ndistinguished_name=dn\nx509_extensions=v3\n[dn]\nCN=" + identity["tls_name"] + "\n[v3]\nsubjectAltName=DNS:" + identity["tls_name"] + "\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\n")
            part = data / ".board-cert.part"
            no_links(part)
            fd = os.open(part, os.O_CREAT | os.O_TRUNC | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
            os.close(fd)
            self.tool(["req", "-new", "-x509", "-sha256", "-key", str(key_path), "-out", str(part), "-days", "365", "-config", str(config)])
            if certificate_key(part) != key_der:
                raise ProvisionError("identity_key_mismatch")
            self.tool(["x509", "-in", str(part), "-noout", "-checkhost", identity["tls_name"], "-checkend", "0"])
            with part.open("rb") as handle:
                os.fsync(handle.fileno())
            os.replace(part, cert_path)
            config.unlink()
            sync_directory(data)
        self.tool(["x509", "-in", str(cert_path), "-noout", "-checkend", "0"])
        if not configured:
            atomic(config_path, expected)
        return {"phase": "identity_ready", "serial": serial, "release": release, "board_id": identity["board_id"], "tls_name": identity["tls_name"], "certificate_pem": self.tool(["x509", "-in", str(cert_path), "-outform", "PEM"]).decode(), "spki_sha256": digest(key_der), "started": False}

    def dispatch(self, body):
        fields = {"cancel": {"release"}, "identity": {"release"}, "renew_identity": {"release"}, "inspect": set(), "begin": {"release", "sha256", "size"}, "chunk": {"token", "offset", "data"}, "publish": {"token"}}
        operation = body.get("operation")
        if not isinstance(operation, str) or operation not in fields or set(body) != {"operation", "expectedSerial"} | fields[operation]:
            raise ProvisionError("invalid_request")
        identity = self.inspect()
        if identity != {"serial": body["expectedSerial"], "uid": 1000, "username": "arduino", "os": "debian", "version": "13"}:
            raise ProvisionError("unsupported_identity_or_permissions")
        if operation == "inspect":
            return dict(self.status(), serial=identity["serial"])
        if not self.apps.is_dir():
            raise ProvisionError("unsupported_storage")
        with self.locked():
            installed = self.installed()
            job = self.job()
            if operation == "cancel":
                if not job or job["release"] != hex_id(body["release"]):
                    raise ProvisionError("different_provisioning_pending")
                if not installed:
                    job["phase"] = "aborted"
                    atomic(self.directory / "job.json", job)
                return dict(self.status(), serial=identity["serial"])
            if operation in {"identity", "renew_identity"}:
                if not installed or installed["release"] != hex_id(body["release"]):
                    raise ProvisionError("different_or_missing_baseline")
                return self.identity_metadata(installed["release"], identity["serial"], renew=operation == "renew_identity")
            if operation == "begin":
                release = hex_id(body["release"])
                hex_id(body["sha256"])
                if type(body["size"]) is not int or not 0 < body["size"] <= MAX_PACKAGE:
                    raise ProvisionError("invalid_request")
                if installed:
                    if installed["release"] != release:
                        raise ProvisionError("baseline_upgrade_requires_official_handoff")
                    return dict(self.status(), serial=identity["serial"])
                if job and job["phase"] != "aborted":
                    if any(job[key] != body[key] for key in ("release", "sha256", "size")):
                        raise ProvisionError("different_provisioning_pending")
                else:
                    if shutil.disk_usage(self.apps).free < 3 * body["size"] + 2 * 1024 * 1024:
                        raise ProvisionError("insufficient_storage")
                    job = dict(phase="receiving", release=release, sha256=body["sha256"], size=body["size"], offset=0)
                    fd = os.open(self.directory / "incoming.json", os.O_CREAT | os.O_TRUNC | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
                    os.close(fd)
                token = secrets.token_urlsafe(32)
                job.update(token_hash=digest(token.encode()), boot=self.boot, expires=self.clock() + 600)
                atomic(self.directory / "job.json", job)
                return dict(self.status(), token=token, serial=identity["serial"])
            self.authorized(job, body["token"])
            if operation == "chunk":
                if job["phase"] != "receiving" or type(body["offset"]) is not int or body["offset"] != job["offset"] or not isinstance(body["data"], str) or len(body["data"]) > 10924:
                    raise ProvisionError("invalid_chunk")
                data = base64.b64decode(body["data"], validate=True)
                if not 0 < len(data) <= 8192 or job["offset"] + len(data) > job["size"]:
                    raise ProvisionError("invalid_chunk")
                fd = os.open(self.directory / "incoming.json", os.O_WRONLY | os.O_NOFOLLOW)
                with os.fdopen(fd, "r+b") as handle:
                    if os.fstat(handle.fileno()).st_size < job["offset"]:
                        raise ProvisionError("corrupt_provisioning_state")
                    handle.truncate(job["offset"])
                    handle.seek(job["offset"])
                    handle.write(data)
                    handle.flush()
                    os.fsync(handle.fileno())
                job["offset"] += len(data)
                atomic(self.directory / "job.json", job)
                return dict(self.status(), serial=identity["serial"])
            if installed:
                if installed["release"] != job["release"]:
                    raise ProvisionError("different_or_modified_baseline")
                return dict(self.status(), serial=identity["serial"])
            raw = read_file(self.directory / "incoming.json", MAX_PACKAGE)
            if job["offset"] != job["size"] or len(raw) != job["size"] or digest(raw) != job["sha256"]:
                raise ProvisionError("incomplete_or_corrupt_package")
            package = json.loads(raw, object_pairs_hook=unique)
            if set(package) != {"format", "release", "files"} or type(package["format"]) is not int or package["format"] != 1 or not isinstance(package["files"], dict) or set(package["files"]) != set(SOURCE_FILES):
                raise ProvisionError("unsupported_package")
            files = {}
            for name, encoded in package["files"].items():
                if not isinstance(encoded, str) or len(encoded) > 349528:
                    raise ProvisionError("unsupported_package")
                data = base64.b64decode(encoded, validate=True)
                if len(data) > 262144:
                    raise ProvisionError("unsupported_package")
                if name.endswith(".py"):
                    compile(data.decode(), name, "exec")
                files[name] = data
            if release_id(files) != job["release"] or package["release"] != job["release"]:
                raise ProvisionError("incomplete_or_corrupt_package")
            staging = self.directory / "source"
            no_links(staging)
            staging.mkdir(mode=0o700, exist_ok=True)
            for name, data in files.items():
                path = staging / name
                no_links(path)
                path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                fd = os.open(path, os.O_CREAT | os.O_TRUNC | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
                with os.fdopen(fd, "wb") as output:
                    output.write(data)
                    output.flush()
                    os.fsync(output.fileno())
            atomic(staging / ".tinkercrew-install.json", {"format": 1, "release": job["release"]})
            actual = {str(path.relative_to(staging)) for path in staging.rglob("*") if not path.is_dir() or path.is_symlink()}
            if actual != set(SOURCE_FILES) | {".tinkercrew-install.json"}:
                raise ProvisionError("unexpected_staging_files")
            for path in sorted((path for path in staging.rglob("*") if path.is_dir()), key=lambda path: len(path.parts), reverse=True):
                sync_directory(path)
            sync_directory(staging)
            job["phase"] = "publishing"
            atomic(self.directory / "job.json", job)
            if self.root.exists() or self.root.is_symlink():
                raise ProvisionError("existing_app_not_overwritten")
            os.rename(staging, self.root)
            sync_directory(self.apps)
            job["phase"] = "published"
            atomic(self.directory / "job.json", job)
            return dict(self.status(), serial=identity["serial"])


def main():
    try:
        raw = sys.stdin.buffer.read(16385)
        if len(raw) > 16384:
            raise ProvisionError("invalid_request")
        body = json.loads(raw, object_pairs_hook=unique)
        if not isinstance(body, dict):
            raise ProvisionError("invalid_request")
        result = CompanionProvisioning("/home/arduino/ArduinoApps").dispatch(body)
        print(json.dumps({"status": "ok", "result": result}))
    except Exception:
        print(json.dumps({"status": "error", "error": "Unsupported or interrupted provisioning. Reinspect the exact Board and preserve existing files; no start or firmware operation was performed."}))
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
