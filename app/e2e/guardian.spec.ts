import { expect, test, type Page } from "@playwright/test";

async function distance(page: Page, millimeters: number) {
  await page
    .getByLabel("Simulated distance (mm)")
    .evaluate((element, value) => {
      (element as HTMLInputElement).value = String(value);
      element.dispatchEvent(new Event("input", { bubbles: true }));
    }, millimeters);
}

async function visitZones(page: Page) {
  await page.getByRole("button", { name: "A — Arm", exact: true }).click();
  for (const [millimeters, state] of [
    [600, "sleeping"],
    [250, "curious"],
    [80, "alert"],
  ] as const) {
    await distance(page, millimeters);
    await expect(page.getByTestId("guardian-state")).toHaveText(state);
  }
}

test("a Crew completes Treasure Guardian, resumes, and retains its remix offline", async ({
  page,
  context,
}) => {
  await page.goto("/legacy");
  await page.getByRole("button", { name: "◆ Mission", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "What are we protecting?" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Hint 1 of 3" }).click();
  await expect(
    page.getByText("What would your creature keep safe?"),
  ).toBeVisible();
  await page.getByRole("button", { name: "Our Crew confirms" }).click();
  await page.getByLabel("Maker 1 name").fill("Nova");
  await page.getByLabel("Maker 1 name").press("Tab");
  await expect(page.getByRole("status")).toHaveText(
    "Progress saved on this Mac only.",
  );
  await page.reload();
  await page.getByRole("button", { name: "◆ Mission", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Choose your Crew roles" }),
  ).toBeVisible();
  await expect(page.getByLabel("Maker 1 name")).toHaveValue("Nova");
  await page.getByRole("button", { name: "Rotate roles" }).click();
  await page.getByRole("button", { name: "Our Crew confirms" }).click();
  await page.getByRole("button", { name: "Our Crew confirms" }).click();
  await page.getByRole("button", { name: "Check and continue" }).click();
  await expect(page.getByRole("alert")).toContainText("live hardware check");
  await page.getByRole("button", { name: "Run hardware self-test" }).click();
  await distance(page, 300);
  for (const label of ["A — Arm", "B — Silence", "C — Disarm"]) {
    await page.getByRole("button", { name: label, exact: true }).click();
  }
  await page.getByLabel("We saw the blue light").check();
  await page.getByLabel("We heard the short tone").check();
  await page.getByRole("button", { name: "Confirm light and sound" }).click();
  await expect(
    page.getByText("Live check: self-test — passed.", { exact: false }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Check and continue" }).click();
  await visitZones(page);
  await page.getByRole("button", { name: "Check and continue" }).click();
  await page.getByRole("button", { name: "Check and continue" }).click();
  await page.getByRole("button", { name: "B — Silence", exact: true }).click();
  await expect(page.getByText("· silenced", { exact: false })).toBeVisible();
  await page.getByRole("button", { name: "C — Disarm", exact: true }).click();
  await expect(page.getByTestId("guardian-state")).toHaveText("disarmed");
  await page.getByRole("button", { name: "Check and continue" }).click();
  await expect(
    page.getByRole("heading", { name: "Test your predictions" }),
  ).toBeVisible();
  await visitZones(page);
  await page.getByRole("button", { name: "B — Silence", exact: true }).click();
  await expect(page.getByText("· silenced", { exact: false })).toBeVisible();
  await page.getByRole("button", { name: "C — Disarm", exact: true }).click();
  await expect(page.getByTestId("guardian-state")).toHaveText("disarmed");
  await page.getByRole("button", { name: "Check and continue" }).click();
  await page
    .getByLabel("What did you change and test?")
    .fill("A bigger warning zone.");
  await page.getByRole("button", { name: "Record our remix" }).click();
  await expect(page.getByRole("alert")).toContainText("Change a rule");
  await page.getByText("Inventor — adjust the rules", { exact: true }).click();
  await page.getByLabel("Warning boundary (mm)").fill("500");
  await page.getByRole("button", { name: "Apply and test rules" }).click();
  await page.getByRole("button", { name: "A — Arm", exact: true }).click();
  await distance(page, 400);
  await expect(page.getByTestId("guardian-state")).toHaveText("curious");
  await page.getByRole("button", { name: "Record our remix" }).click();
  await page
    .getByLabel("What changed?", { exact: true })
    .fill("Warning now starts at 500 mm.");
  await page
    .getByLabel("What failed or surprised you?")
    .fill("We initially made the boundary too close.");
  await page.getByRole("button", { name: "Finish our Mission" }).click();
  await expect(
    page.getByRole("heading", { name: "Mission complete" }),
  ).toBeVisible();
  // Drop external network access while retaining localhost to prove no remote resources are needed.
  await context.route("**/*", (route) =>
    new URL(route.request().url()).hostname === "127.0.0.1"
      ? route.continue()
      : route.abort(),
  );
  await page.reload();
  await page.getByRole("button", { name: "◆ Mission", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Mission complete" }),
  ).toBeVisible();
  await expect(
    page.getByText("A bigger warning zone.", { exact: true }),
  ).toBeVisible();
});
