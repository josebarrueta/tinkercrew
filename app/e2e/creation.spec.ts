import { expect, test } from "@playwright/test";

test("normal USB Connect never looks up saved Mac credentials or falls back to the network", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const host = window as unknown as {
      isTauri: boolean;
      __usbCalls: string[];
      __TAURI_INTERNALS__: { invoke: (command: string) => Promise<unknown> };
    };
    host.isTauri = true;
    host.__usbCalls = [];
    host.__TAURI_INTERNALS__ = {
      invoke: async (command) => {
        host.__usbCalls.push(command);
        if (command === "load_progress") return null;
        if (command === "usb_play_boards") return [{ serial: "test-usb" }];
        if (command === "connect_usb_play")
          throw {
            code: "usb_play_upgrade_required",
            message:
              "One-time Companion software update needed in Parent setup.",
          };
        if (
          ["list_boards", "board_candidates", "probe_board"].includes(command)
        )
          throw {
            code: "keychain_unavailable",
            message: "Unexpected saved-credential access",
          };
        return {};
      },
    };
  });
  await page.goto("/");
  await page
    .getByRole("button", { name: "Connect Board", exact: true })
    .click();
  await expect(
    page.getByText(
      "One-time Companion software update needed in Parent setup.",
      { exact: true },
    ),
  ).toBeVisible();
  await page.waitForTimeout(1200);
  const calls = await page.evaluate(
    () => (window as unknown as { __usbCalls: string[] }).__usbCalls,
  );
  expect(
    calls.filter((name) =>
      ["list_boards", "board_candidates", "probe_board"].includes(name),
    ),
  ).toEqual([]);
  expect(calls.filter((name) => name === "connect_usb_play")).toHaveLength(1);
});

test("opening Create never asks for Board credentials; parent-only saved network access stops after one denial", async ({
  page,
}) => {
  const rendererErrors: string[] = [];
  page.on("pageerror", (error) => rendererErrors.push(error.message));
  await page.addInitScript(() => {
    const host = window as unknown as {
      isTauri: boolean;
      __creationCalls: string[];
      __TAURI_INTERNALS__: { invoke: (command: string) => Promise<unknown> };
    };
    host.isTauri = true;
    host.__creationCalls = [];
    host.__TAURI_INTERNALS__ = {
      invoke: async (command) => {
        host.__creationCalls.push(command);
        if (command === "load_progress") return null;
        if (command === "factory_restore_status") return { phase: "idle" };
        if (command === "list_boards")
          throw {
            code: "keychain_unavailable",
            message:
              "macOS denied Tinkeroo permission. Retry Connect; existing keys are unchanged.",
          };
        return null;
      },
    };
  });
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "First, make something happen." }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window as unknown as { __creationCalls: string[] }).__creationCalls,
    ),
  ).toEqual(["load_progress"]);
  await page
    .getByRole("button", {
      name: "Parent settings & troubleshooting",
      exact: true,
    })
    .click();
  await page
    .getByText("Parent network connection — saved Pairing (optional)", {
      exact: true,
    })
    .click();
  await page
    .getByRole("button", {
      name: "Load saved Boards for network play",
      exact: true,
    })
    .click();
  await expect(
    page.getByText(
      "macOS denied Tinkeroo permission. Retry Connect; existing keys are unchanged.",
      { exact: true },
    ),
  ).toBeVisible();
  await page.waitForTimeout(1200);
  expect(
    await page.evaluate(
      () =>
        (
          window as unknown as { __creationCalls: string[] }
        ).__creationCalls.filter((name) => name === "list_boards").length,
    ),
  ).toBe(1);
  expect(rendererErrors).toEqual([]);
});

