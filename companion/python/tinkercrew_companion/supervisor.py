"""Own exactly one Companion child; promote/rollback code without touching Arduino firmware."""
import fcntl
import os
from pathlib import Path
import secrets
import signal
import subprocess
import sys
import threading
import time

from .deployment import DeploymentError, RuntimeDeployment, _atomic, _file, _digest


def run(root):
    root = Path(root).resolve()
    store = RuntimeDeployment(root)
    fd = os.open(store.directory / "supervisor.lock", os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    stopped = threading.Event()
    child = None
    current = None
    current_certificate = None
    nonce = None
    failed = None
    for number in (signal.SIGTERM, signal.SIGINT):
        signal.signal(number, lambda *_: stopped.set())

    def terminate():
        nonlocal child
        if child is not None:
            if child.poll() is None:
                child.terminate()  # Only this owned Popen handle; never a PID read from a file.
                try:
                    child.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait(timeout=2)
            child = None

    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        while not stopped.is_set():
            if child is not None and child.poll() is not None:
                failed = current
                child = None
            try:
                spec = store.supervise(failed_release=failed)
            except (DeploymentError, OSError, ValueError, TypeError, KeyError) as error:
                if isinstance(error, DeploymentError) and error.code == "deployment_busy":
                    stopped.wait(0.05)
                    continue
                terminate()
                # Integrity failure is not an excuse to execute an unverified fallback.
                _atomic(store.directory / "observed.json", {"boot": store.boot, "seen": time.monotonic(), "state": "blocked", "release": current, "nonce": nonce, "trial": None})
                stopped.wait(0.1)
                continue
            try:
                certificate = _digest(_file(root / "data/board.crt", 8192))
            except (DeploymentError, OSError):
                certificate = None
            if spec["release"] != current or certificate != current_certificate:
                terminate()
                current = spec["release"]
                current_certificate = certificate
                nonce = secrets.token_urlsafe(16)
                failed = None
            if child is None and failed != current:
                environment = os.environ.copy()
                environment["PYTHONPATH"] = spec["python_path"] + os.pathsep + environment.get("PYTHONPATH", "")
                environment["TC_RUNTIME_RELEASE"] = current
                environment["TC_RUNTIME_NONCE"] = nonce
                environment["TC_RUNTIME_STATE"] = str(store.directory)
                environment["TC_RUNTIME_CERTIFICATE_SHA256"] = current_certificate or ""
                child = subprocess.Popen([sys.executable, "-B", "-X", "pycache_prefix=/dev/null", "-m", "tinkercrew_companion.server"], cwd=root, env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            _atomic(store.directory / "observed.json", {"boot": store.boot, "seen": time.monotonic(), "state": "launched" if child is not None else "child_exited", "release": current, "nonce": nonce, "trial": spec["trial"], "certificate_sha256": current_certificate})
            stopped.wait(0.1)
    finally:
        terminate()
        os.close(fd)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("A local app directory is required")
    run(Path(sys.argv[1]))
