import { expect, test, type Page } from "@playwright/test";

import { encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT, type WorkspaceSnapshot } from "../src/lib/workspace-snapshot";
import { monitorBrowserErrors, replaceEditorSource } from "./helpers/playground";

test("Compare keeps the successful Mermaid dark canvas when Merman rejects a custom theme", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPartialCompare(page);
  const mermaid = page.locator('[data-merman-compare-engine="mermaid"]');
  await expect(mermaid.locator("g.node rect").first()).toHaveCSS("fill", "rgb(31, 32, 32)");
  await expect(page.getByLabel("Preview backing", { exact: true })).toHaveValue("automatic");
  await expect(page.locator("#preview-mode-panel")).toHaveAttribute("data-preview-canvas-tone", "dark");
  await expect(mermaid.locator('[data-merman-svg-viewport="true"]')).toHaveAttribute("data-preview-canvas-tone", "dark");
  const svg = mermaid.locator(".preview-container svg").first();
  const original = await svg.elementHandle();
  for (const [backing, tone] of [["light", "light"], ["dark", "dark"], ["transparency", "light"], ["automatic", "dark"]]) {
    await page.getByLabel("Preview backing", { exact: true }).selectOption(backing);
    await expect(page.locator("#preview-mode-panel")).toHaveAttribute("data-preview-canvas-tone", tone);
    expect(await svg.evaluate((element, previous) => element === previous, original)).toBe(true);
  }
  await original?.dispose();
  errors.assertNone();
});

for (const scenario of [
  { name: "scoped source dark overrides global default", diagramTheme: "default", directive: { theme: "default", flowchart: { theme: "dark" } }, tone: "dark", fill: "rgb(31, 32, 32)" },
  { name: "scoped source default overrides initialized dark", diagramTheme: "dark", directive: { flowchart: { theme: "default" } }, tone: "light", fill: "rgb(236, 236, 255)" },
  { name: "automatic selection uses the diagram default", diagramTheme: "auto", tone: "light", fill: "rgb(255, 255, 255)" },
  { name: "global null retains initialized dark palette", diagramTheme: "dark", directive: { theme: "null" }, tone: "dark", fill: "rgb(31, 32, 32)" },
  { name: "scoped null retains initialized dark palette", diagramTheme: "dark", directive: { flowchart: { theme: "null" } }, tone: "dark", fill: "rgb(31, 32, 32)" },
  { name: "invalid source theme retains initialized dark palette", diagramTheme: "dark", directive: { theme: "unknown" }, tone: "dark", fill: "rgb(31, 32, 32)" },
] as const) {
  test(`Compare partial palette: ${scenario.name}`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    const directive = "directive" in scenario ? `%%{init: ${JSON.stringify(scenario.directive)}}%%\n` : "";
    await openPartialCompare(page, {
      diagramTheme: scenario.diagramTheme,
      code: `${directive}flowchart LR\nA --> B`,
    });
    const mermaid = page.locator('[data-merman-compare-engine="mermaid"]');
    await expect(mermaid.locator("g.node rect").first()).toHaveCSS("fill", scenario.fill);
    await expect(page.locator("#preview-mode-panel")).toHaveAttribute("data-preview-canvas-tone", scenario.tone);
    errors.assertNone();
  });
}

