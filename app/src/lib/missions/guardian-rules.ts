import { z } from "zod";

export const guardianRulesSchema = z
  .strictObject({
    intruderMm: z.number().int().min(20).max(3800),
    warningMm: z.number().int().min(40).max(4000),
    safeColor: z.string().regex(/^#[0-9a-f]{6}$/i),
    warningColor: z.string().regex(/^#[0-9a-f]{6}$/i),
    alertColor: z.string().regex(/^#[0-9a-f]{6}$/i),
    alarmMs: z.number().int().min(100).max(1000),
  })
  .refine((rules) => rules.intruderMm < rules.warningMm, {
    path: ["warningMm"],
    message: "Warning boundary must be farther than the intruder boundary",
  });
export type GuardianRules = z.infer<typeof guardianRulesSchema>;
export const defaultRules: GuardianRules = {
  intruderMm: 150,
  warningMm: 350,
  safeColor: "#22c55e",
  warningColor: "#ffb703",
  alertColor: "#ef4444",
  alarmMs: 350,
};
