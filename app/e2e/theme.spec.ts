import { expect, test } from "@playwright/test";

function contrast(foreground: string, background: string) {
  const luminance = (rgb: string) => {
    const channels = rgb
      .match(/[\d.]+/g)!
      .slice(0, 3)
      .map((value) => {
        const channel = Number(value) / 255;
        return channel <= 0.04045
          ? channel / 12.92
          : ((channel + 0.055) / 1.055) ** 2.4;
      });
    return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
  };
  const values = [luminance(foreground), luminance(background)].sort(
    (a, b) => a - b,
  );
  return (values[1] + 0.05) / (values[0] + 0.05);
}

test("Tinkeroo keeps a clear next action, keyboard focus, and calm reduced-motion screens", async ({
  page,
}, testInfo) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 1180, height: 820 });
  await page.goto("/");
  await expect(page).toHaveTitle(/Tinkeroo/);
  await expect(page.getByRole("img", { name: "Tinkeroo" })).toBeVisible();
  const action = page.getByTestId("start-mission");
  await expect(action).toHaveText("Continue Treasure Guardian →");
  await action.focus();
  const styles = await action.evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      foreground: style.color,
      background: style.backgroundColor,
      outline: style.outlineStyle,
      outlineWidth: style.outlineWidth,
      height: element.getBoundingClientRect().height,
    };
  });
  expect(styles.outline).toBe("solid");
  expect(parseFloat(styles.outlineWidth)).toBeGreaterThanOrEqual(3);
  expect(styles.height).toBeGreaterThanOrEqual(44);
  expect(contrast(styles.foreground, styles.background)).toBeGreaterThanOrEqual(
    4.5,
  );
  const navigation = await page
    .locator("nav button.active")
    .evaluate((element) => {
      const style = getComputedStyle(element);
      return { foreground: style.color, background: style.backgroundColor };
    });
  expect(
    contrast(navigation.foreground, navigation.background),
  ).toBeGreaterThanOrEqual(4.5);
  expect(
    await page.evaluate(
      () =>
        document
          .getAnimations()
          .filter((animation) => animation.playState === "running").length,
    ),
  ).toBe(0);
  await page.screenshot({
    path: testInfo.outputPath("tinkeroo-home.png"),
    fullPage: true,
  });
  await action.press("Enter");
  await expect(
    page.getByRole("heading", { name: "What are we protecting?" }),
  ).toBeVisible();
  await expect(
    page.getByRole("progressbar", { name: "Mission progress" }),
  ).toBeVisible();
  await expect(
    page.getByText("Your shared Crew", { exact: true }),
  ).not.toBeVisible();
  await page.screenshot({
    path: testInfo.outputPath("tinkeroo-mission.png"),
    fullPage: true,
  });
  for (const name of ["✦ World Map", "⚙ Hardware Lab", "☰ Parent Settings"]) {
    await page.getByRole("button", { name, exact: true }).click();
    await page.screenshot({
      path: testInfo.outputPath(`${name.slice(2).replaceAll(" ", "-")}.png`),
      fullPage: true,
    });
  }
});
