import { readFile } from "node:fs/promises";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { strFromU8, unzipSync } from "fflate";

import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import type { MermanRuntimeStore } from "../src/runtime/merman-core";
import { THEME_SAMPLE_IDS } from "../src/themes/families";
import { monitorBrowserErrors, waitForPreviewSvg } from "./helpers/playground";
import { chooseThemeOption } from "./helpers/theme-select";

const legacyDesigns = [
  { id: "brutalist", name: "Brutalist", sample: "flowchart", label: "Collect input" },
  { id: "cyberpunk", name: "Cyberpunk", sample: "flowchart", label: "Collect input" },
  { id: "ghibli", name: "Ghibli", sample: "pie", label: "Work by stage" },
  { id: "spotless", name: "Spotless", sample: "sequence", label: "Submit draft" },
  { id: "editor", name: "Editor", sample: "class", label: "Resource" },
  { id: "one-dark", name: "One Dark", sample: "class", label: "Resource" },
  { id: "gruvbox", name: "Gruvbox", sample: "xychart", label: "Quarterly deliveries" },
  { id: "ayu", name: "Ayu", sample: "xychart", label: "Quarterly deliveries" },
] as const;

const workspace = {
  ...DEFAULT_WORKSPACE_SNAPSHOT,
  code: "flowchart LR\nA[Keep this legacy workspace] --> B[Keep the authored source]",
  mermaidConfig: '{"flowchart":{"htmlLabels":false,"curve":"linear"}}',
  diagramTheme: "neutral" as const,
  diagramFont: "trebuchet" as const,
};

async function openDesign(page: Page, id: string) {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.addInitScript(() => {
    if (window === window.top) {
      localStorage.setItem("merman-language", "en");
      localStorage.setItem("merman-ui-theme", "light");
    }
  });
  await page.goto(`./?theme-design=${id}${encodeShareHash(workspace)}`, { waitUntil: "domcontentloaded" });
  const detail = page.getByTestId("theme-showcase-detail");
  await expect(detail).toHaveAttribute("data-theme-design", id);
  await expect(detail.getByRole("button", { name: "Apply to my diagram", exact: true })).toBeEnabled();
  return detail;
}

async function runtimePreset(page: Page, id: string) {
  const manifest = JSON.parse(await readFile(new URL("../dist/.vite/manifest.json", import.meta.url), "utf8")) as Record<string, { name?: string; file: string }>;
  const chunks = Object.values(manifest).filter((entry) => entry.name === "use-merman-runtime");
  expect(chunks).toHaveLength(1);
  return page.evaluate(async ({ path, id }) => {
    const module = await import(new URL(path, location.href).href);
    const store = Object.values(module).find((value) => value !== null && typeof value === "object" &&
      "getState" in value && typeof value.getState === "function") as MermanRuntimeStore | undefined;
    const state = store?.getState();
    if (state?.status !== "ready") throw new Error("The matching browser runtime must be ready.");
    const preset = state.facade.themeCatalog().presets.find((entry) => entry.id === id);
    if (!preset?.available) throw new Error(`Missing available canonical preset ${id}.`);
    return { preset, recipe: state.facade.exportThemePreset(id) };
  }, { path: chunks[0].file, id });
}

async function copyCode(page: Page, adoption: Locator) {
  await page.evaluate(() => navigator.clipboard.writeText(""));
  await adoption.getByRole("button", { name: "Copy", exact: true }).click();
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).not.toBe("");
  return (await page.evaluate(() => navigator.clipboard.readText())).replace(/\r\n/gu, "\n");
}

async function sharedWorkspace(page: Page) {
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  return decodeShareHash(new URL(await page.evaluate(() => navigator.clipboard.readText())).hash)!;
}

