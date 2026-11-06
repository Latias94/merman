import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";
import { strFromU8, unzipSync } from "fflate";

import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import { optionalFeatureOutput } from "./helpers/build-manifest";
import { expectNoDocumentOverflow, monitorBrowserErrors, waitForPreviewSvg } from "./helpers/playground";

const designIds = ["riso", "phosphor", "field-notes", "atlas", "nightfall", "letterpress", "grove"];
const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, code: "flowchart LR\nA[Keep this diagram] --> B[Keep this source]",
  mermaidConfig: '{"flowchart":{"htmlLabels":false,"curve":"linear"}}', diagramTheme: "neutral" as const,
  diagramFont: "trebuchet" as const, themePresetId: "brutalist" };

async function start(page: Page) {
  await page.addInitScript(() => localStorage.setItem("merman-language", "en"));
  await page.goto(`./${encodeShareHash(workspace)}`, { waitUntil: "domcontentloaded" });
  if ((page.viewportSize()?.width ?? 1280) < 768) await page.getByRole("tab", { name: "Preview", exact: true }).click();
  await waitForPreviewSvg(page);
}

async function shareWorkspace(page: Page) {
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  return decodeShareHash(new URL(await page.evaluate(() => navigator.clipboard.readText())).hash)!;
}

test("theme designs load on demand and browsing five families preserves the workspace", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  const output = optionalFeatureOutput("themeShowcase");
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  await start(page);
  expect(requests.some((url) => url.endsWith(output))).toBe(false);
  const launcher = page.locator("[data-theme-showcase-launcher]:visible");
  await launcher.click();
  const gallery = page.getByTestId("theme-showcase");
  await expect(gallery.locator("[data-theme-design-card]")).toHaveCount(7);
  expect(requests.some((url) => url.endsWith(output))).toBe(true);
  await gallery.getByRole("button", { name: "Explore Riso", exact: true }).click();
  await expect(gallery.getByTestId("theme-showcase-detail")).toHaveAttribute("data-theme-design", "riso");
  const samples = gallery.locator("[data-theme-sample]");
  await expect(samples).toHaveCount(5);
  for (const family of ["flowchart", "sequence", "class", "xychart", "pie"]) {
    const image = gallery.locator(`[data-theme-sample="${family}"] img`);
    await image.scrollIntoViewIfNeeded();
    await expect.poll(() => image.evaluate((node) => (node as HTMLImageElement).complete && (node as HTMLImageElement).naturalWidth > 0)).toBe(true);
  }
  await gallery.getByRole("button", { name: "Close theme designs", exact: true }).click();
  await expect(launcher).toBeFocused();
  const saved = await shareWorkspace(page);
  for (const key of Object.keys(workspace) as (keyof typeof workspace)[]) expect(saved[key]).toEqual(workspace[key]);
  errors.assertNone();
});

test("design search and canvas filters reveal independent recipes across the collection", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page);
  await page.getByRole("button", { name: "Theme designs", exact: true }).click();
  const gallery = page.getByTestId("theme-showcase");
  await gallery.getByRole("button", { name: "Dark", exact: true }).click();
  await expect(gallery.locator("[data-theme-design-card]")).toHaveCount(2);
  await expect(gallery.locator('[data-theme-design-card="phosphor"]')).toBeVisible();
  await gallery.getByRole("button", { name: "All designs", exact: true }).click();
  const search = gallery.getByRole("searchbox", { name: "Search designs", exact: true });
  await search.fill("offset");
  await expect(gallery.locator("[data-theme-design-card]")).toHaveCount(1);
  await expect(gallery.locator('[data-theme-design-card="riso"]')).toBeVisible();
  await search.fill("no-such-design");
  await expect(gallery).toContainText("No themes match");
  await search.press("Escape");
  await expect(gallery).toBeVisible();
  await expect(gallery.locator("[data-theme-design-card]")).toHaveCount(7);
  errors.assertNone();
});

