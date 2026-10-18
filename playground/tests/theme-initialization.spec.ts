import { expect, test, type Locator, type Page } from "@playwright/test";
import type { ThemeRecipeV1 } from "@mermanjs/web";

import { encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import type { MermaidThemeSelection } from "../src/lib/mermaid-theme-name";
import { monitorBrowserErrors } from "./helpers/playground";

const cases: {
  name: string;
  theme: MermaidThemeSelection;
  config?: Record<string, unknown>;
  frontmatter?: string;
  directive?: Record<string, unknown>;
  tone: "light" | "dark";
  fill?: string;
}[] = [
  { name: "scoped null preserves initialized dark palette", theme: "dark", directive: { flowchart: { theme: "null" } }, tone: "dark", fill: "rgb(31, 32, 32)" },
  { name: "global null preserves initialized dark palette", theme: "dark", directive: { theme: "null" }, tone: "dark", fill: "rgb(31, 32, 32)" },
  { name: "invalid source theme falls back to initialization", theme: "dark", directive: { theme: "unknown" }, tone: "dark", fill: "rgb(31, 32, 32)" },
  { name: "automatic appearance retains the diagram default", theme: "auto", tone: "light" },
  { name: "explicit classic default remains distinct", theme: "default", tone: "light", fill: "rgb(236, 236, 255)" },
  { name: "later scoped source theme overrides frontmatter and initialization", theme: "dark", frontmatter: "config: {theme: forest}", directive: { flowchart: { theme: "default" } }, tone: "light", fill: "rgb(236, 236, 255)" },
  { name: "authored secure cannot protect theme as a host setting", theme: "dark", config: { secure: ["theme"] }, frontmatter: "config: {theme: forest}", directive: { theme: "neutral", flowchart: { theme: "default" } }, tone: "light", fill: "rgb(236, 236, 255)" },
  { name: "authored secure cannot suppress a source namespace", theme: "default", config: { secure: ["flowchart"], flowchart: { theme: "dark" } }, tone: "dark", fill: "rgb(31, 32, 32)" },
];

for (const scenario of cases) {
  test(`Compare configuration: ${scenario.name}`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    await page.addInitScript(() => {
      if (window === window.top) localStorage.setItem("merman-language", "en");
    });
    const source = [
      ...(scenario.frontmatter ? ["---", scenario.frontmatter, "---"] : []),
      "flowchart TD",
      "A[Configuration contract] --> B[Result]",
      ...(scenario.directive ? [`%%{init: ${JSON.stringify(scenario.directive)}}%%`] : []),
    ].join("\n");
    const hash = encodeShareHash({
      ...DEFAULT_WORKSPACE_SNAPSHOT,
      code: source,
      diagramTheme: scenario.theme,
      diagramFont: "arial",
      mermaidConfig: JSON.stringify({ layout: "dagre", ...scenario.config }),
    });
    const wasmResponse = page.waitForResponse((response) =>
      /\/assets\/merman_wasm_bg-[\w-]+\.wasm(?:\?|$)/.test(response.url()),
    );
    await page.goto(`./${hash}`, { waitUntil: "domcontentloaded" });
    expect((await wasmResponse).ok()).toBe(true);
    await page.getByRole("tab", { name: "Compare", exact: true }).click();
    const expectedFill = scenario.fill ?? "rgb(255, 255, 255)";
    await expect.poll(() => nodeFills(page)).toEqual([expectedFill, expectedFill]);
    const fills = await nodeFills(page);
    expect(fills).toEqual([expectedFill, expectedFill]);
    await expect(page.getByRole("tabpanel", { name: "Compare" }).locator("[data-preview-canvas-tone]").first()).toHaveAttribute(
      "data-preview-canvas-tone", scenario.tone,
    );

    errors.assertNone();
  });
}

async function nodeFills(page: Page): Promise<(string | null)[]> {
  return page.locator(".preview-container > div").evaluateAll((hosts) =>
    hosts.map((host) => {
      const shape = host.shadowRoot?.querySelector("g.node rect");
      return shape ? getComputedStyle(shape).fill : null;
    }),
  );
}

