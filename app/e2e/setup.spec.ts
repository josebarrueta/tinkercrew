import { expect, test } from "@playwright/test";

test("setup preview exposes official handoff and never invents factory success", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "☰ Board Setup", exact: true })
    .click();
  await page
    .getByText("Set up a UNO Q — resumable official-tool handoff guide", {
      exact: true,
    })
    .click();
  const guide = page.getByRole("region", {
    name: "Resumable UNO Q setup guide",
  });
  await expect(guide.getByRole("status")).toContainText("Browser preview only");
  await expect(
    guide.getByRole("button", { name: "Record checkpoint and continue" }),
  ).toBeDisabled();
  await guide
    .getByText("All checkpoints and evidence", { exact: true })
    .click();
  await expect(
    guide.getByText("Establish full USB Pairing: pending implementation", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    guide.getByText(
      "Install or upgrade the Companion atomically: pending implementation",
      { exact: true },
    ),
  ).toBeVisible();
  await expect(guide.locator('input[type="password"]')).toHaveCount(0);
});
