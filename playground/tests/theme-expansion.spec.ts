import { readFile } from "node:fs/promises";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { strFromU8, unzipSync } from "fflate";

import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import { CORE_THEME_SAMPLE_IDS, THEME_SAMPLE_IDS, type ThemeSampleId } from "../src/themes/families";
import { expectNoDocumentOverflow, monitorBrowserErrors, waitForPreviewSvg } from "./helpers/playground";
import { chooseThemeOption } from "./helpers/theme-select";

const expandedDesigns = [
  { id: "metro", extraSamples: ["gitGraph"] },
  { id: "signal", extraSamples: ["gantt"] },
  { id: "carbon-copy", extraSamples: ["kanban"] },
  { id: "field-notes", extraSamples: ["timeline"] },
  { id: "blueprint", extraSamples: ["architecture"] },
  { id: "prism", extraSamples: ["radar", "quadrantChart", "sankey", "treemap"] },
  { id: "monograph", extraSamples: ["requirement", "treeView"] },
] as const satisfies readonly { id: string; extraSamples: readonly ThemeSampleId[] }[];

const legacyDesignIds = ["brutalist", "cyberpunk", "ghibli", "spotless", "editor", "one-dark", "gruvbox", "ayu"];
const designIds = [
  "riso", "pop-art", "metro", "carbon-copy", "signal", "field-notes", "lacquer", "paper-cut", "aurora", "phosphor",
  "letterpress", "grove", "reading", "atlas", "nightfall", "blueprint", "porcelain", "chalk", "prism", "monograph",
  ...legacyDesignIds,
];

const sampleText: Readonly<Record<ThemeSampleId, string | RegExp>> = {
  flowchart: "Collect input", sequence: "Submit draft", class: "Resource", xychart: "Quarterly deliveries",
  pie: "Work by stage", state: "Queued", er: "RESERVATION", mindmap: "Service launch",
  gitGraph: "Integrate", gantt: "Install fixtures", kanban: "Collect field reports", timeline: "Map nesting sites",
  architecture: /Public\s*gateway/u, radar: "Reliability", quadrantChart: "Improvement opportunities",
  sankey: "Premium", treemap: "Checkout", requirement: "DOC-101", treeView: "citation-verification-results.csv",
};

const expandedLegacyCanvas: Readonly<Record<string, string>> = {
  metro: "#071B2C", signal: "#F3F0E5", "carbon-copy": "#F2EDDF", "field-notes": "#F8F3E5", blueprint: "#12365B",
};

function reviewedSamples(extra: readonly ThemeSampleId[]): readonly ThemeSampleId[] {
  return extra.length ? [extra[0], ...CORE_THEME_SAMPLE_IDS, ...extra.slice(1)] : CORE_THEME_SAMPLE_IDS;
}

async function start(page: Page) {
  await page.addInitScript(() => {
    if (window === window.top) {
      localStorage.setItem("merman-language", "en");
      localStorage.setItem("merman-ui-theme", "light");
    }
  });
  await page.goto(`./${encodeShareHash({
    ...DEFAULT_WORKSPACE_SNAPSHOT, code: "flowchart LR\nA[Current workspace] --> B[Preserved source]",
    mermaidConfig: '{"htmlLabels":false}',
  })}`, { waitUntil: "domcontentloaded" });
  if ((page.viewportSize()?.width ?? 1280) < 768) await page.getByRole("tab", { name: "Preview", exact: true }).click();
  await waitForPreviewSvg(page);
  await page.locator("[data-theme-showcase-launcher]:visible").click();
  return page.getByTestId("theme-showcase");
}

async function expectSampleIds(elements: Locator, expected: readonly ThemeSampleId[], attribute = "data-theme-sample") {
  await expect(elements).toHaveCount(expected.length);
  expect(await elements.evaluateAll((nodes, name) => nodes.map((node) => node.getAttribute(name)), attribute)).toEqual(expected);
}

