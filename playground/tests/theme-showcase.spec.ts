import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";
import { strFromU8, unzipSync } from "fflate";

import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import { CORE_THEME_SAMPLE_IDS, type ThemeSampleId } from "../src/themes/families";
import { optionalFeatureOutput } from "./helpers/build-manifest";
import { expectNoDocumentOverflow, monitorBrowserErrors, waitForPreviewSvg } from "./helpers/playground";
import { chooseThemeOption } from "./helpers/theme-select";
import { readSavedThemeRecipe } from "./helpers/theme-recipe";

const changelogDesignIds = ["riso", "pop-art", "metro", "carbon-copy", "signal", "field-notes", "lacquer", "paper-cut", "aurora", "phosphor", "letterpress", "grove", "atlas", "nightfall", "blueprint"];
const authoredDesignIds = [...changelogDesignIds, "reading", "porcelain", "chalk", "prism", "monograph"];
const designIds = [...authoredDesignIds, "brutalist", "cyberpunk", "ghibli", "spotless", "editor", "one-dark", "gruvbox", "ayu"];
const expandedSamples: Readonly<Record<string, readonly ThemeSampleId[]>> = {
  metro: ["gitGraph"], "carbon-copy": ["kanban"], signal: ["gantt"], "field-notes": ["timeline"], blueprint: ["architecture"],
  prism: ["radar", "quadrantChart", "sankey", "treemap"], monograph: ["requirement", "treeView"],
};

function samplesFor(id: string): ThemeSampleId[] {
  if (id === "pop-art") return ["pie", ...CORE_THEME_SAMPLE_IDS.filter((sample) => sample !== "pie")];
  if (id === "paper-cut") return ["sequence", ...CORE_THEME_SAMPLE_IDS.filter((sample) => sample !== "sequence")];
  const extra = expandedSamples[id] ?? [];
  return extra.length ? [extra[0], ...CORE_THEME_SAMPLE_IDS, ...extra.slice(1)] : [...CORE_THEME_SAMPLE_IDS];
}
const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, code: "flowchart LR\nA[Keep this diagram] --> B[Keep this source]",
  mermaidConfig: '{"flowchart":{"htmlLabels":false,"curve":"linear"}}', diagramTheme: "neutral" as const,
  diagramFont: "trebuchet" as const, themePresetId: "brutalist" };

async function start(page: Page) {
  await page.addInitScript(() => {
    if (window === window.top && (location.protocol === "http:" || location.protocol === "https:")) localStorage.setItem("merman-language", "en");
  });
  await page.goto(`./${encodeShareHash(workspace)}`, { waitUntil: "domcontentloaded" });
  if ((page.viewportSize()?.width ?? 1280) < 768) await page.getByRole("tab", { name: "Preview", exact: true }).click();
  await waitForPreviewSvg(page);
}

async function shareWorkspace(page: Page) {
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  return decodeShareHash(new URL(await page.evaluate(() => navigator.clipboard.readText())).hash)!;
}

test("changelog design links open every theme directly without changing the workspace", async ({ page }) => {
  test.setTimeout(90_000);
  const errors = monitorBrowserErrors(page);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.addInitScript(() => {
    if (window === window.top && (location.protocol === "http:" || location.protocol === "https:")) localStorage.setItem("merman-language", "en");
  });
  const changelog = await readFile(new URL("../../CHANGELOG.md", import.meta.url), "utf8");
  const links = [...changelog.matchAll(/\[\*\*([^*]+)\*\*\]\((https:\/\/frankorz\.com\/merman\/\?theme-design=([a-z-]+))\)/gu)];
  expect(links.map((match) => match[3])).toEqual(changelogDesignIds);
  for (const [, name, href, id] of links) {
    const url = new URL(href);
    await page.goto(`./${url.search}${encodeShareHash(workspace)}`, { waitUntil: "domcontentloaded" });
    const gallery = page.getByTestId("theme-showcase");
    await expect(gallery.getByTestId("theme-showcase-detail")).toHaveAttribute("data-theme-design", id);
    await expect(gallery.getByRole("heading", { name, exact: true })).toBeVisible();
    const tiles = gallery.getByTestId("theme-sample-gallery").locator("article[data-theme-sample]");
    await expect(tiles).toHaveCount(samplesFor(id).length);
    expect(await tiles.evaluateAll((elements) => elements.map((element) => element.getAttribute("data-theme-sample")))).toEqual(samplesFor(id));
    await gallery.getByRole("button", { name: "Close theme designs", exact: true }).click();
    await expect(page.locator("[data-theme-showcase-launcher]:visible")).toBeFocused();
  }
  const saved = await shareWorkspace(page);
  for (const key of Object.keys(workspace) as (keyof typeof workspace)[]) expect(saved[key]).toEqual(workspace[key]);
  errors.assertNone();
});

