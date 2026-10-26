import type { BoardClient, BoardEvent } from "../board/types";
import { errorMessage } from "../errors";
import type { Check } from "./definition";
import {
  defaultRules,
  guardianRulesSchema,
  type GuardianRules,
} from "./guardian-rules";

type GuardianState =
  "disarmed" | "sleeping" | "curious" | "alert" | "safe_state";

/** A Mission runtime over any BoardClient. It never evaluates code or executes shell commands. */
export class GuardianRuntime {
  private unsubscribe: (() => void) | null = null;
  private armed = false;
  private silenced = false;
  private state: GuardianState = "disarmed";
  private rules: GuardianRules;
  private zones = new Set<string>();
  private buttons = new Set<number>();
  private selfTestButtons = new Set<number>();
  private moved = false;
  private initialDistance: number | null = null;
  private selfTesting = false;
  private selfConfirmed = false;
  private signals = false;
  private testedRemix = false;
  private inFlight: Promise<void> = Promise.resolve();
  private generation = 0;
  private log: string[] = [];
  private error: string | null = null;
  private outputDirty = true;
  private renderedSilence = false;

  constructor(
    private board: BoardClient,
    rules: GuardianRules = defaultRules,
  ) {
    this.rules = guardianRulesSchema.parse(rules);
  }

  async start() {
    if (this.unsubscribe) return;
    await this.board.renewControl();
    this.unsubscribe = this.board.subscribe((event) => this.receive(event));
    await this.tick();
  }

  private receive(event: BoardEvent) {
    this.log.push(JSON.stringify(event));
    this.log = this.log.slice(-40);
    if (event.type === "health.changed" && event.health !== "ready") {
      this.armed = false;
      this.state = "safe_state";
      this.resetEvidence();
    }
    if (event.type !== "button.changed" || !event.pressed) return;
    if (this.selfTesting) {
      this.selfTestButtons.add(event.button);
      return;
    }
    this.buttons.add(event.button);
    if (event.button === 0) {
      this.armed = true;
      this.silenced = false;
    }
    if (event.button === 1 && this.state === "alert") {
      this.silenced = true;
    }
    if (event.button === 2) {
      this.armed = false;
      this.silenced = false;
    }
  }

  getStatus() {
    return {
      state: this.state,
      armed: this.armed,
      silenced: this.silenced,
      rules: structuredClone(this.rules),
      log: [...this.log],
      error: this.error,
      selfTesting: this.selfTesting,
      testedRemix: this.testedRemix,
      evidence: {
        "self-test":
          this.selfConfirmed && this.moved && this.selfTestButtons.size === 3,
        zones: this.zones.size === 3,
        signals: this.zones.size === 3 && this.signals,
        buttons: this.buttons.size === 3 && this.silenceObserved,
        behavior:
          this.zones.size === 3 &&
          this.signals &&
          this.buttons.size === 3 &&
          this.silenceObserved,
      } satisfies Record<Check, boolean>,
    };
  }

  private silenceObserved = false;

  updateRules(input: unknown) {
    this.rules = guardianRulesSchema.parse(input);
    this.testedRemix = false;
    this.resetChallenge();
    this.outputDirty = true;
  }

  resetChallenge() {
    this.zones.clear();
    this.buttons.clear();
    this.signals = false;
    this.silenceObserved = false;
  }

  private resetEvidence() {
    this.resetChallenge();
    this.selfTestButtons.clear();
    this.moved = false;
    this.selfConfirmed = false;
    this.selfTesting = false;
    this.testedRemix = false;
  }

  async beginSelfTest() {
    await this.inFlight;
    this.armed = false;
    this.resetEvidence();
    this.selfTesting = true;
    this.outputDirty = true;
    try {
      this.initialDistance = (
        await this.board.getSnapshot()
      ).observables.distanceMillimeters;
      await this.board.renewControl();
      await this.board.command({
        operation: "pixels.set",
        arguments: { color: "#2288ff", brightness: 0.25 },
      });
      await this.board.command({
        operation: "buzzer.play",
        arguments: { frequencyHz: 880, durationMs: 250 },
      });
    } catch (cause) {
      this.resetEvidence();
      await this.board.releaseControl().catch(() => {});
      throw cause;
    }
  }

