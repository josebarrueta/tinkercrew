import { expect, it, vi } from "vitest";
import { SimulatorBoardClient } from "../board/simulator";
import { CreationRuntime } from "./runtime";

it("lets brief light outputs expire while a Creation keeps its control lease", async () => {
  vi.useFakeTimers();
  const board = new SimulatorBoardClient();
  const runtime = new CreationRuntime(board);
  try {
    await runtime.start({
      rules: [
        {
          trigger: "A",
          thresholdMm: 200,
          conditionMm: null,
          color: "#4986ff",
          chirp: false,
        },
      ],
    });
    board.controls.pressButton(0);
    await runtime.tick();
    for (let i = 0; i < 7; i++) {
      vi.advanceTimersByTime(250);
      await runtime.tick();
    }
    expect(runtime.getStatus().state).toBe("running");
    expect((await board.getSnapshot()).outputs.pixels).toBeNull();
  } finally {
    await runtime.stop();
    vi.useRealTimers();
  }
});

it("an explicit global Stop clears reported outputs even without a running Creation", async () => {
  const board = new SimulatorBoardClient();
  const runtime = new CreationRuntime(board);
  await board.renewControl();
  await board.command({
    operation: "pixels.set",
    arguments: { color: "#4986ff", brightness: 0.25 },
  });
  await runtime.stop(true);
  expect((await board.getSnapshot()).outputs.pixels).toBeNull();
});

it("cancels a start awaiting its lease and never emits a late effect after Stop", async () => {
  const simulator = new SimulatorBoardClient();
  let unblock!: () => void, entered!: () => void;
  const leasePending = new Promise<void>((resolve) => {
    entered = resolve;
  });
  const blocked = new Promise<void>((resolve) => {
    unblock = resolve;
  });
  const board = {
    getSnapshot: simulator.getSnapshot.bind(simulator),
    subscribe: simulator.subscribe.bind(simulator),
    command: simulator.command.bind(simulator),
    releaseControl: simulator.releaseControl.bind(simulator),
    renewControl: async () => {
      await simulator.renewControl();
      entered();
      await blocked;
    },
  };
  const runtime = new CreationRuntime(board);
  const starting = runtime.start({
    rules: [
      {
        trigger: "A",
        thresholdMm: 200,
        conditionMm: null,
        color: "#4986ff",
        chirp: false,
      },
    ],
  });
  await leasePending;
  const stopping = runtime.stop();
  unblock();
  await Promise.all([starting, stopping]);
  simulator.controls.pressButton(0);
  await runtime.tick();
  expect(runtime.getStatus().state).toBe("stopped");
  expect((await simulator.getSnapshot()).outputs.pixels).toBeNull();
});

it("Stop during a sensor read prevents any later lease renewal or output command", async () => {
  const simulator = new SimulatorBoardClient();
  let defer = false;
  let entered!: () => void, unblock!: () => void;
  const reading = new Promise<void>((resolve) => {
    entered = resolve;
  });
  const blocked = new Promise<void>((resolve) => {
    unblock = resolve;
  });
  const effects: string[] = [];
  const board = {
    getSnapshot: async () => {
      if (defer) {
        entered();
        await blocked;
      }
      return simulator.getSnapshot();
    },
    subscribe: simulator.subscribe.bind(simulator),
    releaseControl: simulator.releaseControl.bind(simulator),
    renewControl: async () => {
      effects.push("lease");
      await simulator.renewControl();
    },
    command: async (command: Parameters<typeof simulator.command>[0]) => {
      effects.push(command.operation);
      await simulator.command(command);
    },
  };
  const runtime = new CreationRuntime(board);
  await runtime.start({
    rules: [
      {
        trigger: "A",
        thresholdMm: 200,
        conditionMm: null,
        color: "#4986ff",
        chirp: true,
      },
    ],
  });
  effects.length = 0;
  defer = true;
  simulator.controls.pressButton(0);
  const ticking = runtime.tick();
  await reading;
  const stopping = runtime.stop();
  unblock();
  await Promise.all([ticking, stopping]);
  expect(effects).toEqual([]);
  expect(runtime.getStatus().state).toBe("stopped");
});