test("invalid design links retain lazy loading and unknown designs open the collection", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const output = optionalFeatureOutput("themeShowcase");
  const requests: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  for (const search of ["?theme-design=", "?theme-design=riso&theme-design=metro", "?theme-design=%3Cscript%3E"]) {
    requests.length = 0;
    await page.goto(`./${search}${encodeShareHash(workspace)}`, { waitUntil: "domcontentloaded" });
    await waitForPreviewSvg(page);
    await expect(page.getByTestId("theme-showcase")).toHaveCount(0);
    expect(requests.some((url) => url.endsWith(output))).toBe(false);
  }
  await page.goto(`./?theme-design=unknown-design${encodeShareHash(workspace)}`, { waitUntil: "domcontentloaded" });
  const gallery = page.getByTestId("theme-showcase");
  await expect(gallery.getByTestId("theme-showcase-detail")).toHaveCount(0);
  const family = gallery.getByRole("combobox", { name: "Diagram family filter", exact: true });
  await expect(family).toHaveAttribute("data-value", "flowchart");
  await chooseThemeOption(page, family, "");
  await expect(gallery.locator("[data-theme-design-card]")).toHaveCount(designIds.length);
  errors.assertNone();
});

test("theme designs load on demand and browsing eight families preserves the workspace", async ({ page }) => {
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
  await chooseThemeOption(page, gallery.getByRole("combobox", { name: "Diagram family filter", exact: true }), "");
  await expect(gallery.locator("[data-theme-design-card]")).toHaveCount(designIds.length);
  await expect(gallery.locator("[data-theme-design-card]").first()).toHaveAttribute("data-theme-design-card", "riso");
  expect((await gallery.locator("[data-theme-design-card]").evaluateAll((elements) =>
    elements.map((element) => element.getAttribute("data-theme-design-card")))).sort()).toEqual([...designIds].sort());
  expect(requests.some((url) => url.endsWith(output))).toBe(true);
  await gallery.getByRole("button", { name: "Explore Riso", exact: true }).click();
  await expect(gallery.getByTestId("theme-showcase-detail")).toHaveAttribute("data-theme-design", "riso");
  const samples = gallery.getByTestId("theme-sample-gallery");
  await expect(samples.locator("article[data-theme-sample]")).toHaveCount(8);
  await expect(samples.getByRole("link")).toHaveCount(8);
  for (const family of CORE_THEME_SAMPLE_IDS) {
    const tile = samples.locator(`article[data-theme-sample="${family}"]`);
    const image = tile.locator("img");
    await tile.scrollIntoViewIfNeeded();
    await expect(tile.getByRole("button", { name: /^Inspect .+ example$/u })).toBeVisible();
    await expect(tile.getByRole("button", { name: "Use this example", exact: true })).toBeVisible();
    await image.scrollIntoViewIfNeeded();
    await expect.poll(() => image.evaluate((node) => (node as HTMLImageElement).complete && (node as HTMLImageElement).naturalWidth > 0)).toBe(true);
  }
  await gallery.getByRole("button", { name: "Close theme designs", exact: true }).click();
  await expect(launcher).toBeFocused();
  const saved = await shareWorkspace(page);
  for (const key of Object.keys(workspace) as (keyof typeof workspace)[]) expect(saved[key]).toEqual(workspace[key]);
  errors.assertNone();
});

