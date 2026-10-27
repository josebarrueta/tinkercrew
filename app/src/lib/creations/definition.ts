import { z } from "zod";

export const ruleSchema = z
  .strictObject({
    trigger: z.enum(["A", "B", "near", "far"]),
    thresholdMm: z.number().int().min(20).max(4000),
    conditionMm: z.number().int().min(20).max(4000).nullable(),
    color: z
      .string()
      .regex(/^#[0-9a-f]{6}$/i)
      .nullable(),
    chirp: z.boolean(),
  })
  .refine(
    (rule) => rule.color !== null || rule.chirp,
    "Choose at least one output.",
  );
export const programSchema = z.strictObject({
  rules: z.array(ruleSchema).min(1).max(8),
});
export type Program = z.infer<typeof programSchema>;
export type Rule = Program["rules"][number];
export const newRule = (): Rule => ({
  trigger: "A",
  thresholdMm: 200,
  conditionMm: null,
  color: "#4986ff",
  chirp: false,
});
export const guardianStarter = (): Program => ({
  rules: [
    {
      trigger: "near",
      thresholdMm: 200,
      conditionMm: null,
      color: "#ef674f",
      chirp: false,
    },
    {
      trigger: "far",
      thresholdMm: 200,
      conditionMm: null,
      color: "#22c55e",
      chirp: false,
    },
  ],
});
export const librarySchema = z
  .strictObject({
    version: z.literal(1),
    creations: z
      .array(
        z.strictObject({
          id: z.string().uuid(),
          title: z.string().trim().min(1).max(60),
          program: programSchema,
        }),
      )
      .max(24),
  })
  .refine(
    (library) =>
      new Set(library.creations.map((item) => item.id)).size ===
      library.creations.length,
    "Duplicate Creation identity.",
  );
export type Library = z.infer<typeof librarySchema>;