test("choosing practice cancels pending USB selection without saved credential access", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const host = window as unknown as {
      isTauri: boolean;
      __calls: string[];
      __resolve: () => void;
      __TAURI_INTERNALS__: { invoke: (command: string) => Promise<unknown> };
    };
    host.isTauri = true;
    host.__calls = [];
    host.__TAURI_INTERNALS__ = {
      invoke: async (command) => {
        host.__calls.push(command);
        if (command === "load_progress") return null;
        if (command === "usb_play_boards")
          return new Promise((resolve) => {
            host.__resolve = () => resolve([{ serial: "late-usb" }]);
          });
        return {};
      },
    };
  });
  await page.goto("/");
  await page
    .getByRole("button", { name: "Connect Board", exact: true })
    .click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as unknown as { __calls: string[] }).__calls.includes(
          "usb_play_boards",
        ),
      ),
    )
    .toBe(true);
  await page.getByRole("button", { name: "Use practice", exact: true }).click();
  await page.evaluate(() =>
    (window as unknown as { __resolve: () => void }).__resolve(),
  );
  await page.waitForTimeout(300);
  expect(
    await page.evaluate(() =>
      (window as unknown as { __calls: string[] }).__calls.filter((name) =>
        ["connect_usb_play", "list_boards", "board_candidates"].includes(name),
      ),
    ),
  ).toEqual([]);
});

test("Practice closes a late USB handle without opening gameplay or reading saved Pairing", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const host = window as unknown as {
      isTauri: boolean;
      __lateCalls: { command: string; args: Record<string, unknown> }[];
      __finishUsb: () => void;
      __TAURI_INTERNALS__: {
        invoke: (
          command: string,
          args?: Record<string, unknown>,
        ) => Promise<unknown>;
      };
    };
    host.isTauri = true;
    host.__lateCalls = [];
    host.__TAURI_INTERNALS__ = {
      invoke: async (command, args = {}) => {
        host.__lateCalls.push({ command, args });
        if (command === "load_progress") return null;
        if (command === "usb_play_boards") return [{ serial: "test-usb" }];
        if (command === "connect_usb_play")
          return new Promise((resolve) => {
            host.__finishUsb = () =>
              resolve({ handle: 42, boardId: "late-board" });
          });
        return {};
      },
    };
  });
  await page.goto("/");
  await page
    .getByRole("button", { name: "Connect Board", exact: true })
    .click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as unknown as { __finishUsb?: () => void }).__finishUsb !==
          undefined,
      ),
    )
    .toBe(true);
  await page.getByRole("button", { name: "Use practice", exact: true }).click();
  await page.evaluate(() =>
    (window as unknown as { __finishUsb: () => void }).__finishUsb(),
  );
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (
            window as unknown as {
              __lateCalls: { command: string; args: Record<string, unknown> }[];
            }
          ).__lateCalls.find(
            (call) => call.command === "disconnect_play_handle",
          )?.args.handle,
      ),
    )
    .toBe(42);
  const calls = await page.evaluate(
    () =>
      (window as unknown as { __lateCalls: { command: string }[] }).__lateCalls,
  );
  expect(
    calls.filter((call) =>
      ["list_boards", "probe_board", "board_request"].includes(call.command),
    ),
  ).toEqual([]);
});

