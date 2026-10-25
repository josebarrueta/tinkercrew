import { describe, expect, it } from "vitest";
import { SetupSession } from "./session";

describe("resumable parent setup checkpoints", () => {
  it("resumes official-tool handoffs but cannot invent full Pairing/deployment success", () => {
    const setup = new SetupSession();
    setup.selectBoard("uno-1234");
    setup.observeUsb([{ serial: "uno-1234", state: "device" }]);
    setup.advance();
    for (let index = 1; index < 7; index++) {
      expect(() => setup.advance()).toThrow("Confirm");
      setup.confirmHumanStep();
      setup.advance();
    }
    expect(setup.step.id).toBe("pair");
    expect(() => setup.confirmHumanStep()).toThrow("native proof");
    expect(() => setup.advance()).toThrow("native proof");
    expect(new SetupSession(setup.getProgress()).step.id).toBe("pair");
    setup.startNew("another-uno");
    expect(setup.getProgress().checkpoints).toEqual([]);
  });

  it("reads native-observed checkpoints but never advances them with a checkbox", () => {
    const guide = new SetupSession();
    guide.selectBoard("uno-1234");
    guide.observeUsb([{ serial: "uno-1234", state: "device" }]);
    guide.advance();
    for (let index = 1; index < 7; index++) {
      guide.confirmHumanStep();
      guide.advance();
    }
    const progress = guide.getProgress();
    const resumed = new SetupSession({
      ...progress,
      stageIndex: 8,
      checkpoints: [
        ...progress.checkpoints,
        { stageId: "pair", source: "observed" },
      ],
    });
    expect(resumed.step.id).toBe("deploy");
    expect(() => resumed.confirmHumanStep()).toThrow("native proof");
    expect(() => resumed.advance()).toThrow("native proof");
  });

  it("rejects secret fields, impossible checkpoints, and unsupported fake completion", () => {
    const initial = new SetupSession().getProgress();
    expect(
      () => new SetupSession({ ...initial, password: "must-not-persist" }),
    ).toThrow();
    expect(() => new SetupSession({ ...initial, stageIndex: 8 })).toThrow();
    expect(() => new SetupSession({ ...initial, stageIndex: 1 })).toThrow(
      "incompatible",
    );
  });
  it("targets one explicit serial and requires fresh USB evidence after restart", () => {
    const setup = new SetupSession();
    setup.selectBoard("uno-1234");
    setup.observeUsb([{ serial: "other", state: "device" }]);
    expect(() => setup.advance()).toThrow("selected Board");
    setup.observeUsb([{ serial: "uno-1234", state: "device" }]);
    setup.advance();
    const resumed = new SetupSession(setup.getProgress());
    resumed.confirmHumanStep();
    expect(() => resumed.advance()).toThrow("fresh USB");
    expect(() => resumed.selectBoard("another-uno")).toThrow("Start a new");
  });
});
