import { readFile } from "node:fs/promises";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT, type WorkspaceSnapshot } from "../src/lib/workspace-snapshot";
import { monitorBrowserErrors, openWorkbenchTool, replaceEditorSource, replaceMonacoText, waitForPreviewSvg } from "./helpers/playground";

const reading = await readFile(new URL("../src/themes/recipes/reading.theme.json", import.meta.url), "utf8");
const workspace: WorkspaceSnapshot = {
  ...DEFAULT_WORKSPACE_SNAPSHOT, code: "flowchart LR\nA[Shared design] --> B[Readable appearance]",
  mermaidConfig: '{"flowchart":{"htmlLabels":false}}',
  themeRecipeJson: reading, themeRecipeLabel: { name: "Reading", source: "exploration" }, themeAppearance: "follow",
};

async function start(page: Page, value = workspace, appTheme = "light") {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.addInitScript((theme) => {
    if (window === window.top) {
      localStorage.setItem("merman-language", "en");
      localStorage.setItem("merman-ui-theme", theme);
    }
  }, appTheme);
  await page.goto(`./${encodeShareHash(value)}`, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  await page.locator("[data-theme-launcher]:visible").click();
  return page.locator("#theme-panel");
}

async function setAppAppearance(page: Page, name: "Light" | "Dark") {
  await page.getByRole("button", { name: "App settings", exact: true }).click();
  await page.getByRole("menuitemradio", { name, exact: true }).click();
}

async function sharedWorkspace(page: Page) {
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await page.getByRole("menuitem", { name: "Copy workspace link", exact: true }).click();
  const link = await page.evaluate(() => navigator.clipboard.readText());
  return decodeShareHash(new URL(link).hash)!;
}

async function canvas(page: Page) {
  return page.locator(".preview-container > div").first().evaluate((host) =>
    host.shadowRoot?.querySelector(".merman-theme-canvas-base")?.getAttribute("fill") ?? "",
  );
}

async function replaceDraft(page: Page, text: string) {
  await replaceMonacoText(page, "Editable theme JSON", text);
}

async function validateDraft(panel: Locator) {
  await panel.getByRole("button", { name: "Validate theme", exact: true }).click();
  await expect(panel.getByRole("button", { name: "Validate theme", exact: true })).toBeEnabled();
}

test("Follow app updates actual SVG paint; an explicit appearance stays fixed and shares its full recipe", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const panel = await start(page);
  const control = panel.getByTestId("theme-appearance-control");
  await expect(control.getByRole("combobox")).toHaveValue("follow");
  await expect.poll(() => canvas(page)).toBe("#F6F7F9");
  await setAppAppearance(page, "Dark");
  await expect(control).toHaveAttribute("data-effective-appearance", "dark");
  await expect.poll(() => canvas(page)).toBe("#171C24");
  await control.getByRole("combobox").selectOption("light");
  await expect.poll(() => canvas(page)).toBe("#F6F7F9");
  await setAppAppearance(page, "Light");
  await setAppAppearance(page, "Dark");
  await expect(control).toHaveAttribute("data-effective-appearance", "light");
  await expect.poll(() => canvas(page)).toBe("#F6F7F9");
  const saved = await sharedWorkspace(page);
  expect(saved.themeAppearance).toBe("light");
  expect(saved.themeRecipeJson).toBe(reading);
  expect(saved.code).toBe(workspace.code);
  expect(saved.mermaidConfig).toBe(workspace.mermaidConfig);
  expect(saved.diagramTheme).toBe("auto");
  errors.assertNone();
});

