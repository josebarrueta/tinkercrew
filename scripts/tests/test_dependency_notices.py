"""Public audit CLI/artifact contracts. Temporary fixture trees; no downloads or signing."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

CLI = Path(__file__).resolve().parents[1] / "dependency_notices.py"


class DependencyNoticeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        (self.root / "app/src-tauri").mkdir(parents=True)
        (self.root / "app/src-tauri/Cargo.lock").write_text('version = 4\n\n[[package]]\nname = "native-fixture"\nversion = "1.2.3"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "' + "a" * 64 + '"\n')
        self.npm = {"lockfileVersion": 3, "packages": {"": {"name": "fixture", "version": "0.1.0"}, "node_modules/frontend-fixture": {"version": "2.3.4", "license": "MIT", "dev": True, "integrity": "sha512-fixture"}}}
        (self.root / "app/package-lock.json").write_text(json.dumps(self.npm))
        self.inventory = self.root / "inventory.json"

    def invoke(self, *args):
        return subprocess.run([sys.executable, str(CLI), *args], capture_output=True, text=True, timeout=10)

    def test_inventory_is_deterministic_and_does_not_mistake_metadata_for_approval(self):
        first = self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory))
        self.assertEqual(first.returncode, 0, first.stderr)
        original = self.inventory.read_bytes()
        second = self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory))
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual(self.inventory.read_bytes(), original)
        data = json.loads(original)
        self.assertEqual(data["schemaVersion"], 1)
        self.assertEqual(data["assessment"], "inventory-only-not-legal-approval")
        self.assertEqual([(entry["ecosystem"], entry["name"], entry["version"]) for entry in data["components"]], [("cargo", "native-fixture", "1.2.3"), ("npm", "frontend-fixture", "2.3.4")])
        self.assertIsNone(data["components"][0]["licenseExpression"])
        self.assertEqual(data["components"][1]["licenseExpression"], "MIT")
        self.assertEqual(data["components"][1]["scope"], "unclassified-build-runtime-or-optional")
        self.assertNotIn(str(self.root), original.decode())
        self.assertNotIn("approved", data)

    def test_check_rejects_lock_drift_and_never_overwrites_existing_evidence(self):
        self.assertEqual(self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory)).returncode, 0)
        original = self.inventory.read_bytes()
        valid = self.invoke("check", "--root", str(self.root), "--inventory", str(self.inventory))
        self.assertEqual(valid.returncode, 0, valid.stderr)
        self.npm["packages"]["node_modules/frontend-fixture"]["version"] = "2.3.5"
        (self.root / "app/package-lock.json").write_text(json.dumps(self.npm))
        stale = self.invoke("check", "--root", str(self.root), "--inventory", str(self.inventory))
        self.assertNotEqual(stale.returncode, 0)
        self.assertIn("stale", stale.stderr.lower())
        self.assertEqual(self.inventory.read_bytes(), original)

    def test_inventory_refuses_duplicate_identities_secret_urls_and_input_overwrite(self):
        cargo = self.root / "app/src-tauri/Cargo.lock"
        original = cargo.read_bytes()
        overwrite = self.invoke("inventory", "--root", str(self.root), "--output", str(cargo))
        self.assertNotEqual(overwrite.returncode, 0)
        self.assertEqual(cargo.read_bytes(), original)
        cargo.write_bytes(original + b"\n[[package]]" + original.split(b"[[package]]", 1)[1])
        duplicate = self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory))
        self.assertNotEqual(duplicate.returncode, 0)
        cargo.write_bytes(original.replace(b"https://github.com", b"https://fixture-secret@github.com"))
        secret = self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory))
        self.assertNotEqual(secret.returncode, 0)
        self.assertNotIn("fixture-secret", secret.stderr + secret.stdout)
        self.assertFalse(self.inventory.exists())

    def test_notices_include_verified_license_text_and_mark_missing_evidence_pending(self):
        self.assertEqual(self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory)).returncode, 0)
        component = json.loads(self.inventory.read_bytes())["components"][0]
        license_text = b"Copyright Fixture Native Authors\nPermission fixture only.\n"
        license_path = self.root / "docs/release/third-party/licenses/fixture.txt"
        license_path.parent.mkdir(parents=True)
        license_path.write_bytes(license_text)
        evidence = self.root / "evidence.json"
        evidence.write_text(json.dumps({"schemaVersion": 1, "entries": [{"component": component, "status": "included", "licenseExpression": "MIT", "sourceAvailability": "https://example.invalid/native/1.2.3", "files": [{"path": "docs/release/third-party/licenses/fixture.txt", "sha256": hashlib.sha256(license_text).hexdigest()}]}]}))
        notice = self.root / "DEPENDENCY_NOTICES.txt"
        result = self.invoke("notices", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--output", str(notice))
        self.assertEqual(result.returncode, 0, result.stderr)
        text = notice.read_text()
        self.assertIn("NOT RELEASE APPROVAL", text)
        self.assertFalse(text.endswith("\n\n"))
        self.assertIn("native-fixture 1.2.3", text)
        self.assertIn(license_text.decode(), text)
        self.assertIn("PENDING: npm frontend-fixture 2.3.4", text)
        license_path.write_text("Changed license evidence")
        failed = self.invoke("notices", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--output", str(notice))
        self.assertNotEqual(failed.returncode, 0)
        self.assertEqual(notice.read_text(), text)
        license_path.write_bytes(b"")
        empty = json.loads(evidence.read_bytes())
        empty["entries"][0]["files"][0]["sha256"] = hashlib.sha256(b"").hexdigest()
        evidence.write_text(json.dumps(empty))
        failed = self.invoke("notices", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--output", str(notice))
        self.assertNotEqual(failed.returncode, 0)
        self.assertEqual(notice.read_text(), text)

    def test_bundle_check_requires_actual_matching_notice_and_inventory_resources(self):
        self.assertEqual(self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory)).returncode, 0)
        evidence = self.root / "evidence.json"
        evidence.write_text('{"schemaVersion":1,"entries":[]}')
        notice = self.root / "DEPENDENCY_NOTICES.txt"
        self.assertEqual(self.invoke("notices", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--output", str(notice)).returncode, 0)
        app = self.root / "Fixture.app"
        resources = app / "Contents/Resources/third-party"
        resources.mkdir(parents=True)
        args = ("bundle-check", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--notices", str(notice), "--app", str(app))
        self.assertNotEqual(self.invoke(*args).returncode, 0)
        (resources / "DEPENDENCY_INVENTORY.json").write_bytes(self.inventory.read_bytes())
        (resources / "DEPENDENCY_NOTICES.txt").write_bytes(notice.read_bytes())
        valid = self.invoke(*args)
        self.assertEqual(valid.returncode, 0, valid.stderr)
        alias = self.root / "Alias.app"
        alias.symlink_to(app, target_is_directory=True)
        self.assertNotEqual(self.invoke(*args[:-1], str(alias)).returncode, 0)
        (resources / "DEPENDENCY_NOTICES.txt").write_text("Tampered notices")
        self.assertNotEqual(self.invoke(*args).returncode, 0)

    def test_release_gate_never_accepts_pending_evidence_or_invents_review(self):
        self.assertEqual(self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory)).returncode, 0)
        evidence = self.root / "evidence.json"
        evidence.write_text('{"schemaVersion":1,"entries":[]}')
        notice = self.root / "DEPENDENCY_NOTICES.txt"
        self.assertEqual(self.invoke("notices", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--output", str(notice)).returncode, 0)
        review = self.root / "review.json"
        review.write_text(json.dumps({"schemaVersion": 1, "decision": "approved-for-release", "reviewer": "Fixture owner only", "reviewedAt": "2026-10-25T00:00:00Z", "inventorySha256": hashlib.sha256(self.inventory.read_bytes()).hexdigest(), "evidenceSha256": hashlib.sha256(evidence.read_bytes()).hexdigest(), "noticesSha256": hashlib.sha256(notice.read_bytes()).hexdigest()}))
        original = review.read_bytes()
        result = self.invoke("release-check", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--notices", str(notice), "--review", str(review))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("pending", result.stderr.lower())
        self.assertEqual(review.read_bytes(), original)

    def test_release_review_is_bound_to_complete_evidence_and_cannot_survive_changes(self):
        self.assertEqual(self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory)).returncode, 0)
        components = json.loads(self.inventory.read_bytes())["components"]
        evidence = self.root / "evidence.json"
        data = {"schemaVersion": 1, "entries": [{"component": component, "status": "excluded", "reason": "Fixture-only build tool; no code shipped in the fixture bundle."} for component in components]}
        evidence.write_text(json.dumps(data))
        notice = self.root / "DEPENDENCY_NOTICES.txt"
        render = ("notices", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--output", str(notice))
        self.assertEqual(self.invoke(*render).returncode, 0)
        review = self.root / "review.json"
        review.write_text(json.dumps({"schemaVersion": 1, "decision": "approved-for-release", "reviewer": "Fixture owner only", "reviewedAt": "2026-10-25T00:00:00Z", "inventorySha256": hashlib.sha256(self.inventory.read_bytes()).hexdigest(), "evidenceSha256": hashlib.sha256(evidence.read_bytes()).hexdigest(), "noticesSha256": hashlib.sha256(notice.read_bytes()).hexdigest()}))
        args = ("release-check", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--notices", str(notice), "--review", str(review))
        valid = self.invoke(*args)
        self.assertEqual(valid.returncode, 0, valid.stderr)
        self.assertIn("identity is not authenticated", valid.stdout)
        original_review = review.read_bytes()
        invalid_time = json.loads(original_review)
        invalid_time["reviewedAt"] = "Z"
        review.write_text(json.dumps(invalid_time))
        self.assertNotEqual(self.invoke(*args).returncode, 0)
        review.write_bytes(original_review)
        data["entries"][0]["reason"] = "Changed classification; requires renewed human review."
        evidence.write_text(json.dumps(data))
        self.assertEqual(self.invoke(*render).returncode, 0)
        stale = self.invoke(*args)
        self.assertNotEqual(stale.returncode, 0)
        self.assertIn("stale", stale.stderr.lower())

    def test_duplicate_json_and_symlinked_lock_inputs_fail_without_output(self):
        lock = self.root / "app/package-lock.json"
        lock.write_text('{"lockfileVersion":3,"packages":{},"packages":{}}')
        result = self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory))
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.inventory.exists())
        lock.unlink()
        outside = self.root / "outside.json"
        outside.write_text(json.dumps(self.npm))
        lock.symlink_to(outside)
        result = self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory))
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.inventory.exists())

    def test_license_evidence_cannot_read_arbitrary_repo_files_or_symlink_escapes(self):
        self.assertEqual(self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory)).returncode, 0)
        component = json.loads(self.inventory.read_bytes())["components"][0]
        secret = b"fixture-only-private-data-not-a-license"
        (self.root / "private.env").write_bytes(secret)
        licenses = self.root / "docs/release/third-party/licenses"
        licenses.mkdir(parents=True)
        (licenses / "escape.txt").symlink_to(self.root / "private.env")
        evidence = self.root / "evidence.json"
        notice = self.root / "notices.txt"
        for path in ("private.env", "../private.env", "docs/release/third-party/licenses/escape.txt"):
            evidence.write_text(json.dumps({"schemaVersion": 1, "entries": [{"component": component, "status": "included", "licenseExpression": "MIT", "sourceAvailability": "https://example.invalid/native/1.2.3", "files": [{"path": path, "sha256": hashlib.sha256(secret).hexdigest()}]}]}))
            result = self.invoke("notices", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--output", str(notice))
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn(secret.decode(), result.stdout + result.stderr)
            self.assertFalse(notice.exists())

    def test_check_detects_stale_source_notices_without_rewriting_them(self):
        self.assertEqual(self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory)).returncode, 0)
        evidence = self.root / "evidence.json"
        evidence.write_text('{"schemaVersion":1,"entries":[]}')
        notice = self.root / "notice.txt"
        self.assertEqual(self.invoke("notices", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--output", str(notice)).returncode, 0)
        args = ("check", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--notices", str(notice))
        good = self.invoke(*args)
        self.assertEqual(good.returncode, 0, good.stderr)
        notice.write_text("Stale disclosure")
        self.assertNotEqual(self.invoke(*args).returncode, 0)
        self.assertEqual(notice.read_text(), "Stale disclosure")

    def test_untrusted_lock_metadata_cannot_become_arbitrary_notice_content(self):
        self.npm["packages"]["node_modules/frontend-fixture"]["license"] = {"password": "fixture-only-secret"}
        (self.root / "app/package-lock.json").write_text(json.dumps(self.npm))
        result = self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory))
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("fixture-only-secret", result.stderr + result.stdout)
        self.assertFalse(self.inventory.exists())

    def test_generation_never_follows_a_symlinked_output(self):
        preserved = self.root / "preserved.txt"
        preserved.write_text("Keep existing data")
        self.inventory.symlink_to(preserved)
        result = self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory))
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(preserved.read_text(), "Keep existing data")
        self.assertTrue(self.inventory.is_symlink())

    def test_bundle_supports_complete_notices_larger_than_one_license_file(self):
        self.assertEqual(self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory)).returncode, 0)
        license_path = self.root / "docs/release/third-party/licenses/large.txt"
        license_path.parent.mkdir(parents=True)
        text = b"Fixture license text only.\n" * 30000
        license_path.write_bytes(text)
        evidence = self.root / "evidence.json"
        evidence.write_text(json.dumps({"schemaVersion": 1, "entries": [{"component": component, "status": "included", "licenseExpression": "MIT", "sourceAvailability": "https://example.invalid/fixture/source", "files": [{"path": "docs/release/third-party/licenses/large.txt", "sha256": hashlib.sha256(text).hexdigest()}]} for component in json.loads(self.inventory.read_bytes())["components"]]}))
        notice = self.root / "notices.txt"
        self.assertEqual(self.invoke("notices", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--output", str(notice)).returncode, 0)
        app = self.root / "Fixture.app"
        resources = app / "Contents/Resources/third-party"
        resources.mkdir(parents=True)
        (resources / "DEPENDENCY_INVENTORY.json").write_bytes(self.inventory.read_bytes())
        (resources / "DEPENDENCY_NOTICES.txt").write_bytes(notice.read_bytes())
        result = self.invoke("bundle-check", "--root", str(self.root), "--inventory", str(self.inventory), "--evidence", str(evidence), "--notices", str(notice), "--app", str(app))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_workspace_links_are_not_silently_counted_as_registry_packages(self):
        self.npm["packages"]["node_modules/frontend-fixture"]["link"] = True
        (self.root / "app/package-lock.json").write_text(json.dumps(self.npm))
        result = self.invoke("inventory", "--root", str(self.root), "--output", str(self.inventory))
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.inventory.exists())


if __name__ == "__main__":
    unittest.main()
