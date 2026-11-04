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

async function start(page: Page) {
  await page.addInitScript(() => localStorage.setItem("merman-language", "en"));
  await page.goto(`./${encodeShareHash(workspace)}`, { waitUntil: "domcontentloaded" });
  if ((page.viewportSize()?.width ?? 1280) < 768) {
    await page.getByRole("tab", { name: "Preview", exact: true }).click();
  }
  await waitForPreviewSvg(page);
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  return page.locator("#theme-panel");
}

async function browse(page: Page) {
  await page.getByRole("button", { name: "Browse themes", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Theme library", exact: true });
  await expect(dialog).toBeVisible();
  return dialog;
}

test("theme gallery loads on demand and browsing preserves the current theme draft", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const galleryOutput = optionalFeatureOutput("themes");
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  const panel = await start(page);
  await panel.getByLabel("Mermaid base theme", { exact: true }).selectOption("forest");
  await expect(panel.locator("[data-theme-draft-status]")).toContainText("Theme preview");
  expect(requests.some((url) => url.endsWith(galleryOutput))).toBe(false);
  let releaseLoad!: () => void;
  const loading = new Promise<void>((resolve) => { releaseLoad = resolve; });
  await page.route(`**/${galleryOutput}`, async (route) => { await loading; await route.continue(); });
  await page.getByRole("button", { name: "Browse themes", exact: true }).click();
  await expect(page.getByRole("dialog")).toContainText("Loading Theme library");
  await page.keyboard.press("Escape");
  releaseLoad();
  await expect(page.getByRole("dialog")).toBeHidden();
  await expect(panel).toBeVisible();
  const dialog = await browse(page);
  expect(requests.some((url) => url.endsWith(galleryOutput))).toBe(true);
  await expect(dialog.locator("[data-theme-exploration]")).toHaveCount(4);
  await dialog.getByLabel("Appearance", { exact: true }).selectOption("dark");
  await expect(dialog.locator("[data-theme-exploration]")).toHaveCount(1);
  await dialog.locator('[data-theme-exploration="nightfall"]').click();
  await expect(dialog.getByRole("heading", { name: "Nightfall", exact: true })).toBeVisible();
  await dialog.getByLabel("Appearance", { exact: true }).selectOption("all");
  await dialog.getByRole("searchbox", { name: "Search themes", exact: true }).fill("missing-theme");
  await expect(dialog).toContainText("No themes match");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(panel).toBeVisible();
  await expect(panel.getByLabel("Mermaid base theme", { exact: true })).toHaveValue("forest");
  await expect(panel.getByLabel("Merman preset", { exact: true })).toHaveValue("preset:brutalist");
  await expect(panel.locator("[data-theme-draft-status]")).toContainText("Theme preview");
  await expect(panel.getByRole("button", { name: "Browse themes", exact: true })).toBeFocused();
  errors.assertNone();
});

test("trying an exploration uses Apply and Cancel and shares the standalone recipe", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  let panel = await start(page);
  let dialog = await browse(page);
  await dialog.locator('[data-theme-exploration="nightfall"]').click();
  await dialog.getByRole("button", { name: "Try on current diagram", exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(panel.getByLabel("Merman preset", { exact: true })).toHaveValue("custom");
  await expect(panel.getByRole("button", { name: "Apply", exact: true })).toBeEnabled();
  await expect(panel.getByTestId("theme-render-evidence")).not.toHaveAttribute("data-theme-evidence-state", "failed");
  await panel.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  panel = page.locator("#theme-panel");
  await expect(panel.getByLabel("Merman preset", { exact: true })).toHaveValue("preset:brutalist");
  dialog = await browse(page);
  await dialog.locator('[data-theme-exploration="atlas"]').click();
  await dialog.getByRole("button", { name: "Try on current diagram", exact: true }).click();
  await expect(panel.getByRole("button", { name: "Apply", exact: true })).toBeEnabled();
  await panel.getByRole("button", { name: "Apply", exact: true }).click();
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  const url = await page.evaluate(() => navigator.clipboard.readText());
  const applied = decodeShareHash(new URL(url).hash)!;
  const recipe = await readFile(new URL("../src/themes/recipes/atlas.theme.json", import.meta.url), "utf8");
  expect(applied.themeRecipeJson).toBe(recipe);
  expect(applied.themePresetId).toBeNull();
  for (const key of ["code", "mermaidConfig", "diagramTheme", "diagramFont", "svgPipeline", "textMeasurementMode"] as const) {
    expect(applied[key]).toBe(workspace[key]);
  }
  await page.goto(url, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  await page.getByRole("button", { name: "Theme", exact: true }).click();
  panel = page.locator("#theme-panel");
  await panel.locator("summary").filter({ hasText: /^Advanced$/ }).click();
  await expect(panel.getByRole("textbox", { name: "Theme JSON", exact: true })).toHaveValue(recipe);
  errors.assertNone();
});

test("the four explorations show five real samples and download their original recipes", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.setViewportSize({ width: 1440, height: 1000 });
  await start(page);
  const dialog = await browse(page);
  for (const id of ["atlas", "nightfall", "letterpress", "grove"]) {
    await dialog.locator(`[data-theme-exploration="${id}"]`).click();
    for (const sample of ["Flowchart", "Sequence", "Class", "XY Chart", "Pie"]) {
      await dialog.getByRole("button", { name: sample, exact: true }).click();
      const preview = dialog.getByTestId("theme-gallery-preview");
      await expect.poll(() => preview.evaluate((image) => (image as HTMLImageElement).complete && (image as HTMLImageElement).naturalWidth > 0)).toBe(true);
      await dialog.getByTestId("theme-gallery-detail").screenshot({ path: test.info().outputPath(`${id}-${sample.replaceAll(" ", "-").toLowerCase()}.png`) });
    }
    const download = page.waitForEvent("download");
    await dialog.getByRole("button", { name: /^Download .+ JSON$/ }).click();
    const artifact = await download;
    expect(artifact.suggestedFilename()).toBe(`${id}.theme.json`);
    expect(await readFile((await artifact.path())!, "utf8"))
      .toBe(await readFile(new URL(`../src/themes/recipes/${id}.theme.json`, import.meta.url), "utf8"));
  }
  await dialog.locator("summary").filter({ hasText: /^View sample source$/ }).click();
  await expect(dialog.locator("pre").first()).toContainText("pie");
  await expect(dialog.locator("pre").last()).toContainText("htmlLabels");
  await expectNoDocumentOverflow(page);
  await page.screenshot({ path: test.info().outputPath("theme-library-desktop.png") });
  errors.assertNone();
});

test("the theme library fits a narrow viewport and Escape retains the sidebar", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  const panel = await start(page);
  const dialog = await browse(page);
  await expectNoDocumentOverflow(page);
  await page.screenshot({ path: test.info().outputPath("theme-library-mobile-browse.png") });
  await dialog.locator('[data-theme-exploration="letterpress"]').click();
  await dialog.getByRole("button", { name: "XY Chart", exact: true }).click();
  await expect(dialog.getByTestId("theme-gallery-preview")).toBeVisible();
  await expectNoDocumentOverflow(page);
  await page.screenshot({ path: test.info().outputPath("theme-library-mobile-detail.png") });
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(panel).toBeVisible();
  await expect(panel.getByRole("button", { name: "Browse themes", exact: true })).toBeFocused();
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
      await page.getByRole("button", { name: "Theme", exact: true }).click();
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
