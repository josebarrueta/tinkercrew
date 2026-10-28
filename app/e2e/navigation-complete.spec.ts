import { expect, test } from "@playwright/test";

test("connection help opens the requested Parent task rather than another menu", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Connect Board", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Parent setup (one time)", exact: true })
    .click();
  await expect(
    page.getByRole("region", { name: "Board configuration and recovery" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { level: 1, name: "Parent settings" }),
  ).toBeFocused();
});

test("pending recovery remains visible even when discovered from another Parent task", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const host = window as unknown as {
      isTauri: boolean;
      __TAURI_INTERNALS__: { invoke: (name: string) => Promise<unknown> };
    };
    host.isTauri = true;
    host.__TAURI_INTERNALS__ = {
      invoke: async (name) => {
        if (name === "load_progress") return null;
        if (name === "factory_restore_status")
          return {
            phase: "outcome_unknown",
            bytes: 0,
            message: "Recovery needs observation; do not clear the journal.",
          };
        return {};
      },
    };
  });
  await page.goto("/");
  await page
    .getByRole("button", { name: "Parent settings", exact: true })
    .click();
  await page.getByRole("button", { name: "Board setup", exact: true }).click();
  await expect(
    page.getByText("Restart, pairing cleanup, and factory recovery", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Return to my Creation", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Stop outputs", exact: true }),
  ).toBeEnabled();
});

test("Parent task groups expose only their relevant tools", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Parent settings", exact: true })
    .click();
  await page.getByRole("button", { name: "Board setup", exact: true }).click();
  await expect(
    page.getByText("Prepare initial Companion files and identity", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByText("Linux password (this request only)", { exact: true }),
  ).not.toBeVisible();
  await page
    .getByRole("button", { name: "Software updates", exact: true })
    .click();
  await expect(
    page.getByText("Upgrade the Mac app", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Recoverable Linux runtime update", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Restart, pairing cleanup, and factory recovery", {
      exact: true,
    }),
  ).not.toBeVisible();
  await page
    .getByRole("button", { name: "Troubleshooting", exact: true })
    .click();
  await expect(
    page.getByText("Restart, pairing cleanup, and factory recovery", {
      exact: true,
    }),
  ).toBeVisible();
});

test("Parent tasks return to the originating Creation with heading focus", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("navigation", { name: "Creation views" })
    .getByRole("button", { name: "Build", exact: true })
    .click();
  await page
    .getByLabel("Creation name", { exact: true })
    .fill("My returning idea");
  await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
  await page
    .getByRole("button", { name: "Parent settings", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { level: 1, name: "Parent settings" }),
  ).toBeFocused();
  await page.getByRole("button", { name: "Board setup", exact: true }).click();
  await expect(
    page.getByRole("region", { name: "Board configuration and recovery" }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Return to my Creation", exact: true })
    .click();
  await expect(page.getByLabel("Creation name", { exact: true })).toHaveValue(
    "My returning idea",
  );
  await expect(
    page.getByRole("heading", {
      level: 1,
      name: "Build a behavior, one block at a time.",
    }),
  ).toBeFocused();
  await expect(page.getByTestId("creation-state")).toHaveText("stopped");
});

test("closing a pending Board connection cancels its late specific handle", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const host = window as unknown as {
      isTauri: boolean;
      finish: () => void;
      calls: { name: string; args: Record<string, unknown> }[];
      __TAURI_INTERNALS__: {
        invoke: (
          name: string,
          args?: Record<string, unknown>,
        ) => Promise<unknown>;
      };
    };
    host.isTauri = true;
    host.calls = [];
    host.__TAURI_INTERNALS__ = {
      invoke: async (name, args = {}) => {
        host.calls.push({ name, args });
        if (name === "load_progress") return null;
        if (name === "usb_play_boards") return [{ serial: "fixture-usb" }];
        if (name === "connect_usb_play")
          return new Promise((resolve) => {
            host.finish = () => resolve({ handle: 42, boardId: "late-board" });
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
        () => typeof (window as unknown as { finish: unknown }).finish,
      ),
    )
    .toBe("function");
  await page
    .getByRole("region", { name: "Board connection", exact: true })
    .getByRole("button", { name: "Close connection panel", exact: true })
    .click();
  await page.evaluate(() =>
    (window as unknown as { finish: () => void }).finish(),
  );
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (
            window as unknown as {
              calls: { name: string; args: Record<string, unknown> }[];
            }
          ).calls.find((x) => x.name === "disconnect_play_handle")?.args.handle,
      ),
    )
    .toBe(42);
  await expect(
    page.getByText("Practice — no real Board outputs", { exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as unknown as { calls: { name: string }[] }).calls.filter((x) =>
        ["board_request", "list_boards", "probe_board"].includes(x.name),
      ),
    ),
  ).toEqual([]);
});

