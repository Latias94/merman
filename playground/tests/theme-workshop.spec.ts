import { readFile } from "node:fs/promises";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import { expectNoDocumentOverflow, monitorBrowserErrors, openPlayground, waitForPreviewSvg } from "./helpers/playground";

const nativeCanvas = (page: Page) => page.locator(".preview-container svg .merman-theme-canvas").first();

async function openWorkshop(page: Page) {
  await openPlayground(page);
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  const panel = page.getByRole("region", { name: "Appearance", exact: true });
  await expect(panel.getByRole("tab", { name: "Customize" })).toHaveAttribute("aria-selected", "true");
  await expect(panel.getByTestId("theme-customize")).toBeVisible();
  return panel;
}

async function downloadDocument(page: Page, panel: Locator) {
  const pending = page.waitForEvent("download");
  await panel.getByRole("button", { name: "Download JSON" }).click();
  const download = await pending;
  return JSON.parse(await readFile((await download.path())!, "utf8"));
}

async function replaceThemeJson(panel: Locator, value: string) {
  const manifest = JSON.parse(await readFile(new URL("../dist/.vite/manifest.json", import.meta.url), "utf8")) as Record<string, { name?: string; file: string }>;
  const chunk = Object.values(manifest).find(({ name }) => name === "monaco");
  if (!chunk) throw new Error("Missing Monaco build output.");
  await panel.getByTestId("theme-json-editor").locator(".monaco-editor").evaluate(async (node, { file, text }) => {
    const exports = await import(new URL(file, location.href).href);
    const monaco = Object.values(exports).find((candidate) => candidate && typeof candidate === "object" &&
      "editor" in candidate && "Uri" in candidate) as typeof import("monaco-editor") | undefined;
    const editor = monaco?.editor.getEditors().find((candidate) => candidate.getDomNode() === node);
    if (!editor) throw new Error("Missing theme JSON editor.");
    editor.setValue(text);
  }, { file: chunk.file, text: value });
}

test("the runtime gallery applies native presets and offers Mermaid Classic", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const panel = await openWorkshop(page);
  await panel.getByRole("tab", { name: "Themes", exact: true }).click();
  const gallery = panel.getByTestId("theme-gallery");
  await expect(gallery.locator("[data-theme-preset]")).toHaveCount(13);
  for (const group of ["Reading", "Editor", "Art"]) await expect(gallery.getByRole("region", { name: group, exact: true })).toBeVisible();
  const studio = gallery.locator('[data-theme-preset="studio"]');
  await expect(studio.locator("[data-preset-swatch]")).toHaveCount(6);
  await studio.click();
  await expect(studio).toHaveAttribute("aria-pressed", "true");
  await waitForPreviewSvg(page);
  await expect(nativeCanvas(page)).toBeAttached();
  await expect(panel.locator('[data-theme-mode="merman_native"]')).toBeVisible();
  const appearance = panel.getByTestId("theme-appearance-control");
  await appearance.getByRole("combobox").click();
  await page.getByRole("option", { name: "Dark", exact: true }).click();
  await expect(page.locator("#preview-mode-panel")).toHaveAttribute("data-preview-canvas-tone", "dark");

  const classic = gallery.getByTestId("mermaid-classic-themes");
  await expect(classic.locator('[data-mermaid-theme="forest"]')).toBeHidden();
  await classic.locator("summary").click();
  await expect(classic.locator('[data-mermaid-theme]')).toHaveCount(12);
  await classic.locator('[data-mermaid-theme="forest"]').click();
  await expect(panel.locator('[data-theme-mode="mermaid_parity"]')).toBeVisible();
  await expect(nativeCanvas(page)).toHaveCount(0);
  const browse = gallery.getByRole("button", { name: "Browse theme library" });
  await browse.click();
  const library = themeLibrary(page);
  await library.getByRole("searchbox").fill("riso");
  await expect(library.locator("[data-browser-preset]")).toHaveCount(1);
  await library.getByRole("button", { name: "Riso", exact: true }).click();
  await expectThemeDetail(library, "Riso");
  await library.getByRole("button", { name: "Close theme library" }).click();
  await expect(browse).toBeFocused();
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Forest");
  await expectNoDocumentOverflow(page);
  errors.assertNone();
});

