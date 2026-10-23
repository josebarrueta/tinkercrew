import unittest

from tinkercrew_companion.core import CommandError, Companion
from tinkercrew_companion.hardware import MemoryHardware


class ManualClock:
    def __init__(self):
        self.now = 100.0

    def __call__(self):
        return self.now


class CompanionTests(unittest.TestCase):
    def setUp(self):
        self.clock = ManualClock()
        self.hardware = MemoryHardware()
        self.companion = Companion(self.hardware, clock=self.clock)

    def test_output_state_expires_when_a_bounded_command_ends(self):
        hardware = MemoryHardware(clock=self.clock)
        hardware.play_buzzer(880, 250, 0.25)
        self.assertIsInstance(hardware.state()["buzzer"], dict)
        self.clock.now += 0.3
        self.assertEqual(hardware.state()["buzzer"], "off")

    def test_reports_stable_board_identity_and_input_age_for_reconnection(self):
        companion = Companion(self.hardware, clock=self.clock, board_id="board-12345678")
        companion.record_distance(428)
        self.clock.now += 0.8
        self.assertEqual(companion.hello()["board"]["board_id"], "board-12345678")
        self.assertAlmostEqual(companion.state()["observables"]["distance"]["age_ms"], 800)

    def test_reports_versions_capabilities_and_distinct_input_health(self):
        self.companion.record_distance(428)
        self.companion.record_buttons([True, False, False])
        hello = self.companion.hello()
        self.assertEqual(hello["versions"], {
            "companion": "0.1.0", "firmware": "0.1.0", "protocol": "1.0", "board": "unknown"
        })
        self.assertEqual(hello["capabilities"]["distance"]["health"], "operational")

        self.clock.now += 0.8
        self.assertEqual(self.companion.hello()["capabilities"]["distance"]["health"], "stale")

        self.companion.mark_disconnected("distance")
        self.assertEqual(self.companion.hello()["capabilities"]["distance"]["health"], "disconnected")

    def test_button_edges_and_distance_samples_are_visible_as_events(self):
        self.companion.record_buttons([False, False, False])
        self.companion.record_buttons([True, False, True])
        self.companion.record_distance(321)
        events = self.companion.events(after_sequence=0)
        self.assertEqual([event["type"] for event in events], [
            "button.changed", "button.changed", "distance.changed"
        ])
        self.assertEqual(events[-1]["payload"], {"millimeters": 321})

    def test_commands_are_bounded_before_hardware_is_called(self):
        self.companion.renew_control_lease()
        self.hardware.calls.clear()
        with self.assertRaisesRegex(CommandError, "command_out_of_range"):
            self.companion.command("buzzer.play", {
                "frequency_hz": 440, "duration_ms": 3001, "volume": 0.25
            })
        with self.assertRaisesRegex(CommandError, "command_out_of_range"):
            self.companion.command("pixels.set", {
                "color": "#ffffff", "brightness": 0.51, "duration_ms": 100
            })
        self.assertEqual(self.hardware.calls, [])

        result = self.companion.command("buzzer.play", {
            "frequency_hz": 440, "duration_ms": 300, "volume": 0.25
        })
        self.assertEqual(result["status"], "ok")
        self.assertEqual(self.hardware.calls[-1], ("buzzer", 440, 300, 0.25))

    def test_expired_control_lease_enters_safe_state_within_two_seconds(self):
        self.companion.renew_control_lease(lease_ms=1500)
        self.companion.command("pixels.set", {
            "color": "#ff0000", "brightness": 0.25, "duration_ms": 5000
        })
        self.clock.now += 1.6
        self.assertTrue(self.companion.check_watchdog())
        self.assertTrue(self.companion.state()["health"]["safe_state"])
        self.assertEqual(self.hardware.calls[-1], ("safe_state",))

    def test_self_test_requires_human_confirmation(self):
        started = self.companion.start_self_test()
        self.assertEqual(started["state"], "awaiting_confirmation")
        completed = self.companion.confirm_self_test(started["id"], lights=True, sound=False)
        self.assertEqual(completed["state"], "failed")
        self.assertEqual(completed["confirmation"], {"lights": True, "sound": False})


if __name__ == "__main__":
    unittest.main()
