import { expect, test } from "@playwright/test";
import type { ThemePresetCatalogEntry } from "@mermanjs/web";
import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { monitorBrowserErrors, replaceEditorSource, waitForPreviewSvg } from "./helpers/playground";

test("theme selection survives family changes with optional whole-document customization", async ({ page }) => {
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
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  const workbench = page.getByRole("region", { name: "Appearance", exact: true });
  const scenes = [
    { family: "Flowchart", source: null, treatment: "dedicated", label: "Designed for this diagram" },
    { family: "Class", source: "classDiagram\nclass Account {\n  +String name\n}", treatment: "base_only", label: "Shared styling" },
    { family: "XY Chart", source: 'xychart-beta\nx-axis [Jan, Feb]\ny-axis "Count" 0 --> 10\nbar [3, 7]', treatment: "dedicated", label: "Designed for this diagram" },
    { family: "Packet", source: 'packet\n0-7: "Header"', treatment: "unreviewed", label: "Not yet reviewed for this diagram" },
  ];
  for (const scene of scenes) {
    if (scene.source) await replaceEditorSource(page, scene.source);
    await waitForPreviewSvg(page);
    if (!scene.source) {
      await page.screenshot({ path: test.info().outputPath("theme-panel-selection.png") });
    }
    await expect(workbench.getByRole("button", { name: "Customize", exact: true })).toBeEnabled();
    await expect(workbench.getByRole("button", { name: /^(New recipe|Create from)/ })).toHaveCount(0);
    await expect(workbench.getByTestId("theme-draft-workbench")).toHaveCount(0);
    await expect(workbench.getByTestId("theme-current-family")).toHaveText(`Current diagram: ${scene.family}`);
    const selected = workbench.locator('[data-theme-choice="preset:brutalist"]');
    await expect(selected).toHaveAttribute("aria-pressed", "true");
    await expect(selected).toHaveAttribute("data-theme-treatment", scene.treatment);
    await expect(selected).toContainText(scene.label);
  }
  await workbench.getByTestId("theme-advanced").locator("summary").first().click();
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
      await page.getByRole("button", { name: language === "en" ? "Appearance" : "外观", exact: true }).click();
      const panel = page.getByRole("region", { name: language === "en" ? "Appearance" : "外观", exact: true });
      await expect(panel.getByTestId("theme-current-family")).toHaveText(language === "en"
        ? `Current diagram: ${scene.family}` : `当前图表：${scene.zhFamily}`);
      const selected = panel.locator('[data-theme-choice="preset:brutalist"]');
      await expect(selected).toHaveAttribute("aria-pressed", "true");
      await expect(selected).toHaveAttribute("data-theme-treatment", "unreviewed");
      await expect(selected).toContainText(language === "en" ? "Not yet reviewed for this diagram" : "尚未针对此图表评审");
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
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  const panel = page.getByRole("region", { name: "Appearance", exact: true });
  const card = panel.locator('[data-theme-choice="preset:ghibli"]');
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Pie Chart");
  await expect(card).toHaveAttribute("data-theme-treatment", "dedicated");
  await panel.getByTestId("theme-advanced").locator("summary").first().click();
  const reviewed = panel.getByTestId("theme-reviewed-families");
  await expect(reviewed.getByRole("listitem")).toHaveCount(1);
  await expect(reviewed.getByRole("listitem")).toContainText("Pie Chart");
  await expect(reviewed.getByRole("listitem")).toContainText("Designed for this diagram");
  const catalogSummary = panel.locator("summary").filter({ hasText: /^Preset runtime details$/ });
  await catalogSummary.click();
  const details = catalogSummary.locator("..");
  const catalogEntry = JSON.parse((await details.locator("pre").textContent())!) as ThemePresetCatalogEntry;
  const appearance = catalogEntry.appearances.find((entry) => entry.appearance === catalogEntry.default_appearance)!;
  expect(appearance).toBeDefined();
  expect(appearance.preview_colors.length).toBeGreaterThan(0);
  const expectedColors = await page.evaluate((colors: string[]) => colors.map((color) => {
    const style = document.createElement("span").style;
    style.backgroundColor = color;
    return style.backgroundColor;
  }), appearance.preview_colors);
  expect(await card.locator("[data-theme-swatches] > span").evaluateAll((spans) =>
    spans.map((span) => (span as HTMLElement).style.backgroundColor),
  )).toEqual(expectedColors);
  await replaceEditorSource(page, "flowchart LR\nA[New source] --> B");
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  await expect(card).toHaveAttribute("aria-pressed", "true");
  await expect(card).toHaveAttribute("data-theme-treatment", "unreviewed");
  await expect(card).toContainText("Not yet reviewed for this diagram");
  await expect(panel.getByTestId("theme-render-evidence")).not.toHaveAttribute("data-theme-evidence-state", "waiting");
  const execution = JSON.parse((await panel.getByTestId("theme-execution-metadata").textContent())!);
  expect(execution.theme_execution_evidence.family_id).toBe("flowchart");
  await expect(reviewed.getByRole("listitem")).toHaveCount(1);
});


test("an unknown saved preset keeps its selection while preview and export use the base theme", async ({ page }) => {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
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
  await waitForPreviewSvg(page);
  await expect(page.getByTestId("theme-preset-fallback")).toContainText("future-preset");
  await expect(page.getByTestId("theme-preset-fallback")).toContainText("Mermaid base theme");
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  const shareUrl = await page.evaluate(() => navigator.clipboard.readText());
  expect(decodeShareHash(new URL(shareUrl).hash)?.themePresetId).toBe("future-preset");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitem", { name: "Export image…", exact: true }).click();
  await expect(page.getByTestId("export-theme-preset-fallback")).toContainText("future-preset was not applied");
  await page.getByRole("button", { name: "Close export", exact: true }).click();
  await page.getByRole("button", { name: "Tools", exact: true }).click();
  await page.getByRole("menuitem", { name: "Compare", exact: true }).click();
  await expect(page.locator('[data-merman-compare-engine="merman"]')).toHaveAttribute("data-merman-artifact-state", "ready");
  await expect(page.locator('[data-merman-compare-engine="mermaid"]')).toHaveAttribute("data-merman-artifact-state", "ready");
  const fills = await page.locator(".preview-container > div").evaluateAll((hosts) => hosts.map((host) => {
    const node = host.shadowRoot?.querySelector("g.node rect");
    return node ? getComputedStyle(node).fill : null;
  }));
  expect(fills).toHaveLength(2);
  expect(fills[0]).not.toBeNull();
  expect(fills[0]).toBe(fills[1]);
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  const workbench = page.getByRole("region", { name: "Appearance", exact: true });
  await expect(workbench.locator("header")).toContainText("future-preset");
  await expect(workbench).toContainText("future-preset");
  const unavailable = workbench.locator('[data-theme-choice="preset:future-preset"]');
  await expect(unavailable).toBeDisabled();
  await expect(unavailable).toHaveAttribute("aria-pressed", "true");
  await expect(unavailable).toContainText("Unavailable in this runtime");
  await expect(unavailable).toContainText("Selection preserved");
  await expect(workbench.getByRole("button", { name: "Download JSON", exact: true })).toBeDisabled();
  await expect(workbench.getByTestId("theme-catalog-choices").locator('button[aria-pressed="true"]')).toHaveCount(1);
  await expect(workbench.getByTestId("theme-outcome")).toContainText("Mermaid base theme");
  await page.screenshot({ path: test.info().outputPath("unavailable-preset-desktop.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByTestId("theme-preset-fallback")).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: test.info().outputPath("unavailable-preset-mobile.png") });
});
