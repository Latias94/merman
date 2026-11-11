import { readFileSync } from "node:fs";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { encodeShareHash } from "../src/lib/share";
import { monitorBrowserErrors, waitForPreviewSvg } from "./helpers/playground";

const sceneSource = (family: string): string => readFileSync(
  new URL(`../../fixtures/themes/public-cyberpunk/${family}.mmd`, import.meta.url),
  "utf8",
);

test.setTimeout(120_000);

const colors = {
  canvas: "rgb(8, 13, 17)",
  surface: "rgb(22, 31, 37)",
  surfaceAlt: "rgb(37, 49, 57)",
  primary: "rgb(234, 244, 240)",
  muted: "rgb(165, 188, 192)",
  yellow: "rgb(243, 230, 0)",
  cyan: "rgb(41, 243, 236)",
  red: "rgb(255, 69, 100)",
  noteSurface: "rgb(53, 28, 41)",
  noteInk: "rgb(255, 233, 239)",
};

type LabelExpectation = string | {
  text: string;
  role: "primary" | "muted" | "yellow" | "noteInk";
  weight?: number;
};
interface ShadowExpectation {
  dx: number;
  dy: number;
  sigma: number;
  color: string;
  alpha: number;
}
interface GlowExpectation {
  selector: string;
  count: number;
  shadows: ShadowExpectation[];
  stroke: string;
  strokeWidth: number;
  fill?: string;
  fillOpacity?: number;
}

const panelShadows: ShadowExpectation[] = [
  { dx: -3, dy: 0, sigma: 0, color: colors.red, alpha: 0.8 },
  { dx: 4, dy: 4, sigma: 0, color: colors.cyan, alpha: 0.85 },
];
const signalGlow: ShadowExpectation[] = [
  { dx: 0, dy: 0, sigma: 1.2, color: colors.cyan, alpha: 0.25 },
];
const flowchartGlows: GlowExpectation[] = [
  { selector: "[data-merman-theme-depth='node'] > .label-container", count: 4, shadows: panelShadows, stroke: colors.yellow, strokeWidth: 1.5, fill: colors.surface },
  { selector: "path.flowchart-link", count: 3, shadows: signalGlow, stroke: colors.cyan, strokeWidth: 1.5 },
];
const flowchartLabels: LabelExpectation[] = [
  ...["Browse Products", "Item in Stock?", "Add to Cart", "Out of Stock"]
    .map((text) => ({ text, role: "yellow" as const, weight: 700 })),
  "Checkout",
  ...["Yes", "No"].map((text) => ({ text, role: "primary" as const, weight: 500 })),
];

function expectComputedColor(actual: string, rgb: string, alpha: number): void {
  const channels = actual.match(/[\d.]+/gu)?.map(Number) ?? [];
  expect(channels.slice(0, 3)).toEqual(rgb.match(/[\d.]+/gu)!.map(Number));
  // CSSOM can serialize an authored alpha through the browser's eight-bit color storage.
  expect(Math.abs((channels[3] ?? 1) - alpha)).toBeLessThanOrEqual(1 / 255);
}

