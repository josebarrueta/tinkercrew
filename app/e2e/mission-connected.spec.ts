import { expect, test, type Page } from "@playwright/test";

// Native IPC boundary only. No real Board, Keychain, SSH or output request.
async function connectedBoard(page: Page) {
  await page.addInitScript(() => {
    const scope = window as unknown as {
      isTauri: boolean;
      __missionCalls: { command: string; args: Record<string, unknown> }[];
      __TAURI_INTERNALS__: {
        invoke: (
          command: string,
          args?: Record<string, unknown>,
        ) => Promise<unknown>;
      };
    };
    scope.isTauri = true;
    scope.__missionCalls = [];
    let saved: { payload: string; revision: number } | null = null;
    scope.__TAURI_INTERNALS__ = {
      invoke: async (command, args = {}) => {
        scope.__missionCalls.push({ command, args });
        if (command === "usb_play_boards") return [{ serial: "test-usb" }];
        if (command === "connect_usb_play")
          return { handle: 1, boardId: "test-board" };
        if (command === "load_progress") return saved;
        if (command === "save_progress") {
          saved = {
            payload: args.payload as string,
            revision: Number(args.expectedRevision) + 1,
          };
          return saved.revision;
        }
        if (command === "board_request") {
          const operation = args.operation as { operation: string };
          const health = {
            state: "safe_state",
            uptime_ms: 1000 + performance.now(),
          };
          if (operation.operation === "hello")
            return {
              protocol: "1.0",
              board: { board_id: "test-board" },
              health,
              capabilities: Object.fromEntries(
                ["buttons", "distance", "pixels", "buzzer", "matrix"].map(
                  (name) => [name, { health: "operational" }],
                ),
              ),
            };
          if (operation.operation === "state")
            return {
              health,
              observables: {
                distance: { value: 600, health: "operational", age_ms: 0 },
                buttons: {
                  value: [false, false, false],
                  health: "operational",
                  age_ms: 0,
                },
              },
              outputs: { pixels: "off", buzzer: "off", matrix: "clear" },
            };
          if (operation.operation === "events") return { events: [] };
          if (operation.operation === "renew_control")
            throw {
              code: "authentication_failed",
              message:
                "The Board rejected this session. Reconnect the selected Board.",
            };
          return {};
        }
        if (command === "factory_restore_status")
          return { phase: "idle", bytes: 0, message: "Fixture only" };
        if (command === "initial_companion_package")
          return { release: "a".repeat(64), bytes: 2048 };
        if (command === "runtime_bundle_info")
          return {
            release: "b".repeat(64),
            contract: "c".repeat(64),
            bytes: 1024,
          };
        if (command === "app_update_info") return { version: "0.1.0" };
        return null;
      },
    };
  });
  await page.goto("/legacy");
  await page
    .getByRole("button", { name: "☰ Board Setup", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Detect USB Board", exact: true })
    .click();
  await expect(
    page.getByText("Board connected", { exact: true }),
  ).toBeVisible();
}

test("native Mission failures show the actionable message rather than object Object", async ({
  page,
}) => {
  await connectedBoard(page);
  await page.getByRole("button", { name: "◆ Mission", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "What are we protecting?" }),
  ).toBeVisible();
  await page
    .getByText("Mission hardware controls (lights & sound)", { exact: true })
    .click();
  await page
    .getByRole("button", { name: "Enable Mission controls", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText(
    "The Board rejected this session. Reconnect the selected Board.",
  );
  await expect(page.getByRole("alert")).not.toContainText("[object Object]");
  await expect(page.getByText("Stage 1 of 10", { exact: true })).toBeVisible();
});

test("USB families can go from connection to their saved Mission without maintenance screens", async ({
  page,
}, testInfo) => {
  await connectedBoard(page);
  await page
    .getByRole("button", { name: "☰ Board Setup", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Connect & play", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", {
      name: "Connect / retry USB Board",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Inspect selected UNO Q", exact: true }),
  ).not.toBeVisible();
  await expect(
    page.getByLabel("Manual local IP or hostname (optional)", { exact: true }),
  ).not.toBeVisible();
  await page.screenshot({
    path: testInfo.outputPath("connect-and-play.png"),
    fullPage: true,
  });
  await page
    .getByRole("button", { name: "Continue to Mission", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "What are we protecting?", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Our Crew confirms", exact: false })
    .click();
  await page
    .getByRole("button", { name: "Our Crew confirms", exact: false })
    .click();
  await page
    .getByRole("button", { name: "Open Board Setup", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Continue to Mission", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Build your own guardian", exact: true }),
  ).toBeVisible();
  await page.screenshot({
    path: testInfo.outputPath("connected-mission-build.png"),
    fullPage: true,
  });
});

test("connected storytelling stays available without acquiring output authority", async ({
  page,
}) => {
  await connectedBoard(page);
  await page.getByRole("button", { name: "◆ Mission", exact: true }).click();
  await expect(
    page.getByText("Mission controls are off.", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Our Crew confirms", exact: false })
    .click();
  await page
    .getByRole("button", { name: "Our Crew confirms", exact: false })
    .click();
  await expect(
    page.getByRole("heading", { name: "Build your own guardian", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(page.getByTestId("guardian-state")).toHaveCount(0);
  const step = await page
    .getByRole("heading", { name: "Build your own guardian", exact: true })
    .boundingBox();
  const controls = await page
    .getByRole("region", { name: "Mission Board connection", exact: true })
    .boundingBox();
  expect(step!.y).toBeLessThan(controls!.y);
  const calls = await page.evaluate(
    () =>
      (
        window as unknown as {
          __missionCalls: {
            command: string;
            args: { operation?: { operation: string } };
          }[];
        }
      ).__missionCalls,
  );
  expect(
    calls
      .filter((call) => call.command === "board_request")
      .every((call) =>
        ["hello", "state", "events"].includes(call.args.operation!.operation),
      ),
  ).toBe(true);
});
