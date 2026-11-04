import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";

import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import { optionalFeatureOutput } from "./helpers/build-manifest";
import { expectNoDocumentOverflow, monitorBrowserErrors, waitForPreviewSvg } from "./helpers/playground";

const workspace = {
  ...DEFAULT_WORKSPACE_SNAPSHOT,
  code: "flowchart LR\nA[Keep this diagram] --> B[Done]",
  mermaidConfig: '{"flowchart":{"htmlLabels":false,"curve":"linear"}}',
  diagramTheme: "neutral" as const,
  diagramFont: "trebuchet" as const,
  themePresetId: "brutalist",
};

async function start(page: Page, openPanel = true) {
  await page.addInitScript(() => localStorage.setItem("merman-language", "en"));
  await page.goto(`./${encodeShareHash(workspace)}`, { waitUntil: "domcontentloaded" });
  if ((page.viewportSize()?.width ?? 1280) < 768) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await waitForPreviewSvg(page);
  if (openPanel) await page.locator("[data-theme-launcher]:visible").click();
  return page.locator("#theme-panel");
}

test("one theme catalog loads with the appearance panel and searches every source", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const galleryOutput = optionalFeatureOutput("themes");
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  const panel = await start(page, false);
  expect(requests.some((url) => url.endsWith(galleryOutput))).toBe(false);
  let releaseLoad!: () => void;
  const loading = new Promise<void>((resolve) => { releaseLoad = resolve; });
  await page.route(`**/${galleryOutput}`, async (route) => { await loading; await route.continue(); });
  try {
    await page.locator("[data-theme-launcher]:visible").click();
    await expect(panel).toContainText("Loading Themes");
    await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
    await expect(panel).toBeHidden();
  } finally { releaseLoad(); }
  await expect(page.locator("[data-theme-launcher]:visible")).toBeFocused();
  await page.locator("[data-theme-launcher]:visible").click();
  const catalog = panel.getByTestId("theme-catalog");
  await expect(catalog.locator("[data-theme-exploration]")).toHaveCount(4);
  await expect(catalog.locator('[data-theme-choice="mermaid:forest"]')).toBeVisible();
  await expect(catalog.locator('[data-theme-choice="preset:brutalist"]')).toHaveAttribute("aria-pressed", "true");
  expect(requests.some((url) => url.endsWith(galleryOutput))).toBe(true);
  await expect(panel.getByRole("button", { name: "Browse themes", exact: true })).toHaveCount(0);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  const search = catalog.getByRole("searchbox", { name: "Search themes", exact: true });
  for (const [query, key] of [["forest", "mermaid:forest"], ["brutalist", "preset:brutalist"], ["nightfall", "exploration:nightfall"]]) {
    await search.fill(query);
    await expect(catalog.locator("[data-theme-choice]")).toHaveCount(1);
    await expect(catalog.locator(`[data-theme-choice="${key}"]`)).toBeVisible();
  }
  await search.fill("missing-theme");
  await expect(catalog).toContainText("No themes match");
  await search.press("Escape");
  await expect(search).toHaveValue("");
  await expect(panel).toBeVisible();
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Brutalist");
  errors.assertNone();
});

test("selecting an exploration updates the current diagram immediately and shares its original recipe and name", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  const panel = await start(page);
  await panel.locator('[data-theme-choice="exploration:atlas"]').click();
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Atlas");
  await expect(panel.getByTestId("theme-current-source")).toHaveText("Exploration");
  await expect(panel.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", "applied");
  await expect(panel.getByRole("button", { name: "Apply", exact: true })).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Cancel", exact: true })).toHaveCount(0);
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  const url = await page.evaluate(() => navigator.clipboard.readText());
  const applied = decodeShareHash(new URL(url).hash)!;
  const recipe = await readFile(new URL("../src/themes/recipes/atlas.theme.json", import.meta.url), "utf8");
  expect(applied.themeRecipeJson).toBe(recipe);
  expect(applied.themePresetId).toBeNull();
  expect(applied.themeRecipeLabel).toEqual({ name: "Atlas", source: "exploration" });
  for (const key of ["code", "mermaidConfig", "diagramTheme", "diagramFont", "svgPipeline", "textMeasurementMode"] as const) {
    expect(applied[key]).toBe(workspace[key]);
  }
  await page.goto(url, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  await page.locator("[data-theme-launcher]:visible").click();
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Atlas");
  await expect(panel.locator('[data-theme-choice="exploration:atlas"]')).toHaveAttribute("aria-pressed", "true");
  await panel.getByTestId("theme-advanced").locator("summary").first().click();
  await expect(panel.getByRole("textbox", { name: "Theme JSON", exact: true })).toHaveValue(recipe);
  errors.assertNone();
});

test("the four explorations label their five fixed samples as examples and download original recipes", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.setViewportSize({ width: 1440, height: 1000 });
  const panel = await start(page);
  for (const id of ["atlas", "nightfall", "letterpress", "grove"]) {
    await panel.locator(`[data-theme-choice="exploration:${id}"]`).click();
    const examples = panel.getByTestId("theme-examples");
    await examples.locator("summary").first().click();
    await expect(examples).toContainText("These are fixed examples");
    for (const sample of ["Flowchart", "Sequence", "Class", "XY Chart", "Pie"]) {
      await examples.getByRole("button", { name: sample, exact: true }).click();
      const preview = examples.getByTestId("theme-gallery-preview");
      await expect(preview).toHaveAttribute("alt", /^Example /);
      await expect.poll(() => preview.evaluate((image) => (image as HTMLImageElement).complete && (image as HTMLImageElement).naturalWidth > 0)).toBe(true);
    }
    const download = page.waitForEvent("download");
    await examples.getByRole("button", { name: /^Download .+ JSON$/ }).click();
    const artifact = await download;
    expect(artifact.suggestedFilename()).toBe(`${id}.theme.json`);
    expect(await readFile((await artifact.path())!, "utf8"))
      .toBe(await readFile(new URL(`../src/themes/recipes/${id}.theme.json`, import.meta.url), "utf8"));
  }
  const examples = panel.getByTestId("theme-examples");
  await examples.locator("summary").filter({ hasText: /^View sample source$/ }).click();
  await expect(examples.locator("pre").first()).toContainText("pie");
  await expect(examples.locator("pre").last()).toContainText("htmlLabels");
  await expectNoDocumentOverflow(page);
  errors.assertNone();
});

