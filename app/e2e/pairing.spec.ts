import { expect, test } from "@playwright/test";

test("installation registration and remote revocation stay separate from local cleanup and setup completion", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByRole("button", { name: "☰ Board Setup", exact: true })
    .click();
  const management = page.getByRole("region", {
    name: "Board configuration and recovery",
  });
  await management
    .getByText("Installation Pairing and lost-Mac revocation", { exact: true })
    .click();
  await expect(
    management.getByText(
      "This requires an updated, already installed Companion",
      { exact: false },
    ),
  ).toBeVisible();
  await expect(
    management.getByRole("button", {
      name: "Register this Mac with the installed TBP Companion",
      exact: true,
    }),
  ).toBeDisabled();
  await expect(
    management.getByRole("button", {
      name: "Revoke selected installation on this Board",
      exact: true,
    }),
  ).toBeDisabled();
  await expect(
    management.getByText(
      "It does not install/upgrade the Companion or finish first-run setup.",
      { exact: false },
    ),
  ).toBeVisible();
  await management
    .getByText("Restart, pairing cleanup, and factory recovery", {
      exact: true,
    })
    .click();
  await expect(
    management.getByRole("button", {
      name: "Forget selected Tinkeroo pairing on this Mac",
      exact: true,
    }),
  ).toBeDisabled();
  await expect(
    management.getByText(
      "Local cleanup does not revoke credentials on the Board.",
      { exact: false },
    ),
  ).toBeVisible();
});