function luminance(rgb: string): number {
  const channels = rgb.match(/[\d.]+/gu)!.slice(0, 3).map((value) => {
    const channel = Number(value) / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
}

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

async function expectPaintedLabels(page: Page, svg: Locator, expected: LabelExpectation[], selector = "text, foreignObject p"): Promise<void> {
  const labelRoles: Exclude<LabelExpectation, string>[] = expected.map((label) =>
    typeof label === "string" ? { text: label, role: "primary" } : label);
  const ink = new Map(labelRoles.map(({ text, role, weight }) => [text, { color: colors[role], weight }]));
  const labels = svg.locator(selector).filter({ hasText: /\S/u });
  const texts = await labels.evaluateAll((elements) => elements.map((element) => {
    // SVG textContent concatenates wrapped row tspans without the visual line break.
    const rows = element.querySelectorAll(":scope > tspan.row");
    return rows.length > 0
      ? [...rows].map((row) => row.textContent?.trim() ?? "").join(" ")
      : element.textContent ?? "";
  }));
  expect(texts.map((text) => text.trim()).sort()).toEqual(labelRoles.map(({ text }) => text).sort());
  const rootBounds = await svg.boundingBox();
  expect(rootBounds).not.toBeNull();
  for (let index = 0; index < texts.length; index += 1) {
    const label = labels.nth(index);
    const paint = await label.evaluate((element) => {
      const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
      const colors: string[] = [];
      const weights: string[] = [];
      const glows: { tag: string; filter: string; textShadow: string }[] = [];
      while (walker.nextNode()) {
        if (!walker.currentNode.textContent?.trim()) continue;
        const leaf = walker.currentNode.parentElement!;
        const style = getComputedStyle(leaf);
        colors.push(leaf instanceof SVGElement ? style.fill : style.color);
        weights.push(style.fontWeight);
        for (let ancestor: Element | null = leaf; ancestor; ancestor = ancestor.parentElement) {
          const style = getComputedStyle(ancestor);
          if (style.filter !== "none" || style.textShadow !== "none") {
            glows.push({ tag: ancestor.localName, filter: style.filter, textShadow: style.textShadow });
          }
          if (ancestor instanceof SVGSVGElement) break;
        }
      }
      return { colors, weights, glows };
    });
    const expectedInk = ink.get(texts[index].trim())!;
    expect(paint.colors.length, texts[index]).toBeGreaterThan(0);
    expect(new Set(paint.colors), texts[index]).toEqual(new Set([expectedInk.color]));
    for (const color of new Set(paint.colors)) {
      for (const background of [colors.surface, colors.canvas]) {
        const foregroundLuminance = luminance(color);
        const backgroundLuminance = luminance(background);
        const contrast = (Math.max(foregroundLuminance, backgroundLuminance) + 0.05)
          / (Math.min(foregroundLuminance, backgroundLuminance) + 0.05);
        expect(contrast, `${texts[index]} must contrast with dark panels and the canvas`).toBeGreaterThanOrEqual(7);
      }
    }
    if (expectedInk.weight !== undefined) {
      expect(new Set(paint.weights), texts[index]).toEqual(new Set([String(expectedInk.weight)]));
    }
    expect(paint.glows, `${texts[index]} and its ancestors must keep glyphs crisp`).toEqual([]);
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

async function expectPaintedEffects(page: Page, svg: Locator, expected: GlowExpectation[], baselineSelector = ":not(*)"): Promise<void> {
  await expect(svg).toHaveCSS("filter", "none");
  const candidates = svg.locator("*");
  const observed = await candidates.evaluateAll((elements, { expectations, baselineSelector }) => elements.flatMap((element, index) => {
    const computed = getComputedStyle(element).filter;
    if (computed === "none") return [];
    const attribute = element.getAttribute("filter");
    const binding = (element as SVGElement | HTMLElement).style.filter || attribute || "";
    const id = /^url\(["']?#([^)"']+)["']?\)$/u.exec(binding)?.[1];
    const root = element.closest("svg")!;
    const filters = id ? [...root.querySelectorAll(`filter[id="${id}"]`)] : [];
    const depth = element.closest("[data-merman-theme-depth]");
    return [{
      index, binding,
      computed, baseline: element.matches(baselineSelector),
      wins: Boolean(id && /#([^)"']+)["']?\)$/u.exec(computed)?.[1] === id),
      attributeMatches: !attribute || /^url\(["']?#([^)"']+)["']?\)$/u.exec(attribute)?.[1] === id,
      definitions: filters.length,
      owners: expectations.flatMap(({ selector }, owner) => element.matches(selector) ? [owner] : []),
      colorSpace: filters[0]?.getAttribute("color-interpolation-filters"),
      stroke: getComputedStyle(element).stroke,
      strokeWidth: getComputedStyle(element).strokeWidth,
      fill: getComputedStyle(element).fill,
      fillOpacity: getComputedStyle(element).fillOpacity,
      deviations: [...(filters[0]?.querySelectorAll("feGaussianBlur") ?? [])]
        .map((blur) => [Number(blur.getAttribute("stdDeviation")), blur.getAttribute("in")]),
      offsets: [...(filters[0]?.querySelectorAll("feOffset") ?? [])]
        .map((offset) => [Number(offset.getAttribute("dx")), Number(offset.getAttribute("dy")), offset.getAttribute("in")]),
      floods: [...(filters[0]?.querySelectorAll("feFlood") ?? [])].map((flood) => ({
        authoredColor: flood.getAttribute("flood-color"),
        color: getComputedStyle(flood).floodColor, opacity: getComputedStyle(flood).floodOpacity,
      })),
      merges: [...(filters[0]?.querySelectorAll("feMerge") ?? [])]
        .map((merge) => [...merge.children].map((node) => node.getAttribute("in"))),
      composites: [...(filters[0]?.querySelectorAll("feComposite") ?? [])].map((composite) => [
        composite.getAttribute("in"), composite.getAttribute("in2"), composite.getAttribute("operator"),
      ]),
      depth: depth ? {
        hidden: depth.getAttribute("aria-hidden"), pointerEvents: getComputedStyle(depth).pointerEvents,
        bodies: depth.children.length,
        identityFree: !depth.matches("[id], [data-id], [data-et], [name]")
          && !depth.querySelector("[id], [data-id], [data-et], [name], text, foreignObject, a, use"),
        beforeConnections: [...root.querySelectorAll("path.flowchart-link, line.actor-line, line.messageLine0, line.messageLine1")]
          .every((edge) => Boolean(depth.compareDocumentPosition(edge) & Node.DOCUMENT_POSITION_FOLLOWING)),
      } : null,
    }];
  }), { expectations: expected, baselineSelector });
  const terminals = observed.filter((terminal) => terminal.binding.includes("-theme-effect-"));
  // Neo retains independent Mermaid shadows on clusters and label backings. They cannot
  // count as authored neon, enclose typed paint, or replace the glyph-ink checks above.
  for (const baseline of observed.filter((terminal) => !terminal.binding.includes("-theme-effect-"))) {
    expect(baseline).toMatchObject({
      binding: "", baseline: true, computed: "drop-shadow(rgb(185, 185, 185) 1px 2px 2px)", definitions: 0,
    });
  }
  expect(terminals).toHaveLength(expected.reduce((sum, { count }) => sum + count, 0));
  for (const terminal of terminals) {
    expect(terminal.binding).toMatch(/^url\(["']?#[^)"']+["']?\)$/u);
    expect(terminal, `effect ${terminal.index} must own one winning local filter`).toMatchObject({
      wins: true, attributeMatches: true, definitions: 1, colorSpace: "sRGB",
    });
    expect(terminal.owners, `effect ${terminal.index} must belong only to an expected body, edge or chart mark`).toHaveLength(1);
    if (terminal.depth) {
      expect(terminal.depth, "body depth must be non-interactive and precede every connection").toEqual({
        hidden: "true", pointerEvents: "none", bodies: 1, identityFree: true, beforeConnections: true,
      });
    }
  }
  for (const [owner, { selector, count, shadows, stroke, strokeWidth, fill, fillOpacity }] of expected.entries()) {
    const owned = terminals.filter((terminal) => terminal.owners.includes(owner));
    expect(owned, selector).toHaveLength(count);
    // Only the two hard body offsets are Independent; single halos use Cascade.
    const source = shadows.length === 2 ? "SourceAlpha" : "SourceGraphic";
    expect(owned.map((terminal) => terminal.deviations), selector)
      .toEqual(Array.from({ length: count }, () => shadows.filter(({ sigma }) => sigma > 0)
        .map(({ sigma }) => [sigma, source])));
    for (const terminal of owned) {
      expect(shadows).toHaveLength(terminal.depth ? 2 : 1);
      expect(terminal.offsets, "hard registration offsets must consume independent silhouettes").toEqual(
        shadows.flatMap(({ dx, dy, sigma }, index) => dx || dy
          ? [[dx, dy, sigma > 0 ? `merman-shadow-${index}-blur` : source]] : []),
      );
      expect(terminal.floods, selector).toHaveLength(shadows.length);
      for (const [index, flood] of terminal.floods.entries()) {
        const { color, alpha } = shadows[index];
        expect(flood.authoredColor).toBe(color.replace("rgb(", "rgba(").replace(")", `, ${alpha})`));
        expectComputedColor(flood.color, color, alpha);
        expect(flood.opacity).toBe("1");
      }
      expect(terminal.composites, "each color must be clipped to its own source silhouette").toEqual(
        shadows.map(({ dx, dy, sigma }, index) => [
          `merman-shadow-${index}-flood`,
          dx || dy ? `merman-shadow-${index}-offset` : sigma > 0 ? `merman-shadow-${index}-blur` : source,
          "in",
        ]),
      );
      expect(terminal, selector).toMatchObject({ stroke, strokeWidth: `${strokeWidth}px` });
      if (fill !== undefined) expect(terminal.fill, selector).toBe(fill);
      if (fillOpacity !== undefined) expect(Number(terminal.fillOpacity), selector).toBeCloseTo(fillOpacity, 5);
      expect(terminal.merges, "body depth omits SourceGraphic while connectors and marks retain their source").toEqual(
        [[...shadows.map((_, index) => `merman-shadow-${index}-shadow`), ...(terminal.depth ? [] : ["SourceGraphic"])]],
      );
    }
  }
  const original = await captureScene(page, svg);
  for (const { index } of terminals) {
    const terminal = candidates.nth(index);
    const originalStyle = await terminal.getAttribute("style");
    const depth = await terminal.evaluate((element) => !!element.closest("[data-merman-theme-depth]"));
    const property = depth ? "visibility" : "filter";
    const disabled = depth ? "hidden" : "none";
    try {
      // Removing a depth filter would reveal its bare silhouette, so hide its paint instead.
      await terminal.evaluate((element, { property, disabled }) =>
        (element as SVGElement).style.setProperty(property, disabled, "important"), { property, disabled });
      await expect(terminal).toHaveCSS(property, disabled);
      expect(await changedPixels(page, original, await captureScene(page, svg)), `effect ${index} must paint`).toBeGreaterThan(0);
    } finally {
      await terminal.evaluate((element, style) => {
        if (style === null) element.removeAttribute("style");
        else element.setAttribute("style", style);
      }, originalStyle);
    }
  }
  const restored = await captureScene(page, svg);
  const restoredDifference = await changedPixels(page, original, restored);
  if (restoredDifference) {
    await test.info().attach("effects-before", { body: original, contentType: "image/png" });
    await test.info().attach("effects-restored", { body: restored, contentType: "image/png" });
  }
  expect(restoredDifference, "effects must restore their pixels").toBe(0);
}

async function expectFlowchartDepthAndGlows(svg: Locator, look: "classic" | "neo"): Promise<void> {
  const facts = await svg.evaluate((root) => {
    const faces = [...root.querySelectorAll<SVGGElement>("g[data-et='node']")];
    const depths = [...root.querySelectorAll<SVGGElement>("[data-merman-theme-depth='node']")];
    const edges = [...root.querySelectorAll("path.flowchart-link")];
    const filterId = (binding: string | null) => /^url\(["']?#([^)"']+)["']?\)$/u.exec(binding ?? "")?.[1];
    const geometry = (body: SVGGraphicsElement) => ({
      tag: body.localName,
      attributes: ["x", "y", "width", "height", "rx", "ry", "points", "d", "transform"]
        .map((name) => body.getAttribute(name)),
      paint: ["fill", "stroke", "stroke-width", "opacity", "fill-opacity", "stroke-opacity", "rx", "ry"]
        .map((name) => getComputedStyle(body).getPropertyValue(name)),
    });
    return {
      faces: faces.map((face) => ({
        id: face.getAttribute("data-id"), look: face.getAttribute("data-look"),
        bodies: face.querySelectorAll(":scope > .label-container").length,
        tag: face.querySelector(".label-container")?.localName,
        filter: getComputedStyle(face.querySelector(".label-container")!).filter,
        fill: getComputedStyle(face.querySelector(".label-container")!).fill,
        stroke: getComputedStyle(face.querySelector(".label-container")!).stroke,
        width: getComputedStyle(face.querySelector(".label-container")!).strokeWidth,
        radius: [face.querySelector(".label-container")?.getAttribute("rx"), face.querySelector(".label-container")?.getAttribute("ry")],
      })),
      depths: depths.map((depth) => {
        const body = depth.querySelector<SVGGraphicsElement>(":scope > .label-container")!;
        const binding = body.getAttribute("filter")!;
        const id = filterId(binding);
        const owners = faces.filter((face) => id?.startsWith(`${face.id}-theme-effect-`));
        const owner = owners[0];
        const face = owner?.querySelector<SVGGraphicsElement>(":scope > .label-container");
        const filters = id ? [...root.querySelectorAll<SVGFilterElement>(`filter[id="${id}"]`)] : [];
        const merge = filters[0]?.lastElementChild;
        const matrix = (element: SVGGraphicsElement) => {
          const ctm = element.getCTM()!;
          return [ctm.a, ctm.b, ctm.c, ctm.d, ctm.e, ctm.f];
        };
        return {
          owner: owner?.getAttribute("data-id"), owners: owners.length,
          look: depth.getAttribute("data-look"), hidden: depth.getAttribute("aria-hidden"),
          pointerEvents: getComputedStyle(depth).pointerEvents,
          layer: depth.parentElement?.getAttribute("data-merman-theme-depth"),
          bodies: depth.children.length,
          identityFree: !depth.matches("[id], [data-id], [data-et], [name]")
            && !depth.querySelector("[id], [data-id], [data-et], [name], text, foreignObject, a, use"),
          beforeEdges: edges.every((edge) => Boolean(depth.compareDocumentPosition(edge) & Node.DOCUMENT_POSITION_FOLLOWING)),
          beforeFace: !!owner && Boolean(depth.compareDocumentPosition(owner) & Node.DOCUMENT_POSITION_FOLLOWING),
          geometry: geometry(body), faceGeometry: face ? geometry(face) : null,
          matrix: matrix(body), faceMatrix: face ? matrix(face) : [],
          faceBinding: face?.getAttribute("filter"),
          inlineId: filterId(body.style.filter), filterId: id, filters: filters.length,
          merge: merge?.localName,
          output: [...(merge?.children ?? [])].map((node) => [node.localName, node.getAttribute("in")]),
        };
      }),
      suppressions: [...root.querySelectorAll<SVGElement>("[style]")]
        .filter((terminal) => terminal.style.filter === "none")
        .map((terminal) => terminal.matches("g[data-et='node'] > .label-container")
          && terminal.getAttribute("filter") === null
          && terminal.style.getPropertyPriority("filter") === "important"
          && getComputedStyle(terminal).filter === "none"),
    };
  });
  const ids = ["Browse", "Cart", "Stock", "Unavailable"];
  expect(facts.faces.map(({ id }) => id).sort()).toEqual(ids);
  for (const face of facts.faces) {
    expect(face).toMatchObject({
      look, bodies: 1, filter: "none", tag: face.id === "Stock" ? "polygon" : "rect",
      fill: colors.surface, stroke: colors.yellow, width: "1.5px",
    });
    if (face.tag === "rect") expect(face.radius).toEqual(["0", "0"]);
  }
  expect(facts.depths.map(({ owner }) => owner).sort()).toEqual(ids);
  for (const depth of facts.depths) {
    expect(depth, `${depth.owner} owns one non-interactive depth before connections and its face`).toMatchObject({
      owners: 1, look, hidden: "true", pointerEvents: "none", layer: "layer", bodies: 1,
      identityFree: true, beforeEdges: true, beforeFace: true, faceBinding: null, filters: 1,
      merge: "feMerge", output: [["feMergeNode", "merman-shadow-0-shadow"], ["feMergeNode", "merman-shadow-1-shadow"]],
    });
    expect(depth.geometry, depth.owner!).toEqual(depth.faceGeometry);
    expect(depth.inlineId, depth.owner!).toBe(depth.filterId);
    for (const [index, value] of depth.matrix.entries()) expect(value, depth.owner!).toBeCloseTo(depth.faceMatrix[index], 5);
  }
  expect(facts.suppressions, "Neo foreground faces must suppress their built-in shadow").toEqual(
    Array<boolean>(look === "neo" ? 4 : 0).fill(true),
  );
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
          authoredColor: stop.getAttribute("stop-color"),
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
  expect(canvas.base).toBe(colors.canvas);
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
  expect(radial.stops.map(({ authoredColor }) => authoredColor)).toEqual(["rgba(243, 230, 0, 0.04)", "rgba(243, 230, 0, 0)"]);
  for (const [index, stop] of radial.stops.entries()) expectComputedColor(stop.color, colors.yellow, [0.04, 0][index]);
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
    expect(gradient.stops.map(({ authoredColor }) => authoredColor)).toEqual([
      "rgba(41, 243, 236, 0.035)", "rgba(41, 243, 236, 0.035)", "rgba(41, 243, 236, 0)", "rgba(41, 243, 236, 0)",
    ]);
    for (const [index, stop] of gradient.stops.entries()) expectComputedColor(stop.color, colors.cyan, [0.035, 0.035, 0, 0][index]);
    expect(gradient.stops.map(({ opacity }) => opacity)).toEqual([1, 1, 1, 1]);
    const pattern = canvas.patterns[index];
    expect(pattern).toMatchObject({ units: "userSpaceOnUse", width: 40, height: 40, fill: `url(#${gradient.id})` });
    expect(canvas.layers[index + 1].fill).toBe(`url(#${pattern.id})`);
  }
  expect(canvas.layers[0].fill).toBe(`url(#${radial.id})`);
}

// Cover both looks and layouts across the existing SVG-text and HTML-label scenes.
for (const { layout, look, htmlLook } of [
  { layout: "elk", look: "neo", htmlLook: "classic" },
  { layout: "dagre", look: "classic", htmlLook: "neo" },
] as const) {
  test(`public Cyberpunk ${look} ${layout} paints crisp yellow SVG glyphs on dark panels above hard registration edges`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    const hash = encodeShareHash({
      code: sceneSource("flowchart"),
      mermaidConfig: JSON.stringify({ htmlLabels: false, flowchart: { look, layout } }),
      diagramTheme: "default",
      themePresetId: "cyberpunk",
      themeRecipeJson: null,
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

    await expectFlowchartDepthAndGlows(svg, look);
    await svg.screenshot({ path: test.info().outputPath("public-cyberpunk.png") });
    await expectPaintedLabels(page, svg, flowchartLabels);
    await expectPaintedEffects(page, svg, flowchartGlows, look === "neo" ? '[data-look="neo"].cluster > rect, [data-look="neo"].node rect' : undefined);
    await expectPaintedArrows(page, svg, 3);
    errors.assertNone();
  });

  test(`public Cyberpunk ${htmlLook} ${layout} keeps HTML glyphs and label backgrounds crisp above hard registration edges`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    const hash = encodeShareHash({
      code: sceneSource("flowchart"),
      mermaidConfig: JSON.stringify({ htmlLabels: true, flowchart: { look: htmlLook, layout } }),
      diagramTheme: "default",
      themePresetId: "cyberpunk",
      themeRecipeJson: null,
      svgPipeline: "parity",
      textMeasurementMode: "browser",
      diagramFont: "trebuchet",
    });
    await page.goto(`./${hash}`, { waitUntil: "domcontentloaded" });
    await waitForPreviewSvg(page);
    await page.evaluate(() => document.fonts.ready);
    const svg = page.locator(".preview-container > div").first().locator("svg");
    await expectCyberpunkCanvas(svg);
    await expectFlowchartDepthAndGlows(svg, htmlLook);
    const glyphs = svg.locator("g.node foreignObject p, g.edgeLabel foreignObject p");
    await expect(glyphs).toHaveCount(6);
    const facts = await glyphs.evaluateAll((nodes) => nodes.map((node) => {
      const style = getComputedStyle(node);
      const box = node.getBoundingClientRect();
      return { text: node.textContent, weight: style.fontWeight, nodeLabel: !!node.closest("g.node"),
        filter: style.filter,
        background: style.backgroundColor, width: box.width, height: box.height };
    }));
    expect(facts.map(({ text }) => text).sort()).toEqual(["Add to Cart", "Browse Products", "Item in Stock?", "No", "Out of Stock", "Yes"]);
    for (const fact of facts) {
      expect(fact.weight).toBe(fact.nodeLabel ? "700" : "500");
      expect(fact.filter).toBe("none");
      expect(fact.background).toBe("rgba(0, 0, 0, 0)");
      expect(fact.width).toBeGreaterThan(0);
      expect(fact.height).toBeGreaterThan(0);
    }
    const backgrounds = svg.locator("foreignObject div.labelBkg").filter({ hasText: /\S/u });
    await expect(backgrounds).toHaveCount(2);
    for (const background of await backgrounds.all()) {
      await expect(background).toHaveCSS("background-color", colors.canvas);
      await expect(background).toHaveCSS("filter", "none");
    }
    await svg.screenshot({ path: test.info().outputPath("public-cyberpunk-html.png") });
    await expectPaintedLabels(page, svg, flowchartLabels);
    await expectPaintedEffects(page, svg, flowchartGlows, htmlLook === "neo" ? '[data-look="neo"].cluster > rect, [data-look="neo"].node rect' : undefined);
    await expectPaintedArrows(page, svg, 3);
    errors.assertNone();
  });
}

for (const scene of [
  {
    family: "sequence",
    labels: [
      ...["Client", "Client", "API", "API"].map((text) => ({ text, role: "yellow" as const, weight: 700 })),
      ...["Request data", "Return data", "loop", "[Each request]"]
        .map((text) => ({ text, role: "primary" as const, weight: 500 })),
      { text: "Verify token", role: "noteInk" as const, weight: 500 },
    ],
    glows: [
      { selector: "[data-merman-theme-depth='actor'] > rect", count: 4, shadows: panelShadows, stroke: colors.yellow, strokeWidth: 1.5, fill: colors.surface },
      { selector: "line.actor-line", count: 2, shadows: signalGlow, stroke: colors.cyan, strokeWidth: 1.5 },
      { selector: "line.messageLine0, line.messageLine1", count: 2, shadows: signalGlow, stroke: colors.cyan, strokeWidth: 1.5 },
      { selector: "line.loopLine", count: 4, shadows: [{ dx: 0, dy: 0, sigma: 1, color: colors.red, alpha: 0.14 }], stroke: colors.red, strokeWidth: 1.5 },
      { selector: "g[data-et='note'] > rect.note", count: 1, shadows: [{ dx: 0, dy: 0, sigma: 1.5, color: colors.red, alpha: 0.18 }], stroke: colors.red, strokeWidth: 1.5, fill: colors.noteSurface },
    ],
    arrows: 2,
  },
  {
    family: "xychart",
    labels: [
      { text: "Request Volume", role: "yellow" as const, weight: 700 },
      ...["Window", "Requests", "A", "B", "C", "D", ...Array.from({ length: 11 }, (_, index) => String(index))]
        .map((text) => ({ text, role: "muted" as const, weight: 500 })),
    ],
    glows: [
      { selector: "g.bar-plot-0 > rect", count: 4, shadows: [{ dx: 0, dy: 0, sigma: 1, color: colors.yellow, alpha: 0.18 }], stroke: colors.yellow, strokeWidth: 1.25, fill: colors.yellow, fillOpacity: 0.85 },
      { selector: "g.line-plot-1 > path", count: 1, shadows: [{ dx: 0, dy: 0, sigma: 1, color: colors.cyan, alpha: 0.18 }], stroke: colors.cyan, strokeWidth: 2 },
      { selector: "g.bar-plot-2 > rect", count: 4, shadows: [{ dx: 0, dy: 0, sigma: 1, color: colors.red, alpha: 0.18 }], stroke: colors.red, strokeWidth: 1.25, fill: colors.red, fillOpacity: 0.85 },
      { selector: "g.line-plot-3 > path", count: 1, shadows: [{ dx: 0, dy: 0, sigma: 1, color: colors.yellow, alpha: 0.18 }], stroke: colors.yellow, strokeWidth: 2 },
    ],
    arrows: 0,
  },
]) {
  test(`public Cyberpunk ${scene.family} keeps crisp semantic labels above visible marks and restrained glows`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    const hash = encodeShareHash({
      code: sceneSource(scene.family),
      mermaidConfig: '{"htmlLabels":false}',
      diagramTheme: "default",
      themePresetId: "cyberpunk",
      themeRecipeJson: null,
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
    if (scene.family === "sequence") {
      for (const { selector, count, fill, stroke, radius } of [
        { selector: "rect.actor[name]", count: 4, fill: colors.surface, stroke: colors.yellow, radius: "0" },
        { selector: "rect.note", count: 1, fill: colors.noteSurface, stroke: colors.red, radius: "0" },
        { selector: "polygon.labelBox", count: 1, fill: colors.surfaceAlt, stroke: colors.yellow },
        { selector: "rect.activation0", count: 1, fill: colors.cyan, stroke: colors.cyan },
      ]) {
        const surfaces = svg.locator(selector);
        await expect(surfaces).toHaveCount(count);
        for (const surface of await surfaces.all()) {
          await expect(surface).toHaveCSS("fill", fill);
          await expect(surface).toHaveCSS("stroke", stroke);
          await expect(surface).toHaveCSS("stroke-width", "1.5px");
          if (radius !== undefined) {
            await expect(surface).toHaveAttribute("rx", radius);
            await expect(surface).toHaveAttribute("ry", radius);
          }
        }
      }
      for (const actor of await svg.locator("rect.actor[name]").all()) {
        await expect(actor).toHaveCSS("filter", "none");
        await expect(actor).not.toHaveAttribute("filter", /theme-effect-/u);
      }
      await expect(svg.locator("rect.activation0")).toHaveCSS("filter", "none");
      await expect(svg.locator("polygon.labelBox")).toHaveCSS("filter", "drop-shadow(rgb(185, 185, 185) 1px 2px 2px)");
      await expect(svg.locator("polygon.labelBox")).not.toHaveAttribute("filter", /theme-effect-/u);
    }
    await expectPaintedLabels(page, svg, scene.labels);
    await expectPaintedEffects(page, svg, scene.glows, scene.family === "sequence" ? "polygon.labelBox" : undefined);
    await expectPaintedArrows(page, svg, scene.arrows);
    await svg.screenshot({ path: test.info().outputPath(`public-cyberpunk-${scene.family}.png`) });
    errors.assertNone();
  });
}

test("public Cyberpunk keeps Alice's exterior actor names and Bob's panel names yellow and crisp", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const hash = encodeShareHash({
    code: "sequenceDiagram\nactor Alice\nparticipant Bob\nAlice->>Bob: Hello Bob\nBob-->>Alice: Hello Alice",
    mermaidConfig: '{"htmlLabels":false}',
    diagramTheme: "default",
    themePresetId: "cyberpunk",
    themeRecipeJson: null,
    svgPipeline: "resvg-safe",
    textMeasurementMode: "browser",
    diagramFont: "trebuchet",
  });
  await page.goto(`./${hash}`, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  await page.evaluate(() => document.fonts.ready);
  const svg = page.locator(".preview-container > div").first().locator("svg");
  const alice = svg.locator("g.actor-man[name='Alice']");
  await expect(alice).toHaveCount(2);
  await expect(alice.locator(":scope > circle")).toHaveCount(2);
  await expect(alice.locator("text.actor-man")).toHaveCount(2);
  await expect(svg.locator("rect.actor[name='Alice']")).toHaveCount(0);
  const bob = svg.locator("rect.actor[name='Bob']");
  await expect(bob).toHaveCount(2);
  await expectCyberpunkCanvas(svg);
  for (const body of await svg.locator("g.actor-man[name='Alice'] > circle, rect.actor[name='Bob']").all()) {
    await expect(body).toHaveCSS("fill", colors.surface);
    await expect(body).toHaveCSS("stroke", colors.yellow);
    await expect(body).toHaveCSS("filter", "none");
  }
  await expectPaintedLabels(page, svg, [
    ...["Alice", "Alice", "Bob", "Bob"].map((text) => ({ text, role: "yellow" as const, weight: 700 })),
    ...["Hello Bob", "Hello Alice"].map((text) => ({ text, role: "primary" as const, weight: 500 })),
  ]);
  await expectPaintedEffects(page, svg, [
    { selector: "[data-merman-theme-depth='actor'] > rect", count: 2, shadows: panelShadows, stroke: colors.yellow, strokeWidth: 1.5, fill: colors.surface },
    { selector: "line.actor-line", count: 2, shadows: signalGlow, stroke: colors.cyan, strokeWidth: 1.5 },
    { selector: "line.messageLine0, line.messageLine1", count: 2, shadows: signalGlow, stroke: colors.cyan, strokeWidth: 1.5 },
  ]);
  await expectPaintedArrows(page, svg, 2);
  errors.assertNone();
});

for (const htmlLabels of [false, true]) {
  test(`public Cyberpunk keeps panel, BPMN annotation and exterior icon names readable with ${htmlLabels ? "HTML" : "native SVG"} node labels`, async ({ page }) => {
    const errors = monitorBrowserErrors(page);
    // Reuse the upstream icon fixture's real top-positioned label and square frame.
    const iconSource = readFileSync(new URL(
      "../../fixtures/flowchart/upstream_docs_flowchart_icon_shape_132.mmd", import.meta.url,
    ), "utf8");
    const hash = encodeShareHash({
      code: `${iconSource.trimEnd()}\nPanel[Panel name]\nAnnotation@{ shape: bpmn-annotation, label: "Annotation" }\nPanel --> Annotation --> A`,
      mermaidConfig: JSON.stringify({ htmlLabels, flowchart: { look: "classic", layout: "dagre" } }),
      diagramTheme: "default",
      themePresetId: "cyberpunk",
      themeRecipeJson: null,
      // Icon names retain HTML terminals even when ordinary node labels use SVG text.
      // Keep those terminals available for computed ink and actual-pixel assertions.
      svgPipeline: "parity",
      textMeasurementMode: "browser",
      diagramFont: "trebuchet",
    });
    await page.goto(`./${hash}`, { waitUntil: "domcontentloaded" });
    await waitForPreviewSvg(page);
    await page.evaluate(() => document.fonts.ready);
    const svg = page.locator(".preview-container > div").first().locator("svg.flowchart");
    await expect(svg).toHaveCount(1);
    const panel = svg.locator("g[data-et='node'][data-id='Panel'] > .label-container");
    const annotation = svg.locator("g[data-et='node'][data-id='Annotation'] .bpmn-annotation-bracket");
    const icon = svg.locator("g[data-et='node'][data-id='A'].icon-shape");
    await expect(panel).toHaveCount(1);
    await expect(annotation).toHaveCount(1);
    await expect(icon.locator(".label")).toHaveText("User Icon");
    for (const id of ["Panel", "Annotation"]) {
      const label = svg.locator(`g[data-et='node'][data-id='${id}'] .label`);
      await expect(label.locator("foreignObject")).toHaveCount(htmlLabels ? 1 : 0);
      await expect(label.locator("text")).toHaveCount(htmlLabels ? 0 : 1);
    }
    await expect(icon.locator(".label foreignObject")).toHaveCount(1);
    await expectCyberpunkCanvas(svg);
    await expect(panel).toHaveCSS("fill", colors.surface);
    await expect(panel).toHaveCSS("stroke", colors.yellow);
    await expect(annotation).toHaveCSS("stroke", colors.yellow);
    // The icon's name must actually sit above its frame, rather than on a panel face.
    const labelBounds = await icon.locator(".label").boundingBox();
    const frameBounds = await icon.locator(":scope > g[transform]").first().boundingBox();
    expect(labelBounds).not.toBeNull();
    expect(frameBounds).not.toBeNull();
    expect(labelBounds!.y + labelBounds!.height).toBeLessThanOrEqual(frameBounds!.y + 1);
    // Scope this probe to semantic names: a missing icon pack may render its own '?' glyph.
    await expectPaintedLabels(page, svg, [
      { text: "Panel name", role: "yellow", weight: 700 },
      // The native BPMN annotation uses the existing Markdown SVG terminal, whose
      // explicit normal-weight runs retain an unsupported-typography residual.
      { text: "Annotation", role: "yellow", weight: htmlLabels ? 700 : 400 },
      { text: "User Icon", role: "yellow", weight: 700 },
    ], ".label text, .label foreignObject p");
    if (!htmlLabels) {
      await page.getByRole("button", { name: "Appearance", exact: true }).click();
      const appearance = page.getByRole("region", { name: "Appearance", exact: true });
      const advanced = appearance.getByTestId("theme-advanced");
      await advanced.locator("summary").first().click();
      const metadata = appearance.getByTestId("theme-execution-metadata");
      await expect(metadata).toBeVisible();
      const evidence = JSON.parse((await metadata.textContent())!).theme_execution_evidence;
      expect(evidence.theme_status).toBe("residual");
      expect(evidence.diagnostics).toEqual(expect.arrayContaining([
        expect.objectContaining({ code: "unsupported-typography", target: "node-label" }),
      ]));
    }
    errors.assertNone();
  });
}
