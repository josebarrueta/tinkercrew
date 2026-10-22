import json
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
