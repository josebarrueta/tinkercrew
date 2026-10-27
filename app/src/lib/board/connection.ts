export type DiscoveryStage = "known" | "mdns" | "local" | "usb";
export interface Candidate {
  host: string;
  port: number;
}
export interface ConnectionError {
  code: string;
  message: string;
}
export interface ConnectionBackend {
  candidates(
    boardId: string,
    stage: DiscoveryStage,
    localConsent: boolean,
  ): Promise<Candidate[]>;
  probe(boardId: string, candidate: Candidate): Promise<number>;
}
export type ConnectionState =
  | { phase: "simulator" }
  | { phase: "searching"; stage: DiscoveryStage | "manual" }
  | { phase: "connected"; transport: DiscoveryStage | "manual" }
  | { phase: "failed"; error: ConnectionError };

export function connectionError(cause: unknown): ConnectionError {
  if (
    cause &&
    typeof cause === "object" &&
    "code" in cause &&
    "message" in cause &&
    typeof cause.code === "string" &&
    typeof cause.message === "string"
  ) {
    return { code: cause.code, message: cause.message };
  }
  return {
    code: "connection_failed",
    message:
      "The Board connection stopped. Check power and the data cable, or retry from Board Setup.",
  };
}

export async function connectBoard(
  backend: ConnectionBackend,
  options: { boardId: string; localConsent: boolean; manual?: Candidate },
  observe: (state: ConnectionState) => void = () => {},
): Promise<{ handle: number; transport: DiscoveryStage | "manual" }> {
  let last: ConnectionError = {
    code: "board_not_detected",
    message:
      "No selected Board was found. Check power, Wi-Fi/VPN restrictions, or USB. You can enter its IP address manually.",
  };
  for (const stage of ["known", "mdns", "local", "usb", "manual"] as const) {
    if (stage === "local" && !options.localConsent) continue;
    if (stage === "manual" && !options.manual) continue;
    observe({ phase: "searching", stage });
    try {
      const candidates =
        stage === "manual"
          ? [options.manual!]
          : await backend.candidates(
              options.boardId,
              stage,
              options.localConsent,
            );
      for (const candidate of candidates.slice(0, 8)) {
        try {
          const handle = await backend.probe(options.boardId, candidate);
          observe({ phase: "connected", transport: stage });
          return { handle, transport: stage };
        } catch (cause) {
          last = connectionError(cause);
          if (
            [
              "authentication_failed",
              "authentication_busy",
              "keychain_unavailable",
              "pairing_required",
              "protocol_incompatible",
              "maintenance_active",
            ].includes(last.code)
          )
            throw last;
        }
      }
    } catch (cause) {
      last = connectionError(cause);
      if (
        [
          "authentication_failed",
          "authentication_busy",
          "keychain_unavailable",
          "pairing_required",
          "protocol_incompatible",
          "maintenance_active",
        ].includes(last.code)
      ) {
        observe({ phase: "failed", error: last });
        throw last;
      }
    }
  }
  observe({ phase: "failed", error: last });
  throw last;
}