test("an explicitly connected Board can test a healthy Buzzer without Distance or an implicit lease", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const host = window as unknown as {
      isTauri: boolean;
      __ops: { operation: string }[];
      __TAURI_INTERNALS__: {
        invoke: (
          command: string,
          args?: Record<string, unknown>,
        ) => Promise<unknown>;
      };
    };
    host.isTauri = true;
    host.__ops = [];
    let control = false;
    host.__TAURI_INTERNALS__ = {
      invoke: async (command, args = {}) => {
        if (command === "load_progress") return null;
        if (command === "usb_play_boards") return [{ serial: "test-usb" }];
        if (command === "connect_usb_play")
          return { handle: 1, boardId: "board-one" };
        if (command !== "board_request") return {};
        const op = args.operation as { operation: string };
        host.__ops.push(op);
        if (op.operation === "renew_control") control = true;
        if (op.operation === "clear") control = false;
        const health = {
          state: control ? "ready" : "safe_state",
          uptime_ms: 1000 + performance.now(),
        };
        if (op.operation === "hello")
          return {
            protocol: "1.0",
            board: { board_id: "board-one" },
            health,
            capabilities: {
              buttons: { health: "operational" },
              distance: { health: "disconnected" },
              pixels: { health: "operational" },
              buzzer: { health: "operational" },
            },
          };
        if (op.operation === "state")
          return {
            health,
            observables: {
              distance: { value: null, health: "disconnected", age_ms: null },
              buttons: {
                value: [false, false, false],
                health: "operational",
                age_ms: 0,
              },
            },
            outputs: { pixels: "off", buzzer: "off", matrix: "clear" },
          };
        if (op.operation === "events") return { events: [] };
        return {};
      },
    };
  });
  await page.goto("/");
  await page
    .getByRole("button", { name: "Connect Board", exact: true })
    .click();
  await expect(
    page.getByText("Real Board · authenticated connection", { exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as unknown as { __ops: { operation: string }[] }).__ops.some(
        (op) => ["renew_control", "pixels", "buzzer"].includes(op.operation),
      ),
    ),
  ).toBe(false);
  await expect(
    page.getByText("No fresh reading", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Try chirp", exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as unknown as { __ops: { operation: string }[] }).__ops.some(
          (op) => op.operation === "buzzer",
        ),
      ),
    )
    .toBe(true);
  expect(
    await page.evaluate(() =>
      (window as unknown as { __ops: { operation: string }[] }).__ops.filter(
        (op) => op.operation === "buzzer",
      ),
    ),
  ).toEqual([{ operation: "buzzer", frequency_hz: 880, duration_ms: 250 }]);
});

test("an unreadable Creation library is preserved and cannot be overwritten", async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem("tinkercrew.creations.v1", "broken-library"),
  );
  await page.goto("/");
  await expect(page.getByRole("alert")).toContainText(
    "Saved creations could not be read",
  );
  await page
    .getByRole("button", { name: "Build with blocks", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Save creation", exact: true }),
  ).toBeDisabled();
  expect(
    await page.evaluate(() => localStorage.getItem("tinkercrew.creations.v1")),
  ).toBe("broken-library");
});

