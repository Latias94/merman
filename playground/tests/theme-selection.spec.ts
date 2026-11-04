import { readFile } from "node:fs/promises";
import { expect, test, type Locator, type Page } from "@playwright/test";

import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT, type WorkspaceSnapshot } from "../src/lib/workspace-snapshot";
import { monitorBrowserErrors, waitForPreviewSvg } from "./helpers/playground";

const workspace = {
  ...DEFAULT_WORKSPACE_SNAPSHOT,
  code: "flowchart LR\nA[Current diagram] --> B[Keep this source]",
  mermaidConfig: '{"flowchart":{"htmlLabels":false,"curve":"linear"},"sequence":{"mirrorActors":false}}',
  diagramTheme: "neutral" as const,
  diagramFont: "trebuchet" as const,
  themePresetId: "brutalist",
};

async function start(page: Page) {
  await page.addInitScript(() => localStorage.setItem("merman-language", "en"));
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto(`./${encodeShareHash(workspace)}`, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  await page.locator("[data-theme-launcher]:visible").click();
  const panel = page.locator("#theme-panel");
  await expect(panel.locator('[data-theme-choice="preset:brutalist"]')).toHaveAttribute("aria-pressed", "true");
  return panel;
}

async function sharedWorkspace(page: Page): Promise<WorkspaceSnapshot> {
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  const url = await page.evaluate(() => navigator.clipboard.readText());
  const snapshot = decodeShareHash(new URL(url).hash);
  expect(snapshot).not.toBeNull();
  return snapshot!;
}

async function importRecipe(panel: Locator, raw: string, name: string) {
  await panel.getByLabel("Import JSON", { exact: true }).setInputFiles({
    name, mimeType: "application/json", buffer: Buffer.from(raw),
  });
}

test("catalog choices take effect immediately, close keeps them, and undo restores the previous choice", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const panel = await start(page);
  await panel.locator('[data-theme-choice="mermaid:forest"]').click();
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Forest");
  const native = await sharedWorkspace(page);
  expect(native.themePresetId).toBeNull();
  expect(native.themeRecipeJson).toBeNull();
  expect(native.diagramTheme).toBe("forest");
  expect(native.code).toBe(workspace.code);
  expect(native.mermaidConfig).toBe(workspace.mermaidConfig);
  expect(native.diagramFont).toBe(workspace.diagramFont);
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  await expect(panel).toBeHidden();
  expect((await sharedWorkspace(page)).diagramTheme).toBe("forest");
  await page.locator("[data-theme-launcher]:visible").click();
  await panel.getByRole("button", { name: "Undo appearance change", exact: true }).click();
  await expect(panel.locator('[data-theme-choice="preset:brutalist"]')).toHaveAttribute("aria-pressed", "true");
  expect(await sharedWorkspace(page)).toEqual(workspace);
  await expect(panel.getByRole("button", { name: "Undo appearance change", exact: true })).toBeDisabled();
  await expect(panel.getByRole("button", { name: "Apply", exact: true })).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Cancel", exact: true })).toHaveCount(0);
  errors.assertNone();
});

test("advanced base and extension combinations and the single font control update the current workspace", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const panel = await start(page);
  await expect(page.getByLabel("Diagram font", { exact: true })).toHaveCount(1);
  await panel.getByTestId("theme-advanced").locator("summary").first().click();
  await panel.getByLabel("Mermaid base theme", { exact: true }).selectOption("dark");
  await expect(panel.getByLabel("Merman extension", { exact: true })).toHaveValue("preset:brutalist");
  await expect(panel.locator('[data-theme-choice="preset:brutalist"]')).toHaveAttribute("aria-pressed", "true");
  const combined = await sharedWorkspace(page);
  expect(combined.diagramTheme).toBe("dark");
  expect(combined.themePresetId).toBe("brutalist");
  expect(combined.code).toBe(workspace.code);
  expect(combined.mermaidConfig).toBe(workspace.mermaidConfig);
  await panel.getByLabel("Diagram font", { exact: true }).selectOption("georgia");
  expect((await sharedWorkspace(page)).diagramFont).toBe("georgia");
  await panel.getByRole("button", { name: "Undo appearance change", exact: true }).click();
  await expect(panel.getByLabel("Diagram font", { exact: true })).toHaveValue("trebuchet");
  await expect(panel.getByLabel("Mermaid base theme", { exact: true })).toHaveValue("dark");
  await panel.getByLabel("Merman extension", { exact: true }).selectOption("none");
  const withoutExtension = await sharedWorkspace(page);
  expect(withoutExtension.themePresetId).toBeNull();
  expect(withoutExtension.diagramTheme).toBe("dark");
  await expect(panel.locator('[data-theme-choice="mermaid:dark"]')).toHaveAttribute("aria-pressed", "true");
  await panel.getByRole("button", { name: "Undo appearance change", exact: true }).click();
  await expect(panel.getByLabel("Merman extension", { exact: true })).toHaveValue("preset:brutalist");
  errors.assertNone();
});