test("Compare backing stays shared and preserves authored white canvas and exported SVG bytes", async ({ page }) => {
  test.setTimeout(90_000);
  const errors = monitorBrowserErrors(page);
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.addInitScript(() => {
    if (window === window.top) localStorage.setItem("merman-language", "en");
  });
  const recipe = {
    schema_version: 1,
    kind: "complete_spec",
    complete_spec: { canvas: { base: "white" } },
  } satisfies ThemeRecipeV1;
  await page.goto(`./${encodeShareHash({
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    code: "flowchart LR\nA[Authored white canvas] --> B[Shared backing]",
    diagramTheme: "dark",
    themeRecipeJson: JSON.stringify(recipe),
  })}`, { waitUntil: "domcontentloaded" });
  await page.getByRole("tab", { name: "Compare", exact: true }).click();
  const merman = page.locator('[data-merman-compare-engine="merman"]');
  const mermaid = page.locator('[data-merman-compare-engine="mermaid"]');
  await expect(merman).toHaveAttribute("data-merman-artifact-state", "ready");
  await expect(mermaid).toHaveAttribute("data-merman-artifact-state", "ready");
  const explanation = page.getByText("Merman applies its selected preset or custom recipe.", { exact: false });
  await expect(explanation).toBeVisible();
  const mermanSvg = merman.locator(".preview-container svg").first();
  const mermaidSvg = mermaid.locator(".preview-container svg").first();
  const authoredCanvas = mermanSvg.locator('[data-merman-theme-canvas="base"]');
  await expect(authoredCanvas).toHaveCSS("fill", "rgb(255, 255, 255)");
  await expect(mermaidSvg.locator("[data-merman-theme-canvas]")).toHaveCount(0);
  const originalMermanRoot = await mermanSvg.elementHandle();
  const originalMermaidRoot = await mermaidSvg.elementHandle();
  const originalMermanSvg = await mermanSvg.evaluate((root) => root.outerHTML);
  const originalMermaidSvg = await mermaidSvg.evaluate((root) => root.outerHTML);
  const copySvg = async (pane: Locator) => {
    await pane.getByRole("button", { name: /^(Copy SVG|Copied!)$/ }).click();
    return page.evaluate(() => navigator.clipboard.readText());
  };
  const mermanExport = await copySvg(merman);
  const mermaidExport = await copySvg(mermaid);
  expect(mermanExport).toContain('data-merman-theme-canvas="base"');
  expect(mermanExport).toContain(`fill="${await authoredCanvas.getAttribute("fill")}"`);
  expect(mermaidExport).not.toContain("data-merman-theme-canvas");
  const backing = page.getByLabel("Preview backing", { exact: true });
  const canvases = page.locator('[data-merman-compare-engine] [data-merman-svg-viewport="true"]');
  await expect(canvases).toHaveCount(2);
  const backgrounds = new Map<string, string>();

  for (const [value, tone] of [
    ["automatic", "dark"], ["light", "light"], ["dark", "dark"],
    ["transparency", "light"], ["automatic", "dark"],
  ] as const) {
    await backing.selectOption(value);
    await expect(page.locator("#preview-mode-panel")).toHaveAttribute("data-preview-backing", value);
    for (const canvas of await canvases.all()) await expect(canvas).toHaveAttribute("data-preview-canvas-tone", tone);
    const paints = await canvases.evaluateAll((elements) => elements.map((element) => ({
      color: getComputedStyle(element).backgroundColor,
      image: getComputedStyle(element).backgroundImage,
    })));
    expect(paints[0]).toEqual(paints[1]);
    backgrounds.set(value, paints[0].color);
    if (value === "transparency") expect(paints[0].image).toContain("conic-gradient");
    await expect(authoredCanvas).toHaveCSS("fill", "rgb(255, 255, 255)");
    expect(await mermanSvg.evaluate((root, original) => root === original, originalMermanRoot)).toBe(true);
    expect(await mermaidSvg.evaluate((root, original) => root === original, originalMermaidRoot)).toBe(true);
    expect(await mermanSvg.evaluate((root) => root.outerHTML)).toBe(originalMermanSvg);
    expect(await mermaidSvg.evaluate((root) => root.outerHTML)).toBe(originalMermaidSvg);
    expect(await copySvg(merman)).toBe(mermanExport);
    expect(await copySvg(mermaid)).toBe(mermaidExport);
  }
  expect(backgrounds.get("light")).not.toBe(backgrounds.get("dark"));
  expect(backgrounds.get("automatic")).toBe(backgrounds.get("dark"));
  await originalMermanRoot?.dispose();
  await originalMermaidRoot?.dispose();

  await page.getByRole("button", { name: "Theme", exact: true }).click();
  const workbench = page.locator("#theme-workbench");
  await workbench.getByRole("tab", { name: "Presets", exact: true }).click();
  const none = workbench.getByRole("button", { name: /^None/ });
  await expect(none).toHaveAttribute("aria-pressed", "false");
  await none.click();
  await expect(none).toHaveAttribute("aria-pressed", "true");
  await workbench.getByRole("button", { name: /^(Validate and apply|Apply)$/ }).click();
  await expect(merman).toHaveAttribute("data-merman-artifact-state", "ready");
  await expect(mermanSvg.locator("[data-merman-theme-canvas]")).toHaveCount(0);
  await expect(explanation).toBeHidden();
  await workbench.getByRole("button", { name: "Cancel", exact: true }).click();
  expect(await copySvg(merman)).not.toContain("data-merman-theme-canvas");
  errors.assertNone();
});


test("Merman keeps host security against configuration, frontmatter and directives", async ({ page }) => {
  const attack = {
    secure: ["theme"], securityLevel: "loose", startOnLoad: true,
    themeCSS: ".security-injection { fill: red; }", maxTextSize: 1, maxEdges: 1,
    fontFamily: "Arial; color:red", themeVariables: { primaryColor: "red; color:blue" },
  };
  const source = [
    "---", `config: ${JSON.stringify(attack)}`, "---", "flowchart TD",
    "A[Security contract] --> B --> C", 'click A "javascript:alert(1)"',
    `%%{init: ${JSON.stringify(attack)}}%%`,
  ].join("\n");
  const hash = encodeShareHash({
    ...DEFAULT_WORKSPACE_SNAPSHOT, code: source, diagramTheme: "dark",
    mermaidConfig: JSON.stringify({ ...attack, layout: "dagre" }),
  });
  await page.goto(`./${hash}`, { waitUntil: "domcontentloaded" });
  const preview = page.locator(".preview-container > div").first();
  await expect.poll(() => preview.evaluate((host) =>
    host.shadowRoot?.querySelector("svg")?.textContent?.includes("Security contract") ?? false,
  )).toBe(true);
  const svg = await preview.evaluate((host) => host.shadowRoot?.querySelector("svg")?.outerHTML ?? "");
  expect(svg).not.toContain("javascript:");
  expect(svg).not.toContain("security-injection");
  expect(svg).not.toContain("color:blue");
});
