import { z } from "zod";
import {
  checks,
  roles,
  tracks,
  type Check,
  type Mission,
  type Track,
} from "./definition";

const note = z.string().trim().min(1).max(2000);
const crewMember = z.strictObject({
  name: z.string().trim().min(1).max(60),
  role: z.enum(roles),
  track: z.enum(tracks),
});
export const progressSchema = z.strictObject({
  schemaVersion: z.literal(1),
  missionId: z.string(),
  missionVersion: z.number().int(),
  stageIndex: z.number().int().min(0),
  crew: z.array(crewMember).length(3),
  completedStages: z.array(z.string()),
  hints: z.record(z.string(), z.number().int().min(0).max(3)),
  remix: note.nullable(),
  reflection: z.strictObject({ changed: note, failed: note }).nullable(),
  completion: z.strictObject({ completedAt: z.string().datetime() }).nullable(),
});
export type MissionProgress = z.infer<typeof progressSchema>;

export class MissionSession {
  private progress: MissionProgress;
  private confirmed = false;
  private evidence = new Set<Check>();

  constructor(
    readonly mission: Mission,
    saved?: unknown,
  ) {
    this.progress =
      saved === undefined
        ? {
            schemaVersion: 1,
            missionId: mission.id,
            missionVersion: mission.version,
            stageIndex: 0,
            crew: roles.map((role, index) => ({
              name: `Maker ${index + 1}`,
              role,
              track: tracks[index],
            })),
            completedStages: [],
            hints: {},
            remix: null,
            reflection: null,
            completion: null,
          }
        : progressSchema.parse(saved);
    const p = this.progress;
    if (
      p.missionId !== mission.id ||
      p.missionVersion !== mission.version ||
      p.stageIndex >= mission.stages.length ||
      (p.completion !== null &&
        (p.stageIndex !== mission.stages.length - 1 ||
          !p.remix ||
          !p.reflection)) ||
      new Set(p.crew.map((member) => member.role)).size !== 3 ||
      p.completedStages.join(",") !==
        mission.stages
          .slice(0, p.completion ? mission.stages.length : p.stageIndex)
          .map((stage) => stage.id)
          .join(",")
    ) {
      throw new Error(
        "Saved Mission progress is incompatible; keep the save and start a new adventure separately.",
      );
    }
  }

  get stage() {
    return this.mission.stages[this.progress.stageIndex];
  }
  getProgress(): MissionProgress {
    return structuredClone(this.progress);
  }

  setCrew(members: { name: string; track: Track }[]) {
    this.progress.crew = z
      .array(crewMember)
      .length(3)
      .parse(
        members.map((member, index) => ({
          ...member,
          role: this.progress.crew[index]?.role,
        })),
      );
  }

  rotateRoles() {
    this.progress.crew = this.progress.crew.map((member) => ({
      ...member,
      role: roles[(roles.indexOf(member.role) + 1) % roles.length],
    }));
  }

  revealHint(): string {
    const level = Math.min(3, (this.progress.hints[this.stage.id] ?? 0) + 1);
    this.progress.hints[this.stage.id] = level;
    return this.stage.hints[level - 1];
  }

  confirm() {
    if (this.stage.condition.kind !== "human")
      throw new Error(
        "This Stage needs observed evidence, not human confirmation.",
      );
    this.confirmed = true;
  }

  observe(check: Check, passed: boolean) {
    z.enum(checks).parse(check);
    if (passed) this.evidence.add(check);
    else this.evidence.delete(check);
  }

  recordRemix(description: string) {
    this.progress.remix = note.parse(description);
  }
  reflect(changed: string, failed: string) {
    this.progress.reflection = {
      changed: note.parse(changed),
      failed: note.parse(failed),
    };
  }

  advance(now = new Date()) {
    if (this.progress.completion) return;
    const condition = this.stage.condition;
    if (condition.kind === "human" && !this.confirmed)
      throw new Error("Confirm this creative step together first.");
    if (condition.kind === "observable" && !this.evidence.has(condition.check))
      throw new Error(
        "Complete the live hardware check first. You can retry at any time.",
      );
    if (condition.kind === "remix" && !this.progress.remix)
      throw new Error("Test and describe at least one creative change first.");
    if (condition.kind === "reflection" && !this.progress.reflection)
      throw new Error("Explain what changed and what failed first.");
    this.progress.completedStages.push(this.stage.id);
    this.confirmed = false;
    this.evidence.clear();
    if (this.progress.stageIndex < this.mission.stages.length - 1)
      this.progress.stageIndex++;
    else this.progress.completion = { completedAt: now.toISOString() };
  }
}
