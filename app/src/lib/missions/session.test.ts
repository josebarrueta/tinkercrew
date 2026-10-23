import { describe, expect, it } from "vitest";
import data from "./treasure-guardian.v1.json";
import { loadMission } from "./definition";
import { MissionSession } from "./session";

const mission = loadMission(data);

describe("shared Crew Mission progression", () => {
  it("rotates roles independently of challenge tracks and requires fresh live evidence after restart", () => {
    const session = new MissionSession(mission);
    session.setCrew([
      { name: "Ash", track: "Hacker" },
      { name: "Bee", track: "Explorer" },
      { name: "Cy", track: "Inventor" },
    ]);
    session.rotateRoles();
    expect(session.getProgress().crew[0]).toEqual({
      name: "Ash",
      track: "Hacker",
      role: "Builder",
    });
    for (let index = 0; index < 3; index++) {
      session.confirm();
      session.advance();
    }
    expect(() => session.confirm()).toThrow("evidence");
    session.observe("self-test", true);
    const resumed = new MissionSession(mission, session.getProgress());
    expect(() => resumed.advance()).toThrow("live hardware");
    session.advance();
    expect(session.stage.id).toBe("zones");
  });

  it("requires remix and reflection before recording completion; retries never lock the Crew out", () => {
    const session = new MissionSession(mission);
    for (const stage of mission.stages.slice(0, -2)) {
      if (stage.condition.kind === "human") session.confirm();
      if (stage.condition.kind === "observable")
        session.observe(stage.condition.check, true);
      session.advance();
    }
    expect(() => session.advance()).toThrow("creative change");
    session.recordRemix("A purple warning after testing a closer boundary.");
    session.advance();
    expect(() => session.advance()).toThrow("what failed");
    session.reflect(
      "Changed warning distance",
      "The first alarm sounded too soon",
    );
    session.advance(new Date("2026-01-01T00:00:00Z"));
    expect(session.getProgress().completion).toEqual({
      completedAt: "2026-01-01T00:00:00.000Z",
    });
    expect(
      new MissionSession(mission, session.getProgress()).getProgress()
        .completion,
    ).not.toBeNull();
  });

  it("rejects incompatible or corrupt saved progress without silently resetting it", () => {
    const session = new MissionSession(mission);
    expect(
      () =>
        new MissionSession(mission, {
          ...session.getProgress(),
          missionVersion: 2,
        }),
    ).toThrow("incompatible");
    expect(
      () =>
        new MissionSession(mission, {
          ...session.getProgress(),
          stageIndex: 5,
        }),
    ).toThrow("incompatible");
  });
  it("requires human confirmation, offers progressive hints, and resumes the current Stage", () => {
    const session = new MissionSession(mission);
    expect(() => session.advance()).toThrow("Confirm");
    expect(session.revealHint()).toBe(mission.stages[0].hints[0]);
    session.confirm();
    session.advance();
    const resumed = new MissionSession(mission, session.getProgress());
    expect(resumed.stage.id).toBe("roles");
    expect(resumed.getProgress().hints.story).toBe(1);
    expect(resumed.getProgress().crew.map((member) => member.role)).toEqual([
      "Storykeeper",
      "Builder",
      "Logic keeper",
    ]);
  });
});
