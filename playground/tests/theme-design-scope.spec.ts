import { expect, test } from "@playwright/test";
import { encodeShareHash } from "../src/lib/share";
import { monitorBrowserErrors, replaceEditorSource, waitForPreviewSvg } from "./helpers/playground";

test("theme selection survives family changes and explains only its declared design scope", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.addInitScript(() => window.localStorage.setItem("merman-language", "en"));
  const hash = encodeShareHash({
    code: "flowchart LR\nA[Start] --> B[Done]",
    mermaidConfig: "{}",
    diagramTheme: "default",
    themePresetId: "cyberpunk",
    themeRecipeJson: null,
    svgPipeline: "parity",
    textMeasurementMode: "browser",
    diagramFont: "trebuchet",
  });
  await page.goto(`./${hash}`, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  const scenes = [
    { family: "Flowchart", source: null, treatment: "Base styling" },
    { family: "Class", source: "classDiagram\nclass Account {\n  +String name\n}", treatment: "Base styling" },
    { family: "XY Chart", source: 'xychart-beta\nx-axis [Jan, Feb]\ny-axis "Count" 0 --> 10\nbar [3, 7]', treatment: "Base styling" },
    { family: "Packet", source: 'packet\n0-7: "Header"', treatment: "Design not yet reviewed" },
  ];
  for (const scene of scenes) {
    if (scene.source) await replaceEditorSource(page, scene.source);
    await page.getByRole("button", { name: "Theme", exact: true }).click();
    const scope = page.getByTestId("theme-design-scope");
    await expect(scope).toContainText(`Visible diagram: ${scene.family}`);
    await expect(scope).toContainText(scene.treatment);
    if (!scene.source) {
      await page.getByRole("menu").evaluate(async (element) => {
        await Promise.all(element.getAnimations({ subtree: true }).map((animation) => animation.finished));
      });
      await page.screenshot({ path: test.info().outputPath("theme-design-menu.png") });
    }
    await expect(scope).toContainText("does not certify export results");
    await expect(page.getByRole("menuitemradio", { name: /^Cyberpunk/ })).toBeChecked();
    await page.keyboard.press("Escape");
  }
  errors.assertNone();
});


test("an unknown saved preset remains selected and is explained as unavailable", async ({ page }) => {
  await page.addInitScript(() => window.localStorage.setItem("merman-language", "en"));
  const hash = encodeShareHash({
    code: "flowchart LR\nA --> B",
    mermaidConfig: "{}",
    diagramTheme: "default",
    themePresetId: "future-preset",
    themeRecipeJson: null,
    svgPipeline: "parity",
    textMeasurementMode: "browser",
    diagramFont: "trebuchet",
  });
  await page.goto(`./${hash}`, { waitUntil: "domcontentloaded" });
  await expect(page.locator("footer")).toContainText("Ready");
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  const selected = page.getByRole("menuitemradio", { name: /^future-preset/ });
  await expect(selected).toBeChecked();
  await expect(selected).toBeDisabled();
  const scope = page.getByTestId("theme-design-scope");
  await expect(scope).toContainText("Design not yet reviewed");
  await expect(scope).toContainText("Unavailable in this runtime");
});
