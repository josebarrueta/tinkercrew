import { invoke, isTauri } from "@tauri-apps/api/core";
import { librarySchema, type Library } from "./definition";
const namespace = "creations-v1";
const browserKey = "tinkercrew.creations.v1";
/** Separate, versioned namespace; never interprets or migrates old Mission saves. */
export class CreationRepository {
  private revision = 0;
  private loaded = false;
  private pending: Promise<unknown> = Promise.resolve();
  async load(): Promise<Library> {
    this.loaded = false;
    const saved = isTauri()
      ? await invoke<{ payload: string; revision: number } | null>(
          "load_progress",
          { missionId: namespace },
        )
      : JSON.parse(localStorage.getItem(browserKey) ?? "null");
    if (
      saved &&
      (!Number.isSafeInteger(saved.revision) ||
        saved.revision < 1 ||
        typeof saved.payload !== "string")
    )
      throw new Error(
        "Saved creations could not be read. Existing work was not replaced.",
      );
    const library = librarySchema.parse(
      saved ? JSON.parse(saved.payload) : { version: 1, creations: [] },
    );
    this.revision = saved?.revision ?? 0;
    this.loaded = true;
    return library;
  }
  save(library: Library): Promise<void> {
    const payload = JSON.stringify(librarySchema.parse(library));
    if (new TextEncoder().encode(payload).length > 128 * 1024)
      return Promise.reject(
        new Error("Creation library is full. Existing work was not replaced."),
      );
    const operation = this.pending
      .catch(() => {})
      .then(async () => {
        if (!this.loaded)
          throw new Error(
            "Load saved creations before saving. Existing work was not replaced.",
          );
        if (isTauri())
          this.revision = await invoke<number>("save_progress", {
            missionId: namespace,
            payload,
            expectedRevision: this.revision,
          });
        else {
          const saved = JSON.parse(localStorage.getItem(browserKey) ?? "null");
          if ((saved?.revision ?? 0) !== this.revision)
            throw new Error(
              "Creations changed in another window. Reload before saving; neither copy was overwritten.",
            );
          const revision = this.revision + 1;
          localStorage.setItem(
            browserKey,
            JSON.stringify({ payload, revision }),
          );
          this.revision = revision;
        }
      });
    this.pending = operation;
    return operation;
  }
}