test("grouped native selection resolves the same literal variant in both Compare engines", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const panel = await start(page, { ...DEFAULT_WORKSPACE_SNAPSHOT, code: "flowchart LR\nA[Native pair] --> B", mermaidConfig: '{"htmlLabels":false}' }, "dark");
  await panel.locator('[data-theme-choice="mermaid:neo"]').click();
  await expect(panel.locator('[data-theme-choice="mermaid:neo-dark"]')).toHaveCount(0);
  await expect(panel.getByTestId("theme-appearance-control")).toHaveAttribute("data-effective-appearance", "dark");
  await openWorkbenchTool(page, "Compare");
  const panes = page.locator("[data-merman-compare-engine]");
  await expect(panes).toHaveCount(2);
  for (const pane of await panes.all()) await expect(pane).toHaveAttribute("data-merman-artifact-state", "ready");
  const fills = await panes.evaluateAll((elements) => elements.map((pane) => {
    const host = pane.querySelector(".preview-container > div");
    const node = host?.shadowRoot?.querySelector("g.node rect");
    if (!node) return null;
    const fill = getComputedStyle(node).fill;
    if (!fill.startsWith("url(")) return { solid: fill };
    const id = fill.match(/#([^"')]+)/)?.[1];
    const gradient = id ? host?.shadowRoot?.querySelector(`#${CSS.escape(id)}`) : null;
    return gradient ? { stops: [...gradient.querySelectorAll("stop")].map((stop) => ({
      offset: stop.getAttribute("offset"), color: getComputedStyle(stop).stopColor,
    })) } : null;
  }));
  expect(fills[0]).not.toBeNull();
  expect(fills[0]).toEqual(fills[1]);
  const saved = await sharedWorkspace(page);
  expect(saved.diagramTheme).toBe("neo");
  expect(saved.themeAppearance).toBe("follow");
  errors.assertNone();
});

test("unsupported explicit native appearance is rejected without claiming a Dark render", async ({ page }) => {
  await page.addInitScript(() => {
    if (window === window.top) localStorage.setItem("merman-language", "en");
  });
  await page.goto(`./${encodeShareHash({
    ...DEFAULT_WORKSPACE_SNAPSHOT, code: "flowchart LR\nA[Forest] --> B",
    diagramTheme: "forest", themeAppearance: "dark",
  })}`, { waitUntil: "domcontentloaded" });
  await page.locator("[data-theme-launcher]:visible").click();
  const control = page.locator("#theme-panel").getByTestId("theme-appearance-control");
  await expect(control).toHaveAttribute("data-effective-appearance", "invalid");
  await expect(control.getByTestId("theme-appearance-error")).toContainText('does not support the "dark" appearance');
  await expect(control.getByTestId("theme-effective-appearance")).toHaveCount(0);
  await openWorkbenchTool(page, "Compare");
  const panes = page.locator("[data-merman-compare-engine]");
  await expect(panes).toHaveCount(2);
  for (const pane of await panes.all()) await expect(pane).toHaveAttribute("data-merman-artifact-state", "rejected");
});

test("concrete preset links remain pinned while their single gallery card represents the design", async ({ page }) => {
  const panel = await start(page, { ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId: "editor-dark" });
  await expect(panel.locator('[data-theme-choice="preset:editor"]')).toHaveAttribute("aria-pressed", "true");
  await expect(panel.locator('[data-theme-choice="preset:editor-dark"]')).toHaveCount(0);
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Editor");
  await expect(panel.getByTestId("theme-appearance-control")).toHaveAttribute("data-effective-appearance", "dark");
  await setAppAppearance(page, "Dark");
  await setAppAppearance(page, "Light");
  expect((await sharedWorkspace(page)).themePresetId).toBe("editor-dark");
  await panel.locator('[data-theme-choice="preset:editor"]').click();
  await expect(panel.getByTestId("theme-appearance-control").getByRole("combobox")).toHaveValue("follow");
});

