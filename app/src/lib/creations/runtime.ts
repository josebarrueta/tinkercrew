import type {
  BoardClient,
  BoardCommand,
  BoardSnapshot,
  Capability,
} from "../board/types";
import { errorMessage } from "../errors";
import { programSchema, type Program } from "./definition";

type State =
  "stopped" | "starting" | "running" | "preview" | "stopping" | "fault";
/** Mac-side bounded data interpreter, never code execution or Board deployment.
 * One serialized authority owner. Stop invalidates pending work before cleanup.
 * Later matching rules win per output. Coalesced events and a 500ms output budget
 * bound work independently of the caller's timer; no loops, paths or shell exist.
 */
export class CreationRuntime {
  private state: State = "stopped";
  private generation = 0;
  private work: Promise<void> = Promise.resolve();
  private unsubscribe: (() => void) | undefined;
  private expiry: ReturnType<typeof setTimeout> | undefined;
  private ticking = false;
  private buttons = new Set<number>();
  private previousDistance = 0;
  private lastOutput = -Infinity;
  private program: Program = { rules: [] };
  private error = "";
  private trace: string[] = [];
  constructor(private board: BoardClient) {}
  getStatus() {
    return { state: this.state, error: this.error, trace: [...this.trace] };
  }
  private log(text: string) {
    this.trace = [text, ...this.trace].slice(0, 12);
  }
  private enqueue(action: () => Promise<void>) {
    const operation = this.work.then(action);
    this.work = operation.catch(() => {});
    return operation;
  }
  private detach() {
    this.unsubscribe?.();
    this.unsubscribe = undefined;
    this.buttons.clear();
    clearTimeout(this.expiry);
  }
  private validate(snapshot: BoardSnapshot, program: Program) {
    if (snapshot.health === "disconnected")
      throw new Error("Board disconnected. Reconnect, then press Run again.");
    const need = new Set<Capability>();
    for (const rule of program.rules) {
      need.add(
        rule.trigger === "A" || rule.trigger === "B" ? "buttons" : "distance",
      );
      if (rule.conditionMm !== null) need.add("distance");
      if (rule.color) need.add("pixels");
      if (rule.chirp) need.add("buzzer");
    }
    for (const capability of need) {
      if (!snapshot.capabilities.includes(capability))
        throw new Error(
          `${capability} is unavailable or stale. Check that module or change the dependent rule, then Run again.`,
        );
    }
    if (
      need.has("distance") &&
      (!Number.isFinite(snapshot.observables.distanceMillimeters) ||
        snapshot.observables.distanceMillimeters < 20 ||
        snapshot.observables.distanceMillimeters > 4000 ||
        !Number.isFinite(snapshot.observables.distanceAgeMs) ||
        snapshot.observables.distanceAgeMs < 0 ||
        snapshot.observables.distanceAgeMs > 750)
    )
      throw new Error(
        "Distance is stale or outside its supported range. Restore fresh readings, then Run again.",
      );
  }
  start(input: unknown): Promise<void> {
    const gen = ++this.generation;
    this.detach();
    this.state = "starting";
    this.error = "";
    return this.enqueue(async () => {
      try {
        if (gen !== this.generation) return;
        await this.board.releaseControl();
        if (gen !== this.generation) return;
        this.program = programSchema.parse(input);
        const snapshot = await this.board.getSnapshot();
        if (gen !== this.generation) return;
        this.validate(snapshot, this.program);
        this.previousDistance = snapshot.observables.distanceMillimeters;
        await this.board.renewControl();
        if (gen !== this.generation) return;
        this.unsubscribe = this.board.subscribe((event) => {
          if (gen !== this.generation) return;
          if (event.type === "health.changed" && event.health !== "ready") {
            this.error =
              "Board health changed. Outputs stopped; reconnect/check modules before Run.";
            void this.stop().catch(() => {});
          }
          if (event.type === "button.changed" && event.pressed) {
            if (event.button === 2) {
              void this.stop().catch(() => {});
              return;
            }
            if (event.button === 0 || event.button === 1)
              this.buttons.add(event.button);
          }
        });
        this.state = "running";
        this.lastOutput = -Infinity;
        this.log("Running. Try an input; C or Stop ends execution.");
      } catch (cause) {
        await this.fail(cause, gen);
      }
    });
  }
  tick(): Promise<void> {
    if (this.state !== "running" || this.ticking) return this.work;
    this.ticking = true;
    const gen = this.generation;
    return this.enqueue(async () => {
      try {
        if (gen !== this.generation) return;
        const snapshot = await this.board.getSnapshot();
        if (gen !== this.generation) return;
        this.validate(snapshot, this.program);
        if (snapshot.health !== "ready")
          throw new Error(
            "Control was lost. Press Run explicitly after checking the Board.",
          );
        await this.board.renewControl();
        if (gen !== this.generation) return;
        const distance = snapshot.observables.distanceMillimeters;
        const pressed = new Set(this.buttons);
        this.buttons.clear();
        const actions = new Map<string, BoardCommand>();
        const matches: number[] = [];
        this.program.rules.forEach((rule, index) => {
          const hit =
            rule.trigger === "A"
              ? pressed.has(0)
              : rule.trigger === "B"
                ? pressed.has(1)
                : rule.trigger === "near"
                  ? this.previousDistance >= rule.thresholdMm &&
                    distance < rule.thresholdMm
                  : this.previousDistance < rule.thresholdMm &&
                    distance >= rule.thresholdMm;
          if (
            !hit ||
            (rule.conditionMm !== null && distance >= rule.conditionMm)
          )
            return;
          matches.push(index + 1);
          if (rule.color)
            actions.set("pixels", {
              operation: "pixels.set",
              arguments: { color: rule.color, brightness: 0.25 },
            });
          if (rule.chirp)
            actions.set("buzzer", {
              operation: "buzzer.play",
              arguments: { frequencyHz: 880, durationMs: 250 },
            });
        });
        this.previousDistance = distance;
        if (actions.size && Date.now() - this.lastOutput < 500) {
          this.log("Fast repeat ignored (short output cooldown).");
          return;
        }
        if (actions.size) this.lastOutput = Date.now();
        for (const action of actions.values()) {
          if (gen !== this.generation) return;
          await this.board.command(action);
          if (gen !== this.generation) return;
        }
        if (matches.length)
          this.log(
            `Rules ${matches.join(", ")} fired. Later rules win for the same output. Command accepted; confirm the physical result yourself.`,
          );
      } catch (cause) {
        await this.fail(cause, gen);
      }
    }).finally(() => {
      this.ticking = false;
    });
  }
  preview(command: BoardCommand): Promise<void> {
    const gen = ++this.generation;
    this.detach();
    this.state = "preview";
    this.error = "";
    return this.enqueue(async () => {
      try {
        if (gen !== this.generation) return;
        await this.board.releaseControl();
        if (gen !== this.generation) return;
        const snapshot = await this.board.getSnapshot();
        if (gen !== this.generation) return;
        const capability =
          command.operation === "pixels.set"
            ? "pixels"
            : command.operation === "buzzer.play"
              ? "buzzer"
              : null;
        if (
          !capability ||
          snapshot.health === "disconnected" ||
          !snapshot.capabilities.includes(capability)
        )
          throw new Error(
            "That module is unavailable. Check its connection before trying again.",
          );
        await this.board.renewControl();
        if (gen !== this.generation) return;
        await this.board.command(command);
        if (gen !== this.generation) return;
        this.log(
          "Preview command accepted. This does not prove a visible/audible physical result.",
        );
        this.expiry = setTimeout(() => {
          void this.stop().catch(() => {});
        }, 1500);
      } catch (cause) {
        await this.fail(cause, gen);
      }
    });
  }
  stop(forceClear = false): Promise<void> {
    if (this.state === "stopping" || (this.state === "stopped" && !forceClear))
      return this.work;
    const gen = ++this.generation;
    this.detach();
    this.state = "stopping";
    return this.enqueue(async () => {
      try {
        await this.board.releaseControl();
      } catch (cause) {
        this.error = errorMessage(
          cause,
          "Clear could not be confirmed. Independent Board watchdogs remain active.",
        );
      } finally {
        if (gen === this.generation) {
          this.state = "stopped";
          this.log("Stopped. No automatic rearming.");
        }
      }
    });
  }
  private async fail(cause: unknown, gen: number) {
    if (gen !== this.generation) return;
    ++this.generation;
    this.detach();
    this.state = "fault";
    this.error = errorMessage(cause);
    await this.board.releaseControl().catch(() => {});
  }
}
