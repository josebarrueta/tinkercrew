import tempfile
import unittest
from pathlib import Path
from scripts.release_manifest import build_manifest


class ReleaseManifestTests(unittest.TestCase):
    def test_manifest_binds_version_target_asset_hash_and_detached_signature(self):
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / "Tinkeroo_0.1.0_aarch64.dmg"
            artifact.write_bytes(b"hello")
            signature = Path(directory) / "artifact.sig"
            signature.write_text("Zml4dHVyZS1zaWduYXR1cmU=\n")
            manifest = build_manifest("0.1.0", "darwin-aarch64", artifact, signature)
            self.assertEqual(manifest["version"], "0.1.0")
            self.assertEqual(set(manifest["artifacts"]), {"darwin-aarch64"})
            self.assertEqual(manifest["artifacts"]["darwin-aarch64"]["sha256"], "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
            self.assertEqual(manifest["artifacts"]["darwin-aarch64"]["url"], "https://github.com/josebarrueta/tinkercrew/releases/download/v0.1.0/Tinkeroo_0.1.0_aarch64.dmg")
            self.assertEqual(manifest["artifacts"]["darwin-aarch64"]["signature"], "Zml4dHVyZS1zaWduYXR1cmU=")
            with self.assertRaises(ValueError):
                build_manifest("../invalid", "darwin-aarch64", artifact, signature)
            for unsupported in ("anything", "darwin-x86_64", "darwin-universal"):
                with self.assertRaises(ValueError):
                    build_manifest("0.1.0", unsupported, artifact, signature)


if __name__ == "__main__":
    unittest.main()