test("multiple block rules compose a conditional chirp and copying preserves the original", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Build with blocks", exact: true })
    .click();
  await page.getByLabel("Creation name").fill("Signal friend");
  await page
    .getByRole("button", { name: "Save creation", exact: true })
    .click();
  await expect(
    page.getByText("Saved on this Mac.", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Add rule", exact: false }).click();
  await page.getByLabel("Rule 2 input", { exact: true }).selectOption("B");
  await page.getByLabel("Rule 2 chirp", { exact: true }).check();
  await page.getByLabel("Rule 2 light enabled", { exact: true }).uncheck();
  await page.getByLabel("Rule 2 condition enabled", { exact: true }).check();
  await page.getByRole("button", { name: "Run creation", exact: true }).click();
  await page.getByLabel("Practice distance (cm)").fill("8");
  await page.getByRole("button", { name: "B", exact: true }).click();
  await expect(
    page.getByText("Rules 2 fired.", { exact: false }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Save a separate copy", exact: true })
    .click();
  await expect(page.getByTestId("creation-state")).toHaveText("stopped");
  await expect(page.getByLabel("Creation name")).toHaveValue(
    "Signal friend (copy)",
  );
  await page
    .getByRole("button", { name: "Signal friend", exact: true })
    .click();
  await expect(
    page.getByRole("article", { name: "Rule 2", exact: true }),
  ).toHaveCount(0);
  await expect(page.getByLabel("Rule 1 input", { exact: true })).toHaveValue(
    "A",
  );
});

test("a stale window cannot replace another maker's saved Creation or earlier Mission data", async ({
  page,
  context,
}) => {
  await page.goto("/");
  await page.evaluate(() =>
    localStorage.setItem(
      "tinkercrew.progress.treasure-guardian",
      "historical-save-preserved",
    ),
  );
  const other = await context.newPage();
  await other.goto("/");
  await other
    .getByRole("button", { name: "Build with blocks", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Build with blocks", exact: true })
    .click();
  await page.getByLabel("Creation name").fill("First maker");
  await page
    .getByRole("button", { name: "Save creation", exact: true })
    .click();
  await expect(
    page.getByText("Saved on this Mac.", { exact: true }),
  ).toBeVisible();
  await other.getByLabel("Creation name").fill("Stale maker");
  await other
    .getByRole("button", { name: "Save creation", exact: true })
    .click();
  await expect(other.getByRole("alert")).toContainText("another window");
  await page.reload();
  await expect(
    page.getByRole("button", { name: "First maker", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Stale maker", exact: true }),
  ).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      localStorage.getItem("tinkercrew.progress.treasure-guardian"),
    ),
  ).toBe("historical-save-preserved");
  await other.close();
});

test("the new workspace exposes its goal and a keyboard-accessible persistent Stop", async ({
  page,
}, info) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");
  const stop = page.getByRole("button", { name: "Stop outputs", exact: true });
  await stop.focus();
  await expect(stop).toBeFocused();
  await expect(stop).toBeEnabled();
  await page.screenshot({
    path: info.outputPath("creation-playground.png"),
    fullPage: true,
  });
  await page
    .getByRole("button", { name: "Build with blocks", exact: true })
    .click();
  await page.screenshot({
    path: info.outputPath("creation-blocks.png"),
    fullPage: true,
  });
  await page.getByRole("button", { name: "Missions", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "You’ll learn", exact: true }),
  ).toBeVisible();
  await expect(page.getByText("Start with:", { exact: false })).toBeVisible();
  await expect(page.getByRole("textbox")).toHaveCount(0);
  await page.screenshot({
    path: info.outputPath("creation-mission.png"),
    fullPage: true,
  });
  await page
    .getByRole("button", { name: "Make my guardian", exact: false })
    .click();
  await expect(page.getByLabel("Creation name")).toHaveValue(
    "My treasure guardian",
  );
  await expect(page.getByTestId("creation-state")).toHaveText("stopped");
  await page.getByRole("button", { name: "Run creation", exact: true }).click();
  await page.getByLabel("Practice distance (cm)").fill("19.9");
  await expect(page.getByText("19.9 cm", { exact: true })).toBeVisible();
  await expect(page.getByTestId("creation-pixels")).toHaveText("#ef674f");
  await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
  const box = await stop.boundingBox();
  expect(box!.y).toBeGreaterThanOrEqual(0);
  expect(box!.y + box!.height).toBeLessThan(1000);
});

test("a maker tries modules, composes blocks, and saves/reopens a stopped Creation", async ({
  page,
}) => {
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "First, make something happen." }),
  ).toBeVisible();
  await expect(
    page.getByText("Practice — no real Board outputs", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Try blue", exact: true }).click();
  await expect(page.getByTestId("creation-pixels")).toHaveText("#4986ff");
  await page.getByRole("button", { name: "Stop outputs", exact: true }).click();
  await page
    .getByRole("button", { name: "Build with blocks", exact: true })
    .click();
  await page.getByRole("button", { name: "Run creation", exact: true }).click();
  await page.getByRole("button", { name: "A", exact: true }).click();
  await expect(page.getByTestId("creation-pixels")).toHaveText("#4986ff");
  await page.getByRole("button", { name: "C · Stop", exact: true }).click();
  await expect(page.getByTestId("creation-state")).toHaveText("stopped");
  await page.getByLabel("Creation name").fill("My space greeter");
  await page
    .getByRole("button", { name: "Save creation", exact: true })
    .click();
  await expect(
    page.getByText("Saved on this Mac.", { exact: true }),
  ).toBeVisible();
  await page.getByLabel("Creation name").fill("Unsaved draft stays here");
  await page.getByLabel("Tinkeroo", { exact: true }).click();
  await page
    .getByRole("button", { name: "Build with blocks", exact: true })
    .click();
  await expect(page.getByLabel("Creation name")).toHaveValue(
    "Unsaved draft stays here",
  );
  await page.reload();
  await page
    .getByRole("button", { name: "My space greeter", exact: true })
    .click();
  await expect(page.getByLabel("Creation name")).toHaveValue(
    "My space greeter",
  );
  await expect(page.getByTestId("creation-state")).toHaveText("stopped");
});
