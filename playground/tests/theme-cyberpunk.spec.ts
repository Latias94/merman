import { expect, test } from "@playwright/test";
import { encodeShareHash } from "../src/lib/share";
import { monitorBrowserErrors, waitForPreviewSvg } from "./helpers/playground";

test("public Cyberpunk paints its layered canvas and separate node and edge glows", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const hash = encodeShareHash({
    code: "flowchart LR\nA[Alpha] -->|Advance| B[Beta]",
    mermaidConfig: '{"htmlLabels":false}',
    diagramTheme: "default",
    themePresetId: "cyberpunk",
    svgPipeline: "resvg-safe",
    textMeasurementMode: "browser",
    diagramFont: "trebuchet",
  });
  await page.goto(`./${hash}`, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  const svg = page.locator(".preview-container > div").first().locator("svg");
  await expect(svg).toHaveCount(1);
  await expect(svg.locator("foreignObject")).toHaveCount(0);
  await page.evaluate(() => document.fonts.ready);

  const canvas = await svg.evaluate((root) => {
    const base = root.querySelector<SVGRectElement>("[data-merman-theme-canvas='base']")!;
    const layers = [...root.querySelectorAll("[data-merman-theme-canvas-layer]")];
    const gradients = [...root.querySelectorAll("radialGradient, linearGradient")];
    return {
      base: getComputedStyle(base).fill,
      bounds: [base.x.baseVal.value, base.y.baseVal.value, base.width.baseVal.value, base.height.baseVal.value],
      layers: layers.map((layer) => ({
        index: layer.getAttribute("data-merman-theme-canvas-layer"),
        blend: getComputedStyle(layer).mixBlendMode,
        fill: layer.querySelector("rect")!.getAttribute("fill"),
      })),
      gradients: gradients.map((gradient) => ({
        id: gradient.id,
        kind: gradient.localName,
        units: gradient.getAttribute("gradientUnits"),
        geometry: (gradient.localName === "radialGradient" ? ["cx", "cy", "r"] : ["x1", "y1", "x2", "y2"])
          .map((name) => Number(gradient.getAttribute(name))),
        stops: [...gradient.querySelectorAll("stop")].map((stop) => ({
          offset: stop.offset.baseVal,
          color: getComputedStyle(stop).stopColor,
          opacity: Number(getComputedStyle(stop).stopOpacity),
        })),
      })),
      patterns: [...root.querySelectorAll("pattern")].map((pattern) => ({
        id: pattern.id,
        units: pattern.getAttribute("patternUnits"),
        width: pattern.width.baseVal.value,
        height: pattern.height.baseVal.value,
        fill: pattern.querySelector("rect")!.getAttribute("fill"),
      })),
    };
  });
  expect(canvas.base).toBe("rgb(5, 20, 35)");
  expect(canvas.layers.map(({ index, blend }) => [index, blend])).toEqual([
    ["0", "screen"], ["1", "screen"], ["2", "screen"],
  ]);
  expect(canvas.gradients).toHaveLength(3);
  const [radial, xGrid, yGrid] = canvas.gradients;
  expect(radial.kind).toBe("radialGradient");
  expect(canvas.gradients.map(({ units }) => units)).toEqual(Array(3).fill("userSpaceOnUse"));
  const [x, y, width, height] = canvas.bounds;
  expect(width).toBeGreaterThan(0);
  expect(height).toBeGreaterThan(0);
  expect(radial.geometry[0]).toBeCloseTo(x + width / 2, 2);
  expect(radial.geometry[1]).toBeCloseTo(y + height / 2, 2);
  expect(radial.geometry[2]).toBeCloseTo(Math.hypot(width, height) / 2, 2);
  expect(radial.stops.map(({ color }) => color)).toEqual(["rgba(0, 242, 255, 0.05)", "rgba(0, 242, 255, 0)"]);
  expect(radial.stops.map(({ offset }) => offset)).toEqual([0, expect.closeTo(0.7, 5)]);
  expect(radial.stops.map(({ opacity }) => opacity)).toEqual([1, 1]);
  expect(xGrid.geometry).toEqual([0, 20, 40, 20]);
  expect(yGrid.geometry).toEqual([20, 0, 20, 40]);
  expect(canvas.patterns).toHaveLength(2);
  for (const [index, gradient] of [xGrid, yGrid].entries()) {
    expect(gradient.kind).toBe("linearGradient");
    expect(gradient.stops.map(({ offset }) => offset)).toEqual([
      0, expect.closeTo(0.025, 5), expect.closeTo(0.025, 5), 1,
    ]);
    expect(gradient.stops.map(({ color }) => color)).toEqual([
      "rgba(0, 242, 255, 0.03)", "rgba(0, 242, 255, 0.03)", "rgba(0, 0, 0, 0)", "rgba(0, 0, 0, 0)",
    ]);
    expect(gradient.stops.map(({ opacity }) => opacity)).toEqual([1, 1, 1, 1]);
    const pattern = canvas.patterns[index];
    expect(pattern).toMatchObject({ units: "userSpaceOnUse", width: 40, height: 40, fill: `url(#${gradient.id})` });
    expect(canvas.layers[index + 1].fill).toBe(`url(#${pattern.id})`);
  }
  expect(canvas.layers[0].fill).toBe(`url(#${radial.id})`);

  const glows = await svg.evaluate((root) => [...root.querySelectorAll("[filter]")].map((terminal) => {
    const binding = terminal.getAttribute("filter")!;
    const id = binding.slice(5, -1);
    const filter = root.querySelector(`[id="${id}"]`)!;
    return {
      kind: terminal.matches("g.node > rect") ? "node"
        : terminal.matches("path.flowchart-link") ? "edge" : "unexpected",
      deviations: [...filter.querySelectorAll("feGaussianBlur")].map((blur) => Number(blur.getAttribute("stdDeviation"))),
    };
  }));
  expect(glows.map(({ kind }) => kind).sort()).toEqual(["edge", "node", "node"]);
  expect(glows.filter(({ kind }) => kind === "node").map(({ deviations }) => deviations)).toEqual([[8, 16], [8, 16]]);
  expect(glows.filter(({ kind }) => kind === "edge").map(({ deviations }) => deviations)).toEqual([[6]]);
  await svg.screenshot({ path: test.info().outputPath("public-cyberpunk.png") });

  // A definition or binding alone does not show that the browser paints the glow.
  await svg.locator("[filter]").evaluateAll((terminals) => terminals.forEach((terminal) => {
    terminal.setAttribute("data-test-filter", terminal.getAttribute("filter")!);
    terminal.removeAttribute("filter");
  }));
  const unfiltered = await svg.screenshot();
  for (const kind of ["node", "edge"]) {
    await svg.locator("[data-test-filter]").evaluateAll((terminals, selected) => terminals.forEach((terminal) => {
      const selector = selected === "node" ? "g.node > rect" : "path.flowchart-link";
      if (terminal.matches(selector)) terminal.setAttribute("filter", terminal.getAttribute("data-test-filter")!);
      else terminal.removeAttribute("filter");
    }), kind);
    expect(await svg.screenshot(), `${kind} glow must change painted pixels`).not.toEqual(unfiltered);
  }
  errors.assertNone();
});