  async confirmSelfTest(lights: boolean, sound: boolean) {
    if (!this.selfTesting) throw new Error("Run the self-test first.");
    if (!this.moved || this.selfTestButtons.size !== 3)
      throw new Error(
        "Move the distance input and press A, B, and C during this self-test first.",
      );
    if (!lights || !sound)
      throw new Error(
        "Check the lights and sound, then retry the self-test. Your LEGO design is not judged.",
      );
    this.selfConfirmed = true;
    this.selfTesting = false;
    await this.board.command({ operation: "outputs.clear", arguments: {} });
  }

  tick(): Promise<void> {
    const generation = this.generation;
    this.inFlight = this.inFlight.then(async () => {
      if (!this.unsubscribe || generation !== this.generation) return;
      try {
        await this.board.renewControl();
        if (generation !== this.generation) return;
        const snapshot = await this.board.getSnapshot();
        if (generation !== this.generation) return;
        const { distanceMillimeters: distance, distanceAgeMs: age } =
          snapshot.observables;
        if (
          snapshot.health !== "ready" ||
          !(["distance", "buttons", "pixels", "buzzer"] as const).every(
            (capability) => snapshot.capabilities.includes(capability),
          ) ||
          !Number.isFinite(distance) ||
          distance < 20 ||
          distance > 4000 ||
          !Number.isFinite(age) ||
          age < 0 ||
          age > 750
        ) {
          throw new Error(
            "Sensor readings are stale or hardware is unavailable. Check power/connections, then retry and arm again.",
          );
        }
        if (this.selfTesting) {
          if (
            this.initialDistance !== null &&
            Math.abs(distance - this.initialDistance) >= 10
          )
            this.moved = true;
          return;
        }
        const previous = this.state;
        this.state = !this.armed
          ? "disarmed"
          : distance < this.rules.intruderMm
            ? "alert"
            : distance < this.rules.warningMm
              ? "curious"
              : "sleeping";
        if (this.state !== "alert") this.silenced = false;
        if (
          (this.state === "disarmed" &&
            (previous !== "disarmed" || this.outputDirty)) ||
          (this.silenced && !this.renderedSilence) ||
          (previous === "alert" && this.state !== "alert")
        ) {
          await this.board.command({
            operation: "outputs.clear",
            arguments: {},
          });
          if (generation !== this.generation) return;
          if (this.silenced) this.silenceObserved = true;
          await this.board.renewControl();
          if (generation !== this.generation) return;
        }
        if (this.armed) {
          const color =
            this.state === "alert"
              ? this.rules.alertColor
              : this.state === "curious"
                ? this.rules.warningColor
                : this.rules.safeColor;
          await this.board.command({
            operation: "pixels.set",
            arguments: { color, brightness: 0.35 },
          });
          if (generation !== this.generation) return;
          if (
            this.state === "alert" &&
            previous !== "alert" &&
            !this.silenced
          ) {
            await this.board.command({
              operation: "buzzer.play",
              arguments: { frequencyHz: 880, durationMs: this.rules.alarmMs },
            });
            if (generation !== this.generation) return;
            this.signals = true;
          }
          this.zones.add(this.state);
          this.testedRemix =
            JSON.stringify(this.rules) !== JSON.stringify(defaultRules);
        }
        this.renderedSilence = this.silenced;
        this.outputDirty = false;
        this.error = null;
      } catch (cause) {
        this.error = errorMessage(cause);
        this.armed = false;
        this.state = "safe_state";
        this.outputDirty = true;
        this.resetEvidence();
        try {
          await this.board.releaseControl();
        } catch {
          /* Board-side watchdog remains authoritative. */
        }
      }
    });
    return this.inFlight;
  }

  async stop() {
    this.generation++;
    this.unsubscribe?.();
    this.unsubscribe = null;
    this.armed = false;
    this.state = "disarmed";
    this.resetEvidence();
    this.outputDirty = true;
    this.renderedSilence = false;
    await this.inFlight;
    await this.board.releaseControl();
  }
}