test("every design downloads its original JSON and sample links carry the actual source and configuration", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page);
  await page.getByRole("button", { name: "Theme designs", exact: true }).click();
  const gallery = page.getByTestId("theme-showcase");
  for (const id of designIds) {
    await gallery.locator(`[data-explore-theme="${id}"]`).click();
    const download = page.waitForEvent("download");
    await gallery.getByRole("button", { name: "Download theme JSON", exact: true }).click();
    const artifact = await download;
    const recipe = await readFile(new URL(`../src/themes/recipes/${id}.theme.json`, import.meta.url), "utf8");
    expect(artifact.suggestedFilename()).toBe(`${id}.theme.json`);
    expect(await readFile((await artifact.path())!, "utf8")).toBe(recipe);
    const links = gallery.locator("[data-theme-sample] a");
    await expect(links).toHaveCount(5);
    for (const link of await links.all()) {
      const url = new URL((await link.getAttribute("href"))!, page.url());
      const shared = decodeShareHash(url.hash)!;
      expect(shared.themeRecipeJson).toBe(recipe);
      expect(shared.themePresetId).toBeNull();
      expect(shared.diagramFont).toBe("inherit");
      const family = await link.locator("..").locator("..").getAttribute("data-theme-sample");
      expect(shared.code).toBe(await readFile(new URL(`../src/themes/samples/${family}.mmd`, import.meta.url), "utf8"));
      expect(shared.mermaidConfig).toBe(await readFile(new URL(`../src/themes/samples/${family}.config.json`, import.meta.url), "utf8"));
    }
    await gallery.getByRole("button", { name: "All designs", exact: true }).click();
    await expect(gallery.locator(`[data-explore-theme="${id}"]`)).toBeFocused();
  }
  errors.assertNone();
});

test("theme adoption copies JSON and configuration and bundles all three consumers for the selected family", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await start(page);
  await page.getByRole("button", { name: "Theme designs", exact: true }).click();
  const gallery = page.getByTestId("theme-showcase");
  await gallery.getByRole("button", { name: "Explore Riso", exact: true }).click();
  const adoption = gallery.getByTestId("theme-showcase-use");
  const recipe = await readFile(new URL("../src/themes/recipes/riso.theme.json", import.meta.url), "utf8");
  await adoption.getByRole("button", { name: "Copy", exact: true }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(recipe);
  await adoption.getByRole("combobox", { name: "Example diagram", exact: true }).selectOption("sequence");
  await adoption.getByRole("tab", { name: "Sample config", exact: true }).click();
  await adoption.getByRole("button", { name: "Copy", exact: true }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(await readFile(new URL("../src/themes/samples/sequence.config.json", import.meta.url), "utf8"));
  for (const [target, label, filename] of [["rust", "Rust", "src/main.rs"], ["cli", "CLI", "render-command.txt"], ["web", "Web", "main.js"]]) {
    await adoption.getByRole("tab", { name: label, exact: true }).click();
    await expect(adoption.getByTestId("theme-showcase-code")).not.toBeEmpty();
    const download = page.waitForEvent("download");
    await adoption.getByRole("button", { name: "Download example project", exact: true }).click();
    const artifact = await download;
    expect(artifact.suggestedFilename()).toBe(`riso-sequence-${target}.zip`);
    const files = unzipSync(await readFile((await artifact.path())!));
    expect(strFromU8(files["theme.json"])).toBe(recipe);
    expect(strFromU8(files["diagram.mmd"])).toContain("sequenceDiagram");
    expect(files[filename]).toBeDefined();
    const record = JSON.parse(strFromU8(files["handoff.json"]));
    expect(record.publication_id).toBeNull();
    expect(record.scope).toBe("theme");
    expect(record.theme.preset_id).toBeNull();
    expect(JSON.parse(strFromU8(files["options.json"])).theme).toEqual(JSON.parse(recipe));
  }
  errors.assertNone();
});

test("applying a design changes only the theme and remains undoable", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await start(page);
  await page.getByRole("button", { name: "Theme designs", exact: true }).click();
  const gallery = page.getByTestId("theme-showcase");
  await gallery.getByRole("button", { name: "Explore Riso", exact: true }).click();
  await gallery.getByRole("button", { name: "Apply to my diagram", exact: true }).click();
  await expect(gallery).toBeHidden();
  const saved = await shareWorkspace(page);
  expect(saved.themeRecipeJson).toBe(await readFile(new URL("../src/themes/recipes/riso.theme.json", import.meta.url), "utf8"));
  expect(saved.themeRecipeLabel).toEqual({ name: "Riso", source: "exploration" });
  expect(saved.themePresetId).toBeNull();
  for (const key of ["code", "mermaidConfig", "diagramTheme", "diagramFont", "svgPipeline", "textMeasurementMode"] as const) expect(saved[key]).toBe(workspace[key]);
  await page.locator("[data-theme-launcher]:visible").click();
  const panel = page.locator("#theme-panel");
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Riso");
  await panel.getByRole("button", { name: "Undo appearance change", exact: true }).click();
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Brutalist");
  errors.assertNone();
});

