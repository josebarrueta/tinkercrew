from __future__ import annotations


class MemoryHardware:
    """Deterministic adapter used by tests and the explicit simulator mode."""

    def __init__(self):
        self.calls = []
        self._state = {"pixels": "off", "buzzer": "off", "matrix": "clear"}

    def set_pixels(self, color: str, brightness: float, duration_ms: int) -> None:
        self.calls.append(("pixels", color, brightness, duration_ms))
        self._state["pixels"] = {"color": color, "brightness": brightness, "duration_ms": duration_ms}

    def play_buzzer(self, frequency_hz: int, duration_ms: int, volume: float) -> None:
        self.calls.append(("buzzer", frequency_hz, duration_ms, volume))
        self._state["buzzer"] = {"frequency_hz": frequency_hz, "duration_ms": duration_ms, "volume": volume}

    def draw_matrix(self, rows: list[str], duration_ms: int) -> None:
        self.calls.append(("matrix", tuple(rows), duration_ms))
        self._state["matrix"] = {"rows": rows, "duration_ms": duration_ms}

    def renew_control_lease(self, lease_ms: int) -> None:
        self.calls.append(("lease", lease_ms))

    def safe_state(self) -> None:
        self.calls.append(("safe_state",))
        self._state = {"pixels": "off", "buzzer": "off", "matrix": "clear"}

    def self_test(self) -> None:
        self.calls.append(("self_test",))

    def state(self) -> dict:
        return self._state.copy()


class BridgeHardware(MemoryHardware):
    """UNO Q adapter using Arduino's shared Router Bridge, never Serial1."""

    def __init__(self, bridge):
        super().__init__()
        self._bridge = bridge

    def set_pixels(self, color: str, brightness: float, duration_ms: int) -> None:
        red, green, blue = (int(color[index:index + 2], 16) for index in (1, 3, 5))
        self._bridge.call("tc_pixels_set", red, green, blue, int(brightness * 100), duration_ms)
        super().set_pixels(color, brightness, duration_ms)

    def play_buzzer(self, frequency_hz: int, duration_ms: int, volume: float) -> None:
        self._bridge.call("tc_buzzer_play", frequency_hz, duration_ms, int(volume * 100))
        super().play_buzzer(frequency_hz, duration_ms, volume)

    def draw_matrix(self, rows: list[str], duration_ms: int) -> None:
        self._bridge.call("tc_matrix_draw", "".join(rows), duration_ms)
        super().draw_matrix(rows, duration_ms)

    def renew_control_lease(self, lease_ms: int) -> None:
        self._bridge.call("tc_control_lease", lease_ms)
        super().renew_control_lease(lease_ms)

    def safe_state(self) -> None:
        self._bridge.call("tc_safe_state")
        super().safe_state()

    def self_test(self) -> None:
        self._bridge.call("tc_self_test")
        super().self_test()
