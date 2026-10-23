import type {
  BoardClient,
  BoardCommand,
  BoardEvent,
  BoardSnapshot,
} from "./types";

const capabilities: BoardSnapshot["capabilities"] = [
  "buttons",
  "distance",
  "pixels",
  "buzzer",
  "matrix",
];

export class SimulatorBoardClient implements BoardClient {
  private listeners = new Set<(event: BoardEvent) => void>();
  private leaseUntil = 0;
  private buzzerUntil = 0;
  private frozenAt: number | null = null;
  private snapshot: BoardSnapshot = {
    mode: "simulator",
    health: "ready",
    capabilities,
    observables: {
      distanceMillimeters: 600,
      distanceAgeMs: 0,
      buttons: [false, false, false],
    },
    outputs: { pixels: null, buzzer: null, matrix: null },
  };

  readonly controls = {
    freezeDistance: (frozen: boolean) => {
      this.frozenAt = frozen ? Date.now() : null;
    },
    setHealth: (health: BoardSnapshot["health"]) => {
      this.snapshot.health = health;
      if (health !== "ready") this.clearOutputs();
      this.emit({ type: "health.changed", health });
    },
    setDistance: (millimeters: number) => {
      if (!Number.isFinite(millimeters)) {
        throw new Error("distance must be a finite number");
      }
      const bounded = Math.max(20, Math.min(4000, Math.round(millimeters)));
      this.snapshot.observables.distanceMillimeters = bounded;
      this.emit({ type: "distance.changed", millimeters: bounded });
    },
    pressButton: (button: number) => {
      if (!Number.isInteger(button) || button < 0 || button > 2) {
        throw new Error("button must be 0, 1, or 2");
      }
      this.snapshot.observables.buttons[button] = true;
      this.emit({ type: "button.changed", button, pressed: true });
      this.snapshot.observables.buttons[button] = false;
      this.emit({ type: "button.changed", button, pressed: false });
    },
  };

  async getSnapshot(): Promise<BoardSnapshot> {
    if (this.leaseUntil && Date.now() >= this.leaseUntil) {
      this.clearOutputs();
      this.snapshot.health = "safe_state";
      this.leaseUntil = 0;
    }
    if (Date.now() >= this.buzzerUntil) this.snapshot.outputs.buzzer = null;
    this.snapshot.observables.distanceAgeMs =
      this.frozenAt === null ? 0 : Date.now() - this.frozenAt;
    return structuredClone(this.snapshot);
  }

  async renewControl(): Promise<void> {
    if (this.snapshot.health === "disconnected")
      throw new Error("Board disconnected; reconnect and retry.");
    this.leaseUntil = Date.now() + 1500;
    this.snapshot.health = "ready";
  }

  async releaseControl(): Promise<void> {
    this.leaseUntil = 0;
    this.clearOutputs();
    if (this.snapshot.health !== "disconnected")
      this.snapshot.health = "safe_state";
  }

  private clearOutputs() {
    this.snapshot.outputs = { pixels: null, buzzer: null, matrix: null };
  }

  subscribe(listener: (event: BoardEvent) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  async command(command: BoardCommand): Promise<void> {
    if (
      command.operation !== "outputs.clear" &&
      Date.now() >= this.leaseUntil
    ) {
      await this.releaseControl();
      throw new Error(
        "Safe state active; renew control before sending outputs.",
      );
    }
    if (command.operation === "pixels.set") {
      const { color, brightness } = command.arguments;
      if (!/^#[0-9a-f]{6}$/i.test(color))
        throw new Error("color must be #rrggbb");
      if (!Number.isFinite(brightness) || brightness < 0 || brightness > 0.5)
        throw new Error("brightness must be a finite number from 0.0..0.5");
      this.snapshot.outputs.pixels = { color, brightness };
      return;
    }
    if (command.operation === "buzzer.play") {
      const { frequencyHz, durationMs } = command.arguments;
      if (
        !Number.isFinite(frequencyHz) ||
        frequencyHz < 100 ||
        frequencyHz > 4000
      )
        throw new Error("frequency must be a finite number from 100..4000 Hz");
      if (!Number.isFinite(durationMs) || durationMs < 1 || durationMs > 3000)
        throw new Error("duration must be a finite number from 1..3000 ms");
      this.snapshot.outputs.buzzer = { frequencyHz, durationMs };
      this.buzzerUntil = Date.now() + durationMs;
      return;
    }
    await this.releaseControl();
  }

  private emit(event: BoardEvent) {
    for (const listener of this.listeners) listener(event);
  }
}
