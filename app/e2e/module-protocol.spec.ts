import { test, expect } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { createInterface } from "node:readline";
import { request as httpsRequest } from "node:https";
import {
  ConnectedBoardClient,
  type GameplayTransport,
} from "../src/lib/board/connected";
import { CreationRuntime } from "../src/lib/creations/runtime";

// Approved BoardClient -> real Companion HTTPS seam. Only generated fixture TLS,
// private Unix ingress and MemoryHardware; no hardware/Keychain or success-only RPC mocks.
for (const inputs of ["missing", "healthy"] as const) {
  test(`real protocol preserves ${inputs} input readings and both bounded module tests across navigation`, async ({
    page,
  }) => {
    const repository = resolve(import.meta.dirname, "../..");
    const directory = mkdtempSync(join(tmpdir(), "tc-ui-"));
    const cert = join(directory, "cert.pem"),
      key = join(directory, "key.pem");
    execFileSync(
      "openssl",
      [
        "req",
        "-x509",
        "-newkey",
        "rsa:2048",
        "-nodes",
        "-subj",
        "/CN=localhost",
        "-addext",
        "subjectAltName=DNS:localhost",
        "-days",
        "1",
        "-keyout",
        key,
        "-out",
        cert,
      ],
      { stdio: "ignore" },
    );
    const python = join(repository, ".venv/bin/python3");
    const child = spawn(
      existsSync(python) ? python : "python3",
      [
        join(repository, "companion/tests/usb_play_fixture.py"),
        cert,
        key,
        directory,
        inputs === "healthy"
          ? "bridge-healthy-inputs"
          : "bridge-notify-before-ack",
      ],
      {
        env: {
          ...process.env,
          PYTHONPATH: join(repository, "companion/python"),
          PYTHONDONTWRITEBYTECODE: "1",
        },
        stdio: ["pipe", "pipe", "pipe"],
      },
    );
    const lines = createInterface({ input: child.stdout });
    let client: ConnectedBoardClient | undefined,
      actor: CreationRuntime | undefined;
    const failures: unknown[] = [];
    try {
      const started = await new Promise<{
        port: number;
        grant: Record<string, string>;
      }>((accept, reject) => {
        const timeout = setTimeout(
          () => reject(new Error("Private TLS fixture startup timed out")),
          5000,
        );
        lines.once("line", (line) => {
          clearTimeout(timeout);
          accept(JSON.parse(line));
        });
        child.once("exit", () => {
          clearTimeout(timeout);
          reject(new Error("Private TLS fixture exited"));
        });
      });
      const ca = readFileSync(cert);
      async function wire(path: string, body?: unknown, token?: string) {
        return new Promise<unknown>((accept, reject) => {
          const encoded = body === undefined ? undefined : JSON.stringify(body);
          const req = httpsRequest(
            {
              hostname: "localhost",
              family: 4,
              port: started.port,
              ca,
              path,
              method: encoded === undefined ? "GET" : "POST",
              headers: {
                ...(token ? { Authorization: `Bearer ${token}` } : {}),
                ...(encoded
                  ? {
                      "Content-Type": "application/json",
                      "Content-Length": Buffer.byteLength(encoded),
                    }
                  : {}),
              },
            },
            (response) => {
              let text = "";
              response.on("data", (chunk) => {
                text += chunk;
                if (text.length > 65536)
                  req.destroy(new Error("Fixture response too large"));
              });
              response.on("end", () => {
                if (response.statusCode !== 200) {
                  reject({
                    code: "command_failed",
                    message: "Real Companion rejected request",
                    status: response.statusCode,
                  });
                  return;
                }
                try {
                  accept(JSON.parse(text));
                } catch {
                  reject(new Error("Invalid fixture response"));
                }
              });
            },
          );
          req.setTimeout(2000, () =>
            req.destroy(new Error("TLS fixture request timeout")),
          );
          req.on("error", reject);
          req.end(encoded);
        });
      }
      const session = (await wire("/v1/usb-sessions", started.grant)) as {
        session_token: string;
      };
      const operations: string[] = [];
      const transport: GameplayTransport = async (_handle, op) => {
        operations.push(op.operation);
        if (op.operation === "hello" || op.operation === "state")
          return wire(`/v1/${op.operation}`, undefined, session.session_token);
        if (op.operation === "events")
          return wire(
            `/v1/events?after=${op.after}`,
            undefined,
            session.session_token,
          );
        if (op.operation === "renew_control")
          return wire(
            "/v1/control-lease",
            { lease_ms: 1500 },
            session.session_token,
          );
        const body =
          op.operation === "clear"
            ? { operation: "outputs.clear", arguments: {} }
            : op.operation === "pixels"
              ? {
                  operation: "pixels.set",
                  arguments: {
                    color: op.color,
                    brightness: op.brightness,
                    duration_ms: 1500,
                  },
                }
              : {
                  operation: "buzzer.play",
                  arguments: {
                    frequency_hz: op.frequency_hz,
                    duration_ms: op.duration_ms,
                    volume: 0.25,
                  },
                };
        return wire("/v1/commands", body, session.session_token);
      };
      client = await ConnectedBoardClient.open(
        1,
        started.grant.board_id,
        transport,
        (error) => failures.push(error),
      );
      expect(operations).not.toContain("renew_control");
      actor = new CreationRuntime(client);
      await actor.preview({
        operation: "pixels.set",
        arguments: { color: "#3366ff", brightness: 0.25 },
      });
      expect(actor.getStatus().state).toBe("preview");
      expect((await client.getSnapshot()).outputs.pixels?.color).toBe(
        "#3366ff",
      );
      await actor.stop();
      expect((await client.getSnapshot()).health).toBe("safe_state");
      await actor.preview({
        operation: "buzzer.play",
        arguments: { frequencyHz: 880, durationMs: 250 },
      });
      expect(actor.getStatus().state).toBe("preview");
      await actor.stop();
      await actor.stop(true);
      expect((await client.getSnapshot()).health).toBe("safe_state");
      expect(failures).toEqual([]);
      client.close();
      operations.splice(0);
      // Replay the rendered UI with real protocol-backed input health.
      // Only the fixed IPC carrier is intercepted; every gameplay request reaches
      // this same real TLS Companion and its actual command/lease implementation.
      let delayStateOnce = false,
        delayedStateStarted = false;
      await page.exposeFunction(
        "__moduleNative",
        async (name: string, args: Record<string, unknown> = {}) => {
          if (name === "load_progress") return null;
          if (name === "usb_play_boards") return [{ serial: "fixture-usb" }];
          if (name === "connect_usb_play")
            return { handle: 1, boardId: started.grant.board_id };
          if (name === "board_request") {
            const operation =
              args.operation as Parameters<GameplayTransport>[1];
            if (operation.operation === "state" && delayStateOnce) {
              delayStateOnce = false;
              const response = await transport(1, operation);
              delayedStateStarted = true;
              await new Promise((resolve) => setTimeout(resolve, 500));
              return response;
            }
            return transport(1, operation);
          }
          return {};
        },
      );
      await page.addInitScript(() => {
        const host = window as unknown as {
          isTauri: boolean;
          __moduleNative: (
            name: string,
            args?: Record<string, unknown>,
          ) => Promise<unknown>;
          __TAURI_INTERNALS__: {
            invoke: (
              name: string,
              args?: Record<string, unknown>,
            ) => Promise<unknown>;
          };
        };
        host.isTauri = true;
        host.__TAURI_INTERNALS__ = {
          invoke: (name, args) => host.__moduleNative(name, args),
        };
      });
      await page.goto("/");
      await page
        .getByRole("button", { name: "Connect Board", exact: true })
        .click();
      await expect(
        page.getByText("Real Board · authenticated connection", {
          exact: true,
        }),
      ).toBeVisible();
      if (inputs === "healthy") {
        await expect(page.getByText("12.3 cm", { exact: true })).toBeVisible();
        await expect(
          page.getByText("A: pressed", { exact: true }),
        ).toBeVisible();
        await expect(
          page.getByText("B: released", { exact: true }),
        ).toBeVisible();
        await expect(
          page.getByText("C: released", { exact: true }),
        ).toBeVisible();
        // Drive only the external SDK fixture, never Companion internals.
        child.stdin.write("B\n");
        await expect(
          page.getByText("B: pressed", { exact: true }),
        ).toBeVisible();
        await expect(
          page.getByText("A: released", { exact: true }),
        ).toBeVisible();
        child.stdin.write("C\n");
        await expect(
          page.getByText("C: pressed", { exact: true }),
        ).toBeVisible();
        await expect(page.getByTestId("creation-state")).toHaveText("stopped");
        child.stdin.write("A\n");
        await expect(
          page.getByText("A: pressed", { exact: true }),
        ).toBeVisible();
        await expect(
          page.getByText("C: released", { exact: true }),
        ).toBeVisible();
      } else {
        await expect(
          page.getByText("No fresh reading", { exact: true }),
        ).toBeVisible();
      }
      await page
        .getByRole("button", { name: "Build with blocks", exact: true })
        .click();
      await page
        .getByRole("button", { name: "Try modules", exact: true })
        .click();
      await page
        .getByRole("button", { name: "My creations", exact: true })
        .click();
      await page.getByRole("button", { name: "Create", exact: true }).click();
      await page
        .getByRole("button", { name: "Parent settings", exact: true })
        .click();
      await page
        .getByRole("button", { name: "Return to my Creation", exact: true })
        .click();
      expect(operations).not.toContain("renew_control");
      if (inputs === "healthy") {
        await expect(page.getByText("12.3 cm", { exact: true })).toBeVisible();
        await expect(
          page.getByText("A: pressed", { exact: true }),
        ).toBeVisible();
      }
      const authorityBeforeDetails = operations.filter((op) =>
        ["renew_control", "clear", "pixels", "buzzer"].includes(op),
      );
      await page
        .getByRole("button", { name: "Board details", exact: true })
        .click();
      const details = page.getByRole("region", {
        name: "Board connection",
        exact: true,
      });
      await expect(details).toBeVisible();
      await details
        .getByRole("button", { name: "Close connection panel", exact: true })
        .click();
      expect(
        operations.filter((op) =>
          ["renew_control", "clear", "pixels", "buzzer"].includes(op),
        ),
      ).toEqual(authorityBeforeDetails);
      delayStateOnce = true;
      await expect.poll(() => delayedStateStarted).toBe(true);
      await page.getByRole("button", { name: "Try blue", exact: true }).click();
      await expect(page.getByTestId("creation-pixels")).toHaveText("#4986ff");
      await page
        .getByRole("button", { name: "Stop outputs", exact: true })
        .click();
      await expect(page.getByTestId("creation-pixels")).toHaveText("off");
      await page
        .getByRole("button", { name: "Try chirp", exact: true })
        .click();
      await expect
        .poll(() => operations.filter((op) => op === "buzzer").length)
        .toBe(1);
      await expect(
        page.getByText("Real Board · authenticated connection", {
          exact: true,
        }),
      ).toBeVisible();
      await page
        .getByRole("button", { name: "Stop outputs", exact: true })
        .click();
      // A click dispatch is not completion of its asynchronous TLS clear.
      // The runtime reports stopped only after that clear finishes successfully.
      await expect(page.getByTestId("creation-state")).toHaveText("stopped");
      expect(
        (
          (await transport(1, { operation: "state" })) as {
            outputs: { pixels: string; buzzer: string };
          }
        ).outputs,
      ).toMatchObject({ pixels: "off", buzzer: "off" });
    } finally {
      await actor?.stop();
      client?.close();
      lines.close();
      child.kill();
      await new Promise<void>((accept) =>
        child.exitCode !== null ? accept() : child.once("exit", () => accept()),
      );
      rmSync(directory, { recursive: true, force: true });
    }
  });
}