async function expectSampleOptions(page: Page, trigger: Locator, expected: readonly ThemeSampleId[]) {
  await trigger.click();
  const listbox = page.getByRole("listbox");
  await expect(listbox).toBeVisible();
  await expectSampleIds(listbox.getByRole("option"), expected, "data-value");
  await page.keyboard.press("Escape");
  await expect(listbox).toBeHidden();
  await expect(trigger).toBeFocused();
}

async function copyAdoptionCode(page: Page, adoption: Locator) {
  await page.evaluate(() => navigator.clipboard.writeText(""));
  await adoption.getByRole("button", { name: "Copy", exact: true }).click();
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).not.toBe("");
  return (await page.evaluate(() => navigator.clipboard.readText())).replace(/\r\n/gu, "\n");
}

async function expectRenderedSample(expanded: Locator, sample: ThemeSampleId, appearance: "light" | "dark", canvas: string) {
  const preview = expanded.getByTestId("theme-library-preview");
  await expect(preview).toHaveAttribute("data-theme-sample", sample);
  await expect(preview.locator("[data-theme-preview-target]")).toHaveAttribute("data-theme-preview-target", sample);
  await expect(preview.locator("[data-theme-preview-appearance]")).toHaveAttribute("data-theme-preview-appearance", appearance);
  const viewport = preview.locator('[data-merman-svg-viewport="true"]');
  await expect(viewport).toHaveAttribute("data-viewport-interaction", "canvas");
  const svg = viewport.locator('.preview-container svg[role~="graphics-document"]');
  await expect(svg).toHaveCount(1);
  await expect(svg).toBeVisible();
  await expect(svg).toContainText(sampleText[sample]);
  await expect(svg.locator(".merman-theme-canvas-base")).toHaveAttribute("fill", new RegExp(`^${canvas}$`, "iu"));
  await expect.poll(() => svg.locator("path, rect, circle, line, polygon, polyline, ellipse").count()).toBeGreaterThan(1);
  await expect(preview.getByRole("alert")).toHaveCount(0);
}

