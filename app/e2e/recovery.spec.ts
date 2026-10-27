import { expect, test } from "@playwright/test";

test("stale input enters Safe state and recovery never silently rearms", async ({
  page,
}) => {
  await page.goto("/legacy");
  await page.getByRole("button", { name: "◆ Mission", exact: true }).click();
  await page.getByRole("button", { name: "A — Arm", exact: true }).click();
  await expect(page.getByTestId("guardian-state")).toHaveText("sleeping");
  await page.getByLabel("Freeze distance readings (safety experiment)").check();
  await expect(page.getByTestId("guardian-state")).toHaveText("safe_state");
  await expect(page.getByText("Pixels: off", { exact: true })).toBeVisible();
  await page
    .getByLabel("Freeze distance readings (safety experiment)")
    .uncheck();
  await expect(page.getByTestId("guardian-state")).toHaveText("disarmed");
  await page
    .getByRole("button", { name: "☰ Board Setup", exact: true })
    .click();
  await expect(
    page.getByText("The RexQualis breadboard power module", { exact: false }),
  ).toBeVisible();
  await expect(page.getByText("3.3 V logic", { exact: true })).toBeVisible();
});

test("stopping Mission controls requires explicit enable and never restores an armed guardian", async ({
  page,
}) => {
  await page.goto("/legacy");
  await page.getByRole("button", { name: "◆ Mission", exact: true }).click();
  await page.getByRole("button", { name: "A — Arm", exact: true }).click();
  await expect(page.getByTestId("guardian-state")).toHaveText("sleeping");
  await page
    .getByText("Mission hardware controls (lights & sound)", { exact: true })
    .click();
  await page
    .getByRole("button", { name: "Stop Mission controls", exact: true })
    .click();
  await expect(page.getByTestId("guardian-state")).toHaveCount(0);
  await expect(
    page.getByText("Mission controls are off.", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Enable Mission controls", exact: true })
    .click();
  await expect(page.getByTestId("guardian-state")).toHaveText("disarmed");
  await expect(page.getByText("Pixels: off", { exact: true })).toBeVisible();
});

test("unreadable progress produces a recovery message without replacing the save", async ({
  page,
}) => {
  await page.goto("/legacy");
  const broken = '{"payload":"broken-json","revision":1}';
  await page.evaluate(
    (value) =>
      localStorage.setItem("tinkercrew.progress.treasure-guardian", value),
    broken,
  );
  await page.getByRole("button", { name: "◆ Mission", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "Existing saves have not been replaced",
  );
  await expect(page.getByRole("button", { name: "Save again" })).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      localStorage.getItem("tinkercrew.progress.treasure-guardian"),
    ),
  ).toBe(broken);
});