test("Compare realm reuse keeps each step's palette and recovers after a failed step", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await openPartialCompare(page);
  const realm = page.locator('iframe[data-merman-realm="compare"]');
  const original = await realm.elementHandle();
  const mermaid = page.locator('[data-merman-compare-engine="mermaid"]');

  for (const [label, code, tone, fill] of [
    ["Light step", '%%{init: {"flowchart": {"theme": "default"}}}%%\nflowchart LR\nA[Light step] --> B', "light", "rgb(236, 236, 255)"],
    ["Dark step", '---\nconfig: {theme: default}\n---\n%%{init: {"flowchart": {"theme": "dark"}}}%%\nflowchart LR\nA[Dark step] --> B', "dark", "rgb(31, 32, 32)"],
    ["Null step", '%%{init: {"theme": "null"}}%%\nflowchart LR\nA[Null step] --> B', "dark", "rgb(31, 32, 32)"],
  ]) {
    await test.step(label, async () => {
      await replaceEditorSource(page, code);
      await expect(mermaid.locator(".preview-container svg").first()).toContainText(label);
      await expect(mermaid.locator("g.node rect").first()).toHaveCSS("fill", fill);
      await expect(page.locator("#preview-mode-panel")).toHaveAttribute("data-preview-canvas-tone", tone);
      expect(await realm.evaluate((element, previous) => element === previous, original)).toBe(true);
    });
  }

  await replaceEditorSource(page, "flowchart LR\nA -->");
  await expect(mermaid).toHaveAttribute("data-merman-artifact-state", "rejected");
  await expect(mermaid.locator(".preview-container svg")).toHaveCount(0);
  await expect(page.locator("#preview-mode-panel")).toHaveAttribute("data-preview-canvas-tone", "light");
  await replaceEditorSource(page, "flowchart LR\nA[Recovered step] --> B");
  await expect(mermaid.locator(".preview-container svg").first()).toContainText("Recovered step");
  await expect(page.locator("#preview-mode-panel")).toHaveAttribute("data-preview-canvas-tone", "dark");
  expect(await realm.evaluate((element, previous) => element === previous, original)).toBe(true);
  await original?.dispose();
  errors.assertNone();
});

test("rapid Compare edits keep the visible artifact's palette until the latest step replaces it", async ({ page }) => {
  await openPartialCompare(page, { code: "flowchart LR\nA[Visible step] --> B" });
  const mermaid = page.locator('[data-merman-compare-engine="mermaid"]');
  const svg = mermaid.locator(".preview-container svg").first();
  const original = await svg.elementHandle();
  await page.clock.install();
  await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1000));
  await replaceEditorSource(page, '%%{init: {"theme": "default"}}%%\nflowchart LR\nA[Superseded step] --> B');
  await replaceEditorSource(page, '%%{init: {"flowchart": {"theme": "default"}}}%%\nflowchart LR\nA[Latest step] --> B');
  await expect(svg).toContainText("Visible step");
  await expect(page.locator("#preview-mode-panel")).toHaveAttribute("data-preview-canvas-tone", "dark");
  expect(await svg.evaluate((element, previous) => element === previous, original)).toBe(true);
  await page.clock.resume();
  await expect(svg).toContainText("Latest step");
  await expect(page.locator("#preview-mode-panel")).toHaveAttribute("data-preview-canvas-tone", "light");
  await original?.dispose();
});

async function openPartialCompare(page: Page, workspace: Partial<WorkspaceSnapshot> = {}) {
  await page.addInitScript(() => {
    if (window === window.top) localStorage.setItem("merman-language", "en");
  });
  await page.goto(`./${encodeShareHash({
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    code: "flowchart LR\nA --> B",
    mermaidConfig: "{}",
    diagramTheme: "dark",
    themePresetId: null,
    themeRecipeJson: '{"schema_version":1,"kind":"complete_spec","complete_spec":{"not_a_theme_field":true}}',
    ...workspace,
  })}`, { waitUntil: "domcontentloaded" });
  await expect(page.locator("footer")).toContainText("Ready");
  await expect(page.getByRole("textbox", { name: "Mermaid source", exact: true })).toBeVisible();
  await page.getByRole("tab", { name: "Compare", exact: true }).click();
  const merman = page.locator('[data-merman-compare-engine="merman"]');
  await expect(merman).toHaveAttribute("data-merman-artifact-state", "rejected");
  await expect(merman).toContainText("not_a_theme_field");
  await expect(page.locator('[data-merman-compare-engine="mermaid"]')).toHaveAttribute("data-merman-artifact-state", "ready");
}
