import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";

import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import { CORE_THEME_SAMPLE_IDS } from "../src/themes/families";
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
  await page.addInitScript(() => {
    if (window === window.top && (location.protocol === "http:" || location.protocol === "https:")) localStorage.setItem("merman-language", "en");
  });
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
  await expect(catalog.locator("[data-theme-exploration]")).toHaveCount(20);
  await expect(catalog.locator('[data-theme-choice="mermaid:forest"]')).toBeVisible();
  await expect(catalog.locator('[data-theme-choice="preset:brutalist"]')).toHaveAttribute("aria-pressed", "true");
  expect(requests.some((url) => url.endsWith(galleryOutput))).toBe(true);
  await expect(panel.getByRole("button", { name: "Browse themes", exact: true })).toHaveCount(0);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  const choices = page.getByTestId("theme-catalog-choices");
  await expect(choices).toHaveCount(1);
  const groups = choices.locator("[data-theme-group]");
  await expect(choices.locator('[data-theme-choice="preset:brutalist"]')).toBeEnabled();
  const presetCount = await choices.locator('[data-theme-group="builtin"] [data-theme-choice]').count();
  const search = catalog.getByRole("searchbox", { name: "Search themes", exact: true });
  for (const [query, key, group] of [
    ["mermaid:forest", "mermaid:forest", "mermaid"],
    ["brutalist", "preset:brutalist", "builtin"],
    ["nightfall", "exploration:nightfall", "exploration"],
    ["mermaid:redux-dark-color", "mermaid:redux-color", "mermaid"],
    ["preset:one-dark", "preset:one-dark", "builtin"],
    ["exploration:atlas", "exploration:atlas", "exploration"],
  ]) {
    await search.fill(query);
    await expect(catalog.locator("[data-theme-choice]")).toHaveCount(1);
    await expect(groups).toHaveCount(1);
    await expect(choices.locator(`[data-theme-group="${group}"] [data-theme-choice="${key}"]`)).toBeVisible();
  }
  for (const [query, group, count] of [
    ["Mermaid.js official", "mermaid", 8],
    ["Merman built-in presets", "builtin", presetCount],
    ["--theme-preset", "builtin", presetCount],
    ["Merman theme examples", "exploration", 20],
    ["Merman example · JSON", "exploration", 20],
  ] as const) {
    await search.fill(query);
    await expect(groups).toHaveCount(1);
    await expect(groups).toHaveAttribute("data-theme-group", group);
    await expect(choices.locator("[data-theme-choice]")).toHaveCount(count);
  }
  await search.fill("missing-theme");
  await expect(catalog).toContainText("No themes match");
  await expect(groups).toHaveCount(0);
  await search.press("Escape");
  await expect(search).toHaveValue("");
  await expect(groups).toHaveCount(4);
  await expect(choices).toHaveCount(1);
  await expect(catalog.locator("[data-theme-exploration]")).toHaveCount(20);
  await expect(panel).toBeVisible();
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Brutalist");
  errors.assertNone();
});