test("design search and canvas filters reveal authored and canonical designs across the collection", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page);
  await page.getByRole("button", { name: "Theme designs", exact: true }).click();
  const gallery = page.getByTestId("theme-showcase");
  await chooseThemeOption(page, gallery.getByRole("combobox", { name: "Diagram family filter", exact: true }), "");
  await gallery.getByRole("button", { name: "Dark", exact: true }).click();
  const darkCards = gallery.locator("[data-theme-design-card]");
  const darkIds = await darkCards.evaluateAll((elements) => elements.map((element) => element.getAttribute("data-theme-design-card")));
  expect(darkIds).toEqual(expect.arrayContaining(["metro", "lacquer", "aurora", "phosphor", "reading", "nightfall", "blueprint", "chalk", "prism", "cyberpunk", "editor"]));
  for (const card of await darkCards.all()) {
    await expect(card.locator("[data-theme-preview-appearance]").first()).toHaveAttribute("data-theme-preview-appearance", "dark");
  }
  await expect(gallery.locator('[data-theme-design-card="phosphor"]')).toBeVisible();
  await expect(gallery.locator('[data-theme-design-card="prism"]')).toHaveCount(1);
  await gallery.getByRole("button", { name: "All designs", exact: true }).click();
  const search = gallery.getByRole("searchbox", { name: "Search designs", exact: true });
  await search.fill("two inks");
  await expect(gallery.locator("[data-theme-design-card]")).toHaveCount(1);
  await expect(gallery.locator('[data-theme-design-card="riso"]')).toBeVisible();
  await search.fill("no-such-design");
  await expect(gallery).toContainText("No matching designs");
  await search.press("Escape");
  await expect(gallery).toBeVisible();
  await expect(gallery.locator("[data-theme-design-card]")).toHaveCount(designIds.length);
  errors.assertNone();
});

