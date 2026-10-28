import { expect, test } from "@playwright/test";

test("a failed save cannot discard the Creation being replaced", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Build with blocks", exact: true })
    .click();
  await page.getByLabel("Creation name", { exact: true }).fill("Keep my work");
  await page.evaluate(() => {
    const write = Storage.prototype.setItem;
    Storage.prototype.setItem = function (key, value) {
      if (key === "tinkercrew.creations.v1") throw new Error("Storage is full");
      write.call(this, key, value);
    };
  });
  await page.getByRole("button", { name: "New creation", exact: true }).click();
  const decision = page.getByRole("region", {
    name: "Unsaved Creation",
    exact: true,
  });
  await decision
    .getByRole("button", { name: "Save and continue", exact: true })
    .click();
  await expect(page.getByRole("alert")).toHaveText("Storage is full");
  await expect(decision).toBeVisible();
  await expect(page.getByLabel("Creation name", { exact: true })).toHaveValue(
    "Keep my work",
  );
  await page.keyboard.press("Escape");
  await expect(decision).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "New creation", exact: true }),
  ).toBeFocused();
});

test("browsing a Mission keeps the draft and choosing its starter protects it", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Build with blocks", exact: true })
    .click();
  await page.getByLabel("Creation name", { exact: true }).fill("My own idea");
  await page.getByRole("button", { name: "Missions", exact: true }).click();
  const decision = page.getByRole("region", {
    name: "Unsaved Creation",
    exact: true,
  });
  await expect(decision).toHaveCount(0);
  await page
    .getByRole("button", { name: "Make my guardian", exact: false })
    .click();
  await expect(
    decision.getByRole("button", { name: "Keep editing", exact: true }),
  ).toBeFocused();
  await decision
    .getByRole("button", { name: "Keep editing", exact: true })
    .click();
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await page
    .getByRole("button", { name: "Build with blocks", exact: true })
    .click();
  await expect(page.getByLabel("Creation name", { exact: true })).toHaveValue(
    "My own idea",
  );
  await page.getByRole("button", { name: "Missions", exact: true }).click();
  await page
    .getByRole("button", { name: "Make my guardian", exact: false })
    .click();
  await decision
    .getByRole("button", { name: "Discard changes", exact: true })
    .click();
  await expect(page.getByLabel("Creation name", { exact: true })).toHaveValue(
    "My treasure guardian",
  );
  await expect(page.getByTestId("creation-state")).toHaveText("stopped");
});

test("edits made during a pending native save are not discarded by its older success", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const host = window as unknown as {
      isTauri: boolean;
      finishSave: () => void;
      __TAURI_INTERNALS__: { invoke: (name: string) => Promise<unknown> };
    };
    host.isTauri = true;
    host.__TAURI_INTERNALS__ = {
      invoke: async (name) => {
        if (name === "load_progress") return null;
        if (name === "save_progress")
          return new Promise((resolve) => {
            host.finishSave = () => resolve(1);
          });
        throw new Error(`Unexpected native operation: ${name}`);
      },
    };
  });
  await page.goto("/");
  await page
    .getByRole("button", { name: "Build with blocks", exact: true })
    .click();
  await page
    .getByLabel("Creation name", { exact: true })
    .fill("Earlier version");
  await page.getByRole("button", { name: "New creation", exact: true }).click();
  const decision = page.getByRole("region", {
    name: "Unsaved Creation",
    exact: true,
  });
  await decision
    .getByRole("button", { name: "Save and continue", exact: true })
    .click();
  await expect(
    decision.getByRole("button", { name: "Discard changes", exact: true }),
  ).toBeDisabled();
  await page
    .getByLabel("Creation name", { exact: true })
    .fill("Newer unsaved edits");
  await page.evaluate(() =>
    (window as unknown as { finishSave: () => void }).finishSave(),
  );
  await expect(
    decision.getByRole("button", { name: "Keep editing", exact: true }),
  ).toBeEnabled();
  await expect(decision).toBeVisible();
  await expect(page.getByLabel("Creation name", { exact: true })).toHaveValue(
    "Newer unsaved edits",
  );
  await expect(
    page.getByRole("button", { name: "Earlier version", exact: true }),
  ).toBeVisible();
  await expect(page.getByTestId("creation-state")).toHaveText("stopped");
});

for (const viewport of [
  { width: 1180, height: 760 },
  { width: 900, height: 640 },
]) {
  test(`Run, Save and Stop remain in view with one or eight rules at ${viewport.width}px`, async ({
    page,
  }) => {
    await page.setViewportSize(viewport);
    await page.goto("/");
    await page
      .getByRole("button", { name: "Build with blocks", exact: true })
      .click();
    for (const name of ["Run creation", "Save creation", "Stop outputs"]) {
      await expect(
        page.getByRole("button", { name, exact: true }),
      ).toBeInViewport({ ratio: 1 });
    }
    for (let i = 1; i < 8; i++)
      await page
        .getByRole("button", { name: "Add rule", exact: false })
        .click();
    await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
    for (const name of ["Run creation", "Save creation", "Stop outputs"]) {
      await expect(
        page.getByRole("button", { name, exact: true }),
      ).toBeInViewport({ ratio: 1 });
    }
    await page
      .getByRole("button", { name: "Save creation", exact: true })
      .click();
    await expect(
      page.getByText("Saved on this Mac.", { exact: true }),
    ).toBeVisible();
  });
}

test("New Creation protects unsaved work with stay, discard and save choices", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "Build with blocks", exact: true })
    .click();
  await page
    .getByLabel("Creation name", { exact: true })
    .fill("My unsaved signal");
  await page.getByLabel("Rule 1 color", { exact: true }).fill("#ff0000");
  await page.getByRole("button", { name: "New creation", exact: true }).click();
  const decision = page.getByRole("region", {
    name: "Unsaved Creation",
    exact: true,
  });
  await expect(decision).toBeVisible();
  await decision
    .getByRole("button", { name: "Keep editing", exact: true })
    .click();
  await expect(page.getByLabel("Creation name", { exact: true })).toHaveValue(
    "My unsaved signal",
  );
  await expect(page.getByLabel("Rule 1 color", { exact: true })).toHaveValue(
    "#ff0000",
  );
  await page.getByRole("button", { name: "New creation", exact: true }).click();
  await decision
    .getByRole("button", { name: "Save and continue", exact: true })
    .click();
  await expect(page.getByLabel("Creation name", { exact: true })).toHaveValue(
    "My invention",
  );
  await page
    .getByRole("button", { name: "My unsaved signal", exact: true })
    .click();
  await decision
    .getByRole("button", { name: "Discard changes", exact: true })
    .click();
  await expect(page.getByLabel("Creation name", { exact: true })).toHaveValue(
    "My unsaved signal",
  );
  await expect(page.getByLabel("Rule 1 color", { exact: true })).toHaveValue(
    "#ff0000",
  );
  await expect(page.getByTestId("creation-state")).toHaveText("stopped");
});
