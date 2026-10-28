import type { Program } from "./definition";

// In-memory UI work only: never Board clients, tokens, keys or execution state.
// Keeps a deliberate legacy detour reversible without changing saved formats.
type Draft = {
  program: Program;
  title: string;
  id: string | null;
  unsaved: boolean;
  view: "play" | "blocks";
};
let returning: Draft | null = null;
export function rememberDraft(draft: Draft) {
  returning = { ...draft, program: structuredClone(draft.program) };
}
export function takeDraft(): Draft | null {
  const draft = returning;
  returning = null;
  return draft;
}
