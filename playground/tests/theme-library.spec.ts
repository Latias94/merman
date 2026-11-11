import { readFile } from "node:fs/promises";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { strFromU8, unzipSync } from "fflate";

import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT, type WorkspaceSnapshot } from "../src/lib/workspace-snapshot";
import { CORE_THEME_SAMPLE_IDS } from "../src/themes/families";
import { expectNoDocumentOverflow, monitorBrowserErrors, waitForPreviewSvg } from "./helpers/playground";
import { chooseThemeOption } from "./helpers/theme-select";

const workspace: WorkspaceSnapshot = {
  ...DEFAULT_WORKSPACE_SNAPSHOT,
  code: "erDiagram\nCURRENT_CUSTOMER ||--o{ CURRENT_ORDER : places\nCURRENT_CUSTOMER {\n string id PK\n string email\n}\nCURRENT_ORDER {\n string id PK\n string customer_id FK\n}",
  mermaidConfig: '{"htmlLabels":false}',
  diagramTheme: "neutral",
  diagramFont: "trebuchet",
  themePresetId: "brutalist",
};

async function start(page: Page, value = workspace, language = "en") {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.addInitScript((locale) => {
    if (window === window.top) {
      localStorage.setItem("merman-language", locale);
      localStorage.setItem("merman-ui-theme", "light");
    }
  }, language);
  await page.goto(`./${encodeShareHash(value)}`, { waitUntil: "domcontentloaded" });
  if ((page.viewportSize()?.width ?? 1280) < 768) await page.getByRole("tab", { name: language === "zh" ? "预览" : "Preview", exact: true }).click();
  await waitForPreviewSvg(page);
  await page.locator("[data-theme-showcase-launcher]:visible").click();
  return page.getByTestId("theme-showcase");
}

async function expectLoadedImage(preview: Locator) {
  await preview.scrollIntoViewIfNeeded();
  const image = preview.locator("img");
  await expect(image).toBeVisible();
  await expect.poll(() => image.evaluate((node) => (node as HTMLImageElement).complete && (node as HTMLImageElement).naturalWidth > 0)).toBe(true);
  return image;
}

async function expectInteractivePreview(preview: Locator, interactionMode: "embedded" | "canvas") {
  const viewport = preview.locator('[data-merman-svg-viewport="true"]');
  await expect(preview.getByTestId("theme-library-preview-canvas")).toHaveAttribute("role", "region");
  await expect(viewport).toBeVisible();
  await expect(viewport).toHaveAttribute("data-viewport-interaction", interactionMode);
  await expect(viewport.locator('.preview-container svg[role~="graphics-document"]')).toHaveCount(1);
  await expect(viewport.locator('.preview-container svg[role~="graphics-document"]')).toBeVisible();
  await viewport.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
  return viewport;
}

async function viewportZoom(viewport: Locator) {
  return Number(await viewport.getAttribute("data-zoom"));
}

async function viewportPosition(viewport: Locator) {
  return viewport.locator('[data-merman-viewport-position-layer="true"]').evaluate((element) => (element as HTMLElement).style.transform);
}

async function expectFittedViewport(viewport: Locator) {
  await expect(viewport).toHaveAttribute("data-auto-fit", "true");
  await expect(viewport.locator('[data-merman-viewport-position-layer="true"]')).toHaveAttribute("style", /translate\(0px, 0px\)/u);
}

async function dispatchTouch(viewport: Locator, type: "pointerdown" | "pointermove" | "pointerup", pointerId: number, clientX: number, clientY: number) {
  await viewport.dispatchEvent(type, {
    bubbles: true, composed: true, button: 0, buttons: type === "pointerup" ? 0 : 1,
    pointerType: "touch", pointerId, isPrimary: pointerId === 71, clientX, clientY,
  });
}

async function sharedWorkspace(page: Page) {
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  return decodeShareHash(new URL(await page.evaluate(() => navigator.clipboard.readText())).hash)!;
}