test("customize writes an editable delta and expands it to a standalone theme", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const panel = await openWorkshop(page);
  const controls = panel.getByTestId("theme-customize");
  await controls.getByLabel("Accent", { exact: true }).fill("#0052cc");
  await controls.getByLabel("Font combination").selectOption("serif");
  await controls.getByLabel("Density").selectOption("compact");
  await controls.getByRole("combobox", { name: "Texture", exact: true }).selectOption("grid");
  await expect(nativeCanvas(page).locator("pattern")).toBeAttached();

  await panel.getByRole("tab", { name: "JSON", exact: true }).click();
  await expect(panel.getByTestId("theme-json-editor").locator(".monaco-editor")).toBeVisible();
  const delta = await downloadDocument(page, panel);
  expect(delta).toMatchObject({
    schema_version: 2, extends: "studio", typography: { body_family: "serif" }, form: { density: "compact" },
    appearances: { light: { canvas: { textures: [{ kind: "grid" }] } } },
  });
  expect(delta.appearances.light.seed.accent.toLowerCase()).toBe("#0052cc");
  await panel.getByRole("button", { name: "Expand to standalone JSON" }).click();
  const expanded = await downloadDocument(page, panel);
  expect(expanded.extends).toBeUndefined();
  expect(expanded.appearances.light.seed).toHaveProperty("ink");
  expect(expanded.appearances.dark.seed).toHaveProperty("canvas");
  await expect(nativeCanvas(page).locator("pattern")).toBeAttached();
  errors.assertNone();
});

test("invalid JSON preserves the live render and contrast fixes change the editable document", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const panel = await openWorkshop(page);
  await panel.getByRole("tab", { name: "JSON", exact: true }).click();
  await expect(panel.getByTestId("theme-json-editor").locator(".monaco-editor")).toBeVisible();

  const valid = '{\n  "schema_version": 2,\n  "extends": "studio",\n  "roles": { "text.primary": "#EEEEEE" }\n}';
  await replaceThemeJson(panel, valid);
  await expect(panel.locator('[data-theme-contrast="warning"]')).toContainText("WCAG contrast warnings");
  await expect(nativeCanvas(page)).toBeAttached();
  const rendered = await nativeCanvas(page).evaluate((node) => node.outerHTML);

  await replaceThemeJson(panel, valid.replace('"#EEEEEE"', 'false'));
  await expect(panel.getByTestId("theme-json-error")).toContainText(/4:\d+/);
  await expect(panel.getByTestId("theme-json-editor").locator(".squiggly-error").first()).toBeAttached();
  await expect(panel.getByRole("button", { name: "Download JSON" })).toBeDisabled();
  expect(await nativeCanvas(page).evaluate((node) => node.outerHTML)).toBe(rendered);

  await panel.getByRole("button", { name: "Discard invalid edits" }).click();
  await panel.getByRole("button", { name: "Auto-fix contrast" }).click();
  await expect(panel.locator('[data-theme-contrast="AA"], [data-theme-contrast="AAA"]')).toBeVisible();
  const fixed = await downloadDocument(page, panel);
  expect(fixed.appearances.light.roles["text.primary"].toUpperCase()).not.toBe("#EEEEEE");
  expect(fixed.appearances.dark.roles["text.primary"]).toBe("#EEEEEE");
  errors.assertNone();
});

test("import applies a standalone v2 document and downloads its original contents", async ({ page }) => {
  const panel = await openWorkshop(page);
  const document = {
    schema_version: 2, id: "my-diagram", name: "My diagram",
    appearances: { dark: { seed: { canvas: "#101820", ink: "#FFFFFF", accent: "#66CCFF" } } },
  };
  await panel.locator('input[type="file"]').setInputFiles({
    name: "my-diagram.theme.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(document)),
  });
  await expect(panel.getByTestId("theme-current-name")).toHaveText("My diagram");
  await expect(page.locator("#preview-mode-panel")).toHaveAttribute("data-preview-canvas-tone", "dark");
  await expect(nativeCanvas(page).locator("rect").first()).toHaveCSS("fill", "rgb(16, 24, 32)");
  expect(await downloadDocument(page, panel)).toEqual(document);
});

const themeLibrary = (page: Page) => page.getByRole("dialog", { name: "Theme library", exact: true });
const browserSvg = (library: Locator) => library.getByTestId("theme-browser-preview").locator("svg");

async function expectThemeDetail(library: Locator, name: string) {
  await expect(library.getByTestId("theme-browser-name")).toHaveText(name);
  await expect(browserSvg(library).locator(".merman-theme-canvas")).toBeVisible();
}