test("earlier adventure is a guarded detour and restores the working draft", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("navigation", { name: "Creation views" })
    .getByRole("button", { name: "Build", exact: true })
    .click();
  await page
    .getByLabel("Creation name", { exact: true })
    .fill("My draft before the detour");
  await page.getByLabel("Rule 1 color", { exact: true }).fill("#ff0000");
  await page
    .getByRole("button", { name: "Start from a Mission", exact: true })
    .click();
  await page
    .getByText("Earlier guided adventure and saved progress", { exact: true })
    .click();
  await page
    .getByRole("link", { name: "Open earlier Mission progress", exact: false })
    .click();
  const decision = page.getByRole("region", { name: "Unsaved Creation" });
  await expect(decision).toBeVisible();
  await decision
    .getByRole("button", { name: "Continue without saving", exact: true })
    .click();
  await expect(page).toHaveURL(/\/legacy/);
  await page
    .getByRole("link", { name: "Return to my Creation", exact: false })
    .click();
  await expect(page.getByLabel("Creation name", { exact: true })).toHaveValue(
    "My draft before the detour",
  );
  await expect(page.getByLabel("Rule 1 color", { exact: true })).toHaveValue(
    "#ff0000",
  );
  await expect(page.getByTestId("creation-state")).toHaveText("stopped");
});

test("Create views, library and browser history retain one stopped Creation", async ({
  page,
}) => {
  await page.goto("/");
  const views = page.getByRole("navigation", { name: "Creation views" });
  await views.getByRole("button", { name: "Build", exact: true }).click();
  await page
    .getByLabel("Creation name", { exact: true })
    .fill("My navigation idea");
  await page.getByLabel("Rule 1 color", { exact: true }).fill("#ff0000");
  await page.getByRole("button", { name: "Run creation", exact: true }).click();
  await views.getByRole("button", { name: "Build", exact: true }).click();
  await expect(page.getByTestId("creation-state")).toHaveText("running");
  await page.getByRole("button", { name: "My creations", exact: true }).click();
  await expect(
    page.getByRole("heading", { level: 1, name: "My creations" }),
  ).toBeFocused();
  await expect(
    page.getByText("No creations saved yet.", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await expect(page.getByLabel("Creation name", { exact: true })).toHaveValue(
    "My navigation idea",
  );
  await expect(page.getByLabel("Rule 1 color", { exact: true })).toHaveValue(
    "#ff0000",
  );
  await expect(page.getByTestId("creation-state")).toHaveText("stopped");
  await views.getByRole("button", { name: "Try modules", exact: true }).click();
  await page.goBack();
  await expect(page.getByLabel("Creation name", { exact: true })).toHaveValue(
    "My navigation idea",
  );
  await page.goForward();
  await expect(
    page.getByRole("heading", {
      level: 1,
      name: "First, make something happen.",
    }),
  ).toBeFocused();
  await expect(
    views.getByRole("button", { name: "Try modules", exact: true }),
  ).toHaveAttribute("aria-current", "page");
});
