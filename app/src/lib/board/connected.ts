import { z } from "zod";
import type {
  BoardClient,
  BoardCommand,
  BoardEvent,
  BoardSnapshot,
  Capability,
} from "./types";
import { connectionError, type ConnectionError } from "./connection";

export type GameplayRequest =
  | { operation: "state" | "hello" | "renew_control" | "clear" }
  | { operation: "events"; after: number }
  | { operation: "pixels"; color: string; brightness: number }
  | { operation: "buzzer"; frequency_hz: number; duration_ms: number };
export type GameplayTransport = (
  handle: number,
  operation: GameplayRequest,
) => Promise<unknown>;

const healthSchema = z.object({
  state: z.enum(["ready", "safe_state"]),
  uptime_ms: z.number().nonnegative(),
});
const helloSchema = z.object({
  protocol: z.literal("1.0"),
  board: z.object({ board_id: z.string() }),
  health: healthSchema,
  capabilities: z.record(z.string(), z.object({ health: z.string() })),
});
const stateSchema = z.object({
  health: healthSchema,
  observables: z.object({
    distance: z.object({
      value: z.number().nullable(),
      health: z.string(),
      age_ms: z.number().nonnegative().nullable(),
    }),
    buttons: z.object({
      value: z.array(z.boolean()).length(3).nullable(),
      health: z.string(),
      age_ms: z.number().nonnegative().nullable(),
    }),
  }),
  outputs: z.object({
    pixels: z.union([
      z.literal("off"),
      z.object({
        color: z.string().regex(/^#[0-9a-f]{6}$/i),
        brightness: z.number().min(0).max(0.5),
      }),
    ]),
    buzzer: z.union([
      z.literal("off"),
      z.object({ frequency_hz: z.number(), duration_ms: z.number() }),
    ]),
    matrix: z.union([
      z.literal("clear"),
      z.object({ rows: z.array(z.string()) }),
    ]),
  }),
});
const eventsSchema = z.object({
  events: z.array(
    z.object({
      sequence: z.number().int().positive(),
      monotonic_ms: z.number().nonnegative(),
      type: z.string(),
      health: z.string(),
      payload: z.record(z.string(), z.unknown()),
    }),
  ),
});

export class ConnectedBoardClient implements BoardClient {
  private listeners = new Set<(event: BoardEvent) => void>();
  private timer: ReturnType<typeof setInterval> | undefined;
  private polling = false;
  private cursor = 0;
  private hello: z.infer<typeof helloSchema> | null = null;
  private helloAt = 0;
  private uptime = 0;
  private uptimeAt = 0;
  private failed = false;

  private constructor(
    private handle: number,
    private boardId: string,
    private transport: GameplayTransport,
    private onFailure: (error: ConnectionError) => void,
  ) {}

  static async open(
    handle: number,
    boardId: string,
    transport: GameplayTransport,
    onFailure: (error: ConnectionError) => void,
  ) {
    const client = new ConnectedBoardClient(
      handle,
      boardId,
      transport,
      onFailure,
    );
    await client.getSnapshot();
    // Never replay previously queued button presses as fresh arm commands on connect/reconnect.
    const baseline = eventsSchema.parse(
      await client.request({ operation: "events", after: 0 }),
    );
    client.cursor = baseline.events.at(-1)?.sequence ?? 0;
    return client;
  }

  private async request(operation: GameplayRequest) {
    if (this.failed)
      throw {
        code: "disconnected",
        message:
          "This connection is no longer active. Reconnect the selected Board.",
      };
    try {
      return await this.transport(this.handle, operation);
    } catch (cause) {
      if (connectionError(cause).code === "command_rejected") {
        // A bounded command rejection is not expired authentication or lost
        // transport. Stop running logic, but preserve reads and explicit retry.
        this.emit({ type: "health.changed", health: "safe_state" });
      } else {
        this.fail(cause);
      }
      throw cause;
    }
  }

  private fail(cause: unknown) {
    if (this.failed) return;
    this.failed = true;
    clearInterval(this.timer);
    this.emit({ type: "health.changed", health: "disconnected" });
    this.onFailure(connectionError(cause));
  }

  async getSnapshot(): Promise<BoardSnapshot> {
    try {
      if (!this.hello || performance.now() - this.helloAt > 1000) {
        this.hello = helloSchema.parse(
          await this.request({ operation: "hello" }),
        );
        this.helloAt = performance.now();
        if (this.hello.board.board_id !== this.boardId)
          throw {
            code: "identity_mismatch",
            message: "The Companion identity changed. Recover through USB.",
          };
      }
      const requestStarted = performance.now();
      const state = stateSchema.parse(
        await this.request({ operation: "state" }),
      );
      if (state.health.uptime_ms < this.uptime)
        throw {
          code: "board_restarted",
          message:
            "The Board restarted. Reconnect, rerun live checks, and arm again.",
        };
      this.uptime = state.health.uptime_ms;
      // Count the entire request time conservatively; wire age alone excludes transit/IPC delay.
      this.uptimeAt = requestStarted;
      const transitMs = Math.max(0, performance.now() - requestStarted);
      const capabilities = (
        ["buttons", "distance", "pixels", "buzzer", "matrix"] as Capability[]
      ).filter((name) =>
        name === "buttons" || name === "distance"
          ? state.observables[name].health === "operational" &&
            state.observables[name].value !== null &&
            (name !== "distance" ||
              (state.observables.distance.value! >= 20 &&
                state.observables.distance.value! <= 4000)) &&
            state.observables[name].age_ms !== null &&
            state.observables[name].age_ms + transitMs <= 750
          : this.hello?.capabilities[name]?.health === "operational",
      );
      return {
        mode: "connected",
        health: state.health.state,
        capabilities,
        capabilityHealth: Object.fromEntries(
          (
            [
              "buttons",
              "distance",
              "pixels",
              "buzzer",
              "matrix",
            ] as Capability[]
          ).map((name) => {
            const input =
              name === "buttons" || name === "distance"
                ? state.observables[name]
                : null;
            return [
              name,
              {
                health: capabilities.includes(name)
                  ? "operational"
                  : input?.health === "operational"
                    ? input.value === null ||
                      (name === "distance" &&
                        (state.observables.distance.value! < 20 ||
                          state.observables.distance.value! > 4000))
                      ? "degraded"
                      : "stale"
                    : (input?.health ??
                      this.hello?.capabilities[name]?.health ??
                      "unknown"),
                ageMs:
                  input?.age_ms === null || !input
                    ? null
                    : input.age_ms + transitMs,
              },
            ];
          }),
        ),
        observables: {
          distanceMillimeters: state.observables.distance.value ?? Number.NaN,
          distanceAgeMs:
            (state.observables.distance.age_ms ?? Number.POSITIVE_INFINITY) +
            transitMs,
          buttons: state.observables.buttons.value ?? [false, false, false],
        },
        outputs: {
          pixels: state.outputs.pixels === "off" ? null : state.outputs.pixels,
          buzzer:
            state.outputs.buzzer === "off"
              ? null
              : {
                  frequencyHz: state.outputs.buzzer.frequency_hz,
                  durationMs: state.outputs.buzzer.duration_ms,
                },
          matrix:
            state.outputs.matrix === "clear" ? null : state.outputs.matrix.rows,
        },
      };
    } catch (cause) {
      this.fail(cause);
      throw cause;
    }
  }

  subscribe(listener: (event: BoardEvent) => void): () => void {
    this.listeners.add(listener);
    if (!this.timer && !this.failed) {
      this.timer = setInterval(() => {
        if (this.polling) return;
        this.polling = true;
        void this.pollEvents()
          .catch((cause) => this.fail(cause))
          .finally(() => {
            this.polling = false;
          });
      }, 250);
    }
    return () => {
      this.listeners.delete(listener);
      if (this.listeners.size === 0) {
        clearInterval(this.timer);
        this.timer = undefined;
      }
    };
  }

  async pollEvents() {
    const result = eventsSchema.parse(
      await this.request({ operation: "events", after: this.cursor }),
    );
    for (const event of result.events) {
      if (event.sequence <= this.cursor) continue;
      if (event.sequence !== this.cursor + 1)
        throw {
          code: "event_gap",
          message:
            "Some input events were lost. Reconnect and repeat the live check.",
        };
      this.cursor = event.sequence;
      const age =
        this.uptime + performance.now() - this.uptimeAt - event.monotonic_ms;
      if (event.type === "button.changed") {
        if (event.health !== "operational" || age > 750)
          throw {
            code: "stale_event",
            message:
              "A delayed button event was discarded. Reconnect before arming.",
          };
        const payload = z
          .object({
            button: z.number().int().min(0).max(2),
            pressed: z.boolean(),
          })
          .parse(event.payload);
        this.emit({ type: "button.changed", ...payload });
      } else if (
        event.type === "distance.changed" &&
        event.health === "operational"
      ) {
        const payload = z
          .object({ millimeters: z.number().min(0).max(5000) })
          .parse(event.payload);
        this.emit({ type: "distance.changed", ...payload });
      } else if (
        event.type === "board.safe_state" &&
        event.payload.reason !== "client_requested" &&
        event.payload.reason !== "self_test_complete"
      ) {
        this.emit({ type: "health.changed", health: "safe_state" });
      } else if (
        event.type === "capability.health_changed" &&
        event.health !== "operational"
      ) {
        this.emit({ type: "health.changed", health: "safe_state" });
      }
    }
  }

  async command(command: BoardCommand) {
    if (command.operation === "outputs.clear") {
      await this.releaseControl();
      return;
    }
    if (command.operation === "pixels.set")
      await this.request({ operation: "pixels", ...command.arguments });
    else
      await this.request({
        operation: "buzzer",
        frequency_hz: command.arguments.frequencyHz,
        duration_ms: command.arguments.durationMs,
      });
  }
  async renewControl() {
    await this.request({ operation: "renew_control" });
  }
  async releaseControl() {
    if (this.failed) return; // Lost control is handled by the Board's watchdog, never replayed later.
    await this.request({ operation: "clear" });
  }
  close() {
    clearInterval(this.timer);
    this.timer = undefined;
    this.listeners.clear();
    this.failed = true;
  }
  private emit(event: BoardEvent) {
    for (const listener of this.listeners) listener(event);
  }
}
