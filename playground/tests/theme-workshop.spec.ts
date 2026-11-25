import { readFile } from "node:fs/promises";
import { expect, test, type Locator, type Page } from "@playwright/test";

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