test("the unified catalog fits a narrow viewport and closing retains the selected theme", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  const panel = await start(page);
  await expectNoDocumentOverflow(page);
  await panel.locator('[data-theme-choice="exploration:letterpress"]').click();
  const examples = panel.getByTestId("theme-examples");
  await examples.locator("summary").first().click();
  await examples.getByRole("button", { name: "XY Chart", exact: true }).click();
  await expect(examples.getByTestId("theme-gallery-preview")).toBeVisible();
  await expectNoDocumentOverflow(page);
  await page.screenshot({ path: test.info().outputPath("appearance-catalog-mobile.png") });
  await page.keyboard.press("Escape");
  await expect(panel).toBeHidden();
  await expect(page.locator("[data-theme-launcher]:visible")).toBeFocused();
  await page.locator("[data-theme-launcher]:visible").click();
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Letterpress");
  await expect(panel.locator('[data-theme-choice="exploration:letterpress"]')).toHaveAttribute("aria-pressed", "true");
  errors.assertNone();
});

for (const id of ["atlas", "nightfall", "letterpress", "grove"]) {
  test(`${id} renders all five samples with the browser runtime`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    await page.addInitScript(() => localStorage.setItem("merman-language", "en"));
    const recipe = await readFile(new URL(`../src/themes/recipes/${id}.theme.json`, import.meta.url), "utf8");
    for (const sample of ["flowchart", "sequence", "class", "xychart", "pie"]) {
      const source = await readFile(new URL(`../src/themes/samples/${sample}.mmd`, import.meta.url), "utf8");
      const config = await readFile(new URL(`../src/themes/samples/${sample}.config.json`, import.meta.url), "utf8");
      await page.goto(`./${encodeShareHash({
        ...DEFAULT_WORKSPACE_SNAPSHOT, code: source, mermaidConfig: config,
        diagramTheme: "default", themeRecipeJson: recipe,
      })}`, { waitUntil: "domcontentloaded" });
      await waitForPreviewSvg(page);
      await page.locator("[data-theme-launcher]:visible").click();
      const panel = page.locator("#theme-panel");
      await expect(panel.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", "applied");
      await expect(panel.getByTestId("theme-diagnostic-reasons")).toHaveCount(0);
      if (sample === "flowchart") {
        const expectedInk: Record<string, string> = {
          atlas: "rgb(31, 42, 61)", nightfall: "rgb(231, 236, 244)",
          letterpress: "rgb(49, 44, 42)", grove: "rgb(38, 57, 47)",
        };
        const labels = await page.locator(".preview-container > div").first().evaluate((host) =>
          Array.from(host.shadowRoot!.querySelectorAll<SVGTextElement>(".edgeLabel text"))
            .filter((label) => ["Yes", "Revise"].includes(label.textContent?.trim() ?? ""))
            .map((label) => ({ text: label.textContent!.trim(), fill: getComputedStyle(label).fill })));
        expect(labels.map((label) => label.text).sort()).toEqual(["Revise", "Yes"]);
        expect(labels.map((label) => label.fill)).toEqual([expectedInk[id], expectedInk[id]]);
      }
    }
    errors.assertNone();
  });
}
