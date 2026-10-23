import { describe, expect, it } from "vitest";
import guardian from "./treasure-guardian.v1.json";
import { loadMission } from "./definition";

describe("versioned Mission definitions", () => {
  it("rejects unsupported versions, duplicate stages, and malformed hints with useful paths", () => {
    expect(() => loadMission({ ...guardian, schemaVersion: 2 })).toThrow(
      "schemaVersion",
    );
    expect(() =>
      loadMission({
        ...guardian,
        stages: [guardian.stages[0], guardian.stages[0]],
      }),
    ).toThrow("unique");
    expect(() =>
      loadMission({
        ...guardian,
        stages: [{ ...guardian.stages[0], hints: [] }],
      }),
    ).toThrow("stages.0.hints");
  });
  it("loads Treasure Guardian with three tracks and creative confirmation", () => {
    const mission = loadMission(guardian);
    expect(mission.id).toBe("treasure-guardian");
    expect(Object.keys(mission.tracks)).toEqual([
      "Explorer",
      "Inventor",
      "Hacker",
    ]);
    expect(
      mission.stages.find((stage) => stage.id === "build")?.condition,
    ).toEqual({
      kind: "human",
    });
  });
});