test("catalog separates source/default from metadata-grouped official designs and sorts Merman choices by name", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const panel = await start(page);
  const choices = page.getByTestId("theme-catalog-choices");
  await expect(choices).toHaveCount(1);
  await expect(choices.locator('[data-theme-choice="preset:brutalist"]')).toBeEnabled();
  await expect.poll(() => choices.locator("[data-theme-group]").evaluateAll((groups) =>
    groups.map((group) => group.getAttribute("data-theme-group"))))
    .toEqual(["automatic", "mermaid", "builtin", "exploration"]);
  for (const [group, title] of [
    ["automatic", "Source / default"],
    ["mermaid", "Mermaid.js official"],
    ["builtin", "Merman built-in presets"],
    ["exploration", "Merman theme examples"],
  ]) {
    await expect(choices.locator(`[data-theme-group="${group}"]`).getByRole("heading").getByText(title, { exact: true })).toBeVisible();
    await expect(choices.locator(`[data-theme-group="${group}"] [data-theme-choice]:not([data-theme-source="${group}"])`)).toHaveCount(0);
  }
  const automatic = choices.locator('[data-theme-group="automatic"]');
  const auto = automatic.locator("[data-theme-choice]");
  await expect(auto).toHaveCount(1);
  await expect(auto).toHaveAttribute("data-theme-choice", "mermaid:auto");
  await expect(auto).toHaveAttribute("data-theme-source", "automatic");
  await expect(auto.locator("[data-theme-source-label]")).toHaveText("Source / default");
  const official = choices.locator('[data-theme-group="mermaid"]');
  await expect.poll(() => official.locator("[data-theme-choice]").evaluateAll((entries) =>
    entries.map((entry) => entry.getAttribute("data-theme-choice"))))
    .toEqual([
      "mermaid:default", "mermaid:base", "mermaid:dark", "mermaid:forest", "mermaid:neutral",
      "mermaid:neo", "mermaid:redux", "mermaid:redux-color",
    ]);
  await expect(official.locator("[data-theme-source-label]")).toHaveText(Array(8).fill("Mermaid.js official"));
  await expect(official).toContainText("Compare");
  await expect(choices.locator('[data-theme-group="builtin"]')).toContainText('theme: { preset: "…" }');
  await expect(choices.locator('[data-theme-group="exploration"] [data-theme-choice]')).toHaveCount(20);
  for (const id of ["prism", "monograph"]) {
    await expect(choices.locator(`[data-theme-choice="exploration:${id}"]`)).toHaveCount(1);
  }
  for (const [group, source] of [["builtin", "Merman preset"], ["exploration", "Merman example · JSON"]]) {
    const entries = choices.locator(`[data-theme-group="${group}"] [data-theme-choice]`);
    const names = await entries.locator("[data-theme-choice-name]").allTextContents();
    expect(names.length).toBeGreaterThan(1);
    expect(names).toEqual([...names].sort((left, right) => left.localeCompare(right, "en", { numeric: true, sensitivity: "base" })));
    await expect(entries.locator("[data-theme-source-label]")).toHaveText(Array(names.length).fill(source));
  }
  await auto.click();
  await expect(auto).toHaveAttribute("aria-pressed", "true");
  await expect(panel.getByTestId("theme-current-source")).toHaveText("Source / default");
  errors.assertNone();
});

for (const [id, name] of [["atlas", "Atlas"]]) {
  test(`selecting ${name} updates the current diagram immediately and shares its original recipe and name`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    const panel = await start(page);
    await panel.locator(`[data-theme-choice="exploration:${id}"]`).click();
    await expect(panel.getByTestId("theme-current-name")).toHaveText(name);
    await expect(panel.getByTestId("theme-current-source")).toHaveText("Merman example · JSON");
    await expect(panel.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", "residual");
    await expect(panel.getByRole("button", { name: "Apply", exact: true })).toHaveCount(0);
    await expect(panel.getByRole("button", { name: "Cancel", exact: true })).toHaveCount(0);
    await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
    await page.getByRole("button", { name: "Share", exact: true }).click();
    await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
    const url = await page.evaluate(() => navigator.clipboard.readText());
    const applied = decodeShareHash(new URL(url).hash)!;
    const recipe = await readFile(new URL(`../src/themes/recipes/${id}.theme.json`, import.meta.url), "utf8");
    expect(applied.themeRecipeJson).toBe(recipe);
    expect(applied.themePresetId).toBeNull();
    expect(applied.themeRecipeLabel).toEqual({ name, source: "exploration" });
    expect(applied.diagramTheme).toBe("auto");
    for (const key of ["code", "mermaidConfig", "diagramFont", "svgPipeline", "textMeasurementMode"] as const) {
      expect(applied[key]).toBe(workspace[key]);
    }
    await page.goto(url, { waitUntil: "domcontentloaded" });
    await waitForPreviewSvg(page);
    await page.locator("[data-theme-launcher]:visible").click();
    await expect(panel.getByTestId("theme-current-name")).toHaveText(name);
    await expect(panel.locator(`[data-theme-choice="exploration:${id}"]`)).toHaveAttribute("aria-pressed", "true");
    await panel.getByTestId("theme-advanced").locator("summary").first().click();
    await expect(panel.getByTestId("theme-json-document")).toHaveAttribute("data-code-language", "json");
    await panel.getByRole("button", { name: "Copy JSON", exact: true }).click();
    expect((await page.evaluate(() => navigator.clipboard.readText())).replace(/\r\n/gu, "\n")).toBe(recipe.replace(/\r\n/gu, "\n"));
    errors.assertNone();
  });
}

