import { invoke, isTauri } from "@tauri-apps/api/core";
import { progressSchema, type MissionProgress } from "./session";

interface SavedProgress {
  payload: string;
  revision: number;
}

/** One repository per open Mission. Serializes writes; stale windows cannot overwrite a newer save. */
export class ProgressRepository {
  private revision = 0;
  private loaded = false;
  private pending: Promise<unknown> = Promise.resolve();

  async load(missionId: string): Promise<unknown | undefined> {
    const saved: SavedProgress | null = isTauri()
      ? await invoke("load_progress", { missionId })
      : JSON.parse(
          localStorage.getItem(`tinkercrew.progress.${missionId}`) ?? "null",
        );
    if (
      saved &&
      (!Number.isSafeInteger(saved.revision) ||
        saved.revision < 1 ||
        typeof saved.payload !== "string")
    ) {
      throw new Error(
        "Saved progress is unreadable. Keep the save and ask a grown-up for help; it has not been overwritten.",
      );
    }
    const progress: unknown = saved
      ? progressSchema.parse(JSON.parse(saved.payload))
      : undefined;
    this.revision = saved?.revision ?? 0;
    this.loaded = true;
    return progress;
  }

  save(progress: MissionProgress): Promise<void> {
    // Capture the caller's snapshot now, not when a previous disk write finishes.
    const payload = JSON.stringify(progressSchema.parse(progress));
    const operation = this.pending
      .catch(() => {})
      .then(async () => {
        if (!this.loaded) throw new Error("Load progress before saving.");
        const missionId = progress.missionId;
        if (isTauri()) {
          this.revision = await invoke("save_progress", {
            missionId,
            payload,
            expectedRevision: this.revision,
          });
        } else {
          const key = `tinkercrew.progress.${missionId}`;
          const saved: SavedProgress | null = JSON.parse(
            localStorage.getItem(key) ?? "null",
          );
          if ((saved?.revision ?? 0) !== this.revision)
            throw new Error(
              "Progress changed in another window. Reload before continuing.",
            );
          const revision = this.revision + 1;
          localStorage.setItem(key, JSON.stringify({ payload, revision }));
          this.revision = revision;
        }
      });
    this.pending = operation;
    return operation;
  }
}