test("all twenty authored designs download their original JSON and sample links carry the actual source and configuration", async ({ page }) => {
  test.setTimeout(90_000);
  const errors = monitorBrowserErrors(page);
  await start(page);
  await page.getByRole("button", { name: "Theme designs", exact: true }).click();
  const gallery = page.getByTestId("theme-showcase");
  expect(authoredDesignIds).toHaveLength(20);
  for (const id of authoredDesignIds) {
    await gallery.locator(`[data-explore-theme="${id}"]`).click();
    const download = page.waitForEvent("download");
    await gallery.getByRole("button", { name: "Download theme JSON", exact: true }).click();
    const artifact = await download;
    const recipe = await readSavedThemeRecipe(id);
    expect(artifact.suggestedFilename()).toBe(`${id}.theme.json`);
    expect(await readFile((await artifact.path())!, "utf8")).toBe(recipe);
    const samples = gallery.getByTestId("theme-sample-gallery");
    const expectedSamples = samplesFor(id);
    await expect(samples.getByRole("link")).toHaveCount(expectedSamples.length);
    expect(await samples.locator("article[data-theme-sample]").evaluateAll((elements) => elements.map((element) => element.getAttribute("data-theme-sample")))).toEqual(expectedSamples);
    for (const family of expectedSamples) {
      const link = samples.locator(`article[data-theme-sample="${family}"]`).getByRole("link");
      await expect(link).toHaveCount(1);
      const url = new URL((await link.getAttribute("href"))!, page.url());
      const shared = decodeShareHash(url.hash)!;
      expect(shared.themeRecipeJson).toBe(recipe);
      expect(shared.themePresetId).toBeNull();
      expect(shared.diagramFont).toBe("inherit");
      expect(shared.code).toBe(await readFile(new URL(`../src/themes/samples/${family}.mmd`, import.meta.url), "utf8"));
      expect(shared.code).not.toMatch(/\p{Script=Han}/u);
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
  await expect(adoption.getByRole("tab", { name: "Sample config", exact: true })).toHaveCount(0);
  await expect(adoption.getByTestId("theme-adoption-guide")).toContainText("already include them");
  await adoption.getByRole("button", { name: "Copy", exact: true }).click();
  expect((await page.evaluate(() => navigator.clipboard.readText())).replace(/\r\n/gu, "\n")).toBe(recipe.replace(/\r\n/gu, "\n"));
  await adoption.getByRole("tab", { name: "Rust", exact: true }).click();
  await chooseThemeOption(page, adoption.getByRole("combobox", { name: "Example diagram for this code", exact: true }), "sequence");
  await expect(gallery.getByTestId("theme-sample-gallery").locator('article[data-theme-sample="sequence"]')).toHaveAttribute("data-theme-sample-selected", "true");
  await adoption.locator("summary").click();
  await adoption.getByRole("button", { name: "Copy sequence.config.json", exact: true }).click();
  expect(JSON.parse(await page.evaluate(() => navigator.clipboard.readText()))).toEqual(JSON.parse(await readFile(new URL("../src/themes/samples/sequence.config.json", import.meta.url), "utf8")));
  await adoption.locator("summary").click();
  for (const [target, label, filename] of [["rust", "Rust", "src/main.rs"], ["cli", "CLI", "render-command.txt"], ["web", "Web", "main.js"]]) {
    await adoption.getByRole("tab", { name: label, exact: true }).click();
    if (target === "rust") await adoption.getByRole("radio", { name: "Project files · include_str!", exact: true }).check();
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

test("Rust offers a highlighted single-file example and explains the include_str project layout", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await start(page);
  await page.getByRole("button", { name: "Theme designs", exact: true }).click();
  const gallery = page.getByTestId("theme-showcase");
  await gallery.getByRole("button", { name: "Explore Riso", exact: true }).click();
  const adoption = gallery.getByTestId("theme-showcase-use");
  await expectHighlighted(adoption.getByTestId("theme-showcase-code"), "json");
  await adoption.getByRole("tab", { name: "Rust", exact: true }).click();
  await expect(adoption.getByRole("radio", { name: "Single file · copy into main.rs", exact: true })).toBeChecked();
  const code = adoption.getByTestId("theme-showcase-code");
  await expectHighlighted(code, "rust");
  await adoption.getByRole("button", { name: "Copy", exact: true }).click();
  const inline = (await page.evaluate(() => navigator.clipboard.readText())).replace(/\r\n/gu, "\n");
  expect(inline).toContain("compile_document");
  expect(inline).toContain("fn main()");
  expect(inline).not.toContain("include_str!");
  expect(inline).toContain('"schema_version": 1');
  await expect(adoption.getByRole("navigation", { name: "Project files", exact: true })).toHaveCount(0);
  await expect(adoption.getByRole("combobox", { name: "File", exact: true })).toHaveCount(0);
  const singleDownload = page.waitForEvent("download");
  await adoption.getByRole("button", { name: "Download main.rs", exact: true }).click();
  const main = await singleDownload;
  expect(main.suggestedFilename()).toBe("main.rs");
  expect(await readFile((await main.path())!, "utf8")).toBe(inline);
  await adoption.getByRole("radio", { name: "Project files · include_str!", exact: true }).check();
  await expect(adoption).toContainText("paths are relative to src/main.rs");
  const tree = adoption.getByRole("navigation", { name: "Project files", exact: true });
  await tree.getByRole("button", { name: "Cargo.toml", exact: true }).click();
  await expect(adoption.getByTestId("theme-showcase-code")).toHaveAttribute("data-code-language", "ini");
  await adoption.getByRole("button", { name: "Copy", exact: true }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toContain("[dependencies]");
  await tree.getByRole("button", { name: "src/main.rs", exact: true }).click();
  await expect(tree.getByRole("button", { name: "src/main.rs", exact: true })).toHaveAttribute("aria-current", "true");
  await adoption.getByRole("button", { name: "Copy", exact: true }).click();
  const files = await page.evaluate(() => navigator.clipboard.readText());
  expect(files).toContain('include_str!("../theme.json")');
  expect(files).toContain('include_str!("../diagram.mmd")');
  await tree.getByRole("button", { name: "diagram.mmd", exact: true }).click();
  await expectHighlighted(adoption.getByTestId("theme-showcase-code"), "mermaid");
  await adoption.getByRole("radio", { name: "Single file · copy into main.rs", exact: true }).check();
  await expect(tree).toHaveCount(0);
  await expect(adoption.getByTestId("theme-showcase-code")).toHaveAttribute("data-code-language", "rust");
  errors.assertNone();
});

async function expectHighlighted(code: import("@playwright/test").Locator, language: string) {
  await expect(code).toHaveAttribute("data-code-language", language);
  await expect.poll(() => code.locator(".view-lines span").evaluateAll((spans, currentLanguage) => {
    const tokens = spans.map((span) => ({ text: span.textContent?.replaceAll("\u00a0", " ").trim() ?? "", color: getComputedStyle(span).color }));
    const first = tokens.find((token) => currentLanguage === "json" ? token.text.includes('"schema_version"') : currentLanguage === "rust" ? token.text === "use" : token.text === "flowchart");
    const second = tokens.find((token) => currentLanguage === "json" ? token.text === "1" : currentLanguage === "rust" ? token.text.includes("merman") : token.text.includes("Review workflow"));
    return Boolean(first && second && first.color !== second.color);
  }, language)).toBe(true);
}

test("applying a design clears the host base while preserving authored inputs and remains undoable", async ({ page }) => {
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
  expect(saved.diagramTheme).toBe("auto");
  for (const key of ["code", "mermaidConfig", "diagramFont", "svgPipeline", "textMeasurementMode"] as const) expect(saved[key]).toBe(workspace[key]);
  await page.locator("[data-theme-launcher]:visible").click();
  const panel = page.locator("#theme-panel");
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Riso");
  await panel.getByRole("button", { name: "Undo appearance change", exact: true }).click();
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Brutalist");
  expect((await shareWorkspace(page)).diagramTheme).toBe(workspace.diagramTheme);
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
  const samples = gallery.getByTestId("theme-sample-gallery");
  await expect(samples.locator("article[data-theme-sample]")).toHaveCount(9);
  for (const family of samplesFor("field-notes")) {
    const tile = samples.locator(`article[data-theme-sample="${family}"]`);
    const inspect = tile.getByRole("button", { name: /^Inspect .+ example$/u });
    await inspect.scrollIntoViewIfNeeded();
    await expect(inspect).toBeInViewport();
    const box = await tile.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.x + box!.width).toBeLessThanOrEqual(391);
  }
  await samples.locator('article[data-theme-sample="mindmap"]').getByRole("button", { name: "Inspect Mindmap example", exact: true }).click();
  const expanded = page.getByTestId("theme-library-expanded-preview");
  await expect(expanded.getByRole("combobox", { name: "Example diagram family", exact: true })).toHaveAttribute("data-value", "mindmap");
  await chooseThemeOption(page, expanded.getByRole("combobox", { name: "Example diagram family", exact: true }), "er");
  await expect(expanded.getByTestId("theme-library-preview")).toHaveAttribute("data-theme-sample", "er");
  await expectNoDocumentOverflow(page);
  await expanded.getByRole("button", { name: "Close expanded preview", exact: true }).click();
  await expect(samples.locator('article[data-theme-sample="mindmap"]').getByRole("button", { name: "Inspect Mindmap example", exact: true })).toBeFocused();
  await samples.locator('article[data-theme-sample="er"]').getByRole("button", { name: "Use this example", exact: true }).click();
  const adoption = gallery.getByTestId("theme-showcase-use");
  await expect(adoption.getByRole("heading")).toBeInViewport();
  await adoption.getByRole("tab", { name: "CLI", exact: true }).click();
  await expect(adoption.getByRole("combobox", { name: "Example diagram for this code", exact: true })).toHaveAttribute("data-value", "er");
  await adoption.getByRole("button", { name: "Download example project", exact: true }).scrollIntoViewIfNeeded();
  await expect(adoption.getByRole("button", { name: "Download example project", exact: true })).toBeInViewport();
  await expect.poll(() => gallery.getByTestId("theme-showcase-detail-viewport").evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
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
    await expect(gallery.getByTestId("theme-showcase-detail").getByText("Current theme", { exact: true })).toBeVisible();
    await gallery.getByRole("button", { name: "Close theme designs", exact: true }).click();
    await expect(panel.getByRole("button", { name: "Theme designs", exact: true })).toBeFocused();
  }
});

test("a failed runtime keeps JSON usable and offers recovery for integration code", async ({ page }) => {
  await page.addInitScript(() => {
    if (window === window.top && (location.protocol === "http:" || location.protocol === "https:")) localStorage.setItem("merman-language", "en");
  });
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
  await expect(adoption.getByTestId("theme-showcase-code")).toContainText("compile_document");
  await expect(adoption.getByRole("alert")).toHaveCount(0);
});