test("customization validates inactive branches, applies atomically and preserves composition and subsequent source edits", async ({ page }) => {
  const panel = await start(page, { ...workspace, diagramTheme: "forest", themeAppearance: "dark" });
  await expect(panel.getByTestId("theme-active-combination")).toBeVisible();
  await panel.getByRole("button", { name: "Customize", exact: true }).click();
  const invalid = JSON.parse(reading);
  invalid.design.appearances.light.tokens.ink.value = "not-a-color";
  await replaceDraft(page, JSON.stringify(invalid, null, 2));
  await validateDraft(panel);
  await expect(panel.getByTestId("theme-draft-workbench").getByRole("alert")).toBeVisible();
  await expect(panel.getByRole("button", { name: "Apply changes", exact: true })).toBeDisabled();
  expect((await sharedWorkspace(page)).themeRecipeJson).toBe(reading);
  const valid = JSON.parse(reading);
  valid.design.appearances.light.tokens.accent.value = "#B00020";
  const raw = `${JSON.stringify(valid, null, 2)}\n`;
  await replaceDraft(page, raw);
  await validateDraft(panel);
  await expect(panel.getByRole("button", { name: "Apply changes", exact: true })).toBeEnabled();
  await panel.getByRole("button", { name: "Apply changes", exact: true }).click();
  const applied = await sharedWorkspace(page);
  expect(applied.themeRecipeJson).toBe(raw);
  expect(applied.themeAppearance).toBe("dark");
  expect(applied.diagramTheme).toBe("forest");
  const later = "flowchart LR\nA[Later source edit] --> B[Retained]";
  await replaceEditorSource(page, later);
  await panel.getByRole("button", { name: "Undo appearance change", exact: true }).click();
  const restored = await sharedWorkspace(page);
  expect(restored.themeRecipeJson).toBe(reading);
  expect(restored.code).toBe(later);
  expect(restored.diagramTheme).toBe("forest");
  expect(restored.themeAppearance).toBe("dark");
});

test("Reading previews the same five sources in both appearances and downloads the complete editable recipe", async ({ page }) => {
  const panel = await start(page);
  await panel.getByRole("button", { name: "Theme designs", exact: true }).click();
  const gallery = page.getByTestId("theme-showcase");
  await expect(gallery.getByTestId("theme-showcase-detail")).toHaveAttribute("data-theme-design", "reading");
  await expect(gallery.locator("[data-theme-sample]")).toHaveCount(5);
  for (const mode of ["light", "dark"]) {
    await gallery.getByTestId("theme-appearance-control").getByRole("combobox").selectOption(mode);
    await expect(gallery.locator(`[data-theme-sample] img[data-theme-preview-appearance="${mode}"]`)).toHaveCount(5);
    for (const link of await gallery.locator("[data-theme-sample] a").all()) {
      const saved = decodeShareHash(new URL((await link.getAttribute("href"))!, page.url()).hash)!;
      expect(saved.themeRecipeJson).toBe(reading);
      expect(saved.themeAppearance).toBe(mode);
      expect(saved.diagramTheme).toBe("auto");
    }
  }
  const download = page.waitForEvent("download");
  await gallery.getByRole("button", { name: "Download theme JSON", exact: true }).click();
  expect(await readFile((await (await download).path())!, "utf8")).toBe(reading);
  const adoption = gallery.getByTestId("theme-showcase-use");
  await adoption.getByRole("tab", { name: "CLI", exact: true }).click();
  await expect(adoption.getByTestId("theme-showcase-code")).toContainText("--theme-appearance dark");
});

test("an open image export keeps the captured concrete appearance across a system appearance change", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  const panel = await start(page, workspace, "system");
  await expect.poll(() => canvas(page)).toBe("#F6F7F9");
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitem", { name: "Export image…", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Export image", exact: true });
  await expect(dialog.getByTestId("export-theme-appearance")).toHaveText("Captured appearance · Light");
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(dialog.getByTestId("export-snapshot-changed")).toBeVisible();
  await expect(dialog.getByTestId("export-theme-appearance")).toHaveText("Captured appearance · Light");
  const download = page.waitForEvent("download");
  await dialog.getByRole("button", { name: "Download", exact: true }).click();
  const svg = await readFile((await (await download).path())!, "utf8");
  expect(svg).toMatch(/class="merman-theme-canvas-base"[^>]*fill="#F6F7F9"|fill="#F6F7F9"[^>]*class="merman-theme-canvas-base"/);
});