test("the theme library previews a shared workspace without applying changes until requested", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const source = "flowchart LR\nA[SharedDraft] --> B[Kept]";
  const hash = encodeShareHash({ ...DEFAULT_WORKSPACE_SNAPSHOT, code: source, diagramTheme: "forest", diagramFont: "georgia" });
  await page.goto(`./?theme-design=tokyo${hash}`, { waitUntil: "domcontentloaded" });

  const library = themeLibrary(page);
  const preview = browserSvg(library);
  const canvasFill = preview.locator(".merman-theme-canvas > rect").first();
  const launcher = page.getByRole("banner").getByRole("button", { name: "Theme library", exact: true });
  const status = page.getByTestId("status-appearance");
  const expectWorkspace = async (theme: string) => {
    await expect(page.getByTestId("editor-startup-surface").locator(".view-line")).toHaveText(source.split("\n"));
    await expect(status).toHaveText(theme);
    await expect(page.locator("footer").getByText("Font: Georgia", { exact: true })).toHaveCount(1);
  };

  await expectThemeDetail(library, "Tokyo");
  await expect(library.locator("[data-browser-preset]")).toHaveCount(13);
  await expectWorkspace("Theme: Forest");

  await library.locator('[data-browser-preset="studio"]').click();
  await expectThemeDetail(library, "Studio");
  await expectWorkspace("Theme: Forest");
  await library.locator('[data-browser-appearance="dark"]').click();
  await expect(canvasFill).toHaveCSS("fill", "rgb(15, 20, 28)");
  await expectWorkspace("Theme: Forest");
  await library.locator('[data-browser-appearance="light"]').click();
  await expect(canvasFill).toHaveCSS("fill", "rgb(248, 250, 252)");
  await expectWorkspace("Theme: Forest");

  for (const [sample, diagram] of [["sequence", /^sequence$/], ["pie", /^pie$/], ["flowchart", /^flowchart/]] as const) {
    await library.locator(`[data-browser-sample="${sample}"]`).click();
    await expect(preview).toHaveAttribute("aria-roledescription", diagram);
    await expectWorkspace("Theme: Forest");
  }
  await library.locator('[data-browser-sample="current"]').click();
  await expect(preview).toContainText("SharedDraft");
  await expectWorkspace("Theme: Forest");
  await page.keyboard.press("Escape");
  await expect(library).toBeHidden();
  await expectWorkspace("Theme: Forest");
  expect(new URL(page.url()).hash).toBe(hash);

  await launcher.click();
  await library.locator('[data-browser-preset="studio"]').click();
  await library.locator('[data-browser-appearance="dark"]').click();
  await library.locator('[data-browser-sample="pie"]').click();
  await expect(preview).toHaveAttribute("aria-roledescription", "pie");
  await expectWorkspace("Theme: Forest");
  await library.getByRole("button", { name: "Apply theme", exact: true }).click();
  await expectWorkspace("Theme: studio · Dark");
  errors.assertNone();
});

for (const [id, name] of [["riso", "Riso"], ["pop-art", "Pop Art"], ["carbon-copy", "Carbon Copy"], ["paper-cut", "Paper Cut"]]) {
  test(`the v0.10.0 ${name} link opens its native SVG preview directly`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    await page.goto(`./?theme-design=${id}`, { waitUntil: "domcontentloaded" });
    const library = themeLibrary(page);
    await expectThemeDetail(library, name);
    await expect(library.getByRole("button", { name: "Apply theme", exact: true })).toBeInViewport();
    await expect(library.locator(`[data-browser-preset="${id}"]`)).toBeInViewport();
    await expect(page.getByTestId("status-appearance")).toHaveText("Theme: Source / default");
    errors.assertNone();
  });
}

test("a retired theme link explains its absence and still opens current presets", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await page.goto("./?theme-design=metro", { waitUntil: "domcontentloaded" });
  const library = themeLibrary(page);
  await expect(library.getByText(/metro.*unavailable|unavailable.*metro/i)).toBeVisible();
  await expect(library.locator('[data-browser-preset="metro"]')).toHaveCount(0);
  await library.locator('[data-browser-preset="riso"]').click();
  await expectThemeDetail(library, "Riso");
  await expect(page.getByTestId("status-appearance")).toHaveText("Theme: Source / default");
  errors.assertNone();
});
