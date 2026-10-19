import { expect, test } from "@playwright/test";
import { encodeShareHash } from "../src/lib/share";
import { monitorBrowserErrors, replaceEditorSource, waitForPreviewSvg } from "./helpers/playground";

test("theme selection survives family changes without authoring controls", async ({ page }) => {
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
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  const workbench = page.locator("#theme-panel");
  const scenes = [
    { family: "flowchart", source: null, treatment: "Base styling" },
    { family: "class", source: "classDiagram\nclass Account {\n  +String name\n}", treatment: "Base styling" },
    { family: "xychart", source: 'xychart-beta\nx-axis [Jan, Feb]\ny-axis "Count" 0 --> 10\nbar [3, 7]', treatment: "Base styling" },
    { family: "packet", source: 'packet\n0-7: "Header"', treatment: "Design not yet reviewed" },
  ];
  for (const scene of scenes) {
    if (scene.source) await replaceEditorSource(page, scene.source);
    await waitForPreviewSvg(page);
    if (!scene.source) {
      await page.screenshot({ path: test.info().outputPath("theme-panel-selection.png") });
    }
    await expect(workbench.getByRole("tab")).toHaveCount(0);
    await expect(workbench.getByRole("button", { name: /^Cyberpunk/ })).toHaveAttribute("aria-pressed", "true");
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
  const workbench = page.locator("#theme-panel");
  await expect(workbench.locator("header")).toContainText("future-preset");
  await expect(workbench).toContainText("future-preset");
  await expect(workbench).toContainText("Unavailable in this runtime:");
  await expect(workbench).toContainText("selection is preserved");
  await expect(workbench.getByRole("button", { name: "Apply", exact: true })).toBeDisabled();
  await expect(workbench.getByRole("button", { name: "Download JSON", exact: true })).toBeDisabled();
  await expect(workbench.locator('button[aria-pressed="true"]')).toHaveCount(0);
});