for (const { id, name, sample, label } of legacyDesigns) {
  test(`${id} direct link renders reviewed labels and exports complete JSON while Apply retains its canonical preset`, async ({ page }) => {
    test.setTimeout(60_000);
    const errors = monitorBrowserErrors(page);
    const detail = await openDesign(page, id);
    await expect(detail.getByRole("heading", { name, exact: true })).toBeVisible();
    const { preset, recipe } = await runtimePreset(page, id);
    const canonical = JSON.stringify(recipe, null, 2);
    expect(recipe.kind).toBe("design");
    const appearance = preset.supported_appearances.includes("light") ? "light" : preset.default_appearance;
    const metadata = preset.appearances.find((entry) => entry.appearance === appearance)!;
    const reviewedIds = metadata.family_designs.filter((entry) =>
      entry.treatment === "dedicated" || entry.treatment === "base_only").map((entry) => entry.family_id);
    const gallery = detail.getByTestId("theme-sample-gallery");
    const tiles = gallery.locator("article[data-theme-sample]");
    await expect.poll(() => tiles.count()).toBeGreaterThan(0);
    const visibleIds = await tiles.evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-theme-sample")!));
    expect(new Set(visibleIds).size).toBe(visibleIds.length);
    expect([...visibleIds].sort()).toEqual(THEME_SAMPLE_IDS.filter((family) => reviewedIds.includes(family)).sort());
    const tile = gallery.locator(`article[data-theme-sample="${sample}"]`);
    const image = tile.locator("img");
    await tile.scrollIntoViewIfNeeded();
    await expect(image).toHaveAttribute("data-theme-preview-appearance", appearance);
    await expect.poll(() => image.evaluate((node) =>
      (node as HTMLImageElement).complete && (node as HTMLImageElement).naturalWidth > 0)).toBe(true);
    const thumbnailLabels = await image.evaluate(async (node) => {
      const response = await fetch((node as HTMLImageElement).currentSrc);
      if (!response.ok) throw new Error(`Theme preview returned HTTP ${response.status}.`);
      return new DOMParser().parseFromString(await response.text(), "image/svg+xml").documentElement.textContent ?? "";
    });
    expect(thumbnailLabels).toContain(label);
    await tile.getByRole("button", { name: /^Inspect .+ example$/u }).click();
    const expanded = page.getByTestId("theme-library-expanded-preview");
    const preview = expanded.getByTestId("theme-library-preview");
    await expect(preview).toHaveAttribute("data-theme-sample", sample);
    await expect(preview.locator("[data-theme-preview-appearance]")).toHaveAttribute("data-theme-preview-appearance", appearance);
    const svg = preview.locator('.preview-container svg[role~="graphics-document"]');
    await expect(svg).toBeVisible();
    await expect(svg).toContainText(label);
    await expect.poll(() => svg.locator("path, rect, circle, line, polygon, polyline, ellipse").count()).toBeGreaterThan(1);
    const treatment = metadata.family_designs.find((entry) => entry.family_id === sample)!.treatment;
    await expect(preview.locator("[data-theme-coverage]")).toHaveAttribute("data-theme-coverage", treatment);
    await expect(preview.locator("[data-theme-coverage]")).toContainText(treatment === "dedicated" ? "Designed for" : "Reviewed shared styling");
    await expect(preview.getByRole("alert")).toHaveCount(0);
    await expanded.getByRole("button", { name: "Close expanded preview", exact: true }).click();

    await detail.getByRole("button", { name: "All designs", exact: true }).click();
    const library = page.getByTestId("theme-showcase");
    await chooseThemeOption(page, library.getByRole("combobox", { name: "Diagram family filter", exact: true }), sample);
    const card = library.locator(`[data-theme-design-card="${id}"]`);
    await expect(card.locator("[data-theme-coverage]")).toHaveAttribute("data-theme-coverage", treatment);
    await expect(card.locator('[data-theme-card-primary="true"]')).toHaveAttribute("data-theme-card-sample", sample);
    await expect(card.locator("[data-theme-card-sample]")).toHaveCount(Math.min(3, visibleIds.length));
    const cardSamples = await card.locator("[data-theme-card-sample]").evaluateAll((nodes) =>
      nodes.map((node) => node.getAttribute("data-theme-card-sample")));
    expect(new Set(cardSamples).size).toBe(cardSamples.length);
    for (const family of cardSamples) expect(visibleIds).toContain(family);
    await card.getByRole("button", { name: `Explore ${name}`, exact: true }).click();

    const linked = decodeShareHash(new URL((await tile.getByRole("link").getAttribute("href"))!, page.url()).hash)!;
    expect(linked.themeRecipeJson).toBe(canonical);
    expect(linked.code).toBe(await readFile(new URL(`../src/themes/samples/${sample}.mmd`, import.meta.url), "utf8"));
    expect(linked.mermaidConfig).toBe(await readFile(new URL(`../src/themes/samples/${sample}.config.json`, import.meta.url), "utf8"));
    const downloading = page.waitForEvent("download");
    await detail.getByRole("button", { name: "Download theme JSON", exact: true }).click();
    const download = await downloading;
    expect(download.suggestedFilename()).toBe(`${id}.theme.json`);
    expect(await readFile((await download.path())!, "utf8")).toBe(canonical);
    const adoption = detail.getByTestId("theme-showcase-use");
    expect(await copyCode(page, adoption)).toBe(canonical.replace(/\r\n/gu, "\n"));
    await adoption.getByRole("tab", { name: "CLI", exact: true }).click();
    expect(await copyCode(page, adoption)).toContain(`--theme-preset ${id}`);
    await adoption.getByRole("tab", { name: "Rust", exact: true }).click();
    expect(await copyCode(page, adoption)).toContain(`compile_preset_selection("${id}", Some(ThemeAppearance::${appearance === "dark" ? "Dark" : "Light"}))`);

    await detail.getByRole("button", { name: "Apply to my diagram", exact: true }).click();
    await expect(page.getByTestId("theme-showcase")).toBeHidden();
    await waitForPreviewSvg(page);
    const saved = await sharedWorkspace(page);
    expect(saved.themePresetId).toBe(id);
    expect(saved.themeRecipeJson).toBeNull();
    expect(saved.diagramTheme).toBe("auto");
    for (const key of ["code", "mermaidConfig", "diagramFont", "svgPipeline", "textMeasurementMode"] as const) {
      expect(saved[key]).toBe(workspace[key]);
    }
    await page.locator("[data-theme-launcher]:visible").click();
    const panel = page.locator("#theme-panel");
    await expect(panel.locator(`[data-theme-choice="preset:${id}"]`)).toHaveAttribute("aria-pressed", "true");
    await expect(panel.locator(`[data-theme-preset="${id}"]`)).toHaveCount(1);
    await expect(panel.locator(`[data-theme-choice="exploration:${id}"]`)).toHaveCount(0);
    errors.assertNone();
  });
}

