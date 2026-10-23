import tempfile
import unittest
from pathlib import Path
from scripts.release_manifest import build_manifest


class ReleaseManifestTests(unittest.TestCase):
    def test_manifest_binds_version_target_asset_hash_and_detached_signature(self):
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / "Tinkeroo_0.1.0_universal.dmg"
            artifact.write_bytes(b"hello")
            signature = Path(directory) / "artifact.sig"
            signature.write_text("Zml4dHVyZS1zaWduYXR1cmU=\n")
            manifest = build_manifest("0.1.0", "darwin-universal", artifact, signature)
            self.assertEqual(manifest["version"], "0.1.0")
            self.assertEqual(manifest["artifacts"]["darwin-universal"]["sha256"], "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
            self.assertEqual(manifest["artifacts"]["darwin-universal"]["url"], "https://github.com/josebarrueta/tinkercrew/releases/download/v0.1.0/Tinkeroo_0.1.0_universal.dmg")
            self.assertEqual(manifest["artifacts"]["darwin-universal"]["signature"], "Zml4dHVyZS1zaWduYXR1cmU=")
            with self.assertRaises(ValueError):
                build_manifest("../invalid", "darwin-universal", artifact, signature)
            with self.assertRaises(ValueError):
                build_manifest("0.1.0", "anything", artifact, signature)


if __name__ == "__main__":
    unittest.main()
