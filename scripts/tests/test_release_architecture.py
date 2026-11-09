"""Real Mach-O fixtures, no signing credentials or Apple submissions."""
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest

CLI = Path(__file__).resolve().parents[1] / 'release_architecture.py'


@unittest.skipUnless(sys.platform == 'darwin' and shutil.which('clang') and shutil.which('lipo'), 'Requires macOS developer tools')
class ReleaseArchitectureTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.app = Path(self.temporary.name) / 'Fixture.app'
        self.binary = self.app / 'Contents/MacOS/fixture'
        self.binary.parent.mkdir(parents=True)
        (self.app / 'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleExecutable': 'fixture'}))

    def compile(self, path, arch):
        subprocess.run(['clang', '-arch', arch, '-x', 'c', '-c', '-', '-o', str(path)], input=b'int main(void) { return 0; }\n', check=True, capture_output=True, timeout=30)

    def invoke(self):
        return subprocess.run([sys.executable, str(CLI), str(self.app)], capture_output=True, text=True, timeout=10)

    def test_accepts_arm64_code_and_noncode_resources(self):
        self.compile(self.binary, 'arm64')
        (self.app / 'Contents/notice.txt').write_text('A normal resource is not Mach-O.')
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_intel_and_universal_main_executables(self):
        arm = Path(self.temporary.name) / 'arm.o'
        intel = Path(self.temporary.name) / 'intel.o'
        self.compile(arm, 'arm64')
        self.compile(intel, 'x86_64')
        shutil.copyfile(intel, self.binary)
        self.assertNotEqual(self.invoke().returncode, 0)
        subprocess.run(['lipo', '-create', str(arm), str(intel), '-output', str(self.binary)], check=True, capture_output=True, timeout=10)
        self.assertNotEqual(self.invoke().returncode, 0)

    def test_rejects_wrong_architecture_embedded_code(self):
        self.compile(self.binary, 'arm64')
        helper = self.binary.parent / 'helper'
        self.compile(helper, 'x86_64')
        self.assertNotEqual(self.invoke().returncode, 0)

    def test_rejects_missing_non_macho_or_escaping_main(self):
        self.assertNotEqual(self.invoke().returncode, 0)
        self.binary.write_text('not an executable')
        self.assertNotEqual(self.invoke().returncode, 0)
        (self.app / 'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleExecutable': '../../outside'}))
        self.assertNotEqual(self.invoke().returncode, 0)


if __name__ == '__main__':
    unittest.main()
