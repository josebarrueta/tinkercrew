import { describe, expect, it, vi } from "vitest";
import { ConnectedBoardClient, type GameplayTransport } from "./connected";

const hello = {
  protocol: "1.0",
  board: { board_id: "board-one" },
  health: { state: "safe_state", uptime_ms: 1000 },
  capabilities: Object.fromEntries(
    ["distance", "buttons", "pixels", "buzzer", "matrix"].map((name) => [
      name,
      { health: "operational" },
    ]),
  ),
};
const state = {
  health: { state: "safe_state", uptime_ms: 1000 },
  observables: {
    distance: { value: 250, health: "operational", age_ms: 40 },
    buttons: {
      value: [false, false, false],
      health: "operational",
      age_ms: 30,
    },
  },
  outputs: { pixels: "off", buzzer: "off", matrix: "clear" },
};
const buttonEvent = (sequence: number) => ({
  sequence,
  monotonic_ms: 1000,
  type: "button.changed",
  health: "operational",
  payload: { button: 0, pressed: true },
});

describe("connected Companion adapter", () => {
  it("a rejected module command stops control without destroying the authenticated read connection", async () => {
    const failures: unknown[] = [];
    const operations: string[] = [];
    const client = await ConnectedBoardClient.open(
      7,
      "board-one",
      async (_handle, request) => {
        operations.push(request.operation);
        if (request.operation === "hello") return hello;
        if (request.operation === "state") return state;
        if (request.operation === "events") return { events: [] };
        throw {
          code: "command_rejected",
          message: "Another controller is using this Board.",
        };
      },
      (error) => failures.push(error),
    );
    try {
      await expect(
        client.command({
          operation: "pixels.set",
          arguments: { color: "#3366ff", brightness: 0.25 },
        }),
      ).rejects.toMatchObject({ code: "command_rejected" });
      expect((await client.getSnapshot()).mode).toBe("connected");
      expect(failures).toEqual([]);
      expect(operations).not.toContain("renew_control");
    } finally {
      client.close();
    }
  });
  it("does not advertise operational input capabilities for null or out-of-range values", async () => {
    const transport: GameplayTransport = async (_handle, request) =>
      request.operation === "hello"
        ? hello
        : request.operation === "state"
          ? {
              ...state,
              observables: {
                distance: { value: -50, health: "operational", age_ms: 0 },
                buttons: { value: null, health: "operational", age_ms: 0 },
              },
            }
          : { events: [] };
    const client = await ConnectedBoardClient.open(
      7,
      "board-one",
      transport,
      () => {},
    );
    try {
      const snapshot = await client.getSnapshot();
      expect(snapshot.capabilities).not.toContain("distance");
      expect(snapshot.capabilities).not.toContain("buttons");
      expect(snapshot.capabilities).toContain("buzzer");
    } finally {
      client.close();
    }
  });
  it("counts request/transit delay against sensor freshness instead of trusting wire age alone", async () => {
    let elapsed = 0;
    vi.spyOn(performance, "now").mockImplementation(() => elapsed);
    let client: ConnectedBoardClient | undefined;
    try {
      const transport: GameplayTransport = async (_handle, request) => {
        if (request.operation === "hello") return hello;
        if (request.operation === "state") {
          elapsed += 800;
          return state;
        }
        return { events: [] };
      };
      client = await ConnectedBoardClient.open(
        7,
        "board-one",
        transport,
        () => {},
      );
      const snapshot = await client.getSnapshot();
      expect(snapshot.observables.distanceAgeMs).toBe(840);
      expect(snapshot.capabilities).not.toContain("distance");
      expect(snapshot.capabilities).not.toContain("buttons");
    } finally {
      client?.close();
      vi.restoreAllMocks();
    }
  });
  it("maps actual Companion wire state and never replays historical arm events on reconnect", async () => {
    let events = [buttonEvent(1)];
    const transport: GameplayTransport = async (_handle, request) =>
      request.operation === "hello"
        ? hello
        : request.operation === "state"
          ? state
          : {
              events: events.filter(
                (event) =>
                  request.operation !== "events" ||
                  event.sequence > request.after,
              ),
            };
    const client = await ConnectedBoardClient.open(
      7,
      "board-one",
      transport,
      () => {},
    );
    const observed: string[] = [];
    const unsubscribe = client.subscribe((event) => observed.push(event.type));
    try {
      const snapshot = await client.getSnapshot();
      expect(snapshot).toMatchObject({
        mode: "connected",
        observables: { distanceMillimeters: 250 },
      });
      expect(snapshot.observables.distanceAgeMs).toBeGreaterThanOrEqual(40);
      await client.pollEvents();
      expect(observed).toEqual([]);
      events = [...events, buttonEvent(2)];
      await client.pollEvents();
      expect(observed).toEqual(["button.changed"]);
    } finally {
      unsubscribe();
      client.close();
    }
  });

  it("rejects input gaps rather than silently accepting a later arm command", async () => {
    let events: ReturnType<typeof buttonEvent>[] = [];
    const transport: GameplayTransport = async (_handle, request) =>
      request.operation === "hello"
        ? hello
        : request.operation === "state"
          ? state
          : { events };
    const client = await ConnectedBoardClient.open(
      7,
      "board-one",
      transport,
      () => {},
    );
    try {
      events = [buttonEvent(12)];
      await expect(client.pollEvents()).rejects.toMatchObject({
        code: "event_gap",
      });
    } finally {
      client.close();
    }
  });

  it("fails closed on a changed identity or missing freshness fields", async () => {
    const transport: GameplayTransport = async (_handle, request) =>
      request.operation === "hello" ? hello : state;
    await expect(
      ConnectedBoardClient.open(7, "different-board", transport, () => {}),
    ).rejects.toMatchObject({ code: "identity_mismatch" });
    const obsolete: GameplayTransport = async (_handle, request) =>
      request.operation === "hello"
        ? hello
        : {
            ...state,
            observables: {
              ...state.observables,
              distance: { value: 250, health: "operational" },
            },
          };
    await expect(
      ConnectedBoardClient.open(7, "board-one", obsolete, () => {}),
    ).rejects.toThrow();
  });
});
