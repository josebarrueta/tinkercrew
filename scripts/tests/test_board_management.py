import importlib.util
import json
import sys
import pathlib
import subprocess
import unittest

path = pathlib.Path(__file__).resolve().parents[2] / "app/src-tauri/src/board_management.py"
spec = importlib.util.spec_from_file_location("board_management", path)
management = importlib.util.module_from_spec(spec)
spec.loader.exec_module(management)


class BoardManagementTests(unittest.TestCase):
    def setUp(self):
        self.files = {
            "/proc/device-tree/compatible": b"arduino,imola\x00qcom,qcs2290\x00",
            "/sys/devices/soc0/serial_number": b"123456789\n",
            "/etc/os-release": b'ID=debian\nVERSION_ID="13"\n',
            "/etc/hostname": b"dragon\n",
        }
        self.calls = []

    def run_command(self, args, secret=None):
        self.calls.append((args, secret))
        return subprocess.CompletedProcess(args, 0, "", "")

    def call(self, request):
        return management.handle(request, reader=lambda path: self.files[path], runner=self.run_command, username="arduino")

    def test_reads_physical_identity_and_rejects_a_different_board_before_any_change(self):
        self.assertEqual(self.call({"operation": "inspect"})["serial"], "123456789")
        with self.assertRaises(ValueError):
            self.call({"operation": "hostname", "expectedSerial": "other", "hostname": "dragon", "linuxPassword": "private-linux"})
        self.assertEqual(self.calls, [])
        self.files["/proc/device-tree/compatible"] = b"arduino,monza\x00"
        with self.assertRaises(ValueError):
            self.call({"operation": "inspect"})

    def test_build_identity_is_observed_and_never_invented_for_legacy_images(self):
        self.assertEqual(self.call({"operation": "inspect"})["imageVersion"], "unknown")
        self.files["/etc/buildinfo"] = b'BUILD_ID="20260528-558"\n'
        self.assertEqual(self.call({"operation": "inspect"})["imageVersion"], "20260528-558")
        self.files["/sys/devices/soc0/serial_number"] = b"0x2a\n"
        self.assertEqual(self.call({"operation": "inspect"})["serial"], "42")

    def test_enabling_ssh_is_explicit_current_account_only_and_cli_errors_are_sanitized(self):
        request = {"operation": "enable_ssh", "expectedSerial": "123456789", "username": "other", "linuxPassword": "fixture-only-linux-password"}
        with self.assertRaises(ValueError):
            self.call(request)
        self.assertEqual(self.calls, [])
        request["username"] = "arduino"
        self.assertEqual(self.call(request)["state"], "ssh_enabled")
        self.assertEqual(self.calls[0][0][-4:], ["systemctl", "enable", "--now", "ssh.service"])
        self.assertNotIn(request["linuxPassword"], repr(self.calls[0][0]))
        # Run the real transient helper locally, with an unsupported request that cannot mutate anything.
        result = subprocess.run([sys.executable, str(path)], input=json.dumps({"operation": "shell", "linuxPassword": "fixture-only-linux-password"}), text=True, capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(json.loads(result.stdout)["status"], "error")
        self.assertNotIn("fixture-only-linux-password", result.stdout + result.stderr)

    def test_password_only_goes_to_stdin_and_hostname_is_not_shell_text(self):
        result = self.call({"operation": "hostname", "expectedSerial": "123456789", "hostname": "dragon-den", "linuxPassword": "private-linux"})
        self.assertEqual(result["hostname"], "dragon-den")
        self.assertNotIn("private-linux", repr(self.calls[0][0]))
        self.assertEqual(self.calls[0][1], "private-linux\n")
        with self.assertRaises(ValueError):
            self.call({"operation": "hostname", "expectedSerial": "123456789", "hostname": "x; rm -rf /", "linuxPassword": "private-linux"})

    def test_wifi_password_does_not_enter_process_arguments_or_response(self):
        result = self.call({"operation": "wifi_join", "expectedSerial": "123456789", "ssid": "Family WiFi", "wifiPassword": "private-wifi"})
        self.assertEqual(result["state"], "connected")
        self.assertNotIn("private-wifi", repr(self.calls[0][0]))
        self.assertNotIn("private-wifi", repr(result))
        self.assertEqual(self.calls[0][1], "private-wifi\n")

    def test_reboot_reports_scheduled_not_completed_and_arbitrary_operations_are_rejected(self):
        result = self.call({"operation": "reboot", "expectedSerial": "123456789", "linuxPassword": "private-linux"})
        self.assertEqual(result["state"], "reboot_scheduled")
        self.assertIn("+1", self.calls[0][0])
        with self.assertRaises(ValueError):
            self.call({"operation": "shell", "command": "anything"})
