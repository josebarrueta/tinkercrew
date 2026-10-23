import { describe, expect, it, vi } from "vitest";
import { SimulatorBoardClient } from "./simulator";

describe("SimulatorBoardClient", () => {
  it("expires the buzzer and clears outputs when control is lost", async () => {
    vi.useFakeTimers();
    try {
      const board = new SimulatorBoardClient();
      await board.renewControl();
      await board.command({
        operation: "buzzer.play",
        arguments: { frequencyHz: 880, durationMs: 250 },
      });
      vi.advanceTimersByTime(251);
      expect((await board.getSnapshot()).outputs.buzzer).toBeNull();
      await board.command({
        operation: "pixels.set",
        arguments: { color: "#22c55e", brightness: 0.3 },
      });
      vi.advanceTimersByTime(1500);
      expect(await board.getSnapshot()).toMatchObject({
        health: "safe_state",
        outputs: { pixels: null, buzzer: null },
      });
      await expect(
        board.command({
          operation: "pixels.set",
          arguments: { color: "#22c55e", brightness: 0.3 },
        }),
      ).rejects.toThrow("Safe state");
    } finally {
      vi.useRealTimers();
    }
  });
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
    await board.renewControl();

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