test("visible family, appearance and style filters compose while clearing preserves search text", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const library = await start(page);
  const filters = library.getByTestId("theme-library-filters");
  const family = filters.getByRole("combobox", { name: "Diagram family filter", exact: true });
  const search = filters.getByRole("searchbox", { name: "Search designs", exact: true });
  await expect(family).toHaveAttribute("data-value", "er");
  await expect(library.getByTestId("theme-library-search-area").getByTestId("theme-library-family-chip")).toBeVisible();
  await search.fill("Reading");
  await expect(library.locator("[data-theme-design-card]")).toHaveCount(1);
  await expect(library.locator('[data-theme-design-card="reading"]')).toHaveAttribute("data-theme-design-family", "er");
  const readingCard = library.locator('[data-theme-design-card="reading"]');
  await expect(readingCard.locator("[data-theme-card-sample]")).toHaveCount(3);
  await expect(readingCard.locator('[data-theme-card-primary="true"]')).toHaveAttribute("data-theme-card-sample", "er");
  await expect(readingCard.locator('[data-theme-card-primary="false"]')).toHaveCount(2);
  await expect(readingCard.locator("img")).toHaveCount(3);

  await filters.getByRole("button", { name: "Remove ER Diagram filter", exact: true }).click();
  await expect(family).toBeFocused();
  await expect(search).toHaveValue("Reading");
  await expect(family).toHaveAttribute("data-value", "");
  await expect(library.locator('[data-theme-design-card="reading"]')).toHaveAttribute("data-theme-design-family", "collage");
  await expect(readingCard.locator('[data-theme-card-primary="true"]')).toHaveAttribute("data-theme-card-sample", "flowchart");
  await chooseThemeOption(page, family, "state");
  await expect(library.locator('[data-theme-design-card="reading"]')).toHaveAttribute("data-theme-design-family", "state");
  await expect(readingCard.locator('[data-theme-card-primary="true"]')).toHaveAttribute("data-theme-card-sample", "state");
  await expect(readingCard.locator("[data-theme-card-sample]")).toHaveCount(3);
  await chooseThemeOption(page, family, "sequence");
  await expect(readingCard.locator('[data-theme-card-primary="true"]')).toHaveAttribute("data-theme-card-sample", "sequence");
  const previews = await readingCard.locator("[data-theme-card-sample]").evaluateAll((elements) => elements.map((element) => element.getAttribute("data-theme-card-sample")));
  expect(previews).toHaveLength(3);
  expect(new Set(previews).size).toBe(3);
  await filters.getByRole("button", { name: "Light + dark", exact: true }).click();
  await chooseThemeOption(page, filters.getByRole("combobox", { name: "Design style filter", exact: true }), "technical");
  await expect(library.locator("[data-theme-design-card]")).toHaveCount(1);
  await expect(library.locator('[data-theme-design-card="editor"]')).toBeVisible();
  await chooseThemeOption(page, filters.getByRole("combobox", { name: "Design style filter", exact: true }), "print");
  const empty = library.getByTestId("theme-library-empty");
  await expect(empty).toBeVisible();
  await empty.getByRole("button", { name: "Clear filters", exact: true }).click();
  await expect(search).toBeFocused();
  await expect(search).toHaveValue("Reading");
  await expect(family).toHaveAttribute("data-value", "");
  await expect(filters.getByRole("combobox", { name: "Design style filter", exact: true })).toHaveAttribute("data-value", "all");
  await expect(filters.getByRole("button", { name: "All designs", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(library.locator("[data-theme-design-card]")).toHaveCount(4);
  expect(await library.locator("[data-theme-design-card]").evaluateAll((cards) =>
    cards.map((card) => card.getAttribute("data-theme-design-card")).sort())).toEqual(["ayu", "editor", "gruvbox", "reading"]);
  await filters.getByRole("button", { name: "Clear search", exact: true }).click();
  await expect.poll(() => library.locator("[data-theme-design-card]").count()).toBeGreaterThan(1);
  await expectNoDocumentOverflow(page);
  const searchArea = await library.getByTestId("theme-library-search-area").boundingBox();
  const familyBox = await family.boundingBox();
  expect(searchArea).not.toBeNull();
  expect(familyBox).not.toBeNull();
  expect(familyBox!.x).toBeGreaterThanOrEqual(searchArea!.x);
  expect(familyBox!.x + familyBox!.width).toBeLessThanOrEqual(searchArea!.x + searchArea!.width + 1);
  errors.assertNone();
});

test("styled filter options preserve selection on Escape and commit keyboard choices with trigger focus", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const library = await start(page);
  const filters = library.getByTestId("theme-library-filters");
  const family = filters.getByRole("combobox", { name: "Diagram family filter", exact: true });
  const listbox = page.getByRole("listbox");
  await family.click();
  await expect(listbox).toBeVisible();
  const selected = listbox.locator('[role="option"][data-value="er"]');
  await expect(selected).toHaveAttribute("data-state", "checked");
  await expect(selected).toHaveAttribute("data-highlighted", "");
  await expect(selected).toHaveAttribute("aria-selected", "true");
  await expect.poll(() => listbox.evaluate((element) => {
    const probe = document.createElement("span");
    probe.style.backgroundColor = "var(--popover)";
    element.append(probe);
    const expected = getComputedStyle(probe).backgroundColor;
    probe.remove();
    return expected !== "rgba(0, 0, 0, 0)" && getComputedStyle(element).backgroundColor === expected;
  })).toBe(true);
  await page.keyboard.press("Home");
  await expect(listbox.locator('[role="option"][data-value=""]')).toHaveAttribute("data-highlighted", "");
  await page.keyboard.press("Escape");
  await expect(listbox).toBeHidden();
  await expect(library).toBeVisible();
  await expect(family).toHaveAttribute("data-value", "er");
  await expect(family).toBeFocused();

  const style = filters.getByRole("combobox", { name: "Design style filter", exact: true });
  await style.press("Enter");
  await expect(listbox).toBeVisible();
  await page.keyboard.press("Home");
  await expect(listbox.locator('[role="option"][data-value="all"]')).toHaveAttribute("data-highlighted", "");
  await page.keyboard.press("ArrowDown");
  await expect(listbox.locator('[role="option"][data-value="reading"]')).toHaveAttribute("data-highlighted", "");
  await page.keyboard.press("Enter");
  await expect(listbox).toBeHidden();
  await expect(style).toHaveAttribute("data-value", "reading");
  await expect(style).toBeFocused();
  await chooseThemeOption(page, style, "all");
  await chooseThemeOption(page, family, "");
  await expect(family).toBeFocused();
  await expect(library.getByTestId("theme-library-family-chip")).toHaveCount(0);
  errors.assertNone();
});

test("returning from expanded preview, detail and reopening retain browsing position and keyboard focus", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const library = await start(page);
  const viewport = library.getByTestId("theme-showcase-viewport");
  const card = library.getByRole("button", { name: "Explore Blueprint", exact: true });
  await card.scrollIntoViewIfNeeded();
  await card.focus();
  const scrollTop = await viewport.evaluate((element) => element.scrollTop);
  expect(scrollTop).toBeGreaterThan(0);
  await card.press("Enter");
  const detail = library.getByTestId("theme-showcase-detail");
  await expect(detail.getByRole("heading", { name: "Blueprint", exact: true })).toBeFocused();
  const gallery = detail.getByTestId("theme-sample-gallery");
  await expect(gallery.locator("article[data-theme-sample]")).toHaveCount(9);
  await expect(gallery.locator('article[data-theme-sample="architecture"]')).toHaveCount(1);
  await expect(gallery.locator('article[data-theme-sample="er"]')).toHaveAttribute("data-theme-sample-selected", "true");
  await expect(detail.getByRole("button", { name: "Diagram gallery", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(detail.getByRole("combobox", { name: "Example diagram family", exact: true })).toHaveCount(0);
  await expect(detail.getByTestId("theme-library-preview")).toHaveCount(0);
  await expect(gallery.getByRole("link")).toHaveCount(9);
  const inspect = gallery.locator('article[data-theme-sample="mindmap"]').getByRole("button", { name: "Inspect Mindmap example", exact: true });
  await inspect.scrollIntoViewIfNeeded();
  await inspect.focus();
  const detailViewport = library.getByTestId("theme-showcase-detail-viewport");
  const detailScrollTop = await detailViewport.evaluate((element) => element.scrollTop);
  expect(detailScrollTop).toBeGreaterThan(0);
  await inspect.press("Enter");
  const expanded = page.getByTestId("theme-library-expanded-preview");
  const preview = expanded.getByTestId("theme-library-preview");
  const family = expanded.getByRole("combobox", { name: "Example diagram family", exact: true });
  await expect(family).toHaveAttribute("data-value", "mindmap");
  await expect(preview).toHaveAttribute("data-theme-sample", "mindmap");
  const expandedViewport = await expectInteractivePreview(preview, "canvas");
  await expect(preview.locator("[data-theme-preview-appearance]")).toHaveAttribute("data-theme-preview-appearance", "dark");
  await expect(expandedViewport.locator('.preview-container svg[role~="graphics-document"]')).toContainText("Service launch");
  await expect(preview.getByRole("link", { name: "Open Blueprint Mindmap sample in a new tab", exact: true })).toHaveCount(1);
  await expect(detail.getByTestId("theme-adoption-appearance")).toHaveAttribute("data-theme-adoption-sample", "mindmap");
  await chooseThemeOption(page, family, "sequence");
  await expect(preview).toHaveAttribute("data-theme-sample", "sequence");
  await expect(expandedViewport.locator('.preview-container svg[role~="graphics-document"]')).toContainText("Submit draft");
  await expect(expandedViewport.locator('.preview-container svg[role~="graphics-document"]')).not.toContainText("Service launch");
  await expanded.getByRole("button", { name: "Next example", exact: true }).click();
  await expect(family).toHaveAttribute("data-value", "class");
  await expect(preview).toHaveAttribute("data-theme-sample", "class");
  await expanded.getByRole("button", { name: "Previous example", exact: true }).click();
  await expect(family).toHaveAttribute("data-value", "sequence");
  await expect(preview).toHaveAttribute("data-theme-sample", "sequence");
  await expectInteractivePreview(preview, "canvas");
  await expect(expandedViewport.locator('.preview-container svg[role~="graphics-document"]')).toContainText("Submit draft");
  await expect(preview.locator("[data-theme-preview-appearance]")).toHaveAttribute("data-theme-preview-appearance", "dark");
  await expect(preview.getByRole("link", { name: "Open Blueprint Sequence sample in a new tab", exact: true })).toHaveCount(1);
  await expanded.getByRole("button", { name: "Close expanded preview", exact: true }).click();
  await expect(expanded).toHaveCount(0);
  await expect(inspect).toBeFocused();
  await expect(inspect).toBeInViewport();
  await expect.poll(() => detailViewport.evaluate((element) => element.scrollTop)).toBeCloseTo(detailScrollTop, 0);
  await expect(gallery.locator('article[data-theme-sample="sequence"]')).toHaveAttribute("data-theme-sample-selected", "true");
  await expect(detail.getByTestId("theme-adoption-appearance")).toHaveAttribute("data-theme-adoption-sample", "sequence");
  await expect(gallery.locator("article[data-theme-sample]")).toHaveCount(9);
  await expect(gallery.locator('article[data-theme-sample="architecture"]')).toHaveCount(1);
  await expect(gallery.getByRole("link")).toHaveCount(9);
  const adoption = detail.getByTestId("theme-showcase-use");
  await adoption.getByRole("tab", { name: "CLI", exact: true }).click();
  await expect(adoption.getByRole("combobox", { name: "Example diagram for this code", exact: true })).toHaveAttribute("data-value", "sequence");
  await adoption.getByRole("navigation", { name: "Project files", exact: true }).getByRole("button", { name: "diagram.mmd", exact: true }).click();
  await expect(adoption.getByTestId("theme-showcase-code")).toContainText("sequenceDiagram");
  await detail.getByRole("button", { name: "All designs", exact: true }).click();
  await expect(card).toBeFocused();
  await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBeCloseTo(scrollTop, 0);
  await library.getByRole("button", { name: "Close theme designs", exact: true }).click();
  const launcher = page.locator("[data-theme-showcase-launcher]:visible");
  await expect(launcher).toBeFocused();
  await launcher.click();
  await expect(card).toBeFocused();
  await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBeCloseTo(scrollTop, 0);

  const search = library.getByRole("searchbox", { name: "Search designs", exact: true });
  await search.fill("reading");
  await search.evaluate((element) => (element as HTMLInputElement).setSelectionRange(2, 5));
  await library.getByRole("button", { name: "Close theme designs", exact: true }).click();
  await launcher.click();
  await expect(search).toHaveValue("reading");
  await expect(search).toBeFocused();
  expect(await search.evaluate((element) => [(element as HTMLInputElement).selectionStart, (element as HTMLInputElement).selectionEnd])).toEqual([2, 5]);
  errors.assertNone();
});

test("reopening retains the adoption target, project file, disclosure and keyboard focus", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const library = await start(page);
  await library.getByRole("button", { name: "Explore Riso", exact: true }).click();
  const detail = library.getByTestId("theme-showcase-detail");
  await detail.getByRole("button", { name: "Use in my project", exact: true }).click();
  const adoption = detail.getByTestId("theme-showcase-use");
  await adoption.getByRole("tab", { name: "Rust", exact: true }).click();
  const files = adoption.getByRole("radio", { name: "Project files · include_str!", exact: true });
  await files.check();
  const disclosure = adoption.locator("details");
  await disclosure.locator("summary").click();
  await expect(disclosure).toHaveJSProperty("open", true);
  const readme = adoption.getByRole("navigation", { name: "Project files", exact: true }).getByRole("button", { name: "README.txt", exact: true });
  await readme.click();
  await expect(readme).toHaveAttribute("aria-current", "true");
  const code = adoption.getByTestId("theme-showcase-code");
  await expect(code).not.toBeEmpty();
  const content = await code.innerText();
  const viewport = library.getByTestId("theme-showcase-detail-viewport");
  const scrollTop = await viewport.evaluate((element) => element.scrollTop);
  expect(scrollTop).toBeGreaterThan(0);
  await readme.press("Escape");
  await expect(library).toHaveCount(0);
  await page.locator("[data-theme-showcase-launcher]:visible").click();
  await expect(adoption.getByRole("tab", { name: "Rust", exact: true })).toHaveAttribute("aria-selected", "true");
  await expect(files).toBeChecked();
  await expect(readme).toHaveAttribute("aria-current", "true");
  await expect(readme).toBeFocused();
  await expect(disclosure).toHaveJSProperty("open", true);
  await expect(code).toHaveText(content, { useInnerText: true });
  await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBeCloseTo(scrollTop, 0);
  await expectNoDocumentOverflow(page);
  errors.assertNone();
});

test("using different gallery tiles synchronizes sample links, adoption code and downloads in the selected appearance", async ({ page }) => {
  test.setTimeout(90_000);
  const errors = monitorBrowserErrors(page);
  const library = await start(page);
  await library.getByRole("button", { name: "Light + dark", exact: true }).click();
  await library.getByRole("button", { name: "Explore Reading", exact: true }).click();
  const detail = library.getByTestId("theme-showcase-detail");
  const gallery = detail.getByTestId("theme-sample-gallery");
  await expect(gallery.locator("article[data-theme-sample]")).toHaveCount(8);
  await expect(gallery.locator('article[data-theme-sample="er"]')).toHaveAttribute("data-theme-sample-selected", "true");
  await expect(detail.getByRole("combobox", { name: "Example diagram family", exact: true })).toHaveCount(0);
  await chooseThemeOption(page, detail.getByTestId("theme-appearance-control").getByRole("combobox"), "dark");
  const adoption = detail.getByTestId("theme-showcase-use");
  await adoption.getByRole("tab", { name: "CLI", exact: true }).click();
  await expect(adoption.getByTestId("theme-library-minimum-version")).toHaveText("Requires v0.10.0-alpha.2 or newer");

  for (const sampleId of CORE_THEME_SAMPLE_IDS) {
    const tile = gallery.locator(`article[data-theme-sample="${sampleId}"]`);
    const image = await expectLoadedImage(tile);
    await expect(image).toHaveAttribute("data-theme-preview-appearance", "dark");
    const source = await readFile(new URL(`../src/themes/samples/${sampleId}.mmd`, import.meta.url), "utf8");
    const config = await readFile(new URL(`../src/themes/samples/${sampleId}.config.json`, import.meta.url), "utf8");
    const href = await tile.getByRole("link").getAttribute("href");
    const saved = decodeShareHash(new URL(href!, page.url()).hash)!;
    expect(saved.code).toBe(source);
    expect(saved.mermaidConfig).toBe(config);
    expect(saved.themeAppearance).toBe("dark");
    expect(saved.themeRecipeLabel).toEqual({ name: "Reading", source: "exploration" });
    await tile.getByRole("button", { name: "Use this example", exact: true }).click();
    await expect(tile).toHaveAttribute("data-theme-sample-selected", "true");
    await expect(gallery.locator('article[data-theme-sample-selected="true"]')).toHaveCount(1);
    await expect(adoption.getByRole("heading")).toBeInViewport();
    await expect(adoption.getByTestId("theme-adoption-appearance")).toHaveAttribute("data-theme-adoption-sample", sampleId);
    await expect(adoption.getByTestId("theme-adoption-appearance")).toHaveAttribute("data-theme-adoption-appearance", "dark");
    await expect(adoption.getByRole("combobox", { name: "Example diagram for this code", exact: true })).toHaveAttribute("data-value", sampleId);
    await adoption.getByRole("navigation", { name: "Project files", exact: true }).getByRole("button", { name: "diagram.mmd", exact: true }).click();
    await expect(adoption.getByTestId("theme-showcase-code")).toContainText(source.trim().split(/\r?\n/u)[0]);
    if (sampleId === "er" || sampleId === "mindmap") {
      const pending = page.waitForEvent("download");
      await adoption.getByRole("button", { name: "Download example project", exact: true }).click();
      const download = await pending;
      expect(download.suggestedFilename()).toBe(`reading-${sampleId}-cli.zip`);
      const files = unzipSync(await readFile((await download.path())!));
      expect(strFromU8(files["diagram.mmd"])).toContain(source.trim());
      expect(JSON.parse(strFromU8(files["options.json"])).theme.appearance).toBe("dark");
      expect(strFromU8(files["theme.json"])).toBe(saved.themeRecipeJson);
      expect(strFromU8(files["render-command.txt"])).toContain("--theme-appearance dark");
    }
  }
  await chooseThemeOption(page, adoption.getByRole("combobox", { name: "Example diagram for this code", exact: true }), "state");
  await expect(gallery.locator('article[data-theme-sample="state"]')).toHaveAttribute("data-theme-sample-selected", "true");
  await adoption.getByRole("navigation", { name: "Project files", exact: true }).getByRole("button", { name: "diagram.mmd", exact: true }).click();
  await expect(adoption.getByTestId("theme-showcase-code")).toContainText("stateDiagram-v2");
  await expect(gallery.locator("article[data-theme-sample]")).toHaveCount(8);
  await expect(detail.getByTestId("theme-library-preview")).toHaveCount(0);
  await expectNoDocumentOverflow(page);
  errors.assertNone();
});

test("inline current-diagram preview scrolls the detail, zooms with Control and pans with the mouse", async ({ page, isMobile }) => {
  test.skip(isMobile, "Wheel and mouse gestures are covered on desktop.");
  const errors = monitorBrowserErrors(page);
  const library = await start(page);
  await library.getByRole("button", { name: "Explore Riso", exact: true }).click();
  const detail = library.getByTestId("theme-showcase-detail");
  await detail.getByRole("button", { name: "My current diagram", exact: true }).click();
  const preview = detail.getByTestId("theme-library-preview");
  const viewport = await expectInteractivePreview(preview, "embedded");
  const detailViewport = library.getByTestId("theme-showcase-detail-viewport");
  await viewport.scrollIntoViewIfNeeded();
  await viewport.hover();
  const initialZoom = await viewportZoom(viewport);
  const initialScrollTop = await detailViewport.evaluate((element) => element.scrollTop);
  expect(await detailViewport.evaluate((element) => element.scrollHeight - element.clientHeight - element.scrollTop)).toBeGreaterThan(120);
  await page.mouse.wheel(0, 120);
  await expect.poll(() => detailViewport.evaluate((element) => element.scrollTop)).toBeGreaterThan(initialScrollTop);
  await expect.poll(() => detailViewport.evaluate((element) => element.scrollTop)).toBeCloseTo(initialScrollTop + 120, 0);
  await expect.poll(() => viewportZoom(viewport)).toBe(initialZoom);

  await viewport.hover();
  const scrollBeforeZoom = await detailViewport.evaluate((element) => element.scrollTop);
  await page.keyboard.down("Control");
  try {
    await page.mouse.wheel(0, -200);
    await expect.poll(() => viewportZoom(viewport)).toBeGreaterThan(initialZoom);
  } finally {
    await page.keyboard.up("Control");
  }
  await expect.poll(() => detailViewport.evaluate((element) => element.scrollTop)).toBeCloseTo(scrollBeforeZoom, 0);
  await expect(viewport).toHaveAttribute("data-auto-fit", "false");

  const beforeDrag = await viewportPosition(viewport);
  const box = await viewport.boundingBox();
  expect(box).not.toBeNull();
  const x = box!.x + box!.width / 2;
  const y = box!.y + box!.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 48, y + 24, { steps: 8 });
  await expect(viewport).toHaveAttribute("data-dragging", "true");
  await page.mouse.up();
  await expect(viewport).toHaveAttribute("data-dragging", "false");
  await expect.poll(() => viewportPosition(viewport)).not.toBe(beforeDrag);
  await preview.getByRole("button", { name: "Fit", exact: true }).click();
  await expectFittedViewport(viewport);
  await expectNoDocumentOverflow(page);
  errors.assertNone();
});

test("current-diagram inspection is isolated until Apply and supports source, fit and expanded gestures", async ({ page }) => {
  test.setTimeout(60_000);
  const errors = monitorBrowserErrors(page);
  const library = await start(page);
  await library.getByRole("button", { name: "Explore Riso", exact: true }).click();
  const detail = library.getByTestId("theme-showcase-detail");
  const gallery = detail.getByTestId("theme-sample-gallery");
  await gallery.locator('article[data-theme-sample="mindmap"]').getByRole("button", { name: "Use this example", exact: true }).click();
  await detail.getByRole("button", { name: "My current diagram", exact: true }).click();
  await expect(gallery).toBeHidden();
  await detail.getByRole("button", { name: "Diagram gallery", exact: true }).click();
  await expect(gallery.locator("article[data-theme-sample]")).toHaveCount(8);
  await expect(gallery.locator('article[data-theme-sample="mindmap"]')).toHaveAttribute("data-theme-sample-selected", "true");
  await detail.getByRole("button", { name: "My current diagram", exact: true }).click();
  const preview = detail.getByTestId("theme-library-preview");
  const viewport = await expectInteractivePreview(preview, "embedded");
  await expect(viewport.locator('.preview-container svg[role~="graphics-document"]')).toContainText("CURRENT_ORDER");
  await expect(preview.locator('[data-theme-preview-target="current"]')).toBeVisible();
  const fittedZoom = await viewportZoom(viewport);
  await preview.getByRole("button", { name: "Zoom in on design", exact: true }).click();
  await expect.poll(() => viewportZoom(viewport)).toBeGreaterThan(fittedZoom);
  const sourceToggleZoom = await viewportZoom(viewport);
  const viewportElement = await viewport.elementHandle();
  await preview.getByRole("button", { name: "Show diagram source", exact: true }).click();
  await expect(detail.getByTestId("theme-library-source")).toContainText("CURRENT_ORDER");
  await expect(viewport).toBeHidden();
  await preview.getByRole("button", { name: "Show diagram preview", exact: true }).click();
  await expect(viewport).toBeVisible();
  expect(await viewportElement!.evaluate((element) => element.isConnected)).toBe(true);
  await expect.poll(() => viewportZoom(viewport)).toBe(sourceToggleZoom);
  await expect(viewport.locator('.preview-container svg[role~="graphics-document"]')).toContainText("CURRENT_ORDER");
  await preview.getByRole("button", { name: "Expand preview", exact: true }).click();
  await viewportElement!.dispose();
  const expanded = page.getByTestId("theme-library-expanded-preview");
  const expandedPreview = expanded.getByTestId("theme-library-preview");
  const expandedViewport = await expectInteractivePreview(expandedPreview, "canvas");
  await expect(expandedViewport.locator('.preview-container svg[role~="graphics-document"]')).toContainText("CURRENT_ORDER");
  const beforeButtonZoom = await viewportZoom(expandedViewport);
  await expanded.getByRole("button", { name: "Zoom in on design", exact: true }).click();
  await expect.poll(() => viewportZoom(expandedViewport)).toBeGreaterThan(beforeButtonZoom);
  const beforeZoomOut = await viewportZoom(expandedViewport);
  await expanded.getByRole("button", { name: "Zoom out of design", exact: true }).click();
  await expect.poll(() => viewportZoom(expandedViewport)).toBeLessThan(beforeZoomOut);
  await expanded.getByRole("button", { name: "Reset view", exact: true }).click();
  await expect.poll(() => viewportZoom(expandedViewport)).toBe(1);
  await expect(expandedViewport.locator('[data-merman-viewport-position-layer="true"]')).toHaveAttribute("style", /translate\(0px, 0px\)/u);
  await expandedViewport.hover();
  const beforeWheelZoom = await viewportZoom(expandedViewport);
  await page.mouse.wheel(0, -200);
  await expect.poll(() => viewportZoom(expandedViewport)).toBeGreaterThan(beforeWheelZoom);
  await expanded.getByRole("button", { name: "Fit", exact: true }).click();
  await expectFittedViewport(expandedViewport);

  const box = await expandedViewport.boundingBox();
  expect(box).not.toBeNull();
  const x = box!.x + box!.width / 2;
  const y = box!.y + box!.height / 2;
  const beforePinch = await viewportZoom(expandedViewport);
  await dispatchTouch(expandedViewport, "pointerdown", 71, x - 30, y);
  await dispatchTouch(expandedViewport, "pointerdown", 72, x + 30, y);
  await dispatchTouch(expandedViewport, "pointermove", 71, x - 60, y);
  await dispatchTouch(expandedViewport, "pointermove", 72, x + 60, y);
  await dispatchTouch(expandedViewport, "pointerup", 71, x - 60, y);
  await dispatchTouch(expandedViewport, "pointerup", 72, x + 60, y);
  await expect.poll(() => viewportZoom(expandedViewport)).toBeGreaterThan(beforePinch);
  await expect(expandedViewport).toHaveAttribute("data-dragging", "false");
  await expanded.getByRole("button", { name: "Fit", exact: true }).click();
  await expectFittedViewport(expandedViewport);
  await expanded.getByRole("button", { name: "Close expanded preview", exact: true }).click();
  await expect(preview.getByRole("button", { name: "Expand preview", exact: true })).toBeFocused();
  await library.getByRole("button", { name: "Close theme designs", exact: true }).click();
  const unchanged = await sharedWorkspace(page);
  for (const key of Object.keys(workspace) as (keyof WorkspaceSnapshot)[]) expect(unchanged[key]).toEqual(workspace[key]);

  await page.locator("[data-theme-showcase-launcher]:visible").click();
  await expect(detail.getByRole("combobox", { name: "Example diagram family", exact: true })).toHaveCount(0);
  await expect(detail.getByTestId("theme-adoption-appearance")).toHaveAttribute("data-theme-adoption-sample", "mindmap");
  await expect(detail.getByRole("button", { name: "My current diagram", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(detail.getByRole("button", { name: "Expand preview", exact: true })).toBeFocused();
  await detail.getByRole("button", { name: "Apply to my diagram", exact: true }).click();
  await expect(library).toHaveCount(0);
  const applied = await sharedWorkspace(page);
  expect(applied.code).toBe(workspace.code);
  expect(applied.mermaidConfig).toBe(workspace.mermaidConfig);
  expect(applied.diagramFont).toBe(workspace.diagramFont);
  expect(applied.svgPipeline).toBe(workspace.svgPipeline);
  expect(applied.textMeasurementMode).toBe(workspace.textMeasurementMode);
  expect(applied.themeRecipeJson).toBe(await readFile(new URL("../src/themes/recipes/riso.theme.json", import.meta.url), "utf8"));
  errors.assertNone();
});

test("a search without a reviewed design for the current family offers a clear action without claiming unsupported rendering", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const library = await start(page, { ...workspace, code: "gantt\n title Delivery\n dateFormat YYYY-MM-DD\n section Plan\n Design :a, 2026-11-01, 2d" });
  const family = library.getByRole("combobox", { name: "Diagram family filter", exact: true });
  await expect(family).toHaveAttribute("data-value", "gantt");
  await library.getByRole("searchbox", { name: "Search designs", exact: true }).fill("Riso");
  const empty = library.getByTestId("theme-library-empty");
  await expect(empty).toContainText("No reviewed designs");
  await expect(empty).not.toContainText(/unsupported/i);
  await empty.getByRole("button", { name: "Clear filters", exact: true }).click();
  await expect(family).toHaveAttribute("data-value", "");
  await expect(library.getByRole("searchbox", { name: "Search designs", exact: true })).toHaveValue("Riso");
  await expect(library.getByRole("button", { name: "Explore Riso", exact: true })).toBeVisible();
  await library.getByRole("button", { name: "Close theme designs", exact: true }).click();
  await page.locator("[data-theme-showcase-launcher]:visible").click();
  await expect(family).toHaveAttribute("data-value", "");
  await expect(library.getByTestId("theme-library-empty")).toHaveCount(0);
  errors.assertNone();
});

test("Chinese gallery controls keep all families reachable on mobile and retain the collection filter", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.setViewportSize({ width: 390, height: 844 });
  const library = await start(page, workspace, "zh");
  await expect(library.getByRole("heading", { name: "主题设计库", exact: true })).toBeVisible();
  const family = library.getByRole("combobox", { name: "图表类型筛选", exact: true });
  await family.click();
  const listbox = page.getByRole("listbox");
  await expect(listbox).toBeVisible();
  await expectNoDocumentOverflow(page);
  const optionsBox = await listbox.boundingBox();
  expect(optionsBox).not.toBeNull();
  expect(optionsBox!.x).toBeGreaterThanOrEqual(0);
  expect(optionsBox!.x + optionsBox!.width).toBeLessThanOrEqual(page.viewportSize()!.width + 1);
  await page.keyboard.press("Escape");
  await expect(listbox).toBeHidden();
  await expect(library).toBeVisible();
  await expect(family).toBeFocused();
  await chooseThemeOption(page, family, "state");
  await library.getByRole("searchbox", { name: "搜索主题设计", exact: true }).fill("双色油墨");
  await library.getByRole("button", { name: "查看 Riso 设计", exact: true }).click();
  const detail = library.getByTestId("theme-showcase-detail");
  await expect(detail.getByRole("button", { name: "图表画廊", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(detail.getByRole("combobox", { name: "示例图表类型", exact: true })).toHaveCount(0);
  const gallery = detail.getByTestId("theme-sample-gallery");
  await expect(gallery.locator("article[data-theme-sample]")).toHaveCount(8);
  const state = gallery.locator('article[data-theme-sample="state"]');
  await expect(state).toHaveAttribute("data-theme-sample-selected", "true");
  await expect(state.getByRole("link", { name: "在新标签页打开 Riso 的状态图样例", exact: true })).toHaveCount(1);
  const inspect = state.getByRole("button", { name: "放大查看状态图示例", exact: true });
  await inspect.click();
  const expanded = page.getByTestId("theme-library-expanded-preview");
  const sample = expanded.getByRole("combobox", { name: "示例图表类型", exact: true });
  await expect(sample).toHaveAttribute("data-value", "state");
  await chooseThemeOption(page, sample, "er");
  await expect(expanded.getByTestId("theme-library-preview")).toHaveAttribute("data-theme-sample", "er");
  const viewport = await expectInteractivePreview(expanded.getByTestId("theme-library-preview"), "canvas");
  await expect(viewport.locator('.preview-container svg[role~="graphics-document"]')).toContainText("RESERVATION");
  await expect(expanded.locator("[data-theme-preview-appearance]")).toHaveAttribute("data-theme-preview-appearance", "light");
  await expectNoDocumentOverflow(page);
  await expanded.getByRole("button", { name: "关闭展开预览", exact: true }).click();
  await expect(inspect).toBeFocused();
  await expect(gallery.locator('article[data-theme-sample="er"]')).toHaveAttribute("data-theme-sample-selected", "true");
  await expect.poll(() => gallery.evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
  await detail.getByRole("button", { name: "全部设计", exact: true }).click();
  await expect(family).toHaveAttribute("data-value", "state");
  await library.getByRole("button", { name: "移除状态图筛选", exact: true }).click();
  await expect(library.getByRole("searchbox", { name: "搜索主题设计", exact: true })).toHaveValue("双色油墨");
  await expectNoDocumentOverflow(page);
  errors.assertNone();
});
