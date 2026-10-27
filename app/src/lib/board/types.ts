export type Capability =
  "buttons" | "distance" | "pixels" | "buzzer" | "matrix";

export type BoardEvent =
  | { type: "distance.changed"; millimeters: number }
  | { type: "button.changed"; button: number; pressed: boolean }
  | { type: "health.changed"; health: BoardSnapshot["health"] };

export type BoardCommand =
  | {
      operation: "pixels.set";
      arguments: { color: string; brightness: number };
    }
  | {
      operation: "buzzer.play";
      arguments: { frequencyHz: number; durationMs: number };
    }
  | { operation: "outputs.clear"; arguments: Record<string, never> };

export interface BoardSnapshot {
  mode: "simulator" | "connected";
  health: "ready" | "disconnected" | "safe_state";
  capabilities: Capability[];
  capabilityHealth?: Partial<
    Record<Capability, { health: string; ageMs?: number | null }>
  >;
  observables: {
    distanceMillimeters: number;
    distanceAgeMs: number;
    buttons: boolean[];
  };
  outputs: {
    pixels: { color: string; brightness: number } | null;
    buzzer: { frequencyHz: number; durationMs: number } | null;
    matrix: string[] | null;
  };
}

export interface BoardClient {
  getSnapshot(): Promise<BoardSnapshot>;
  subscribe(listener: (event: BoardEvent) => void): () => void;
  command(command: BoardCommand): Promise<void>;
  renewControl(): Promise<void>;
  releaseControl(): Promise<void>;
}