test("an imported recipe keeps its own name and source in the same catalog even when it matches an exploration", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const panel = await start(page);
  const raw = await readFile(new URL("../src/themes/recipes/atlas.theme.json", import.meta.url), "utf8");
  await importRecipe(panel, raw, "Team review.theme.json");
  await expect(panel.getByTestId("theme-import-candidate")).toHaveAttribute("data-theme-import-state", "ready");
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Brutalist");
  expect(await sharedWorkspace(page)).toEqual(workspace);
  await panel.getByRole("button", { name: "Use imported theme", exact: true }).click();
  const imported = panel.locator('[data-theme-choice="recipe:current"]');
  await expect(imported).toHaveAttribute("aria-pressed", "true");
  await expect(imported).toHaveAttribute("data-theme-source", "imported");
  await expect(imported.locator("[data-theme-choice-name]")).toHaveText("Team review");
  await expect(panel.locator('[data-theme-choice="exploration:atlas"]')).toHaveAttribute("aria-pressed", "false");
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Team review");
  await expect(panel.getByTestId("theme-current-source")).toHaveText("Imported");
  const snapshot = await sharedWorkspace(page);
  expect(snapshot.themeRecipeJson).toBe(raw);
  expect(snapshot.themeRecipeLabel).toEqual({ name: "Team review", source: "imported" });
  await panel.locator('[data-theme-choice="preset:brutalist"]').click();
  await expect(imported).toHaveCount(0);
  await panel.getByRole("button", { name: "Undo appearance change", exact: true }).click();
  await expect(imported).toHaveAttribute("aria-pressed", "true");
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Team review");
  await panel.getByTestId("theme-advanced").locator("summary").first().click();
  await expect(panel.getByRole("textbox", { name: "Theme JSON", exact: true })).toHaveValue(raw);
  errors.assertNone();
});

test("a late file read cannot revive an import after a newer catalog selection", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const panel = await start(page);
  const raw = await readFile(new URL("../src/themes/recipes/nightfall.theme.json", import.meta.url), "utf8");
  await page.evaluate(() => {
    const original = File.prototype.text;
    File.prototype.text = function () {
      if (this.name !== "delayed.theme.json") return original.call(this);
      return new Promise<string>((resolve) => {
        window.addEventListener("release-theme-import", (event) => resolve((event as CustomEvent<string>).detail), { once: true });
      });
    };
  });
  await importRecipe(panel, raw, "delayed.theme.json");
  await expect(panel.getByTestId("theme-import-candidate")).toHaveAttribute("data-theme-import-state", "reading");
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Brutalist");
  await expect(panel.getByTestId("theme-render-evidence")).not.toHaveAttribute("data-theme-evidence-state", "waiting");
  await panel.locator('[data-theme-choice="mermaid:forest"]').click();
  await page.evaluate(async (recipeJson) => {
    window.dispatchEvent(new CustomEvent("release-theme-import", { detail: recipeJson }));
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  }, raw);
  await expect(panel.getByTestId("theme-import-candidate")).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "Use imported theme", exact: true })).toHaveCount(0);
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Forest");
  const selected = await sharedWorkspace(page);
  expect(selected.diagramTheme).toBe("forest");
  expect(selected.themeRecipeJson).toBeNull();
  expect(selected.themePresetId).toBeNull();
  expect(selected.themeRecipeLabel).toBeUndefined();
  errors.assertNone();
});