test("the unified catalog fits a narrow viewport and closing retains the selected theme", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  const panel = await start(page);
  await expectNoDocumentOverflow(page);
  await panel.locator('[data-theme-choice="exploration:letterpress"]').click();
  await panel.getByRole("button", { name: "Theme designs", exact: true }).click();
  const showcase = page.getByTestId("theme-showcase");
  await expect(showcase.getByRole("heading", { name: "Letterpress", exact: true })).toBeVisible();
  const samples = showcase.getByTestId("theme-sample-gallery");
  await expect(samples.locator("article[data-theme-sample]")).toHaveCount(8);
  await expect(samples.getByRole("link")).toHaveCount(8);
  await expect(showcase.getByRole("button", { name: "Diagram gallery", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect.poll(() => samples.evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
  await showcase.getByRole("button", { name: "Close theme designs", exact: true }).click();
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

for (const id of ["atlas", "nightfall", "letterpress", "grove", "riso", "phosphor", "field-notes", "blueprint", "pop-art", "lacquer", "paper-cut", "aurora", "metro", "carbon-copy", "signal", "porcelain", "chalk"]) {
  test(`${id} renders all eight samples with the browser runtime`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    await page.addInitScript(() => {
      if (window === window.top && (location.protocol === "http:" || location.protocol === "https:")) localStorage.setItem("merman-language", "en");
    });
    const recipe = await readFile(new URL(`../src/themes/recipes/${id}.theme.json`, import.meta.url), "utf8");
    for (const sample of CORE_THEME_SAMPLE_IDS) {
      const source = await readFile(new URL(`../src/themes/samples/${sample}.mmd`, import.meta.url), "utf8");
      const config = await readFile(new URL(`../src/themes/samples/${sample}.config.json`, import.meta.url), "utf8");
      // Share hydration runs at startup; a fragment-only navigation keeps the previous document.
      await page.goto("about:blank");
      await page.goto(`./${encodeShareHash({
        ...DEFAULT_WORKSPACE_SNAPSHOT, code: source, mermaidConfig: config,
        diagramTheme: "auto", themeRecipeJson: recipe,
      })}`, { waitUntil: "domcontentloaded" });
      await waitForPreviewSvg(page);
      await page.locator("[data-theme-launcher]:visible").click();
      const panel = page.locator("#theme-panel");
      await expect(panel.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", "residual");
      await expect(panel.locator("[data-theme-reason-code]")).toHaveCount(1);
      await expect(panel.locator("[data-theme-reason-code]")).toHaveAttribute("data-theme-reason-code", "compatibility-unverified");
      if (sample === "sequence") {
        const containment = await page.locator(".preview-container > div").first().evaluate((host) => {
          const svg = host.shadowRoot!.querySelector("svg")!;
          const background = svg.querySelector<SVGRectElement>(".merman-theme-canvas-base")!.getBBox();
          return [...svg.querySelectorAll<SVGLineElement>(".actor-line")].map((line) => {
            const bounds = line.getBBox();
            return { endpoint: bounds.y + bounds.height, canvasBottom: background.y + background.height };
          });
        });
        expect(containment).toHaveLength(3);
        for (const line of containment) expect(line.endpoint).toBeLessThanOrEqual(line.canvasBottom);
      }
      if (sample === "flowchart") {
        const expectedInk: Record<string, string> = {
          atlas: "rgb(31, 42, 61)", nightfall: "rgb(231, 236, 244)",
          letterpress: "rgb(49, 44, 42)", grove: "rgb(38, 57, 47)",
          riso: "rgb(35, 63, 120)", phosphor: "rgb(226, 239, 215)", "field-notes": "rgb(44, 75, 57)",
          blueprint: "rgb(227, 243, 250)", "pop-art": "rgb(25, 22, 32)", lacquer: "rgb(242, 233, 214)",
          "paper-cut": "rgb(36, 62, 74)", aurora: "rgb(235, 245, 255)",
          metro: "rgb(244, 243, 233)", "carbon-copy": "rgb(75, 44, 99)", signal: "rgb(34, 37, 31)",
          porcelain: "rgb(39, 55, 75)", chalk: "rgb(243, 238, 220)",
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
