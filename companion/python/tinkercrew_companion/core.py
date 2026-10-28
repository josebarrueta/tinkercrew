from __future__ import annotations

import re
import threading
import time
import uuid
from collections import deque
from dataclasses import dataclass
from typing import Callable

PROTOCOL_VERSION = "1.0"
COMPANION_VERSION = "0.1.0"
FIRMWARE_VERSION = "0.1.0"
STALE_AFTER_SECONDS = {"buttons": 0.75, "distance": 0.75}


class CommandError(ValueError):
    pass


@dataclass
class Observable:
    value: object | None = None
    observed_at: float | None = None
    connected: bool = True


class Companion:
    """Thread-safe mission-independent Board state and bounded command service."""

    def __init__(self, hardware, clock: Callable[[], float] = time.monotonic, board_version: str = "unknown", board_id: str = "unprovisioned"):
        self._hardware = hardware
        self._board_id = board_id
        self._board_version = board_version
        self._clock = clock
        self._started_at = clock()
        self._lock = threading.RLock()
        self._observables = {
            "buttons": Observable(),
            "distance": Observable(),
        }
        self._output_health = {"pixels": "operational", "buzzer": "operational", "matrix": "operational"}
        self._events = deque(maxlen=256)
        self._sequence = 0
        self._last_buttons = None
        self._lease_deadline = None
        self._safe_state = True
        self._self_tests = {}

    def _health(self, capability: str) -> str:
        observable = self._observables[capability]
        if not observable.connected:
            return "disconnected"
        if observable.observed_at is None:
            return "stale"
        if self._clock() - observable.observed_at > STALE_AFTER_SECONDS[capability]:
            return "stale"
        return "operational"

    def _emit(self, event_type: str, capability: str, health: str, payload: dict, *, observed_at=None) -> None:
        self._sequence += 1
        self._events.append({
            "protocol": PROTOCOL_VERSION,
            "event_id": str(uuid.uuid4()),
            "sequence": self._sequence,
            "monotonic_ms": int(((self._clock() if observed_at is None else observed_at) - self._started_at) * 1000),
            "type": event_type,
            "capability": capability,
            "health": health,
            "payload": payload,
        })

    def record_distance(self, millimeters: int, *, observed_at=None) -> None:
        received = self._clock() if observed_at is None else observed_at
        if not isinstance(millimeters, int) or not 0 <= millimeters <= 5000 or not isinstance(received, (int, float)) or not self._started_at <= received <= self._clock():
            return
        with self._lock:
            observable = self._observables["distance"]
            if observable.observed_at is not None and received < observable.observed_at:
                return
            recovered = not observable.connected
            observable.value = millimeters
            observable.observed_at = received
            observable.connected = True
            if recovered:
                self._emit("capability.health_changed", "distance", "operational", {})
            self._emit("distance.changed", "distance", "operational", {"millimeters": millimeters}, observed_at=received)

    def record_buttons(self, pressed: list[bool], *, observed_at=None) -> None:
        received = self._clock() if observed_at is None else observed_at
        if len(pressed) != 3 or any(type(item) is not bool for item in pressed) or not isinstance(received, (int, float)) or not self._started_at <= received <= self._clock():
            return
        with self._lock:
            observable = self._observables["buttons"]
            if observable.observed_at is not None and received < observable.observed_at:
                return
            recovered = not observable.connected
            observable.value = list(pressed)
            observable.observed_at = received
            observable.connected = True
            previous = self._last_buttons
            self._last_buttons = list(pressed)
            if recovered:
                self._emit("capability.health_changed", "buttons", "operational", {})
            if previous is not None:
                for index, value in enumerate(pressed):
                    if value != previous[index]:
                        self._emit("button.changed", "buttons", "operational", {
                            "button": index,
                            "pressed": value,
                        }, observed_at=received)

    def mark_disconnected(self, capability: str) -> None:
        if capability not in self._observables:
            return
        with self._lock:
            observable = self._observables[capability]
            if observable.connected:
                observable.connected = False
                self._emit("capability.health_changed", capability, "disconnected", {})

    def record_capability_health(self, capability: str, health: str) -> None:
        if capability not in self._output_health or health not in {"operational", "disconnected", "degraded"}:
            return
        with self._lock:
            if self._output_health[capability] != health:
                self._output_health[capability] = health
                self._emit("capability.health_changed", capability, health, {})

    def hello(self) -> dict:
        with self._lock:
            return {
                "protocol": PROTOCOL_VERSION,
                "board": {"board_id": self._board_id, "model": "arduino-uno-q", "version": self._board_version},
                "versions": {
                    "companion": COMPANION_VERSION,
                    "firmware": FIRMWARE_VERSION,
                    "protocol": PROTOCOL_VERSION,
                    "board": self._board_version,
                },
                "health": {
                    "state": "safe_state" if self._safe_state else "ready",
                    "uptime_ms": int((self._clock() - self._started_at) * 1000),
                    "safe_state": self._safe_state,
                },
                "capabilities": {
                    "buttons": {"kind": "observable", "health": self._health("buttons"), "stale_after_ms": 750},
                    "distance": {"kind": "observable", "health": self._health("distance"), "unit": "millimeter", "stale_after_ms": 750},
                    "pixels": {"kind": "output", "health": self._output_health["pixels"], "count": 8, "maximum_brightness": 0.5},
                    "buzzer": {"kind": "output", "health": self._output_health["buzzer"], "minimum_hz": 100, "maximum_hz": 4000, "maximum_duration_ms": 3000},
                    "matrix": {"kind": "output", "health": self._output_health["matrix"], "width": 13, "height": 8},
                },
                "limits": {"maximum_message_bytes": 65536, "maximum_commands_per_second": 20},
            }

    def state(self) -> dict:
        with self._lock:
            return {
                "health": self.hello()["health"],
                "observables": {
                    name: {"value": item.value, "health": self._health(name), "age_ms": None if item.observed_at is None else max(0, (self._clock() - item.observed_at) * 1000)}
                    for name, item in self._observables.items()
                },
                "outputs": self._hardware.state(),
            }

    def events(self, after_sequence: int = 0) -> list[dict]:
        with self._lock:
            return [event.copy() for event in self._events if event["sequence"] > after_sequence]

    def renew_control_lease(self, lease_ms: int = 1500) -> dict:
        if not isinstance(lease_ms, int) or not 100 <= lease_ms <= 2000:
            raise CommandError("command_out_of_range: lease_ms must be 100..2000")
        with self._lock:
            self._hardware.renew_control_lease(lease_ms)
            self._lease_deadline = self._clock() + lease_ms / 1000
            self._safe_state = False
            return {"status": "ok", "lease_ms": lease_ms}

    def check_watchdog(self) -> bool:
        with self._lock:
            if self._lease_deadline is None or self._clock() <= self._lease_deadline:
                return False
            self.enter_safe_state("control_lease_expired")
            return True

    def enter_safe_state(self, reason: str) -> None:
        with self._lock:
            self._hardware.safe_state()
            self._safe_state = True
            self._lease_deadline = None
            self._emit("board.safe_state", "board", "operational", {"reason": reason})

    def command(self, operation: str, arguments: dict) -> dict:
        if not isinstance(arguments, dict):
            raise CommandError("invalid_message: arguments must be an object")
        with self._lock:
            if operation != "outputs.clear" and (self._lease_deadline is None or self._clock() > self._lease_deadline):
                self.enter_safe_state("control_lease_missing")
                raise CommandError("safe_state_active: renew the control lease before sending commands")
            capability = {"pixels.set": "pixels", "buzzer.play": "buzzer", "matrix.draw": "matrix"}.get(operation)
            if capability and self._output_health[capability] != "operational":
                raise CommandError(f"capability_unavailable: {capability} is {self._output_health[capability]}")
            if operation == "pixels.set":
                color = arguments.get("color")
                brightness = arguments.get("brightness")
                duration = arguments.get("duration_ms")
                if not isinstance(color, str) or not re.fullmatch(r"#[0-9a-fA-F]{6}", color):
                    raise CommandError("command_out_of_range: color must be #rrggbb")
                if not isinstance(brightness, (int, float)) or not 0 <= brightness <= 0.5:
                    raise CommandError("command_out_of_range: brightness must be 0.0..0.5")
                if not isinstance(duration, int) or not 1 <= duration <= 10000:
                    raise CommandError("command_out_of_range: duration_ms must be 1..10000")
                self._hardware.set_pixels(color, float(brightness), duration)
            elif operation == "buzzer.play":
                frequency = arguments.get("frequency_hz")
                duration = arguments.get("duration_ms")
                volume = arguments.get("volume")
                if not isinstance(frequency, int) or not 100 <= frequency <= 4000:
                    raise CommandError("command_out_of_range: frequency_hz must be 100..4000")
                if not isinstance(duration, int) or not 1 <= duration <= 3000:
                    raise CommandError("command_out_of_range: duration_ms must be 1..3000")
                if not isinstance(volume, (int, float)) or not 0 <= volume <= 0.5:
                    raise CommandError("command_out_of_range: volume must be 0.0..0.5")
                self._hardware.play_buzzer(frequency, duration, float(volume))
            elif operation == "matrix.draw":
                rows = arguments.get("rows")
                duration = arguments.get("duration_ms")
                if not isinstance(rows, list) or len(rows) != 8 or any(not isinstance(row, str) or not re.fullmatch(r"[01]{13}", row) for row in rows):
                    raise CommandError("command_out_of_range: rows must be eight 13-bit strings")
                if not isinstance(duration, int) or not 1 <= duration <= 10000:
                    raise CommandError("command_out_of_range: duration_ms must be 1..10000")
                self._hardware.draw_matrix(rows, duration)
            elif operation == "outputs.clear":
                if arguments:
                    raise CommandError("invalid_message: outputs.clear has no arguments")
                self.enter_safe_state("client_requested")
            else:
                raise CommandError("invalid_message: unknown operation")
            return {"status": "ok"}

    def start_self_test(self) -> dict:
        with self._lock:
            test_id = str(uuid.uuid4())
            self._hardware.self_test()
            result = {"id": test_id, "state": "awaiting_confirmation"}
            self._self_tests[test_id] = result
            return result.copy()

    def confirm_self_test(self, test_id: str, lights: bool, sound: bool) -> dict:
        with self._lock:
            if test_id not in self._self_tests or type(lights) is not bool or type(sound) is not bool:
                raise CommandError("invalid_message: unknown self-test or invalid confirmation")
            result = {
                "id": test_id,
                "state": "passed" if lights and sound else "failed",
                "confirmation": {"lights": lights, "sound": sound},
            }
            self._self_tests[test_id] = result
            self.enter_safe_state("self_test_complete")
            return result.copy()
