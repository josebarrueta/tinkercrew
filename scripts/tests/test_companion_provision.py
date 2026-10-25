"""Fixed initial-baseline/identity boundary. Temporary app trees; no Board or live credentials."""
import base64
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("companion_provision", ROOT / "app/src-tauri/src/companion_provision.py")
provision = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provision)


class ProvisionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.apps = Path(self.temporary.name).resolve() / "apps"
        self.apps.mkdir()
        self.store = provision.CompanionProvisioning(self.apps, inspect=lambda: {"serial": "42", "uid": 1000, "username": "arduino", "os": "debian", "version": "13"}, openssl=shutil.which("openssl"))
        self.bundle = provision.build_package(ROOT / "companion")

    def request(self, operation, **body):
        return self.store.dispatch(dict(operation=operation, expectedSerial="42", **body))

    def transfer(self):
        job = self.request("begin", release=self.bundle["release"], sha256=hashlib.sha256(self.bundle["bytes"]).hexdigest(), size=len(self.bundle["bytes"]))
        for offset in range(job["offset"], len(self.bundle["bytes"]), 8192):
            self.request("chunk", token=job["token"], offset=offset, data=base64.b64encode(self.bundle["bytes"][offset:offset + 8192]).decode())
        return job["token"]

    def test_interrupted_publication_retries_and_existing_credentials_are_not_migrated(self):
        from unittest.mock import patch
        token = self.transfer()
        with patch("os.rename", side_effect=OSError("fixture interrupted publication")):
            with self.assertRaises(OSError):
                self.request("publish", token=token)
        self.assertFalse((self.apps / "tinkercrew-companion").exists())
        resumed = self.transfer()
        self.assertEqual(self.request("publish", token=resumed)["phase"], "awaiting_identity")
        data = self.apps / "tinkercrew-companion/data"
        data.mkdir(mode=0o700)
        legacy = b'{"api_token":"fixture-only-not-a-real-token"}'
        (data / "companion.json").write_bytes(legacy)
        with self.assertRaises(provision.ProvisionError):
            self.request("identity", release=self.bundle["release"])
        self.assertEqual((data / "companion.json").read_bytes(), legacy)
        self.assertFalse((data / "board.key").exists())

    def test_cancel_invalidates_transfer_authority_and_never_deletes_published_files(self):
        old = self.transfer()
        self.assertEqual(self.request("cancel", release=self.bundle["release"])["phase"], "aborted")
        with self.assertRaises(provision.ProvisionError):
            self.request("publish", token=old)
        token = self.transfer()
        self.request("publish", token=token)
        self.assertEqual(self.request("cancel", release=self.bundle["release"])["phase"], "awaiting_identity")
        self.assertTrue((self.apps / "tinkercrew-companion/python/main.py").is_file())

    def test_symlinked_app_root_or_ancestor_is_not_resolved_into_another_target(self):
        ancestor = self.apps.parent / "ancestor-alias"
        ancestor.symlink_to(self.apps.parent, target_is_directory=True)
        direct = self.apps.parent / "app-alias"
        direct.symlink_to(self.apps, target_is_directory=True)
        for path in (ancestor / "apps", direct):
            with self.assertRaises(provision.ProvisionError):
                provision.CompanionProvisioning(path)
        self.assertEqual(list(self.apps.iterdir()), [])

    def test_existing_nonprivate_data_directory_is_not_used_for_identity(self):
        token = self.transfer()
        self.request("publish", token=token)
        data = self.apps / "tinkercrew-companion/data"
        data.mkdir(mode=0o755)
        data.chmod(0o755)
        with self.assertRaises(provision.ProvisionError):
            self.request("identity", release=self.bundle["release"])
        self.assertEqual(list(data.iterdir()), [])

    def test_wrong_hardware_permissions_and_unknown_operations_fail_before_writes(self):
        for operation in ("start", "shell", "flash", "reboot"):
            with self.assertRaises(provision.ProvisionError):
                self.request(operation)
        with self.assertRaises(provision.ProvisionError):
            self.store.dispatch({"operation": "inspect", "expectedSerial": "43"})
        self.store = provision.CompanionProvisioning(self.apps, inspect=lambda: {"serial": "42", "uid": 0, "username": "root", "os": "debian", "version": "13"})
        with self.assertRaises(provision.ProvisionError):
            self.request("identity", release=self.bundle["release"])
        self.assertEqual(list(self.apps.iterdir()), [])

    def test_renewal_refuses_pending_runtime_work_without_changing_the_certificate(self):
        token = self.transfer()
        self.request("publish", token=token)
        before = self.request("identity", release=self.bundle["release"])
        runtime = self.apps / "tinkercrew-companion/data/runtime"
        runtime.mkdir(mode=0o700)
        (runtime / "job.json").write_text(json.dumps({"phase": "awaiting_health"}))
        with self.assertRaises(provision.ProvisionError):
            self.request("renew_identity", release=self.bundle["release"])
        self.assertEqual((self.apps / "tinkercrew-companion/data/board.crt").read_text(), before["certificate_pem"])

    def test_identity_is_reused_and_renewal_preserves_its_key_without_exporting_secrets(self):
        from cryptography import x509
        from cryptography.hazmat.primitives import serialization
        import ssl
        token = self.transfer()
        self.request("publish", token=token)
        first = self.request("identity", release=self.bundle["release"])
        repeated = self.request("identity", release=self.bundle["release"])
        self.assertEqual(first, repeated)
        self.assertEqual(first["phase"], "identity_ready")
        self.assertEqual(set(first), {"phase", "serial", "release", "board_id", "tls_name", "certificate_pem", "spki_sha256", "started"})
        data = self.apps / "tinkercrew-companion/data"
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(data / "board.crt", data / "board.key")
        before = (data / "board.key").read_bytes()
        renewed = self.request("renew_identity", release=self.bundle["release"])
        self.assertEqual(renewed["board_id"], first["board_id"])
        self.assertEqual(renewed["tls_name"], first["tls_name"])
        self.assertNotEqual(renewed["certificate_pem"], first["certificate_pem"])
        self.assertEqual((data / "board.key").read_bytes(), before)
        certificate = x509.load_pem_x509_certificate(renewed["certificate_pem"].encode())
        actual = hashlib.sha256(certificate.public_key().public_bytes(serialization.Encoding.DER, serialization.PublicFormat.SubjectPublicKeyInfo)).hexdigest()
        self.assertEqual(actual, first["spki_sha256"])
        self.assertEqual(renewed["spki_sha256"], actual)
        self.assertEqual(json.loads((data / "companion.json").read_text())["auth_mode"], "tbp")
        self.assertEqual((data / "board.key").stat().st_mode & 0o777, 0o600)

    def test_initial_publication_is_verified_resumable_and_never_starts_or_overwrites_an_app(self):
        self.assertEqual(self.request("inspect")["phase"], "absent")
        job = self.request("begin", release=self.bundle["release"], sha256=hashlib.sha256(self.bundle["bytes"]).hexdigest(), size=len(self.bundle["bytes"]))
        raw = self.bundle["bytes"][:8192]
        self.request("chunk", token=job["token"], offset=0, data=base64.b64encode(raw).decode())
        self.store = provision.CompanionProvisioning(self.apps, inspect=self.store.inspect, openssl=shutil.which("openssl"))
        token = self.transfer()
        self.assertFalse((self.apps / "tinkercrew-companion").exists())
        result = self.request("publish", token=token)
        self.assertEqual(result["phase"], "awaiting_identity")
        self.assertEqual(result["release"], self.bundle["release"])
        self.assertFalse(result["started"])
        self.assertEqual(self.request("publish", token=token)["phase"], "awaiting_identity")
        (self.apps / "tinkercrew-companion/python/main.py").write_text("# unrelated or modified source")
        with self.assertRaises(provision.ProvisionError):
            self.request("begin", release=self.bundle["release"], sha256="a" * 64, size=100)
        self.assertEqual((self.apps / "tinkercrew-companion/python/main.py").read_text(), "# unrelated or modified source")