test("theme designs keep all controls reachable on a narrow screen", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await start(page);
  await page.getByRole("button", { name: "Theme designs", exact: true }).click();
  const gallery = page.getByTestId("theme-showcase");
  await expectNoDocumentOverflow(page);
  await gallery.getByRole("button", { name: "Explore Field Notes", exact: true }).click();
  await expect(gallery.locator("[data-theme-sample]")).toHaveCount(5);
  await gallery.getByTestId("theme-showcase-use").scrollIntoViewIfNeeded();
  await gallery.getByRole("tab", { name: "CLI", exact: true }).click();
  await expect(gallery.getByRole("button", { name: "Download example project", exact: true })).toBeInViewport();
  await expectNoDocumentOverflow(page);
  await page.screenshot({ path: test.info().outputPath("theme-designs-mobile.png") });
  await gallery.getByRole("button", { name: "Close theme designs", exact: true }).click();
  await expect(page.locator("[data-theme-showcase-launcher]:visible")).toBeFocused();
  errors.assertNone();
});

test("the appearance-panel entry follows the selected design when reopened", async ({ page }) => {
  await start(page);
  await page.locator("[data-theme-launcher]:visible").click();
  const panel = page.locator("#theme-panel");
  for (const [id, name] of [["riso", "Riso"], ["phosphor", "Phosphor"]]) {
    await panel.locator(`[data-theme-choice="exploration:${id}"]`).click();
    await panel.getByRole("button", { name: "Theme designs", exact: true }).click();
    const gallery = page.getByTestId("theme-showcase");
    await expect(gallery.getByTestId("theme-showcase-detail")).toHaveAttribute("data-theme-design", id);
    await expect(gallery.getByRole("heading", { name, exact: true })).toBeVisible();
    await expect(gallery.getByText("Current theme", { exact: true })).toBeVisible();
    await gallery.getByRole("button", { name: "Close theme designs", exact: true }).click();
    await expect(panel.getByRole("button", { name: "Theme designs", exact: true })).toBeFocused();
  }
});

test("a failed runtime keeps JSON usable and offers recovery for integration code", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("merman-language", "en"));
  let failWasm = true;
  await page.route(/merman_wasm_bg-[\w-]+\.wasm(?:\?|$)/, async (route) => {
    if (failWasm) await route.abort("failed");
    else await route.continue();
  });
  await page.goto("./", { waitUntil: "domcontentloaded" });
  await page.getByRole("button", { name: "Theme designs", exact: true }).click();
  const gallery = page.getByTestId("theme-showcase");
  await gallery.getByRole("button", { name: "Explore Riso", exact: true }).click();
  const adoption = gallery.getByTestId("theme-showcase-use");
  await expect(adoption.getByTestId("theme-showcase-code")).toContainText('"kind": "design"');
  await adoption.getByRole("tab", { name: "Rust", exact: true }).click();
  await expect(adoption.getByRole("alert")).toContainText("You can still copy the theme JSON");
  failWasm = false;
  await adoption.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(adoption.getByTestId("theme-showcase-code")).toContainText("compile_recipe");
  await expect(adoption.getByRole("alert")).toHaveCount(0);
});
