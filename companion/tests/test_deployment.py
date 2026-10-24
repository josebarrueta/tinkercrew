"""Deployment interface tests: real private files/processes, never a Board or Arduino CLI."""
import base64
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest
import subprocess
import sys
import time

from tinkercrew_companion.deployment import DeploymentError, RuntimeDeployment, build_bundle


class DeploymentTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / "app"
        source = Path(__file__).resolve().parents[1]
        shutil.copytree(source / "python", self.root / "python", ignore=shutil.ignore_patterns("__pycache__"))
        shutil.copytree(source / "sketch", self.root / "sketch")
        shutil.copy(source / "app.yaml", self.root / "app.yaml")
        (self.root / "data").mkdir(mode=0o700)
        (self.root / "data/credentials-sentinel").write_bytes(b"keep-existing-identity-and-secrets")
        self.candidate = Path(self.temp.name) / "candidate"
        shutil.copytree(self.root, self.candidate)
        server = self.candidate / "python/tinkercrew_companion/server.py"
        server.write_text(server.read_text() + "\n# changed runtime, not firmware or dependencies\n")
        self.bundle = build_bundle(self.candidate)
        self.store = RuntimeDeployment(self.root, boot_id="test-boot", clock=lambda: 1000)

    def tearDown(self):
        self.temp.cleanup()

    def upload(self, store, bundle):
        begun = store.dispatch("begin", {"release": bundle["release"], "sha256": hashlib.sha256(bundle["bytes"]).hexdigest(), "size": len(bundle["bytes"]), "contract": bundle["contract"]})
        for offset in range(begun["offset"], len(bundle["bytes"]), 8192):
            store.dispatch("chunk", {"token": begun["token"], "offset": offset, "data": base64.b64encode(bundle["bytes"][offset:offset + 8192]).decode()})
        store.dispatch("stage", {"token": begun["token"]})
        return begun["token"]

    def wait_for(self, predicate, *, timeout=5):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if predicate():
                return
            time.sleep(0.05)
        self.fail("runtime did not reach the expected state")

    def test_crashing_trial_rolls_back_in_a_real_supervisor_without_a_mac_connection(self):
        baseline = self.root / "python/tinkercrew_companion/server.py"
        baseline.write_text('import time\nwhile True: time.sleep(0.05)\n')
        shutil.copy(baseline, self.candidate / "python/tinkercrew_companion/server.py")
        (self.candidate / "python/tinkercrew_companion/server.py").write_text('raise RuntimeError("fixture trial rejected")\n')
        store = RuntimeDeployment(self.root)
        bundle = build_bundle(self.candidate)
        process = subprocess.Popen([sys.executable, "-m", "tinkercrew_companion.supervisor", str(self.root)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            self.wait_for(lambda: (self.root / "data/runtime/observed.json").exists())
            token = self.upload(store, bundle)
            self.assertEqual(store.dispatch("commit", {"token": token})["phase"], "awaiting_health")
            self.wait_for(lambda: store.dispatch("status", {})["phase"] == "rolled_back")
            status = store.dispatch("status", {})
            self.assertEqual(status["reason"], "child_exited")
            self.assertEqual(status["current"], build_bundle(self.root)["release"])
            self.assertEqual((self.root / "data/credentials-sentinel").read_bytes(), b"keep-existing-identity-and-secrets")
        finally:
            process.terminate()
            process.wait(timeout=4)

    def test_missing_health_ack_times_out_and_stale_ack_cannot_accept_a_trial(self):
        baseline = self.root / "python/tinkercrew_companion/server.py"
        baseline.write_text('import time\nwhile True: time.sleep(0.05)\n')
        (self.candidate / "python/tinkercrew_companion/server.py").write_text(baseline.read_text() + '# candidate\n')
        store = RuntimeDeployment(self.root, trial_seconds=0.4)
        bundle = build_bundle(self.candidate)
        process = subprocess.Popen([sys.executable, "-m", "tinkercrew_companion.supervisor", str(self.root)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            self.wait_for(lambda: (self.root / "data/runtime/observed.json").exists())
            token = self.upload(store, bundle)
            store.dispatch("commit", {"token": token})
            with self.assertRaises(Exception):
                store.dispatch("finish", {"token": token, "nonce": "stale-runtime-observation"})
            self.wait_for(lambda: store.dispatch("status", {})["phase"] == "rolled_back")
            self.assertEqual(store.dispatch("status", {})["reason"], "health_timeout")
        finally:
            process.terminate()
            process.wait(timeout=4)

    def test_current_trial_nonce_accepts_only_the_running_candidate_and_survives_restart(self):
        baseline = self.root / "python/tinkercrew_companion/server.py"
        baseline.write_text('import time\nwhile True: time.sleep(0.05)\n')
        (self.candidate / "python/tinkercrew_companion/server.py").write_text(baseline.read_text() + '# candidate\n')
        store = RuntimeDeployment(self.root)
        bundle = build_bundle(self.candidate)
        process = subprocess.Popen([sys.executable, "-m", "tinkercrew_companion.supervisor", str(self.root)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            self.wait_for(lambda: (self.root / "data/runtime/observed.json").exists())
            token = self.upload(store, bundle)
            store.dispatch("commit", {"token": token})
            self.wait_for(lambda: store.dispatch("status", {})["observed"]["release"] == bundle["release"])
            observation = store.dispatch("status", {})["observed"]
            self.assertEqual(store.dispatch("finish", {"token": token, "nonce": observation["nonce"]})["phase"], "committed")
            self.assertEqual(RuntimeDeployment(self.root).dispatch("status", {})["current"], bundle["release"])
            repeated = store.dispatch("begin", {"release": bundle["release"], "sha256": hashlib.sha256(bundle["bytes"]).hexdigest(), "size": len(bundle["bytes"]), "contract": bundle["contract"]})
            self.assertEqual(repeated["phase"], "current")
            self.assertNotIn("token", repeated)
        finally:
            process.terminate()
            process.wait(timeout=4)

    def test_real_tls_health_binds_the_candidate_nonce_before_commit(self):
        import datetime
        import http.client
        import os
        import socket
        import ssl
        from cryptography import x509
        from cryptography.hazmat.primitives import hashes, serialization
        from cryptography.hazmat.primitives.asymmetric import ed25519, rsa
        from cryptography.x509.oid import NameOID
        from tinkercrew_companion.auth import PairingRegistry
        board_id = "98f6411f-20ec-438a-a1de-6ce20fb0370e"
        installation = "1f33fd77-7015-41c6-a882-650f37a06c18"
        key = ed25519.Ed25519PrivateKey.generate()
        ssh = ed25519.Ed25519PrivateKey.generate()
        registry = PairingRegistry(self.root / "data", board_id, ssh_directory=self.root / ".ssh")
        registration = {"installation_id": installation, "label": "Runtime test", "public_key": base64.urlsafe_b64encode(key.public_key().public_bytes_raw()).decode().rstrip("="), "ssh_public_key": ssh.public_key().public_bytes(serialization.Encoding.OpenSSH, serialization.PublicFormat.OpenSSH).decode()}
        bootstrap = registry.begin_pairing(registration)
        registry.complete_pairing(dict(registration, pairing_token=bootstrap["pairing_token"]))
        registry.activate_pairing(installation)
        cert_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
        name = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "localhost")])
        now = datetime.datetime.now(datetime.timezone.utc)
        cert = x509.CertificateBuilder().subject_name(name).issuer_name(name).public_key(cert_key.public_key()).serial_number(x509.random_serial_number()).not_valid_before(now - datetime.timedelta(minutes=1)).not_valid_after(now + datetime.timedelta(days=1)).add_extension(x509.SubjectAlternativeName([x509.DNSName("localhost")]), critical=False).sign(cert_key, hashes.SHA256())
        certificate = self.root / "data/board.crt"
        private = self.root / "data/board.key"
        certificate.write_bytes(cert.public_bytes(serialization.Encoding.PEM))
        private.write_bytes(cert_key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption()))
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        (self.root / "data/companion.json").write_text(json.dumps({"auth_mode": "tbp", "board_id": board_id, "tls_name": "localhost", "tls_cert": "data/board.crt", "tls_key": "data/board.key", "bind": "127.0.0.1", "port": port}))
        initializer = self.candidate / "python/tinkercrew_companion/__init__.py"
        initializer.write_text(initializer.read_text() + '\nimport faulthandler\n_fixture_stack = open("data/fixture-stack.txt", "w")\nfaulthandler.dump_traceback_later(4, repeat=True, file=_fixture_stack)\n')
        self.bundle = build_bundle(self.candidate)
        environment = {key: value for key, value in os.environ.items() if not key.startswith("TC_")}
        environment["TC_SIMULATE_HARDWARE"] = "1"
        process = subprocess.Popen([sys.executable, "-m", "tinkercrew_companion.supervisor", str(self.root)], env=environment, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        context = ssl.create_default_context(cafile=str(certificate))
        diagnostic = {"route": "not_requested", "http": None, "exception": None}
        def request(path, body=None, token=None):
            diagnostic.update(route=path, http=None, exception=None)
            # Match native pinned routing: fixed IPv4 TCP peer, verified localhost TLS
            # identity. Do not depend on the runner's localhost IPv6/DNS policy.
            peer = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
            try:
                diagnostic["step"] = "tcp_connect"
                stream = socket.create_connection(("127.0.0.1", port), timeout=3)
                diagnostic["step"] = "tls_handshake"
                try:
                    peer.sock = context.wrap_socket(stream, server_hostname="localhost")
                except Exception:
                    stream.close()
                    raise
                diagnostic["step"] = "http_response"
                headers = {"Authorization": "Bearer " + token} if token else {}
                peer.request("POST" if body is not None else "GET", path, json.dumps(body) if body is not None else None, headers)
                response = peer.getresponse()
                diagnostic["http"] = response.status
                result = json.load(response)
                return result if response.status == 200 else {}
            finally:
                peer.close()
        hello = {}
        def authenticated_health():
            nonlocal hello
            try:
                challenge = request("/v1/sessions/challenge", {"board_id": board_id, "installation_id": installation})
                message = f'TBP1\n{board_id}\n{installation}\n{challenge["challenge_id"]}\n{challenge["nonce"]}\n{challenge["expires_at"]}\n'.encode()
                session = request("/v1/sessions", {"board_id": board_id, "installation_id": installation, "challenge_id": challenge["challenge_id"], "signature": base64.urlsafe_b64encode(key.sign(message)).decode().rstrip("=")})
                hello = request("/v1/hello", token=session["session_token"])
                return hello["runtime"]["release"] == self.bundle["release"]
            except (OSError, KeyError, http.client.HTTPException) as error:
                diagnostic["exception"] = type(error).__name__
                return False
        try:
            store = RuntimeDeployment(self.root)
            self.wait_for(lambda: (self.root / "data/runtime/observed.json").exists())
            token = self.upload(store, self.bundle)
            store.dispatch("commit", {"token": token})
            # Cold CI interpreters compile uncached modules before serving TLS. This is
            # a test-observation budget, not an extension of the 45-second trial.
            self.wait_for(lambda: (store.dispatch("status", {}).get("observed") or {}).get("release") == self.bundle["release"], timeout=15)
            try:
                self.wait_for(authenticated_health, timeout=20)
            except AssertionError:
                status = store.dispatch("status", {})
                observed = status.get("observed") or {}
                stack = self.root / "data/fixture-stack.txt"
                print(stack.read_text()[-8192:] if stack.exists() else "fixture stack unavailable")
                self.fail(f'authenticated trial health unavailable: phase={status["phase"]}, reason={status.get("reason", "none")}, child={observed.get("state", "unobserved")}, request={diagnostic}')
            self.assertTrue(hello["health"]["safe_state"])
            self.assertEqual(hello["board"]["board_id"], board_id)
            self.assertEqual(store.dispatch("finish", {"token": token, "nonce": hello["runtime"]["nonce"]})["phase"], "committed")
            self.assertEqual((self.root / "data/board.crt").read_bytes(), cert.public_bytes(serialization.Encoding.PEM))
        finally:
            process.terminate()
            process.wait(timeout=4)

    def test_changed_firmware_contract_and_wrong_payload_hash_fail_before_promotion(self):
        original = self.store.dispatch("status", {})["current"]
        (self.candidate / "sketch/sketch.ino").write_text("// firmware change is not a Linux runtime update\n")
        incompatible = build_bundle(self.candidate)
        with self.assertRaises(DeploymentError) as failure:
            self.store.dispatch("begin", {"release": incompatible["release"], "sha256": hashlib.sha256(incompatible["bytes"]).hexdigest(), "size": len(incompatible["bytes"]), "contract": incompatible["contract"]})
        self.assertEqual(failure.exception.code, "unsupported_runtime")
        begun = self.store.dispatch("begin", {"release": self.bundle["release"], "sha256": "0" * 64, "size": len(self.bundle["bytes"]), "contract": self.bundle["contract"]})
        for offset in range(0, len(self.bundle["bytes"]), 8192):
            self.store.dispatch("chunk", {"token": begun["token"], "offset": offset, "data": base64.b64encode(self.bundle["bytes"][offset:offset + 8192]).decode()})
        with self.assertRaises(DeploymentError) as corrupted:
            self.store.dispatch("stage", {"token": begun["token"]})
        self.assertEqual(corrupted.exception.code, "bundle_hash_mismatch")
        self.assertEqual(self.store.dispatch("status", {})["current"], original)
        self.assertEqual(self.store.dispatch("cancel", {"release": self.bundle["release"]})["phase"], "aborted")

    def test_reboot_before_health_ack_restores_the_previous_runtime(self):
        # A harmless baseline child; no hardware imports are executed.
        baseline = self.root / "python/tinkercrew_companion/server.py"
        baseline.write_text('import time\nwhile True: time.sleep(0.05)\n')
        (self.candidate / "python/tinkercrew_companion/server.py").write_text(baseline.read_text() + '# candidate\n')
        store = RuntimeDeployment(self.root)
        process = subprocess.Popen([sys.executable, "-m", "tinkercrew_companion.supervisor", str(self.root)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            self.wait_for(lambda: (self.root / "data/runtime/observed.json").exists())
            bundle = build_bundle(self.candidate)
            token = self.upload(store, bundle)
            store.dispatch("commit", {"token": token})
            process.terminate()
            process.wait(timeout=4)
            # A different kernel boot is an external clock/boot seam, not a real Board reboot.
            restarted = RuntimeDeployment(self.root, boot_id="another-kernel-boot")
            restarted.supervise()
            self.assertEqual(restarted.dispatch("status", {})["phase"], "rolled_back")
            self.assertEqual(restarted.dispatch("status", {})["reason"], "board_rebooted")
        finally:
            process.terminate()
            process.wait(timeout=4)

    def test_interrupted_second_publication_keeps_the_last_good_not_an_older_release(self):
        from unittest.mock import patch
        import os
        baseline = self.root / "python/tinkercrew_companion/server.py"
        baseline.write_text('import time\nwhile True: time.sleep(0.05)\n')
        candidate = self.candidate / "python/tinkercrew_companion/server.py"
        candidate.write_text(baseline.read_text() + '# first accepted update\n')
        store = RuntimeDeployment(self.root)
        process = subprocess.Popen([sys.executable, "-m", "tinkercrew_companion.supervisor", str(self.root)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            self.wait_for(lambda: (self.root / "data/runtime/observed.json").exists())
            accepted = build_bundle(self.candidate)
            token = self.upload(store, accepted)
            store.dispatch("commit", {"token": token})
            self.wait_for(lambda: store.dispatch("status", {})["observed"]["release"] == accepted["release"])
            store.dispatch("finish", {"token": token, "nonce": store.dispatch("status", {})["observed"]["nonce"]})
            candidate.write_text(candidate.read_text() + '# next trial\n')
            next_bundle = build_bundle(self.candidate)
            next_token = self.upload(store, next_bundle)
            replace = os.replace
            def lose_publication(source, destination):
                if Path(destination).name == "selection.json":
                    raise OSError("fixture interruption before atomic publication")
                return replace(source, destination)
            with patch("os.replace", lose_publication):
                with self.assertRaises(OSError):
                    store.dispatch("commit", {"token": next_token})
            self.wait_for(lambda: store.dispatch("status", {})["phase"] == "rolled_back")
            self.assertEqual(store.dispatch("status", {})["current"], accepted["release"])
        finally:
            process.terminate()
            process.wait(timeout=4)

    def test_committed_runtime_that_later_exits_restores_a_validated_previous_release(self):
        baseline = self.root / "python/tinkercrew_companion/server.py"
        baseline.write_text('import time\nwhile True: time.sleep(0.05)\n')
        (self.candidate / "python/tinkercrew_companion/server.py").write_text('from pathlib import Path\nimport time\nwhile not Path("data/fixture-exit").exists(): time.sleep(0.05)\n')
        store = RuntimeDeployment(self.root)
        bundle = build_bundle(self.candidate)
        process = subprocess.Popen([sys.executable, "-m", "tinkercrew_companion.supervisor", str(self.root)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            self.wait_for(lambda: (self.root / "data/runtime/observed.json").exists())
            token = self.upload(store, bundle)
            store.dispatch("commit", {"token": token})
            self.wait_for(lambda: store.dispatch("status", {})["observed"]["release"] == bundle["release"])
            store.dispatch("finish", {"token": token, "nonce": store.dispatch("status", {})["observed"]["nonce"]})
            (self.root / "data/fixture-exit").write_text("exit only the test child")
            self.wait_for(lambda: store.dispatch("status", {})["phase"] == "rolled_back")
            self.assertEqual(store.dispatch("status", {})["current"], build_bundle(self.root)["release"])
        finally:
            process.terminate()
            process.wait(timeout=4)

    def test_corrupt_durable_job_is_rejected_without_silently_resetting_it(self):
        path = self.root / "data/runtime/job.json"
        for raw in (b"null", b"{}", b'{"phase":"unknown"}'):
            with self.subTest(raw=raw):
                path.write_bytes(raw)
                with self.assertRaises(DeploymentError):
                    self.store.dispatch("status", {})
                self.assertEqual(path.read_bytes(), raw)

    def test_transfer_resumes_and_corruption_never_changes_the_active_runtime_or_credentials(self):
        begun = self.store.dispatch("begin", {"release": self.bundle["release"], "sha256": hashlib.sha256(self.bundle["bytes"]).hexdigest(), "size": len(self.bundle["bytes"]), "contract": self.bundle["contract"]})
        self.store.dispatch("chunk", {"token": begun["token"], "offset": 0, "data": base64.b64encode(self.bundle["bytes"][:4096]).decode()})
        resumed = RuntimeDeployment(self.root, boot_id="test-boot", clock=lambda: 1000)
        self.assertEqual(resumed.dispatch("status", {})["offset"], 4096)
        with self.assertRaises(Exception):
            resumed.dispatch("chunk", {"token": begun["token"], "offset": 4097, "data": "eA=="})
        for offset in range(4096, len(self.bundle["bytes"]), 8192):
            resumed.dispatch("chunk", {"token": begun["token"], "offset": offset, "data": base64.b64encode(self.bundle["bytes"][offset:offset + 8192]).decode()})
        self.assertEqual(resumed.dispatch("stage", {"token": begun["token"]})["phase"], "staged")
        self.assertEqual((self.root / "data/credentials-sentinel").read_bytes(), b"keep-existing-identity-and-secrets")
        self.assertEqual(resumed.dispatch("status", {})["current"], build_bundle(self.root)["release"])