test("new family filters retain reviewed authored and canonical designs with honest coverage and companion previews", async ({ page }) => {
  test.setTimeout(90_000);
  const errors = monitorBrowserErrors(page);
  const library = await start(page);
  await chooseThemeOption(page, library.getByRole("combobox", { name: "Diagram family filter", exact: true }), "");
  const cards = library.locator("[data-theme-design-card]");
  await expect(cards).toHaveCount(designIds.length);
  expect((await cards.evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-theme-design-card")))).sort()).toEqual([...designIds].sort());
  expect(CORE_THEME_SAMPLE_IDS).toHaveLength(8);
  expect(THEME_SAMPLE_IDS).toHaveLength(19);
  const family = library.getByRole("combobox", { name: "Diagram family filter", exact: true });

  for (const design of expandedDesigns) {
    for (const sample of design.extraSamples) {
      await chooseThemeOption(page, family, sample);
      const matchingDesigns = expandedDesigns.filter((candidate) => candidate.extraSamples.some((id) => id === sample));
      for (const matching of matchingDesigns) {
        await expect(library.locator(`[data-theme-design-card="${matching.id}"]`)).toHaveCount(1);
      }
      const visibleIds = await cards.evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-theme-design-card")!));
      expect(visibleIds.filter((id) => !legacyDesignIds.includes(id)).sort())
        .toEqual(matchingDesigns.map((candidate) => candidate.id).sort());
      for (const id of visibleIds) {
        expect(designIds).toContain(id);
        const card = library.locator(`[data-theme-design-card="${id}"]`);
        await expect(card).toHaveAttribute("data-theme-design-family", sample);
        await expect(card.locator("[data-theme-coverage]")).toHaveAttribute("data-theme-coverage", /^(dedicated|base_only)$/u);
        await expect(card.locator('[data-theme-card-primary="true"]')).toHaveAttribute("data-theme-card-sample", sample);
        const samples = await card.locator("[data-theme-card-sample]").evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-theme-card-sample")));
        expect(samples.length).toBeGreaterThan(0);
        expect(samples.length).toBeLessThanOrEqual(3);
        expect(new Set(samples).size).toBe(samples.length);
        const matching = matchingDesigns.find((candidate) => candidate.id === id);
        if (matching) await expect(card.locator('[data-theme-card-primary="false"]')).toHaveCount(2);
        for (const companion of samples) {
          expect(matching ? [...CORE_THEME_SAMPLE_IDS, ...matching.extraSamples] : THEME_SAMPLE_IDS).toContain(companion);
        }
      }
      await expect(library.getByTestId("theme-library-family-chip")).toBeVisible();
    }
  }
  await library.getByTestId("theme-library-family-chip").getByRole("button").click();
  await expect(family).toHaveAttribute("data-value", "");
  await expect(cards).toHaveCount(designIds.length);
  await library.getByRole("button", { name: "Light + dark", exact: true }).click();
  expect((await cards.evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-theme-design-card")))).sort())
    .toEqual(["ayu", "editor", "gruvbox", "prism", "reading"]);
  errors.assertNone();
});

test("removing a family filter restores each featured preview and filtering a core family takes precedence", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const library = await start(page);
  const family = library.getByRole("combobox", { name: "Diagram family filter", exact: true });

  for (const filtered of [false, true, false]) {
    if (filtered) await chooseThemeOption(page, family, "sequence");
    else await library.getByTestId("theme-library-family-chip").getByRole("button").click();
    for (const design of expandedDesigns) {
      const card = library.locator(`[data-theme-design-card="${design.id}"]`);
      await expect(card.locator('[data-theme-card-primary="true"]')).toHaveAttribute("data-theme-card-sample", filtered ? "sequence" : design.extraSamples[0]);
      await expect(card.locator('[data-theme-card-primary="false"]')).toHaveCount(2);
      await expect(card.locator("[data-theme-card-sample]")).toHaveCount(3);
    }
    await expectNoDocumentOverflow(page);
  }
  errors.assertNone();
});

for (const design of [...expandedDesigns, { id: "reading", extraSamples: [] }, { id: "riso", extraSamples: [] }]) {
  const samples = reviewedSamples(design.extraSamples);
  test(`${design.id} keeps exactly its ${samples.length} reviewed families in the gallery, navigation and adoption selector`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    const library = await start(page);
    const family = library.getByRole("combobox", { name: "Diagram family filter", exact: true });
    const selected = design.extraSamples[0] ?? "flowchart";
    await chooseThemeOption(page, family, selected);
    const explore = library.locator(`[data-explore-theme="${design.id}"]`);
    await explore.click();
    const detail = library.getByTestId("theme-showcase-detail");
    await expect(detail).toHaveAttribute("data-theme-design", design.id);
    const gallery = detail.getByTestId("theme-sample-gallery");
    await expectSampleIds(gallery.locator("article[data-theme-sample]"), samples);
    await expect(gallery.getByRole("link")).toHaveCount(samples.length);
    const tile = gallery.locator(`article[data-theme-sample="${selected}"]`);
    await expect(tile).toHaveAttribute("data-theme-sample-selected", "true");
    await tile.getByRole("button", { name: /^Inspect .+ example$/u }).click();
    const expanded = page.getByTestId("theme-library-expanded-preview");
    if (expandedLegacyCanvas[design.id]) {
      await expectRenderedSample(expanded, selected, design.id === "metro" || design.id === "blueprint" ? "dark" : "light", expandedLegacyCanvas[design.id]);
    }
    const previewFamily = expanded.getByRole("combobox", { name: "Example diagram family", exact: true });
    await expectSampleOptions(page, previewFamily, samples);
    await chooseThemeOption(page, previewFamily, samples.at(-1)!);
    await expanded.getByRole("button", { name: "Next example", exact: true }).click();
    await expect(previewFamily).toHaveAttribute("data-value", samples[0]);
    await expect(expanded.getByTestId("theme-library-preview")).toHaveAttribute("data-theme-sample", samples[0]);
    await expanded.getByRole("button", { name: "Previous example", exact: true }).click();
    await expect(previewFamily).toHaveAttribute("data-value", samples.at(-1)!);
    await expanded.getByRole("button", { name: "Close expanded preview", exact: true }).click();
    const adoption = detail.getByTestId("theme-showcase-use");
    await adoption.getByRole("tab", { name: "CLI", exact: true }).click();
    const adoptionFamily = adoption.getByRole("combobox", { name: "Example diagram for this code", exact: true });
    await expect(adoptionFamily).toHaveAttribute("data-value", samples.at(-1)!);
    await expectSampleOptions(page, adoptionFamily, samples);
    await chooseThemeOption(page, adoptionFamily, "er");
    await expect(gallery.locator('article[data-theme-sample="er"]')).toHaveAttribute("data-theme-sample-selected", "true");
    await detail.getByRole("button", { name: "All designs", exact: true }).click();
    await expect(family).toHaveAttribute("data-value", selected);
    await expect(explore).toBeFocused();
    errors.assertNone();
  });
}

test("returning from an expanded design cannot carry its unreviewed sample into another design", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const library = await start(page);
  await chooseThemeOption(page, library.getByRole("combobox", { name: "Diagram family filter", exact: true }), "treemap");
  await library.locator('[data-explore-theme="prism"]').click();
  let detail = library.getByTestId("theme-showcase-detail");
  await detail.getByTestId("theme-sample-gallery").locator('article[data-theme-sample="treemap"]').getByRole("button", { name: "Use this example", exact: true }).click();
  await detail.getByRole("button", { name: "All designs", exact: true }).click();
  await library.getByTestId("theme-library-family-chip").getByRole("button").click();

  for (const id of ["monograph", "reading"]) {
    await library.locator(`[data-explore-theme="${id}"]`).click();
    detail = library.getByTestId("theme-showcase-detail");
    const samples = reviewedSamples(id === "monograph" ? ["requirement", "treeView"] : []);
    const gallery = detail.getByTestId("theme-sample-gallery");
    await expectSampleIds(gallery.locator("article[data-theme-sample]"), samples);
    await expect(gallery.locator('article[data-theme-sample-selected="true"]')).toHaveCount(1);
    await expect(gallery.locator(`article[data-theme-sample="${samples[0]}"]`)).toHaveAttribute("data-theme-sample-selected", "true");
    await expect(gallery.locator('article[data-theme-sample="treemap"]')).toHaveCount(0);
    await detail.getByTestId("theme-showcase-use").getByRole("tab", { name: "Web", exact: true }).click();
    await expectSampleOptions(page, detail.getByRole("combobox", { name: "Example diagram for this code", exact: true }), samples);
    await detail.getByRole("button", { name: "All designs", exact: true }).click();
  }
  errors.assertNone();
});

for (const manualSample of [null, "sankey"] as const) {
  test(`a direct Prism link ${manualSample ? "retains a manual sample" : "adopts the current quadrant sample"} after delayed first detection`, async ({ page }) => {
    test.setTimeout(90_000);
    const errors = monitorBrowserErrors(page);
    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.addInitScript(() => {
      if (window === window.top) {
        localStorage.setItem("merman-language", "en");
        localStorage.setItem("merman-ui-theme", "light");
      }
    });
    const [source, config] = await Promise.all([
      readFile(new URL("../src/themes/samples/quadrantChart.mmd", import.meta.url), "utf8"),
      readFile(new URL("../src/themes/samples/quadrantChart.config.json", import.meta.url), "utf8"),
    ]);
    let releaseWasm!: () => void;
    let wasmRequested = false;
    const loading = new Promise<void>((resolve) => { releaseWasm = resolve; });
    await page.route(/\/merman_wasm_bg(?:-[\w-]+)?\.wasm(?:\?|$)/u, async (route) => {
      wasmRequested = true;
      await loading;
      await route.continue();
    });
    try {
      await page.goto(`./?theme-design=prism${encodeShareHash({
        ...DEFAULT_WORKSPACE_SNAPSHOT, code: source, mermaidConfig: config,
      })}`, { waitUntil: "domcontentloaded" });
      const library = page.getByTestId("theme-showcase");
      await expect(library).toBeVisible();
      if ((page.viewportSize()?.width ?? 1280) < 768) {
        await expect(page.getByRole("tab", { name: "Editor", exact: true, includeHidden: true })).toHaveAttribute("aria-selected", "true");
      }
      const detail = library.getByTestId("theme-showcase-detail");
      await expect(detail).toHaveAttribute("data-theme-design", "prism");
      const gallery = detail.getByTestId("theme-sample-gallery");
      await expect(gallery.locator('article[data-theme-sample="radar"]')).toHaveAttribute("data-theme-sample-selected", "true");
      await expect.poll(() => wasmRequested).toBe(true);
      const collectionFamily = library.getByTestId("theme-library-filters").locator('[aria-label="Diagram family filter"]');
      await expect(collectionFamily).toHaveAttribute("data-value", "");
      const adoption = detail.getByTestId("theme-showcase-use");
      await adoption.getByRole("tab", { name: "CLI", exact: true }).click();
      const adoptionFamily = adoption.getByRole("combobox", { name: "Example diagram for this code", exact: true });
      await expect(adoptionFamily).toHaveAttribute("data-value", "radar");
      if (manualSample) {
        await chooseThemeOption(page, adoptionFamily, manualSample);
        await expect(gallery.locator(`article[data-theme-sample="${manualSample}"]`)).toHaveAttribute("data-theme-sample-selected", "true");
      }

      releaseWasm();
      await expect(collectionFamily).toHaveAttribute("data-value", "quadrantChart");
      const selected = manualSample ?? "quadrantChart";
      await expect(gallery.locator('article[data-theme-sample-selected="true"]')).toHaveCount(1);
      await expect(gallery.locator(`article[data-theme-sample="${selected}"]`)).toHaveAttribute("data-theme-sample-selected", "true");
      await expect(adoptionFamily).toHaveAttribute("data-value", selected);
      await expect(adoption.getByTestId("theme-adoption-appearance")).toHaveAttribute("data-theme-adoption-sample", selected);
      const diagramFile = adoption.getByRole("navigation", { name: "Project files", exact: true }).getByRole("button", { name: "diagram.mmd", exact: true });
      await diagramFile.click();
      await expect(diagramFile).toHaveAttribute("aria-current", "true");
      const adoptedSource = await copyAdoptionCode(page, adoption);
      const selectedSource = manualSample ? await readFile(new URL(`../src/themes/samples/${manualSample}.mmd`, import.meta.url), "utf8") : source;
      expect(adoptedSource).toContain(selectedSource.trim().replace(/\r\n/gu, "\n"));
      expect(adoptedSource).toContain(sampleText[selected]);
      await detail.getByRole("button", { name: "All designs", exact: true }).click();
      await expect(collectionFamily).toHaveAttribute("data-value", "quadrantChart");
      const cards = library.locator("[data-theme-design-card]");
      await expect(library.locator('[data-theme-design-card="prism"]')).toHaveCount(1);
      for (const card of await cards.all()) {
        await expect(card).toHaveAttribute("data-theme-design-family", "quadrantChart");
        await expect(card.locator("[data-theme-coverage]")).toHaveAttribute("data-theme-coverage", /^(dedicated|base_only)$/u);
        await expect(card.locator('[data-theme-card-primary="true"]')).toHaveAttribute("data-theme-card-sample", "quadrantChart");
      }
      await library.getByTestId("theme-library-family-chip").getByRole("button").click();
      await expect(collectionFamily).toHaveAttribute("data-value", "");
      await library.getByRole("button", { name: "Close theme designs", exact: true }).click();
      await expect(library).toBeHidden();
      await page.locator("[data-theme-showcase-launcher]:visible").click();
      await expect(library).toBeVisible();
      await expect(collectionFamily).toHaveAttribute("data-value", "");
      await expect(library.locator("[data-theme-design-card]")).toHaveCount(designIds.length);
      errors.assertNone();
    } finally {
      releaseWasm();
    }
  });
}

for (const appearance of ["light", "dark"] as const) {
  test(`Prism ${appearance} renders its twelve reviewed families and pins the selected sample and appearance in all consumers`, async ({ page }) => {
    test.setTimeout(120_000);
    const errors = monitorBrowserErrors(page);
    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    const library = await start(page);
    await library.locator('[data-explore-theme="prism"]').click();
    const detail = library.getByTestId("theme-showcase-detail");
    await chooseThemeOption(page, detail.getByTestId("theme-appearance-control").getByRole("combobox"), appearance);
    const gallery = detail.getByTestId("theme-sample-gallery");
    const samples = reviewedSamples(["radar", "quadrantChart", "sankey", "treemap"]);
    await expectSampleIds(gallery.locator("article[data-theme-sample]"), samples);
    for (const sample of samples) {
      const image = gallery.locator(`article[data-theme-sample="${sample}"] img`);
      await image.scrollIntoViewIfNeeded();
      await expect(image).toHaveAttribute("data-theme-preview-appearance", appearance);
      await expect(image).toHaveAttribute("src", /^(?!blob:).+\.svg$/u);
      const assetName = new URL((await image.getAttribute("src"))!, page.url()).pathname.split("/").at(-1)!;
      expect(assetName).toMatch(new RegExp(`^prism-${appearance}-${sample}(?:-[\\w-]+)?\\.svg$`, "u"));
      const dimensions = await image.evaluate(async (element) => {
        const thumbnail = element as HTMLImageElement;
        await thumbnail.decode();
        return { width: thumbnail.naturalWidth, height: thumbnail.naturalHeight };
      });
      expect(dimensions.width).toBeGreaterThan(0);
      expect(dimensions.height).toBeGreaterThan(0);
    }
    await gallery.locator(`article[data-theme-sample="${samples[0]}"]`).getByRole("button", { name: /^Inspect .+ example$/u }).click();
    const expanded = page.getByTestId("theme-library-expanded-preview");
    const family = expanded.getByRole("combobox", { name: "Example diagram family", exact: true });
    for (const sample of samples) {
      if (sample !== samples[0]) await chooseThemeOption(page, family, sample);
      await expectRenderedSample(expanded, sample, appearance, appearance === "light" ? "#F5F7FB" : "#171F2D");
      if (sample === "sankey") {
        const svg = expanded.getByTestId("theme-library-preview").locator('.preview-container svg[role~="graphics-document"]');
        const rendered = await svg.evaluate((root) => ({
          layers: Array.from(root.children).filter((element) => element.matches("g.links, g.nodes, g.node-labels"))
            .map((element) => element.getAttribute("class")),
          linkBlendModes: Array.from(root.querySelectorAll(":scope > g.links > g.link"))
            .map((link) => getComputedStyle(link).mixBlendMode),
          nodeFills: Array.from(root.querySelectorAll(":scope > g.nodes > g.node > rect"))
            .map((node) => getComputedStyle(node).fill),
          labels: Array.from(root.querySelectorAll("text")).map((label) => ({
            text: label.textContent?.replace(/\s+/gu, " ").trim(), fill: getComputedStyle(label).fill,
          })),
        }));
        expect(rendered.layers).toEqual(["links", "nodes", "node-labels"]);
        expect(rendered.linkBlendModes).toEqual(Array(10).fill("normal"));
        expect(rendered.nodeFills).toEqual(appearance === "dark" ? [
          "rgb(119, 171, 255)", "rgb(247, 175, 106)", "rgb(88, 207, 186)", "rgb(185, 154, 240)",
          "rgb(241, 140, 179)", "rgb(219, 202, 112)", "rgb(119, 171, 255)",
        ] : [
          "rgb(40, 100, 220)", "rgb(185, 93, 18)", "rgb(19, 127, 117)", "rgb(132, 85, 199)",
          "rgb(186, 66, 111)", "rgb(138, 117, 21)", "rgb(40, 100, 220)",
        ]);
        const ink = appearance === "dark" ? "rgb(234, 240, 248)" : "rgb(34, 48, 71)";
        expect(rendered.labels).toEqual([
          "Search 60", "Trial 80", "Direct 60", "Referrals 40", "Campaigns 40", "Standard 72", "Premium 68",
        ].map((text) => ({ text, fill: ink })));
        await svg.screenshot({ path: test.info().outputPath(`prism-${appearance}-sankey.png`) });
      }
    }
    await expanded.getByRole("button", { name: "Close expanded preview", exact: true }).click();

    const sample = appearance === "light" ? "radar" : "treemap";
    const tile = gallery.locator(`article[data-theme-sample="${sample}"]`);
    await tile.getByRole("button", { name: "Use this example", exact: true }).click();
    const recipe = await readFile(new URL("../src/themes/recipes/prism.theme.json", import.meta.url), "utf8");
    const source = await readFile(new URL(`../src/themes/samples/${sample}.mmd`, import.meta.url), "utf8");
    const config = await readFile(new URL(`../src/themes/samples/${sample}.config.json`, import.meta.url), "utf8");
    const saved = decodeShareHash(new URL((await tile.getByRole("link").getAttribute("href"))!, page.url()).hash)!;
    expect(saved.themeRecipeJson).toBe(recipe);
    expect(saved.themeAppearance).toBe(appearance);
    expect(saved.themeRecipeLabel).toEqual({ name: "Prism", source: "exploration" });
    expect(saved.code).toBe(source);
    expect(saved.mermaidConfig).toBe(config);
    const themeDownload = page.waitForEvent("download");
    await detail.getByRole("button", { name: "Download theme JSON", exact: true }).click();
    expect(await readFile((await (await themeDownload).path())!, "utf8")).toBe(recipe);
    const adoption = detail.getByTestId("theme-showcase-use");
    await expect(adoption.getByTestId("theme-adoption-appearance")).toHaveAttribute("data-theme-adoption-sample", sample);
    await expect(adoption.getByTestId("theme-adoption-appearance")).toHaveAttribute("data-theme-adoption-appearance", appearance);
    await adoption.getByRole("tab", { name: "CLI", exact: true }).click();
    const opposite = appearance === "light" ? "dark" : "light";
    const appearanceControl = detail.getByTestId("theme-appearance-control").getByRole("combobox");
    await chooseThemeOption(page, appearanceControl, opposite);
    expect(await copyAdoptionCode(page, adoption)).toContain(`--theme-appearance ${opposite}`);
    await expect(adoption.getByTestId("theme-adoption-appearance")).toHaveAttribute("data-theme-adoption-sample", sample);
    await chooseThemeOption(page, appearanceControl, appearance);
    expect(await copyAdoptionCode(page, adoption)).toContain(`--theme-appearance ${appearance}`);
    await expect(adoption.getByTestId("theme-adoption-appearance")).toHaveAttribute("data-theme-adoption-appearance", appearance);

    for (const [target, label, filename] of [["cli", "CLI", "render-command.txt"], ["web", "Web", "main.js"], ["rust", "Rust", "src/main.rs"]] as const) {
      await adoption.getByRole("tab", { name: label, exact: true }).click();
      await expect(adoption.getByRole("combobox", { name: "Example diagram for this code", exact: true })).toHaveAttribute("data-value", sample);
      if (target === "rust") {
        await expect(adoption.getByTestId("theme-showcase-code")).toHaveAttribute("data-code-language", "rust");
        const inline = await copyAdoptionCode(page, adoption);
        expect(inline).toContain(`compile_document(recipe, Some(ThemeAppearance::${appearance === "light" ? "Light" : "Dark"}))`);
        expect(inline).toContain(sampleText[sample]);
        const singleDownload = page.waitForEvent("download");
        await adoption.getByRole("button", { name: "Download main.rs", exact: true }).click();
        const main = await singleDownload;
        expect(main.suggestedFilename()).toBe("main.rs");
        expect((await readFile((await main.path())!, "utf8")).replace(/\r\n/gu, "\n")).toBe(inline);
        await adoption.getByRole("radio", { name: "Project files · include_str!", exact: true }).check();
      }
      const pending = page.waitForEvent("download");
      await adoption.getByRole("button", { name: "Download example project", exact: true }).click();
      const download = await pending;
      expect(download.suggestedFilename()).toBe(`prism-${sample}-${target}.zip`);
      const files = Object.fromEntries(Object.entries(unzipSync(await readFile((await download.path())!))).map(([name, bytes]) => [name, strFromU8(bytes)]));
      expect(files["theme.json"]).toBe(recipe);
      const directive = `%%{init: ${JSON.stringify(JSON.parse(config))}}%%`;
      const frontmatter = source.match(/^---\r?\n[\s\S]*?\r?\n---(?:\r?\n|$)/u)?.[0] ?? "";
      const newline = source.match(/\r\n|[\r\n]/u)?.[0] ?? "\n";
      expect(files["diagram.mmd"].indexOf(directive)).toBe(frontmatter.length);
      expect(files["diagram.mmd"].slice(0, frontmatter.length)).toBe(frontmatter);
      expect(files["diagram.mmd"].replace(`${directive}${newline}`, "")).toBe(source);
      const options = JSON.parse(files["options.json"]);
      expect(options.theme).toEqual({ recipe: JSON.parse(recipe), appearance });
      const handoff = JSON.parse(files["handoff.json"]);
      expect(handoff.target).toBe(target);
      expect(handoff.theme).toMatchObject({ label: { name: "Prism", source: "exploration" }, appearance_preference: appearance, resolved_appearance: appearance });
      const selectedFile = adoption.getByRole("navigation", { name: "Project files", exact: true }).getByRole("button", { name: filename, exact: true });
      await selectedFile.click();
      await expect(selectedFile).toHaveAttribute("aria-current", "true");
      expect(await copyAdoptionCode(page, adoption)).toBe(files[filename].replace(/\r\n/gu, "\n"));
      if (target === "cli") expect(files[filename]).toContain(`--theme-appearance ${appearance}`);
      if (target === "rust") expect(files[filename]).toContain(`compile_document(recipe, Some(ThemeAppearance::${appearance === "light" ? "Light" : "Dark"}))`);
      if (target === "web") {
        expect(files[filename]).toContain('new URL("./diagram.mmd", import.meta.url)');
        expect(files[filename]).toContain('new URL("./options.json", import.meta.url)');
        expect(files[filename]).toContain("renderSvgResult(source, optionsJson)");
      }
    }
    errors.assertNone();
  });
}

test("Monograph actually renders all ten reviewed families, including requirement relationships and tree paths", async ({ page }) => {
  test.setTimeout(90_000);
  const errors = monitorBrowserErrors(page);
  const library = await start(page);
  await library.locator('[data-explore-theme="monograph"]').click();
  const detail = library.getByTestId("theme-showcase-detail");
  const gallery = detail.getByTestId("theme-sample-gallery");
  const samples = reviewedSamples(["requirement", "treeView"]);
  await expectSampleIds(gallery.locator("article[data-theme-sample]"), samples);
  await gallery.locator(`article[data-theme-sample="${samples[0]}"]`).getByRole("button", { name: /^Inspect .+ example$/u }).click();
  const expanded = page.getByTestId("theme-library-expanded-preview");
  for (const sample of samples) {
    if (sample !== samples[0]) await chooseThemeOption(page, expanded.getByRole("combobox", { name: "Example diagram family", exact: true }), sample);
    await expectRenderedSample(expanded, sample, "light", "#FFFFFF");
    if (sample === "requirement") await expect(expanded.locator('.preview-container svg[role~="graphics-document"]')).toContainText("document_review_service");
    if (sample === "treeView") await expect(expanded.locator('.preview-container svg[role~="graphics-document"]')).toContainText("publication-checklist.md");
  }
  await expanded.getByRole("button", { name: "Close expanded preview", exact: true }).click();
  await expect(gallery.locator('article[data-theme-sample="treeView"]')).toHaveAttribute("data-theme-sample-selected", "true");
  errors.assertNone();
});
