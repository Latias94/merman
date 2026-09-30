import { expect, test, type Page } from "@playwright/test";

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
