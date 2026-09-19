import { readFileSync } from "node:fs";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { encodeShareHash } from "../src/lib/share";
import { monitorBrowserErrors, waitForPreviewSvg } from "./helpers/playground";

const sceneSource = (family: string): string => readFileSync(
  new URL(`../../crates/merman-theme-fixtures/fixtures/public-cyberpunk/${family}.mmd`, import.meta.url),
  "utf8",
);

test.setTimeout(120_000);

async function captureScene(page: Page, svg: Locator, clip?: { x: number; y: number; width: number; height: number }): Promise<Buffer> {
  await svg.evaluate((element) => {
    const style = element.getAttribute("style");
    element.style.setProperty("display", "none", "important");
    element.getBoundingClientRect();
    if (style === null) element.removeAttribute("style");
    else element.setAttribute("style", style);
    element.getBoundingClientRect();
  });
  return clip ? page.screenshot({ clip, animations: "disabled" }) : svg.screenshot({ animations: "disabled" });
}

async function changedPixels(page: Page, before: Buffer, after: Buffer): Promise<number> {
  return page.evaluate(async ([first, second]) => {
    const decode = async (encoded: string) => {
      const bytes = Uint8Array.from(atob(encoded), (character) => character.charCodeAt(0));
      const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/png" }));
      const canvas = document.createElement("canvas");
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const context = canvas.getContext("2d")!;
      context.drawImage(bitmap, 0, 0);
      bitmap.close();
      return context.getImageData(0, 0, canvas.width, canvas.height);
    };
    const original = await decode(first);
    const changed = await decode(second);
    if (original.width !== changed.width || original.height !== changed.height) {
      throw new Error("Screenshot dimensions changed during a terminal probe");
    }
    let count = 0;
    for (let offset = 0; offset < original.data.length; offset += 4) {
      if ([0, 1, 2, 3].some((channel) => original.data[offset + channel] !== changed.data[offset + channel])) {
        count += 1;
      }
    }
    return count;
  }, [before.toString("base64"), after.toString("base64")]);
}

async function expectPaintedLabels(page: Page, svg: Locator, expected: string[]): Promise<void> {
  const labels = svg.locator("text, foreignObject p").filter({ hasText: /\S/u });
  const texts = await labels.allTextContents();
  expect(texts.map((text) => text.trim()).sort()).toEqual([...expected].sort());
  const rootBounds = await svg.boundingBox();
  expect(rootBounds).not.toBeNull();
  for (let index = 0; index < texts.length; index += 1) {
    const label = labels.nth(index);
    const paints = await label.evaluate((element) => {
      const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
      const colors: string[] = [];
      while (walker.nextNode()) {
        if (!walker.currentNode.textContent?.trim()) continue;
        const leaf = walker.currentNode.parentElement!;
        const style = getComputedStyle(leaf);
        colors.push(leaf instanceof SVGElement ? style.fill : style.color);
      }
      return colors;
    });
    expect(paints.length, texts[index]).toBeGreaterThan(0);
    expect(new Set(paints), texts[index]).toEqual(new Set([
      texts[index].trim() === "Verify token" ? "rgb(255, 0, 255)" : "rgb(0, 242, 255)",
    ]));
    const bounds = await label.boundingBox();
    expect(bounds, texts[index]).not.toBeNull();
    expect(bounds!.width, texts[index]).toBeGreaterThan(0);
    expect(bounds!.height, texts[index]).toBeGreaterThan(0);
    expect(bounds!.x, texts[index]).toBeGreaterThanOrEqual(rootBounds!.x - 1);
    expect(bounds!.y, texts[index]).toBeGreaterThanOrEqual(rootBounds!.y - 1);
    expect(bounds!.x + bounds!.width, texts[index]).toBeLessThanOrEqual(rootBounds!.x + rootBounds!.width + 1);
    expect(bounds!.y + bounds!.height, texts[index]).toBeLessThanOrEqual(rootBounds!.y + rootBounds!.height + 1);
    const originalStyle = await label.getAttribute("style");
    const before = await captureScene(page, svg, bounds!);
    await label.evaluate((element) => (element as SVGElement | HTMLElement).style.setProperty("visibility", "hidden", "important"));
    const hidden = await captureScene(page, svg, bounds!);
    await label.evaluate((element, style) => {
      if (style === null) element.removeAttribute("style");
      else element.setAttribute("style", style);
    }, originalStyle);
    expect(await changedPixels(page, before, hidden), `${texts[index]} must contribute pixels inside its own bounds`).toBeGreaterThan(0);
    const restored = await captureScene(page, svg, bounds!);
    const restoredDifference = await changedPixels(page, before, restored);
    if (restoredDifference) {
      await test.info().attach("label-before", { body: before, contentType: "image/png" });
      await test.info().attach("label-restored", { body: restored, contentType: "image/png" });
    }
    expect(restoredDifference, `${texts[index]} must restore its pixels`).toBe(0);
  }
}

