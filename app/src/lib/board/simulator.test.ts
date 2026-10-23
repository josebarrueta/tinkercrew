import { describe, expect, it } from "vitest";
import { SimulatorBoardClient } from "./simulator";

describe("SimulatorBoardClient", () => {
  it("reports the same capabilities expected from a connected Board", async () => {
    const board = new SimulatorBoardClient();

    await expect(board.getSnapshot()).resolves.toMatchObject({
      mode: "simulator",
      health: "ready",
      capabilities: ["buttons", "distance", "pixels", "buzzer", "matrix"],
    });
  });

  it("delivers simulated input through the Board event interface", () => {
    const board = new SimulatorBoardClient();
    const events: string[] = [];
    const unsubscribe = board.subscribe((event) => events.push(event.type));

    board.controls.setDistance(280);
    board.controls.pressButton(1);
    unsubscribe();
    board.controls.setDistance(500);

    expect(events).toEqual([
      "distance.changed",
      "button.changed",
      "button.changed",
    ]);
    expect(() => board.controls.setDistance(Number.NaN)).toThrow("finite");
  });

  it("applies bounded output commands to visible Board state", async () => {
    const board = new SimulatorBoardClient();

    await board.command({
      operation: "pixels.set",
      arguments: { color: "#ffb703", brightness: 0.4 },
    });

    expect((await board.getSnapshot()).outputs.pixels).toEqual({
      color: "#ffb703",
      brightness: 0.4,
    });
    await expect(
      board.command({
        operation: "pixels.set",
        arguments: { color: "#ffffff", brightness: 0.8 },
      }),
    ).rejects.toThrow("brightness");
    await expect(
      board.command({
        operation: "buzzer.play",
        arguments: { frequencyHz: Number.NaN, durationMs: 500 },
      }),
    ).rejects.toThrow("frequency");
  });
});