test("Editor dark preserves its paired editable export, preset snippets and single Appearance directory", async ({ page }) => {
  test.setTimeout(60_000);
  const errors = monitorBrowserErrors(page);
  const detail = await openDesign(page, "editor");
  const { preset, recipe } = await runtimePreset(page, "editor");
  expect(recipe.kind).toBe("design");
  if (recipe.kind !== "design") throw new Error("Editor must export an editable design.");
  expect(Object.keys(recipe.design.appearances!).sort()).toEqual(["dark", "light"]);
  const canonical = JSON.stringify(recipe, null, 2);
  await chooseThemeOption(page, detail.getByTestId("theme-appearance-control").getByRole("combobox"), "dark");
  const tile = detail.getByTestId("theme-sample-gallery").locator('article[data-theme-sample="class"]');
  await tile.getByRole("button", { name: /^Inspect .+ example$/u }).click();
  const expanded = page.getByTestId("theme-library-expanded-preview");
  const preview = expanded.getByTestId("theme-library-preview");
  await expect(preview.locator("[data-theme-preview-appearance]")).toHaveAttribute("data-theme-preview-appearance", "dark");
  const svg = preview.locator('.preview-container svg[role~="graphics-document"]');
  await expect(svg).toContainText("Resource");
  const darkCanvas = preset.appearances.find((entry) => entry.appearance === "dark")!.preview_colors[0];
  await expect(svg.locator(".merman-theme-canvas-base")).toHaveAttribute("fill", new RegExp(`^${darkCanvas}$`, "iu"));
  await expanded.getByRole("button", { name: "Close expanded preview", exact: true }).click();
  const adoption = detail.getByTestId("theme-showcase-use");
  expect(await copyCode(page, adoption)).toBe(canonical);
  await adoption.getByRole("tab", { name: "CLI", exact: true }).click();
  await chooseThemeOption(page, adoption.getByRole("combobox", { name: "Example diagram for this code", exact: true }), "sequence");
  await expect(detail.getByTestId("theme-sample-gallery").locator('article[data-theme-sample="sequence"]')).toHaveAttribute("data-theme-sample-selected", "true");
  const cli = await copyCode(page, adoption);
  expect(cli).toContain("--theme-preset editor");
  expect(cli).toContain("--theme-appearance dark");
  await adoption.getByRole("tab", { name: "Rust", exact: true }).click();
  expect(await copyCode(page, adoption)).toContain('compile_preset_selection("editor", Some(ThemeAppearance::Dark))');
  await adoption.getByRole("tab", { name: "Web", exact: true }).click();
  const downloading = page.waitForEvent("download");
  await adoption.getByRole("button", { name: "Download example project", exact: true }).click();
  const files = unzipSync(await readFile((await (await downloading).path())!));
  expect(files["theme.json"]).toBeUndefined();
  expect(strFromU8(files["diagram.mmd"])).toContain("Submit draft");
  expect(JSON.parse(strFromU8(files["options.json"])).theme).toEqual({ preset: "editor", appearance: "dark" });
  expect(JSON.parse(strFromU8(files["handoff.json"])).theme).toMatchObject({ preset_id: "editor", resolved_appearance: "dark" });
  await adoption.getByRole("tab", { name: "Theme JSON", exact: true }).click();
  expect(await copyCode(page, adoption)).toBe(canonical);
  const themeDownload = page.waitForEvent("download");
  await detail.getByRole("button", { name: "Download theme JSON", exact: true }).click();
  expect(await readFile((await (await themeDownload).path())!, "utf8")).toBe(canonical);

  await detail.getByRole("button", { name: "Apply to my diagram", exact: true }).click();
  const saved = await sharedWorkspace(page);
  expect(saved.themePresetId).toBe("editor");
  expect(saved.themeAppearance).toBe("dark");
  expect(saved.themeRecipeJson).toBeNull();
  await page.locator("[data-theme-launcher]:visible").click();
  const panel = page.locator("#theme-panel");
  await expect(panel.locator('[data-theme-group="builtin"]')).toHaveCount(1);
  await expect(panel.locator('[data-theme-group="builtin"] [data-theme-choice]')).toHaveCount(11);
  await expect(panel.locator('[data-theme-group="exploration"] [data-theme-choice]')).toHaveCount(17);
  await expect(panel.locator('[data-theme-choice="preset:editor"]')).toHaveAttribute("aria-pressed", "true");
  await expect(panel.locator('[data-theme-choice="preset:editor-dark"]')).toHaveCount(0);
  for (const { id } of legacyDesigns) await expect(panel.locator(`[data-theme-choice="exploration:${id}"]`)).toHaveCount(0);
  await panel.getByRole("button", { name: "Theme designs", exact: true }).click();
  const reopened = page.getByTestId("theme-showcase-detail");
  await expect(reopened).toHaveAttribute("data-theme-design", "editor");
  await expect(reopened.getByTestId("theme-appearance-control").getByRole("combobox")).toHaveAttribute("data-value", "dark");
  await expect(reopened.getByTestId("theme-appearance-control")).toHaveAttribute("data-effective-appearance", "dark");
  await reopened.getByRole("button", { name: "Apply to my diagram", exact: true }).click();
  expect((await sharedWorkspace(page)).themeAppearance).toBe("dark");
  await panel.getByTestId("theme-advanced").locator("summary").first().click();
  await panel.getByRole("button", { name: "Copy JSON", exact: true }).click();
  expect((await page.evaluate(() => navigator.clipboard.readText())).replace(/\r\n/gu, "\n")).toBe(canonical);
  const panelDownload = page.waitForEvent("download");
  await panel.getByRole("button", { name: "Download JSON", exact: true }).click();
  expect(await readFile((await (await panelDownload).path())!, "utf8")).toBe(canonical);
  await panel.getByRole("button", { name: "Customize", exact: true }).click();
  await expect(panel.getByRole("textbox", { name: "Editable theme JSON", exact: true })).toBeVisible();
  await panel.getByRole("button", { name: "Validate theme", exact: true }).click();
  await expect(panel.getByRole("button", { name: "Apply changes", exact: true })).toBeEnabled();
  await panel.getByRole("button", { name: "Apply changes", exact: true }).click();
  const customized = await sharedWorkspace(page);
  expect(customized.themePresetId).toBeNull();
  expect(customized.themeRecipeJson).toBe(canonical);
  expect(customized.themeAppearance).toBe("dark");
  expect(customized.themeRecipeLabel).toMatchObject({ source: "imported", derivedFrom: "preset:editor" });
  errors.assertNone();
});