async function expectPaintedEffects(page: Page, svg: Locator, count: number): Promise<void> {
  const terminals = svg.locator("[filter], [style*='filter:']");
  await expect(terminals).toHaveCount(count);
  const original = await captureScene(page, svg);
  for (let index = 0; index < count; index += 1) {
    const terminal = terminals.nth(index);
    const filter = await terminal.evaluate((element) => element.getAttribute("filter") ?? (element as SVGElement | HTMLElement).style.filter);
    expect(filter).toMatch(/^url\(["']?#[^)"']+["']?\)$/u);
    const originalStyle = await terminal.getAttribute("style");
    await terminal.evaluate((element) => (element as SVGElement).style.setProperty("filter", "none", "important"));
    await expect(terminal).toHaveCSS("filter", "none");
    expect(await changedPixels(page, original, await captureScene(page, svg)), `effect ${index} must paint`).toBeGreaterThan(0);
    await terminal.evaluate((element, style) => {
      if (style === null) element.removeAttribute("style");
      else element.setAttribute("style", style);
    }, originalStyle);
  }
  const restored = await captureScene(page, svg);
  const restoredDifference = await changedPixels(page, original, restored);
  if (restoredDifference) {
    await test.info().attach("effects-before", { body: original, contentType: "image/png" });
    await test.info().attach("effects-restored", { body: restored, contentType: "image/png" });
  }
  expect(restoredDifference, "effects must restore their pixels").toBe(0);
}

async function expectPaintedArrows(page: Page, svg: Locator, count: number): Promise<void> {
  const edges = svg.locator("[marker-end]");
  await expect(edges).toHaveCount(count);
  const original = await captureScene(page, svg);
  for (let index = 0; index < count; index += 1) {
    const edge = edges.nth(index);
    const originalStyle = await edge.getAttribute("style");
    await edge.evaluate((element) => (element as SVGElement).style.setProperty("marker-end", "none", "important"));
    expect(await changedPixels(page, original, await captureScene(page, svg)), `arrow ${index} must paint`).toBeGreaterThan(0);
    await edge.evaluate((element, style) => {
      if (style === null) element.removeAttribute("style");
      else element.setAttribute("style", style);
    }, originalStyle);
  }
  expect(await changedPixels(page, original, await captureScene(page, svg)), "arrows must restore their pixels").toBe(0);
}

async function expectCyberpunkCanvas(svg: Locator): Promise<void> {
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
}

