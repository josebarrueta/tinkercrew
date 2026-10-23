import { describe, expect, it } from "vitest";
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
      await expect(client.getSnapshot()).resolves.toMatchObject({
        mode: "connected",
        observables: { distanceMillimeters: 250, distanceAgeMs: 40 },
      });
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