it("a stale Distance input stops only programs depending on it, with explicit Run needed for recovery", async () => {
  vi.useFakeTimers();
  const board = new SimulatorBoardClient();
  const runtime = new CreationRuntime(board);
  try {
    board.controls.freezeDistance(true);
    vi.advanceTimersByTime(800);
    await runtime.start({
      rules: [
        {
          trigger: "A",
          thresholdMm: 200,
          conditionMm: null,
          color: "#4986ff",
          chirp: false,
        },
      ],
    });
    board.controls.pressButton(0);
    await runtime.tick();
    expect((await board.getSnapshot()).outputs.pixels?.color).toBe("#4986ff");
    await runtime.start({
      rules: [
        {
          trigger: "near",
          thresholdMm: 200,
          conditionMm: null,
          color: "#ef674f",
          chirp: false,
        },
      ],
    });
    expect(runtime.getStatus().state).toBe("fault");
    board.controls.freezeDistance(false);
    await runtime.tick();
    expect(runtime.getStatus().state).toBe("fault");
    expect((await board.getSnapshot()).outputs.pixels).toBeNull();
  } finally {
    await runtime.stop();
    vi.useRealTimers();
  }
});

it("rejects fresh-looking but out-of-range Distance before control starts", async () => {
  const simulator = new SimulatorBoardClient();
  const board = {
    getSnapshot: async () => {
      const value = await simulator.getSnapshot();
      value.observables.distanceMillimeters = -50;
      return value;
    },
    subscribe: simulator.subscribe.bind(simulator),
    renewControl: simulator.renewControl.bind(simulator),
    releaseControl: simulator.releaseControl.bind(simulator),
    command: simulator.command.bind(simulator),
  };
  const runtime = new CreationRuntime(board);
  await runtime.start({
    rules: [
      {
        trigger: "near",
        thresholdMm: 200,
        conditionMm: null,
        color: "#4986ff",
        chirp: false,
      },
    ],
  });
  expect(runtime.getStatus().state).toBe("fault");
  expect((await simulator.getSnapshot()).outputs.pixels).toBeNull();
});

it("combines rules and conditions with later matching actions winning per output", async () => {
  vi.useFakeTimers();
  const board = new SimulatorBoardClient();
  const runtime = new CreationRuntime(board);
  try {
    await runtime.start({
      rules: [
        {
          trigger: "A",
          thresholdMm: 200,
          conditionMm: null,
          color: "#4986ff",
          chirp: false,
        },
        {
          trigger: "A",
          thresholdMm: 200,
          conditionMm: 200,
          color: "#ef674f",
          chirp: true,
        },
      ],
    });
    board.controls.pressButton(0);
    await runtime.tick();
    expect((await board.getSnapshot()).outputs.pixels?.color).toBe("#4986ff");
    expect((await board.getSnapshot()).outputs.buzzer).toBeNull();
    vi.advanceTimersByTime(600);
    board.controls.setDistance(80);
    board.controls.pressButton(0);
    await runtime.tick();
    expect((await board.getSnapshot()).outputs.pixels?.color).toBe("#ef674f");
    expect((await board.getSnapshot()).outputs.buzzer?.durationMs).toBe(250);
  } finally {
    await runtime.stop();
    vi.useRealTimers();
  }
});

it("runs a button-to-light Creation only after explicit Run; C stops it", async () => {
  const board = new SimulatorBoardClient();
  const runtime = new CreationRuntime(board);
  board.controls.pressButton(0);
  await runtime.tick();
  expect((await board.getSnapshot()).outputs.pixels).toBeNull();
  await runtime.start({
    rules: [
      {
        trigger: "A",
        thresholdMm: 200,
        conditionMm: null,
        color: "#4986ff",
        chirp: false,
      },
    ],
  });
  board.controls.pressButton(0);
  await runtime.tick();
  expect((await board.getSnapshot()).outputs.pixels?.color).toBe("#4986ff");
  board.controls.pressButton(2);
  await runtime.tick();
  await runtime.stop();
  expect((await board.getSnapshot()).outputs.pixels).toBeNull();
  expect(runtime.getStatus().state).toBe("stopped");
});