test("public Cyberpunk paints its layered canvas and separate shape, edge and text glows", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const hash = encodeShareHash({
    code: sceneSource("flowchart"),
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
  await expectCyberpunkCanvas(svg);

  const glows = await svg.evaluate((root) => [...root.querySelectorAll("[filter]")].map((terminal) => {
    const binding = terminal.getAttribute("filter")!;
    const id = binding.slice(5, -1);
    const filter = root.querySelector(`[id="${id}"]`)!;
    return {
      kind: terminal.matches("g.node > .label-container") ? "node"
        : terminal.matches("path.flowchart-link") ? "edge"
          : terminal.querySelector("text") ? "text" : "unexpected",
      deviations: [...filter.querySelectorAll("feGaussianBlur")].map((blur) => Number(blur.getAttribute("stdDeviation"))),
    };
  }));
  expect(glows.map(({ kind }) => kind).sort()).toEqual([
    ...Array<string>(3).fill("edge"), ...Array<string>(4).fill("node"), ...Array<string>(6).fill("text"),
  ]);
  expect(glows.filter(({ kind }) => kind === "node").map(({ deviations }) => deviations)).toEqual(Array(4).fill([8, 16]));
  expect(glows.filter(({ kind }) => kind === "edge").map(({ deviations }) => deviations)).toEqual(Array(3).fill([6]));
  expect(glows.filter(({ kind }) => kind === "text").map(({ deviations }) => deviations)).toEqual(Array(6).fill([5]));
  await svg.screenshot({ path: test.info().outputPath("public-cyberpunk.png") });
  await expectPaintedLabels(page, svg, ["Browse Products", "Item in Stock?", "Add to Cart", "Out of Stock", "Checkout", "Yes", "No"]);
  await expectPaintedEffects(page, svg, 13);
  await expectPaintedArrows(page, svg, 3);
  errors.assertNone();
});

test("public Cyberpunk keeps HTML glyph glow separate from the label background", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const hash = encodeShareHash({
    code: sceneSource("flowchart"),
    mermaidConfig: '{"htmlLabels":true}',
    diagramTheme: "default",
    themePresetId: "cyberpunk",
    svgPipeline: "parity",
    textMeasurementMode: "browser",
    diagramFont: "trebuchet",
  });
  await page.goto(`./${hash}`, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  await page.evaluate(() => document.fonts.ready);
  const svg = page.locator(".preview-container > div").first().locator("svg");
  await expectCyberpunkCanvas(svg);
  const glyphs = svg.locator("g.node foreignObject p, g.edgeLabel foreignObject p");
  await expect(glyphs).toHaveCount(6);
  const facts = await glyphs.evaluateAll((nodes) => nodes.map((node) => {
    const style = getComputedStyle(node);
    const box = node.getBoundingClientRect();
    const owner = style.filter !== "none" ? node : node.closest("[filter]");
    return { text: node.textContent, weight: style.fontWeight,
      filter: owner ? getComputedStyle(owner).filter : "none",
      filtersBackground: !!owner?.querySelector("rect"),
      background: style.backgroundColor, width: box.width, height: box.height };
  }));
  expect(facts.map(({ text }) => text).sort()).toEqual(["Add to Cart", "Browse Products", "Item in Stock?", "No", "Out of Stock", "Yes"]);
  for (const fact of facts) {
    expect(fact.weight).toBe("600");
    expect(fact.filter).toContain("url(");
    expect(fact.filtersBackground).toBe(false);
    expect(fact.background).toBe("rgba(0, 0, 0, 0)");
    expect(fact.width).toBeGreaterThan(0);
    expect(fact.height).toBeGreaterThan(0);
  }
  const backgrounds = svg.locator("foreignObject div.labelBkg").filter({ hasText: /\S/u });
  await expect(backgrounds).toHaveCount(2);
  for (const background of await backgrounds.all()) {
    await expect(background).toHaveCSS("background-color", "rgb(5, 20, 35)");
  }
  await svg.screenshot({ path: test.info().outputPath("public-cyberpunk-html.png") });
  await expectPaintedLabels(page, svg, ["Browse Products", "Item in Stock?", "Add to Cart", "Out of Stock", "Checkout", "Yes", "No"]);
  await expectPaintedEffects(page, svg, 13);
  await expectPaintedArrows(page, svg, 3);
  errors.assertNone();
});

for (const scene of [
  {
    family: "sequence",
    labels: ["Client", "Client", "API", "API", "Request data", "Return data", "Verify token", "loop", "[Each request]"],
    effects: 21,
    arrows: 2,
  },
  {
    family: "xychart",
    labels: ["Request Volume", "Window", "Requests", "A", "B", "C", "D", ...Array.from({ length: 11 }, (_, index) => String(index))],
    effects: 28,
    arrows: 0,
  },
]) {
  test(`public Cyberpunk ${scene.family} keeps every label, glow and arrow visible`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    const hash = encodeShareHash({
      code: sceneSource(scene.family),
      mermaidConfig: '{"htmlLabels":false}',
      diagramTheme: "default",
      themePresetId: "cyberpunk",
      svgPipeline: "resvg-safe",
      textMeasurementMode: "browser",
      diagramFont: "trebuchet",
    });
    await page.goto(`./${hash}`, { waitUntil: "domcontentloaded" });
    await waitForPreviewSvg(page);
    await page.evaluate(() => document.fonts.ready);
    const svg = page.locator(".preview-container > div").first().locator("svg");
    await expect(svg.locator("foreignObject")).toHaveCount(0);
    await expectCyberpunkCanvas(svg);
    await expectPaintedLabels(page, svg, scene.labels);
    await expectPaintedEffects(page, svg, scene.effects);
    await expectPaintedArrows(page, svg, scene.arrows);
    await svg.screenshot({ path: test.info().outputPath(`public-cyberpunk-${scene.family}.png`) });
    errors.assertNone();
  });
}
