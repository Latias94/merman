import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import type { editor } from "monaco-editor";
import type { AppState, useAppStore } from "../src/store";
import { optionalFeatureOutput } from "./helpers/build-manifest";
import { monitorBrowserErrors, openPlayground, replaceEditorSource, waitForPreviewSvg } from "./helpers/playground";

const manifest = JSON.parse(readFileSync(new URL("../dist/.vite/manifest.json", import.meta.url), "utf8")) as Record<string, { file: string; name?: string }>;
const chunk = (name: string) => {
  const entry = Object.values(manifest).find((entry) => entry.name === name);
  if (!entry) throw new Error(`Missing ${name} emitted module.`);
  return entry.file;
};
const paths = { monaco: chunk("monaco"), store: chunk("store") };
const readingRecipe = readFileSync(new URL("../src/themes/recipes/reading.theme.json", import.meta.url), "utf8");

declare global {
  interface Window {
    sourceSyncFixture: {
      editor: editor.ICodeEditor;
      store: typeof useAppStore;
    };
  }
}

async function bindFixture(page: Page) {
  await expect(page.getByRole("textbox", { name: "Mermaid source", exact: true })).toBeVisible();
  await page.evaluate(async (paths) => {
    const storeModule = await import(new URL(paths.store, location.href).href);
    const store = Object.values(storeModule).find((value) => typeof value === "function" &&
      "getState" in value && (value as typeof useAppStore).getState().setCode) as typeof useAppStore;
    const monacoModule = await import(new URL(paths.monaco, location.href).href);
    const monaco = Object.values(monacoModule).find((value) => value && typeof value === "object" &&
      "editor" in value && "Uri" in value) as typeof import("monaco-editor");
    const instance = monaco.editor.getEditors().find((instance) => instance.getModel()?.uri.toString() === "file:///merman/playground.mmd");
    if (!store || !instance) throw new Error("Missing live editor/store fixture.");
    window.sourceSyncFixture = { editor: instance, store };
  }, paths);
}

test("external document replacement, empty source, undo and local Unicode edits stay synchronized", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await waitForPreviewSvg(page);
  await bindFixture(page);
  const external = 'flowchart LR\nalpha[你好 😀 é] --> beta[External source]';
  await page.evaluate((code) => window.sourceSyncFixture.store.getState().setCode(code), external);
  await expect.poll(() => page.evaluate(() => window.sourceSyncFixture.editor.getValue())).toBe(external);
  await page.evaluate(() => window.sourceSyncFixture.store.getState().setCode(""));
  await expect.poll(() => page.evaluate(() => window.sourceSyncFixture.editor.getValue())).toBe("");
  const textbox = page.getByRole("textbox", { name: "Mermaid source", exact: true });
  await textbox.focus();
  await page.keyboard.press("Control+z");
  await expect.poll(() => page.evaluate(() => window.sourceSyncFixture.store.getState().code)).toBe(external);
  await page.keyboard.press("Control+Shift+z");
  await expect.poll(() => page.evaluate(() => window.sourceSyncFixture.store.getState().code)).toBe("");
  await replaceEditorSource(page, external + "\n%% café 中文 😀");
  const previous = await page.locator(".monaco-editor").elementHandle();
  await page.getByRole("tab", { name: "Config", exact: true }).click();
  await page.getByRole("tab", { name: "Code", exact: true }).click();
  expect(await page.locator(".monaco-editor").evaluate((node, original) => node === original, previous)).toBe(true);
  const values = await page.evaluate(() => ({ model: window.sourceSyncFixture.editor.getValue(), store: window.sourceSyncFixture.store.getState().code }));
  expect(values.model).toBe(values.store);
  expect(values.store).toContain("café 中文 😀");
  errors.assertNone();
});

test("a delayed editor mount adopts the latest external source instead of its creation snapshot", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  let release!: () => void;
  const blocked = new Promise<void>((resolve) => { release = resolve; });
  let requested = false;
  await page.route(`**/${optionalFeatureOutput("editor")}`, async (route) => {
    requested = true;
    await blocked;
    await route.continue();
  });
  const latest = "flowchart LR\nlatest[Latest source during loading] --> kept[Kept]";
  try {
    await openPlayground(page);
    await expect.poll(() => requested).toBe(true);
    await page.evaluate(async ({ storePath, latest }) => {
      const module = await import(new URL(storePath, location.href).href);
      const store = Object.values(module).find((value) => typeof value === "function" && "getState" in value &&
        (value as typeof useAppStore).getState().setCode) as typeof useAppStore;
      store.getState().setCode("flowchart LR\nold[Superseded while loading]");
      store.getState().setCode(latest);
    }, { storePath: paths.store, latest });
  } finally {
    release();
  }
  await bindFixture(page);
  await expect.poll(() => page.evaluate(() => window.sourceSyncFixture.editor.getValue())).toBe(latest);
  await expect(page.locator("footer")).toContainText(`${latest.length} Chars`);
  errors.assertNone();
});

test("workspace-only changes invalidate preview actions and restore them with the current render", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await waitForPreviewSvg(page);
  await bindFixture(page);
  const copy = page.getByRole("button", { name: "Copy SVG", exact: true });
  await expect(copy).toBeEnabled();
  const changes: readonly Partial<AppState>[] = [
    { code: "flowchart LR\nchanged[Different source]" },
    { mermaidConfig: '{"htmlLabels":false}' },
    { diagramTheme: "dark" },
    { themePresetId: "reading" },
    { themeRecipeJson: readingRecipe },
    { svgPipeline: "readable" },
    { textMeasurementMode: "headless" },
    { diagramFont: "monospace" },
    { themeAppearance: "follow" },
  ];
  for (const change of changes) {
    const initial = await page.evaluate((fields) => {
      const { store } = window.sourceSyncFixture;
      const previous = Object.fromEntries(Object.keys(fields).map((key) => [key, store.getState()[key as keyof ReturnType<typeof store.getState>]]));
      store.setState(fields);
      return previous;
    }, change);
    await expect(copy).toBeDisabled();
    await page.evaluate((previous) => window.sourceSyncFixture.store.setState(previous), initial);
    await expect(copy).toBeEnabled();
  }
  errors.assertNone();
});
