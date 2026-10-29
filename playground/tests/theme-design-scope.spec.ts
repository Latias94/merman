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
    themePresetId: "brutalist",
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
    { family: "Flowchart", source: null, treatment: "dedicated", label: "Dedicated design" },
    { family: "Class", source: "classDiagram\nclass Account {\n  +String name\n}", treatment: "base_only", label: "Base styling" },
    { family: "XY Chart", source: 'xychart-beta\nx-axis [Jan, Feb]\ny-axis "Count" 0 --> 10\nbar [3, 7]', treatment: "dedicated", label: "Dedicated design" },
    { family: "Packet", source: 'packet\n0-7: "Header"', treatment: "unreviewed", label: "Design not yet reviewed" },
  ];
  for (const scene of scenes) {
    if (scene.source) await replaceEditorSource(page, scene.source);
    await waitForPreviewSvg(page);
    if (!scene.source) {
      await page.screenshot({ path: test.info().outputPath("theme-panel-selection.png") });
    }
    await expect(workbench.getByRole("tab")).toHaveCount(0);
    await expect(workbench.getByTestId("theme-current-family")).toHaveText(`Current diagram: ${scene.family}`);
    const selected = workbench.getByRole("button", { name: /^Brutalist/ });
    await expect(selected).toHaveAttribute("aria-pressed", "true");
    await expect(selected).toHaveAttribute("data-theme-treatment", scene.treatment);
    await expect(selected).toContainText(scene.label);
  }
  await expect(workbench.getByTestId("theme-reviewed-families")).toContainText("does not mean unsupported");
  errors.assertNone();
});

for (const scene of [
  { family: "Git Graph", zhFamily: "Git 图", source: "gitGraph\n  commit\n  commit" },
  { family: "Quadrant Chart", zhFamily: "象限图", source: "quadrantChart\n  x-axis Low --> High\n  y-axis Low --> High\n  Item: [0.3, 0.6]" },
]) {
  for (const language of ["en", "zh"] as const) {
    test(`${scene.family} uses syntax-based family detection and ${language} metadata labels`, async ({ page }) => {
      await page.addInitScript((locale) => localStorage.setItem("merman-language", locale), language);
      await page.goto(`./${encodeShareHash({
        code: scene.source, mermaidConfig: "{}", diagramTheme: "default", themePresetId: "brutalist",
        themeRecipeJson: null, svgPipeline: "parity", textMeasurementMode: "browser", diagramFont: "trebuchet",
      })}`, { waitUntil: "domcontentloaded" });
      await waitForPreviewSvg(page);
      await page.getByRole("button", { name: language === "en" ? "Theme" : "主题", exact: true }).click();
      const panel = page.locator("#theme-panel");
      await expect(panel.getByTestId("theme-current-family")).toHaveText(language === "en"
        ? `Current diagram: ${scene.family}` : `当前图表：${scene.zhFamily}`);
      const selected = panel.getByRole("button", { name: language === "en" ? /^Brutalist/ : /^粗野主义/ });
      await expect(selected).toHaveAttribute("aria-pressed", "true");
      await expect(selected).toHaveAttribute("data-theme-treatment", "unreviewed");
      await expect(selected).toContainText(language === "en" ? "Design not yet reviewed" : "设计尚未评审");
    });
  }
}

test("Ghibli lists only its reviewed Pie design and retains selection for unreviewed families", async ({ page }) => {
  await page.addInitScript(() => window.localStorage.setItem("merman-language", "en"));
  await page.goto(`./${encodeShareHash({
    code: 'pie\n"Tea": 4\n"Cake": 3', mermaidConfig: "{}", diagramTheme: "default",
    themePresetId: "ghibli", themeRecipeJson: null, svgPipeline: "parity", textMeasurementMode: "browser", diagramFont: "trebuchet",
  })}`, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  const panel = page.locator("#theme-panel");
  const card = panel.getByRole("button", { name: /^Ghibli/ });
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Pie Chart");
  await expect(card).toHaveAttribute("data-theme-treatment", "dedicated");
  const reviewed = panel.getByTestId("theme-reviewed-families");
  await expect(reviewed.getByRole("listitem")).toHaveCount(1);
  await expect(reviewed.getByRole("listitem")).toContainText("Pie Chart");
  await expect(reviewed.getByRole("listitem")).toContainText("Dedicated design");
  await panel.locator("summary").filter({ hasText: /^Advanced$/ }).click();
  const catalogSummary = panel.locator("summary").filter({ hasText: /^Preset runtime details$/ });
  await catalogSummary.click();
  const details = catalogSummary.locator("..");
  const catalogEntry = JSON.parse((await details.locator("pre").textContent())!);
  expect(catalogEntry.preview_colors.length).toBeGreaterThan(0);
  const expectedColors = await page.evaluate((colors: string[]) => colors.map((color) => {
    const style = document.createElement("span").style;
    style.backgroundColor = color;
    return style.backgroundColor;
  }), catalogEntry.preview_colors);
  expect(await card.locator("[data-theme-swatches] > span").evaluateAll((spans) =>
    spans.map((span) => (span as HTMLElement).style.backgroundColor),
  )).toEqual(expectedColors);
  await replaceEditorSource(page, "flowchart LR\nA[New source] --> B");
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  await expect(card).toHaveAttribute("aria-pressed", "true");
  await expect(card).toHaveAttribute("data-theme-treatment", "unreviewed");
  await expect(card).toContainText("Design not yet reviewed");
  await expect(panel.getByRole("button", { name: "Apply", exact: true })).toBeEnabled();
  await expect(reviewed.getByRole("listitem")).toHaveCount(1);
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
