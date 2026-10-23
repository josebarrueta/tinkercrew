import { describe, expect, it } from "vitest";
import { connectBoard, type ConnectionBackend } from "./connection";

describe("selected Board connection ladder", () => {
  it("tries parent-approved neighbors before USB and manual entry last", async () => {
    const stages: string[] = [];
    const backend: ConnectionBackend = {
      async candidates(_id, stage) {
        stages.push(stage);
        return [];
      },
      async probe(id, candidate) {
        expect(id).toBe("chosen-board");
        expect(candidate.host).toBe("dragon.local");
        return 3;
      },
    };
    await expect(
      connectBoard(backend, {
        boardId: "chosen-board",
        localConsent: true,
        manual: { host: "dragon.local", port: 7443 },
      }),
    ).resolves.toEqual({ handle: 3, transport: "manual" });
    expect(stages).toEqual(["known", "mdns", "local", "usb"]);
  });

  it.each(["authentication_failed", "maintenance_active"])(
    "does not hunt for another Board after %s",
    async (code) => {
      const states: string[] = [];
      const backend: ConnectionBackend = {
        async candidates(_id, stage) {
          states.push(stage);
          return [{ host: "192.168.1.2", port: 7443 }];
        },
        async probe() {
          throw {
            code,
            message: "Pause gameplay and recover through Board Setup",
          };
        },
      };
      await expect(
        connectBoard(backend, { boardId: "chosen", localConsent: true }),
      ).rejects.toMatchObject({ code });
      expect(states).toEqual(["known"]);
    },
  );
  it("falls back in order without scanning neighbors without consent", async () => {
    const attempts: string[] = [];
    const backend: ConnectionBackend = {
      async candidates(boardId, stage) {
        attempts.push(`${boardId}:${stage}`);
        if (stage === "mdns")
          throw { code: "mdns_unavailable", message: "Blocked multicast" };
        return [{ host: stage, port: 7443 }];
      },
      async probe(_boardId, candidate) {
        attempts.push(`probe:${candidate.host}`);
        if (candidate.host !== "usb")
          throw { code: "network_unavailable", message: "No Wi-Fi" };
        return 7;
      },
    };
    const result = await connectBoard(backend, {
      boardId: "selected",
      localConsent: false,
    });
    expect(result).toEqual({ handle: 7, transport: "usb" });
    expect(attempts).toEqual([
      "selected:known",
      "probe:known",
      "selected:mdns",
      "selected:usb",
      "probe:usb",
    ]);
  });
});
