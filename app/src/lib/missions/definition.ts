import { z } from "zod";

export const tracks = ["Explorer", "Inventor", "Hacker"] as const;
export const roles = ["Storykeeper", "Builder", "Logic keeper"] as const;
export const checks = [
  "self-test",
  "zones",
  "signals",
  "buttons",
  "behavior",
] as const;
const text = z.string().trim().min(1).max(2000);
const capability = z.enum([
  "buttons",
  "distance",
  "pixels",
  "buzzer",
  "matrix",
]);

export const missionSchema = z
  .strictObject({
    schemaVersion: z.literal(1),
    id: z.string().regex(/^[a-z][a-z0-9-]{0,63}$/),
    version: z.literal(1),
    world: text,
    title: text,
    narrative: text,
    tracks: z.strictObject({ Explorer: text, Inventor: text, Hacker: text }),
    components: z.strictObject({
      required: z.array(capability).min(1),
      optional: z.array(capability),
    }),
    stages: z
      .array(
        z.strictObject({
          id: z.string().regex(/^[a-z][a-z0-9-]{0,63}$/),
          title: text,
          goal: text,
          condition: z.discriminatedUnion("kind", [
            z.strictObject({ kind: z.literal("human") }),
            z.strictObject({
              kind: z.literal("observable"),
              check: z.enum(checks),
            }),
            z.strictObject({ kind: z.literal("remix") }),
            z.strictObject({ kind: z.literal("reflection") }),
          ]),
          hints: z.tuple([text, text, text]),
        }),
      )
      .min(1)
      .max(30),
    remixPrompts: z.array(text).min(1),
    freeBuild: z
      .array(
        z.strictObject({
          prompt: text,
          components: z.array(capability).min(1),
          constraint: text,
        }),
      )
      .min(1),
  })
  .superRefine((mission, ctx) => {
    const ids = mission.stages.map((stage) => stage.id);
    if (new Set(ids).size !== ids.length) {
      ctx.addIssue({
        code: "custom",
        path: ["stages"],
        message: "Stage IDs must be unique",
      });
    }
  });

export type Mission = z.infer<typeof missionSchema>;
export type Track = (typeof tracks)[number];
export type Role = (typeof roles)[number];
export type Check = (typeof checks)[number];

export function loadMission(input: unknown): Mission {
  const result = missionSchema.safeParse(input);
  if (!result.success) {
    throw new Error(
      `Invalid Mission definition: ${result.error.issues.map((issue) => `${issue.path.join(".")}: ${issue.message}`).join("; ")}`,
    );
  }
  return result.data;
}