for (const selection of [
  { preset: "editor-dark", preference: "default", expected: "dark", appearance: "dark" },
  { preset: "editor-dark", preference: "follow", expected: "dark", appearance: "dark" },
  { preset: "editor", preference: "follow", expected: "follow", appearance: "light" },
] as const) {
  test(`Appearance opens ${selection.preset}/${selection.preference} without changing its resolved appearance`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.addInitScript(() => {
      localStorage.setItem("merman-language", "en");
      localStorage.setItem("merman-ui-theme", "light");
    });
    await page.goto(`./${encodeShareHash({ ...workspace, themePresetId: selection.preset, themeAppearance: selection.preference })}`);
    await waitForPreviewSvg(page);
    await page.locator("[data-theme-launcher]:visible").click();
    await page.locator("#theme-panel").getByRole("button", { name: "Theme designs", exact: true }).click();
    const detail = page.getByTestId("theme-showcase-detail");
    await expect(detail).toHaveAttribute("data-theme-design", "editor");
    const control = detail.getByTestId("theme-appearance-control");
    await expect(control.getByRole("combobox")).toHaveAttribute("data-value", selection.expected);
    await expect(control).toHaveAttribute("data-effective-appearance", selection.appearance);
    await detail.getByRole("button", { name: "Apply to my diagram", exact: true }).click();
    const saved = await sharedWorkspace(page);
    expect(saved.themePresetId).toBe("editor");
    expect(saved.themeAppearance).toBe(selection.expected);
    expect(saved.code).toBe(workspace.code);
    errors.assertNone();
  });
}

test("a built-in design deep link retains its requested detail through a runtime fetch failure and retry", async ({ page }) => {
  let blocked = true;
  await page.route(/\/assets\/merman_wasm_bg-[\w-]+\.wasm(?:\?|$)/u, async (route) => {
    if (blocked) await route.abort("failed");
    else await route.continue();
  });
  await page.addInitScript(() => {
    localStorage.setItem("merman-language", "en");
  });
  await page.goto(`./?theme-design=editor${encodeShareHash(workspace)}`, { waitUntil: "domcontentloaded" });
  const showcase = page.getByRole("dialog", { name: "Theme designs", exact: true });
  await expect(showcase.getByRole("alert")).toContainText("The rendering runtime could not load");
  await expect(showcase.getByTestId("theme-showcase-detail")).toHaveCount(0);
  blocked = false;
  await showcase.getByRole("button", { name: "Retry", exact: true }).click();
  const detail = showcase.getByTestId("theme-showcase-detail");
  await expect(detail).toHaveAttribute("data-theme-design", "editor");
  await expect(detail.getByRole("button", { name: "Apply to my diagram", exact: true })).toBeEnabled();
  await expect(showcase.getByRole("alert")).toHaveCount(0);
});
