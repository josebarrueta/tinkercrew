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

    def old_owned_preparation(self):
        original = self.bundle
        package = json.loads(original["bytes"])
        files = {name: base64.b64decode(value) for name, value in package["files"].items()}
        # Freeze the historic correction target, not the evolving runtime bundle.
        files['python/tinkercrew_companion/server.py'] = (Path(__file__).parent/'fixtures/companion-pre-usb-server.py').read_bytes()
        corrected = dict(package, release=provision.release_id(files), files={name:base64.b64encode(value).decode() for name,value in files.items()})
        original = {'release':corrected['release'],'bytes':json.dumps(corrected,sort_keys=True,separators=(',',':')).encode()}
        files["sketch/sketch.yaml"] = b"profiles:\n  default:\n    platforms:\n      - platform: arduino:zephyr\n    libraries:\n      - Arduino_Modulino\n      - Arduino_RouterBridge\ndefault_profile: default\n"
        release = provision.release_id(files)
        self.assertEqual(release, "63e90d374bc27c69b2fd810f851fed9a51d00abaaa82ce6369ba4b7cac7feef3")
        package.update(release=release, files={name: base64.b64encode(value).decode() for name, value in files.items()})
        self.bundle = {"release": release, "bytes": json.dumps(package, sort_keys=True, separators=(",", ":")).encode()}
        token = self.transfer()
        self.request("publish", token=token)
        identity = self.request("identity", release=release)
        self.bundle = original
        self.store.app_state = lambda: "uninitialized"
        return identity

    def test_owned_unstarted_profile_correction_preserves_identity_and_can_restore_exact_old_source(self):
        self.store.app_state = lambda: "uninitialized"  # External official CLI boundary only.
        identity = self.old_owned_preparation()
        root = self.apps / "tinkercrew-companion"
        before = {name: (root / "data" / name).read_bytes() for name in ("board.key", "board.crt", "companion.json")}
        unrelated = self.apps / "unrelated-app"
        unrelated.mkdir()
        (unrelated / "keep").write_text("untouched")
        result = self.request("correct_prepared_profile", release=self.bundle["release"])
        self.assertEqual(result["correctionPhase"], "corrected")
        self.assertEqual(result["release"], self.bundle["release"])
        self.assertFalse(result["started"])
        after = self.request("identity", release=self.bundle["release"])
        for field in ("board_id", "tls_name", "spki_sha256", "certificate_pem"):
            self.assertEqual(after[field], identity[field])
        for name, value in before.items():
            self.assertEqual((root / "data" / name).read_bytes(), value)
        self.assertEqual(self.request("correct_prepared_profile", release=self.bundle["release"])["correctionPhase"], "corrected")
        restored = self.request("restore_prepared_profile", release=self.bundle["release"])
        self.assertEqual(restored["correctionPhase"], "restored")
        self.assertEqual(restored["release"], identity["release"])
        self.assertEqual(self.request("identity", release=identity["release"]), identity)
        self.assertEqual((unrelated / "keep").read_text(), "untouched")

    def test_profile_correction_recovers_before_and_after_atomic_exchange_without_rekeying(self):
        identity = self.old_owned_preparation()
        native_exchange = self.store.exchange
        def interrupted_before(first, second):
            raise OSError("fixture filesystem interruption before exchange")
        self.store.exchange = interrupted_before
        with self.assertRaises(OSError):
            self.request("correct_prepared_profile", release=self.bundle["release"])
        self.assertEqual(self.request("inspect")["release"], identity["release"])
        def interrupted_after(first, second):
            native_exchange(first, second)
            raise OSError("fixture filesystem interruption after exchange")
        self.store.exchange = interrupted_after
        with self.assertRaises(OSError):
            self.request("correct_prepared_profile", release=self.bundle["release"])
        self.assertEqual(self.request("inspect")["release"], self.bundle["release"])
        with self.assertRaises(provision.ProvisionError):
            self.request("identity", release=self.bundle["release"])
        self.store.exchange = native_exchange
        self.assertEqual(self.request("correct_prepared_profile", release=self.bundle["release"])["correctionPhase"], "corrected")
        self.assertEqual(self.request("identity", release=self.bundle["release"])["spki_sha256"], identity["spki_sha256"])
        self.store.exchange = interrupted_after
        with self.assertRaises(OSError):
            self.request("restore_prepared_profile", release=self.bundle["release"])
        self.store.exchange = native_exchange
        self.assertEqual(self.request("restore_prepared_profile", release=self.bundle["release"])["correctionPhase"], "restored")
        self.assertEqual(self.request("identity", release=identity["release"]), identity)

    def test_profile_correction_resumes_only_its_known_partial_staging(self):
        from unittest.mock import patch
        identity = self.old_owned_preparation()
        original_open = provision.os.open
        def interrupted(path, *args, **kwargs):
            if str(path).endswith("prepared-profile-backup/python/main.py"):
                raise OSError("fixture interrupted staging write")
            return original_open(path, *args, **kwargs)
        with patch("os.open", side_effect=interrupted):
            with self.assertRaises(OSError):
                self.request("correct_prepared_profile", release=self.bundle["release"])
        self.assertEqual(self.request("inspect")["release"], identity["release"])
        stage = self.apps / ".tinkercrew-provision/prepared-profile-backup"
        unknown = stage / "unowned"
        unknown.write_text("preserve unknown content")
        with self.assertRaises(provision.ProvisionError):
            self.request("correct_prepared_profile", release=self.bundle["release"])
        self.assertEqual(unknown.read_text(), "preserve unknown content")
        unknown.unlink()  # Test-owned fixture cleanup only; production never removes unknown files.
        self.assertEqual(self.request("correct_prepared_profile", release=self.bundle["release"])["correctionPhase"], "corrected")

    def test_restore_after_partial_staging_leaves_original_root_in_place(self):
        from unittest.mock import patch
        identity = self.old_owned_preparation()
        original_open = provision.os.open
        def interrupted(path, *args, **kwargs):
            if str(path).endswith("prepared-profile-backup/python/main.py"):
                raise OSError("fixture partial staging")
            return original_open(path, *args, **kwargs)
        with patch("os.open", side_effect=interrupted):
            with self.assertRaises(OSError):
                self.request("correct_prepared_profile", release=self.bundle["release"])
        def no_exchange(first, second):
            self.fail("Original root must not move when backing out partial staging")
        self.store.exchange = no_exchange
        self.assertEqual(self.request("restore_prepared_profile", release=self.bundle["release"])["correctionPhase"], "restored")
        self.assertEqual(self.request("identity", release=identity["release"]), identity)

    def test_started_or_state_changed_app_is_never_corrected_or_restored(self):
        identity = self.old_owned_preparation()
        for status in ("running", "stopped", "starting", None):
            self.store.app_state = lambda status=status: status
            with self.assertRaises(provision.ProvisionError):
                self.request("correct_prepared_profile", release=self.bundle["release"])
            self.assertEqual(self.request("inspect")["release"], identity["release"])
        states = iter(("uninitialized", "running"))
        self.store.app_state = lambda: next(states)
        with self.assertRaises(provision.ProvisionError):
            self.request("correct_prepared_profile", release=self.bundle["release"])
        self.assertEqual(self.request("inspect")["release"], identity["release"])
        self.store.app_state = lambda: "uninitialized"
        self.request("correct_prepared_profile", release=self.bundle["release"])
        self.store.app_state = lambda: "running"
        with self.assertRaises(provision.ProvisionError):
            self.request("restore_prepared_profile", release=self.bundle["release"])
        self.assertEqual(self.request("inspect")["release"], self.bundle["release"])

    def test_extra_registry_cache_and_changed_source_cannot_enter_profile_correction(self):
        identity = self.old_owned_preparation()
        root = self.apps / "tinkercrew-companion"
        for path in (root / "data/pairings.json", root / ".cache/used"):
            path.parent.mkdir(mode=0o700, exist_ok=True)
            path.write_text("preserve existing state")
            with self.assertRaises(provision.ProvisionError):
                self.request("correct_prepared_profile", release=self.bundle["release"])
            self.assertEqual(path.read_text(), "preserve existing state")
            path.unlink()
            if path.parent.name == ".cache":
                path.parent.rmdir()
        original = (root / "sketch/sketch.ino").read_bytes()
        (root / "sketch/sketch.ino").write_text("modified source")
        with self.assertRaises(provision.ProvisionError):
            self.request("correct_prepared_profile", release=self.bundle["release"])
        self.assertEqual((root / "sketch/sketch.ino").read_text(), "modified source")
        (root / "sketch/sketch.ino").write_bytes(original)
        self.assertEqual(self.request("identity", release=identity["release"]), identity)

    def test_profile_correction_refuses_foreign_stage_symlinks_and_hardlinked_private_storage(self):
        identity = self.old_owned_preparation()
        stage = self.apps / ".tinkercrew-provision/prepared-profile-backup"
        victim = self.apps / "other-private-data"
        victim.mkdir(mode=0o700)
        (victim / "keep").write_text("not ours")
        stage.symlink_to(victim, target_is_directory=True)
        with self.assertRaises(provision.ProvisionError):
            self.request("correct_prepared_profile", release=self.bundle["release"])
        self.assertEqual((victim / "keep").read_text(), "not ours")
        stage.unlink()
        import os
        os.link(self.apps / "tinkercrew-companion/data/board.key", victim / "key-link")
        with self.assertRaises(provision.ProvisionError):
            self.request("correct_prepared_profile", release=self.bundle["release"])
        self.assertEqual(self.request("inspect")["release"], identity["release"])

    def test_damaged_private_backup_is_never_reported_as_recoverable_or_restored(self):
        identity = self.old_owned_preparation()
        self.request("correct_prepared_profile", release=self.bundle["release"])
        backup_source = self.apps / ".tinkercrew-provision/prepared-profile-backup/python/main.py"
        backup_source.write_text("unknown backup modification")
        for operation in ("correct_prepared_profile", "restore_prepared_profile"):
            with self.assertRaises(provision.ProvisionError):
                self.request(operation, release=self.bundle["release"])
        self.assertEqual(backup_source.read_text(), "unknown backup modification")
        self.assertEqual(self.request("identity", release=self.bundle["release"])["spki_sha256"], identity["spki_sha256"])
        self.assertEqual(self.request("inspect")["release"], self.bundle["release"])

    def test_unknown_cli_versions_fail_closed_using_only_read_only_commands(self):
        from types import SimpleNamespace
        from unittest.mock import patch
        identity = self.old_owned_preparation()
        self.store.app_state = provision.inspect_app_state
        def external_cli(args, **kwargs):
            self.assertEqual(args, ["/usr/bin/arduino-app-cli", "version", "--format", "json"])
            self.assertIs(kwargs["stdin"], provision.subprocess.DEVNULL)
            self.assertEqual(kwargs["env"]["HOME"], "/home/arduino")
            self.assertEqual(kwargs["env"]["ARDUINO_APP_CLI__APPS_DIR"], "/home/arduino/ArduinoApps")
            self.assertNotIn("DOCKER_HOST", kwargs["env"])
            self.assertNotIn("ARDUINO_APP_CLI__DATA_DIR", kwargs["env"])
            return SimpleNamespace(returncode=0, stdout=b'{"version":"0.10.0","daemon_version":"0.10.0"}', stderr=b"")
        with patch("subprocess.run", side_effect=external_cli):
            with self.assertRaises(provision.ProvisionError):
                self.request("correct_prepared_profile", release=self.bundle["release"])
        self.assertEqual(self.request("inspect")["release"], identity["release"])
        self.assertFalse((self.apps / ".tinkercrew-provision/profile-correction.json").exists())

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
