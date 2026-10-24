import json
import http.client
import threading
import unittest
import urllib.error
import urllib.request
from http.server import ThreadingHTTPServer

from tinkercrew_companion.core import Companion
from tinkercrew_companion.hardware import MemoryHardware
from tinkercrew_companion.server import make_handler


class ServerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.token = "t" * 32
        cls.companion = Companion(MemoryHardware())
        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(cls.companion, cls.token))
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.base = f"http://127.0.0.1:{cls.server.server_port}"

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join()

    def request(self, path, *, authenticated=False, body=None):
        headers = {}
        if authenticated:
            headers["Authorization"] = f"Bearer {self.token}"
        data = None
        if body is not None:
            data = json.dumps(body).encode()
            headers["Content-Type"] = "application/json"
        return urllib.request.urlopen(urllib.request.Request(self.base + path, data=data, headers=headers), timeout=2)

    def test_pending_runtime_update_blocks_other_clients_and_watchdog_disarms_without_a_request(self):
        import tempfile
        import time
        from pathlib import Path
        from tinkercrew_companion.server import CompanionHTTPServer, runtime_maintenance
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            core = Companion(MemoryHardware())
            server = CompanionHTTPServer(("127.0.0.1", 0), make_handler(core, self.token, maintenance=lambda: runtime_maintenance(directory)))
            worker = threading.Thread(target=server.serve_forever, daemon=True)
            worker.start()
            base = f"http://127.0.0.1:{server.server_port}"
            def call(path, body=None):
                data = json.dumps(body).encode() if body is not None else None
                return urllib.request.urlopen(urllib.request.Request(base + path, data=data, headers={"Authorization": "Bearer " + self.token, "Content-Type": "application/json"}), timeout=2)
            try:
                with call("/v1/control-lease", {"lease_ms": 2000}):
                    pass
                (directory / "job.json").write_text(json.dumps({"phase": "uploading"}))
                deadline = time.monotonic() + 1
                while not core.hello()["health"]["safe_state"] and time.monotonic() < deadline:
                    time.sleep(0.01)
                self.assertTrue(core.hello()["health"]["safe_state"])
                with call("/v1/hello") as response:
                    self.assertEqual(response.status, 200)
                with self.assertRaises(urllib.error.HTTPError) as busy:
                    call("/v1/control-lease", {"lease_ms": 1500})
                self.assertEqual(busy.exception.code, 503)
                self.assertEqual(json.load(busy.exception)["error"]["code"], "maintenance_active")
                busy.exception.close()
                (directory / "job.json").write_text(json.dumps({"phase": "aborted"}))
                self.assertTrue(core.hello()["health"]["safe_state"])
                with call("/v1/control-lease", {"lease_ms": 1500}) as response:
                    self.assertEqual(response.status, 200)
            finally:
                server.shutdown()
                server.server_close()
                worker.join(timeout=2)

    def test_numeric_listener_starts_and_serves_when_reverse_dns_is_unavailable(self):
        from unittest.mock import patch
        from tinkercrew_companion.server import CompanionHTTPServer
        with patch("socket.getfqdn", side_effect=OSError("fixture reverse DNS unavailable")):
            server = CompanionHTTPServer(("127.0.0.1", 0), make_handler(Companion(MemoryHardware()), self.token))
            worker = threading.Thread(target=server.serve_forever, daemon=True)
            worker.start()
            try:
                request = urllib.request.Request(f"http://127.0.0.1:{server.server_port}/v1/hello", headers={"Authorization": "Bearer " + self.token})
                with urllib.request.urlopen(request, timeout=2) as response:
                    self.assertEqual(response.status, 200)
                    self.assertEqual(json.load(response)["protocol"], "1.0")
            finally:
                server.shutdown()
                server.server_close()
                worker.join(timeout=2)

    def test_body_framing_limits_are_rejected_before_reading(self):
        for framing, expected in [
            ([("Content-Length", "65537")], "message_too_large"),
            ([("Content-Length", "0"), ("Content-Length", "0")], "invalid_message"),
            ([("Transfer-Encoding", "chunked")], "invalid_message"),
        ]:
            with self.subTest(framing=framing):
                connection = http.client.HTTPConnection("127.0.0.1", self.server.server_port, timeout=0.3)
                try:
                    connection.putrequest("POST", "/v1/control-lease")
                    connection.putheader("Authorization", f"Bearer {self.token}")
                    for name, value in framing:
                        connection.putheader(name, value)
                    connection.endheaders()
                    response = connection.getresponse()
                    self.assertEqual(response.status, 400)
                    self.assertEqual(json.load(response)["error"]["code"], expected)
                finally:
                    connection.close()

    def test_negative_body_length_is_rejected_without_waiting_for_a_body(self):
        connection = http.client.HTTPConnection("127.0.0.1", self.server.server_port, timeout=0.3)
        try:
            connection.request("POST", "/v1/control-lease", headers={
                "Authorization": f"Bearer {self.token}", "Content-Length": "-1",
            })
            response = connection.getresponse()
            self.assertEqual(response.status, 400)
            self.assertEqual(json.load(response)["error"]["code"], "invalid_message")
        finally:
            connection.close()

    def test_post_command_rate_is_bounded(self):
        statuses = []
        for _ in range(21):
            try:
                with self.request("/v1/control-lease", authenticated=True, body={"lease_ms": 1500}) as response:
                    statuses.append(response.status)
            except urllib.error.HTTPError as error:
                statuses.append(error.code)
                self.assertEqual(json.load(error)["error"]["code"], "rate_limited")
                error.close()
        self.assertIn(429, statuses)

    def test_detailed_health_requires_authentication(self):
        with self.assertRaises(urllib.error.HTTPError) as caught:
            self.request("/v1/hello")
        self.assertEqual(caught.exception.code, 401)
        caught.exception.close()
        with self.request("/v1/hello", authenticated=True) as response:
            self.assertEqual(json.load(response)["protocol"], "1.0")

    def test_authenticated_client_can_read_events_and_send_bounded_command(self):
        self.companion.record_distance(222)
        with self.request("/v1/events?after=0", authenticated=True) as response:
            self.assertEqual(json.load(response)["events"][-1]["payload"], {"millimeters": 222})
        with self.request("/v1/control-lease", authenticated=True, body={"lease_ms": 1500}):
            pass
        with self.request("/v1/commands", authenticated=True, body={
            "operation": "buzzer.play",
            "arguments": {"frequency_hz": 440, "duration_ms": 100, "volume": 0.2},
        }) as response:
            self.assertEqual(json.load(response), {"status": "ok"})


if __name__ == "__main__":
    unittest.main()
