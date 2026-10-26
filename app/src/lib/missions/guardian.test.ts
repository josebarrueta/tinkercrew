import { describe, expect, it, vi } from "vitest";
import { SimulatorBoardClient } from "../board/simulator";
import { GuardianRuntime } from "./guardian";

describe("Treasure Guardian on a Board adapter", () => {
  it("does not send new output effects when stopped during an in-flight sensor read", async () => {
    const simulator = new SimulatorBoardClient();
    const effects: string[] = [];
    let deferred = false;
    let releaseRead: (() => void) | undefined;
    let entered: (() => void) | undefined;
    const reading = new Promise<void>((resolve) => {
      entered = resolve;
    });
    const board = {
      async getSnapshot() {
        if (deferred) {
          entered!();
          await new Promise<void>((resolve) => {
            releaseRead = resolve;
          });
        }
        return simulator.getSnapshot();
      },
      subscribe: simulator.subscribe.bind(simulator),
      renewControl: simulator.renewControl.bind(simulator),
      releaseControl: simulator.releaseControl.bind(simulator),
      async command(command: Parameters<SimulatorBoardClient["command"]>[0]) {
        effects.push(command.operation);
        await simulator.command(command);
      },
    };
    const guardian = new GuardianRuntime(board);
    await guardian.start();
    effects.length = 0;
    simulator.controls.pressButton(0);
    deferred = true;
    const tick = guardian.tick();
    await reading;
    const stopping = guardian.stop();
    releaseRead!();
    await Promise.all([tick, stopping]);
    expect(effects.filter((effect) => effect !== "outputs.clear")).toEqual([]);
    expect(guardian.getStatus().armed).toBe(false);
  });
  it("requires fresh self-test inputs and human light/sound confirmation", async () => {
    const board = new SimulatorBoardClient();
    const guardian = new GuardianRuntime(board);
    await guardian.start();
    try {
      await guardian.beginSelfTest();
      await expect(guardian.confirmSelfTest(true, true)).rejects.toThrow(
        "Move",
      );
      board.controls.setDistance(300);
      for (const button of [0, 1, 2]) board.controls.pressButton(button);
      await guardian.tick();
      await expect(guardian.confirmSelfTest(true, false)).rejects.toThrow(
        "retry",
      );
      expect(guardian.getStatus().evidence["self-test"]).toBe(false);
      await guardian.confirmSelfTest(true, true);
      expect(guardian.getStatus().evidence["self-test"]).toBe(true);
    } finally {
      await guardian.stop();
    }
  });

  it("disarms on stale input, clears evidence, and never automatically rearms on recovery", async () => {
    vi.useFakeTimers();
    const board = new SimulatorBoardClient();
    const guardian = new GuardianRuntime(board);
    try {
      await guardian.start();
      board.controls.pressButton(0);
      await guardian.tick();
      board.controls.freezeDistance(true);
      vi.advanceTimersByTime(800);
      await guardian.tick();
      expect(guardian.getStatus()).toMatchObject({
        state: "safe_state",
        armed: false,
        testedRemix: false,
      });
      expect((await board.getSnapshot()).outputs.pixels).toBeNull();
      board.controls.freezeDistance(false);
      await guardian.tick();
      expect(guardian.getStatus().state).toBe("disarmed");
    } finally {
      await guardian.stop();
      vi.useRealTimers();
    }
  });

  it("requires an actually changed and tested rule for remix evidence, and fresh trials for the behavior challenge", async () => {
    const board = new SimulatorBoardClient();
    const guardian = new GuardianRuntime(board);
    await guardian.start();
    try {
      const rules = guardian.getStatus().rules;
      expect(() => guardian.updateRules({ ...rules, intruderMm: 500 })).toThrow(
        "farther",
      );
      guardian.updateRules({ ...rules, warningMm: 500 });
      expect(guardian.getStatus().testedRemix).toBe(false);
      board.controls.pressButton(0);
      for (const distance of [600, 400, 80]) {
        board.controls.setDistance(distance);
        await guardian.tick();
      }
      board.controls.pressButton(1);
      await guardian.tick();
      board.controls.pressButton(2);
      await guardian.tick();
      expect(guardian.getStatus().evidence.behavior).toBe(true);
      expect(guardian.getStatus().testedRemix).toBe(true);
      guardian.resetChallenge();
      expect(guardian.getStatus().evidence.behavior).toBe(false);
    } finally {
      await guardian.stop();
    }
  });
  it("arms and signals all three zones through the Board interface", async () => {
    const board = new SimulatorBoardClient();
    const guardian = new GuardianRuntime(board);
    await guardian.start();
    try {
      board.controls.pressButton(0);
      await guardian.tick();
      expect((await board.getSnapshot()).outputs.pixels?.color).toBe("#22c55e");
      board.controls.setDistance(250);
      await guardian.tick();
      expect((await board.getSnapshot()).outputs.pixels?.color).toBe("#ffb703");
      board.controls.setDistance(80);
      await guardian.tick();
      expect((await board.getSnapshot()).outputs).toMatchObject({
        pixels: { color: "#ef4444" },
        buzzer: { durationMs: 350 },
      });
      board.controls.pressButton(1);
      await guardian.tick();
      expect((await board.getSnapshot()).outputs.buzzer).toBeNull();
      board.controls.pressButton(2);
      await guardian.tick();
      expect((await board.getSnapshot()).outputs.pixels).toBeNull();
    } finally {
      await guardian.stop();
    }
  });
});
