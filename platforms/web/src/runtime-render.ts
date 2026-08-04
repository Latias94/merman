import { encodeOptions, getMerman } from "./runtime-core.js";
import { assertSafeSvgForDom } from "./svg-safety.js";
import type {
  BrowserTextMeasurementSession,
  HostTextMeasureRequest,
  HostTextMeasureResult,
  HostTextMetricsResult,
  HostTextMeasurer,
  SvgBindingOptions,
  SvgPlanResult,
} from "./public-types.js";

export function renderSvg(source: string, options?: SvgBindingOptions | string): string {
  return getMerman().renderSvg(source, encodeOptions(options));
}

export function svgPlanJson(
  source: string,
  options?: SvgBindingOptions | string
): SvgPlanResult {
  return getMerman().svgPlanJson(source, encodeOptions(options));
}

export function renderSvgWithTextMeasurer(
  source: string,
  measurer: HostTextMeasurer,
  options?: SvgBindingOptions | string
): string {
  const renderWithMeasurer = getMerman().renderSvgWithTextMeasurer;
  if (!renderWithMeasurer) {
    throw new Error(
      "Merman WASM does not expose renderSvgWithTextMeasurer(). Rebuild @mermanjs/web."
    );
  }
  return renderWithMeasurer(source, encodeOptions(options), measurer);
}

export function layoutJsonWithTextMeasurer(
  source: string,
  measurer: HostTextMeasurer,
  options?: SvgBindingOptions | string
): string {
  const layoutWithMeasurer = getMerman().layoutJsonWithTextMeasurer;
  if (!layoutWithMeasurer) {
    throw new Error(
      "Merman WASM does not expose layoutJsonWithTextMeasurer(). Rebuild @mermanjs/web."
    );
  }
  return layoutWithMeasurer(source, encodeOptions(options), measurer);
}

export function createBrowserTextMeasurementSession(): BrowserTextMeasurementSession {
  let probes: BrowserTextMeasureProbes | null = null;
  let disposed = false;

  const measure: HostTextMeasurer = (request) => {
    if (disposed) {
      return undefined;
    }

    try {
      probes ??= createTextMeasureProbes();
      if (!probes) {
        return undefined;
      }
      return measureWithBrowserProbes(probes, request);
    } catch {
      return undefined;
    }
  };

  return {
    measure,
    dispose() {
      if (disposed) {
        return;
      }
      disposed = true;
      if (probes) {
        disposeTextMeasureProbes(probes);
        probes = null;
      }
    },
  };
}

const SVG_NAMESPACE = "http://www.w3.org/2000/svg";

interface BrowserTextMeasureProbes {
  html: HTMLDivElement;
  svg: SVGSVGElement;
  directText: SVGTextElement;
  tspanText: SVGTextElement;
  tspan: SVGTSpanElement;
  wrappedText: SVGTextElement;
  formattedTextGroup: SVGGElement;
  formattedText: SVGTextElement;
  mermaidDimensionsSvg: SVGSVGElement;
  mermaidDimensionsText: SVGTextElement;
  mermaidDimensionsTspan: SVGTSpanElement;
  canvasContext?: CanvasRenderingContext2D | null;
}

type SvgProbeShape = "direct" | "tspan";

function measureWithBrowserProbes(
  probes: BrowserTextMeasureProbes,
  request: HostTextMeasureRequest
): HostTextMeasureResult | undefined {
  if (!request.text) {
    return emptyMeasurement(request);
  }

  switch (request.operation) {
    case "measure":
      return svgMetrics(probes, request, "direct");
    case "computed-length": {
      prepareSvgText(probes, request, "tspan", "start");
      if (typeof probes.tspan.getComputedTextLength !== "function") {
        return undefined;
      }
      return { kind: "length", length: Math.max(0, probes.tspan.getComputedTextLength()) };
    }
    case "bbox-x":
    case "bbox-x-with-ascii-overhang":
    case "title-bbox-x": {
      const bbox = svgBBox(probes, request, "direct", "middle");
      return {
        kind: "horizontal-extents",
        bbox_left: Math.max(0, -bbox.x),
        bbox_right: Math.max(0, bbox.x + bbox.width),
      };
    }
    case "simple-bbox-width":
    case "wrap-probe-bbox-width":
      return { kind: "length", length: Math.max(0, svgBBox(probes, request, "tspan").width) };
    case "raw-bbox-width":
      return { kind: "length", length: Math.max(0, svgBBox(probes, request, "direct").width) };
    case "raw-bbox-height":
      return { kind: "length", length: Math.max(0, svgBBox(probes, request, "direct").height) };
    case "bounding-client-rect-width": {
      const text = prepareSvgText(probes, request, "direct", "start");
      return { kind: "length", length: Math.max(0, text.getBoundingClientRect().width) };
    }
    case "tspan-bbox-width":
      return { kind: "length", length: Math.max(0, svgBBox(probes, request, "tspan").width) };
    case "tspan-bbox-height":
      return { kind: "length", length: Math.max(0, svgBBox(probes, request, "tspan").height) };
    case "create-text-bbox-y-offset":
      return { kind: "length", length: svgCreateTextBBoxYOffset(probes, request, false) };
    case "create-text-middle-bbox-y-offset":
      return { kind: "length", length: svgCreateTextBBoxYOffset(probes, request, true) };
    case "mermaid-calculate-text-dimensions":
      return mermaidCalculateTextDimensions(probes, request);
    case "canvas-measure-text-width":
      return canvasTextWidth(probes, request);
    case "simple-bbox-height":
      return { kind: "length", length: Math.max(0, svgBBox(probes, request, "tspan").height) };
    case "wrapped":
      return request.wrap_mode === "html-like"
        ? htmlWrappedMetrics(probes.html, request).metrics
        : svgWrappedMetrics(probes, request);
    case "wrapped-with-raw-width": {
      const measured = wrappedMetrics(probes, request);
      return {
        ...measured.metrics,
        kind: "wrapped-with-raw-width",
        raw_width: measured.rawWidth,
      };
    }
  }
}

function emptyMeasurement(request: HostTextMeasureRequest): HostTextMeasureResult {
  switch (request.operation) {
    case "bbox-x":
    case "bbox-x-with-ascii-overhang":
    case "title-bbox-x":
      return { kind: "horizontal-extents", bbox_left: 0, bbox_right: 0 };
    case "tspan-bbox-height":
    case "simple-bbox-height":
    case "raw-bbox-height":
    case "create-text-bbox-y-offset":
    case "create-text-middle-bbox-y-offset":
      return { kind: "length", length: 0 };
    case "measure":
    case "wrapped":
    case "mermaid-calculate-text-dimensions":
      return { kind: "metrics", width: 0, height: 0, line_count: 1 };
    case "wrapped-with-raw-width":
      return {
        kind: "wrapped-with-raw-width",
        width: 0,
        height: 0,
        line_count: 1,
        raw_width: 0,
      };
    default:
      return { kind: "length", length: 0 };
  }
}

function mermaidCalculateTextDimensions(
  probes: BrowserTextMeasureProbes,
  request: HostTextMeasureRequest
): HostTextMetricsResult {
  const text = probes.mermaidDimensionsText;
  const tspan = probes.mermaidDimensionsTspan;

  resetElement(text);
  resetElement(tspan);
  text.setAttribute("x", "0");
  text.setAttribute("y", "0");
  text.style.textAnchor = "start";
  text.style.fontSize = `${Number.isFinite(request.font_size) ? request.font_size : 12}px`;
  text.style.fontWeight = request.font_weight ?? "400";
  text.style.fontFamily = request.font_family ?? "Arial";
  tspan.setAttribute("x", "0");
  tspan.textContent = request.text;
  text.appendChild(tspan);
  const bbox = text.getBBox();
  if (bbox.width === 0 && bbox.height === 0) {
    throw new Error("svg element not in render tree");
  }
  return {
    kind: "metrics",
    width: Math.max(0, bbox.width),
    height: Math.max(0, bbox.height),
    line_count: 1,
  };
}

function resetElement(element: SVGElement): void {
  element.replaceChildren();
  element.style.cssText = "";
  for (const attributeName of element.getAttributeNames()) {
    element.removeAttribute(attributeName);
  }
}

function canvasTextWidth(
  probes: BrowserTextMeasureProbes,
  request: HostTextMeasureRequest
): HostTextMeasureResult | undefined {
  if (probes.canvasContext === undefined) {
    probes.canvasContext = document.createElement("canvas").getContext("2d");
  }
  const context = probes.canvasContext;
  if (!context || typeof context.measureText !== "function") {
    return undefined;
  }

  const fontStyle = request.font_style;
  const fontWeight = request.font_weight ?? "normal";
  const fontSize = Number.isFinite(request.font_size) ? request.font_size : 16;
  const fontFamily = request.font_family ?? "sans-serif";
  context.font = `${fontStyle} ${fontWeight} ${fontSize}px ${fontFamily}`;
  const width = context.measureText(request.text).width;
  if (!Number.isFinite(width)) {
    return undefined;
  }
  return { kind: "length", length: Math.max(0, width) };
}

function wrappedMetrics(
  probes: BrowserTextMeasureProbes,
  request: HostTextMeasureRequest
): { metrics: HostTextMetricsResult; rawWidth: number } {
  if (request.wrap_mode === "html-like") {
    return htmlWrappedMetrics(probes.html, request);
  }

  const rawWidth = Math.max(0, svgBBox(probes, request, "tspan").width);
  return {
    metrics: svgWrappedMetrics(probes, request),
    rawWidth,
  };
}

function htmlWrappedMetrics(
  probe: HTMLDivElement,
  request: HostTextMeasureRequest
): { metrics: HostTextMetricsResult; rawWidth: number } {
  applyHtmlTextMeasureStyle(probe, request);
  const natural = measureHtmlProbe(probe, request.text, {
    display: "inline-block",
    width: "auto",
    maxWidth: "none",
    whiteSpace: "nowrap",
  });
  const maxWidth = normalizeMeasureMaxWidth(request);
  if (maxWidth === null || natural.width <= maxWidth) {
    return { metrics: natural, rawWidth: natural.width };
  }

  return {
    metrics: measureHtmlProbe(probe, request.text, {
      display: "table",
      width: `${maxWidth}px`,
      maxWidth: `${maxWidth}px`,
      whiteSpace: "break-spaces",
    }),
    rawWidth: natural.width,
  };
}

function svgWrappedMetrics(
  probes: BrowserTextMeasureProbes,
  request: HostTextMeasureRequest
): HostTextMetricsResult {
  const maxWidth = normalizeMeasureMaxWidth(request);
  const breakLongWords = request.wrap_mode === "svg-like";
  const lines = splitExplicitLines(request.text).flatMap((line) =>
    maxWidth === null
      ? [line]
      : wrapSvgLine(probes, request, line, maxWidth, breakLongWords)
  );

  applySvgTextStyle(probes.wrappedText, request);
  probes.wrappedText.setAttribute("x", "0");
  probes.wrappedText.setAttribute("y", "0");
  probes.wrappedText.setAttribute("text-anchor", "start");
  probes.wrappedText.textContent = "";
  for (const [index, line] of lines.entries()) {
    const tspan = document.createElementNS(SVG_NAMESPACE, "tspan");
    tspan.setAttribute("x", "0");
    tspan.setAttribute("dy", index === 0 ? "0" : `${Math.max(1, request.line_height)}px`);
    tspan.textContent = line || "\u200b";
    probes.wrappedText.appendChild(tspan);
  }
  const bbox = probes.wrappedText.getBBox();
  return {
    kind: "metrics",
    width: Math.max(0, bbox.width),
    height: Math.max(0, bbox.height),
    line_count: Math.max(1, lines.length),
  };
}

function svgCreateTextBBoxYOffset(
  probes: BrowserTextMeasureProbes,
  request: HostTextMeasureRequest,
  middleBaseline: boolean
): number {
  const group = probes.formattedTextGroup;
  const text = probes.formattedText;
  for (const attribute of ["dy", "alignment-baseline", "dominant-baseline", "text-anchor"]) {
    group.removeAttribute(attribute);
  }
  if (middleBaseline) {
    group.setAttribute("dy", "1em");
    group.setAttribute("alignment-baseline", "middle");
    group.setAttribute("dominant-baseline", "middle");
    group.setAttribute("text-anchor", "middle");
  }
  applySvgTextStyle(text, request);
  text.removeAttribute("x");
  text.removeAttribute("text-anchor");
  text.setAttribute("y", "-10.1");
  text.textContent = "";

  const tspan = document.createElementNS(SVG_NAMESPACE, "tspan");
  tspan.setAttribute("x", "0");
  tspan.setAttribute("y", "-0.1em");
  tspan.setAttribute("dy", "1.1em");
  tspan.textContent = request.text || "\u200b";
  text.appendChild(tspan);
  return text.getBBox().y;
}

function splitExplicitLines(text: string): string[] {
  const lines = text.split(/(?:<br\s*\/?>|\r?\n)/gi);
  return lines.length === 0 ? [""] : lines;
}

function splitTextToGraphemes(text: string): string[] {
  if (typeof Intl.Segmenter === "function") {
    return [...new Intl.Segmenter().segment(text)].map(({ segment }) => segment);
  }
  return Array.from(text);
}

function wrapSvgLine(
  probes: BrowserTextMeasureProbes,
  request: HostTextMeasureRequest,
  line: string,
  maxWidth: number,
  breakLongWords: boolean
): string[] {
  const words = line.trim().split(/\s+/u).filter(Boolean);
  if (words.length === 0) {
    return [""];
  }

  const lines: string[] = [];
  let current = "";
  for (const word of words) {
    const candidate = current ? `${current} ${word}` : word;
    if (svgTextWidth(probes, request, candidate) <= maxWidth) {
      current = candidate;
      continue;
    }
    if (current) {
      lines.push(current);
      current = "";
    }
    if (!breakLongWords || svgTextWidth(probes, request, word) <= maxWidth) {
      current = word;
      continue;
    }

    let segment = "";
    for (const character of splitTextToGraphemes(word)) {
      const candidateSegment = `${segment}${character}`;
      if (segment && svgTextWidth(probes, request, candidateSegment) > maxWidth) {
        lines.push(segment);
        segment = character;
      } else {
        segment = candidateSegment;
      }
    }
    current = segment;
  }
  if (current) {
    lines.push(current);
  }
  return lines.length === 0 ? [""] : lines;
}

function svgTextWidth(
  probes: BrowserTextMeasureProbes,
  request: HostTextMeasureRequest,
  text: string
): number {
  prepareSvgText(probes, { ...request, text }, "tspan", "start");
  return Math.max(0, probes.tspanText.getBBox().width);
}

function svgMetrics(
  probes: BrowserTextMeasureProbes,
  request: HostTextMeasureRequest,
  shape: SvgProbeShape
): HostTextMetricsResult {
  const bbox = svgBBox(probes, request, shape);
  return {
    kind: "metrics",
    width: Math.max(0, bbox.width),
    height: Math.max(0, bbox.height),
    line_count: 1,
  };
}

function svgBBox(
  probes: BrowserTextMeasureProbes,
  request: HostTextMeasureRequest,
  shape: SvgProbeShape,
  anchor: "start" | "middle" = "start"
): DOMRect {
  const element = prepareSvgText(probes, request, shape, anchor);
  return element.getBBox() as DOMRect;
}

function prepareSvgText(
  probes: BrowserTextMeasureProbes,
  request: HostTextMeasureRequest,
  shape: SvgProbeShape,
  anchor: "start" | "middle"
): SVGTextElement {
  const element = shape === "direct" ? probes.directText : probes.tspanText;
  applySvgTextStyle(element, request);
  element.setAttribute("x", "0");
  element.setAttribute("y", "0");
  element.setAttribute("text-anchor", anchor);
  if (shape === "direct") {
    element.textContent = request.text;
  } else {
    probes.tspan.textContent = request.text;
  }
  return element;
}

function applyHtmlTextMeasureStyle(
  probe: HTMLDivElement,
  request: HostTextMeasureRequest
) {
    const style = probe.style;
    style.fontFamily = request.font_family || "sans-serif";
    style.fontSize = `${Math.max(1, request.font_size)}px`;
    style.fontWeight = request.font_weight || "normal";
    style.fontStyle = request.font_style || "normal";
    style.lineHeight = `${Math.max(1, request.line_height || request.font_size)}px`;
    style.letterSpacing = `${request.letter_spacing || 0}px`;
    style.wordSpacing = `${request.word_spacing || 0}px`;
    probe.dir = request.direction;
    style.direction = request.direction === "auto" ? "" : request.direction;
}

function applySvgTextStyle(element: SVGTextElement, request: HostTextMeasureRequest) {
  const style = element.style;
  style.fontFamily = request.font_family || "sans-serif";
  style.fontSize = `${Math.max(1, request.font_size)}px`;
  style.fontWeight = request.font_weight || "normal";
  style.fontStyle = request.font_style || "normal";
  style.letterSpacing = `${request.letter_spacing || 0}px`;
  style.wordSpacing = `${request.word_spacing || 0}px`;
  style.direction = request.direction === "auto" ? "" : request.direction;
  style.whiteSpace = request.white_space;
}

function measureHtmlProbe(
  probe: HTMLDivElement,
  text: string,
  styleOverride: Pick<
    CSSStyleDeclaration,
    "display" | "width" | "maxWidth" | "whiteSpace"
  >
): HostTextMetricsResult {
  probe.style.display = styleOverride.display;
  probe.style.width = styleOverride.width;
  probe.style.maxWidth = styleOverride.maxWidth;
  probe.style.whiteSpace = styleOverride.whiteSpace;
  setHtmlProbeText(probe, text);
  const rect = probe.getBoundingClientRect();
  const lineHeight = Math.max(1, parseFloat(probe.style.lineHeight) || 1);
  const height = Math.max(lineHeight, rect.height);
  return {
    kind: "metrics",
    width: Math.max(0, rect.width),
    height,
    line_count: Math.max(1, Math.round(height / lineHeight)),
  };
}

function setHtmlProbeText(probe: HTMLDivElement, text: string): void {
  const lines = splitExplicitLines(text);
  if (lines.length === 1) {
    probe.textContent = lines[0];
    return;
  }

  // Mermaid 11.16 measures the sanitized HTML produced by `createText.ts:addHtmlSpan`, where
  // explicit line breaks are real `<br>` elements. Assigning the normalized `\n` carrier to
  // `textContent` under `white-space: nowrap` would collapse it before the natural-size check.
  const children: Node[] = [];
  for (const [index, line] of lines.entries()) {
    if (index > 0) {
      children.push(document.createElement("br"));
    }
    children.push(document.createTextNode(line));
  }
  probe.replaceChildren(...children);
}

function normalizeMeasureMaxWidth(
  request: HostTextMeasureRequest
): number | null {
  if (
    !request.has_max_width ||
    typeof request.max_width !== "number" ||
    !Number.isFinite(request.max_width) ||
    request.max_width <= 0
  ) {
    return null;
  }
  return request.max_width;
}

function createTextMeasureProbes(): BrowserTextMeasureProbes | null {
  if (
    typeof document === "undefined" ||
    !document.body ||
    typeof document.createElementNS !== "function"
  ) {
    return null;
  }

  let html: HTMLDivElement | null = null;
  let svg: SVGSVGElement | null = null;
  let mermaidDimensionsSvg: SVGSVGElement | null = null;

  try {
    html = document.createElement("div");
    html.setAttribute("aria-hidden", "true");
    html.setAttribute("data-merman-text-measure-probe", "html");
    Object.assign(html.style, {
      position: "fixed",
      left: "-10000px",
      top: "-10000px",
      visibility: "hidden",
      contain: "layout style paint",
      boxSizing: "border-box",
      padding: "0",
      margin: "0",
      border: "0",
      display: "block",
    });
    document.body.appendChild(html);

    svg = document.createElementNS(SVG_NAMESPACE, "svg") as SVGSVGElement;
    svg.setAttribute("aria-hidden", "true");
    svg.setAttribute("data-merman-text-measure-probe", "svg");
    svg.setAttribute("width", "0");
    svg.setAttribute("height", "0");
    Object.assign(svg.style, {
      position: "fixed",
      left: "-10000px",
      top: "-10000px",
      visibility: "hidden",
      overflow: "visible",
    });
    const directText = document.createElementNS(SVG_NAMESPACE, "text") as SVGTextElement;
    const tspanText = document.createElementNS(SVG_NAMESPACE, "text") as SVGTextElement;
    const tspan = document.createElementNS(SVG_NAMESPACE, "tspan") as SVGTSpanElement;
    const wrappedText = document.createElementNS(SVG_NAMESPACE, "text") as SVGTextElement;
    const formattedTextGroup = document.createElementNS(SVG_NAMESPACE, "g") as SVGGElement;
    const formattedText = document.createElementNS(SVG_NAMESPACE, "text") as SVGTextElement;
    formattedTextGroup.appendChild(formattedText);
    tspanText.appendChild(tspan);
    svg.appendChild(directText);
    svg.appendChild(tspanText);
    svg.appendChild(wrappedText);
    svg.appendChild(formattedTextGroup);
    document.body.appendChild(svg);

    mermaidDimensionsSvg = document.createElementNS(
      SVG_NAMESPACE,
      "svg"
    ) as SVGSVGElement;
    mermaidDimensionsSvg.setAttribute("aria-hidden", "true");
    mermaidDimensionsSvg.setAttribute(
      "data-merman-text-measure-probe",
      "mermaid-dimensions"
    );
    mermaidDimensionsSvg.setAttribute("width", "0");
    mermaidDimensionsSvg.setAttribute("height", "0");
    Object.assign(mermaidDimensionsSvg.style, {
      position: "fixed",
      left: "-10000px",
      top: "-10000px",
      visibility: "hidden",
      overflow: "visible",
    });
    const mermaidDimensionsText = document.createElementNS(
      SVG_NAMESPACE,
      "text"
    ) as SVGTextElement;
    const mermaidDimensionsTspan = document.createElementNS(
      SVG_NAMESPACE,
      "tspan"
    ) as SVGTSpanElement;
    mermaidDimensionsText.appendChild(mermaidDimensionsTspan);
    mermaidDimensionsSvg.appendChild(mermaidDimensionsText);
    document.body.appendChild(mermaidDimensionsSvg);

    return {
      html,
      svg,
      directText,
      tspanText,
      tspan,
      wrappedText,
      formattedTextGroup,
      formattedText,
      mermaidDimensionsSvg,
      mermaidDimensionsText,
      mermaidDimensionsTspan,
    };
  } catch (error) {
    removeTextMeasureProbe(mermaidDimensionsSvg);
    removeTextMeasureProbe(svg);
    removeTextMeasureProbe(html);
    throw error;
  }
}

function disposeTextMeasureProbes(probes: BrowserTextMeasureProbes): void {
  probes.canvasContext = null;
  removeTextMeasureProbe(probes.mermaidDimensionsSvg);
  removeTextMeasureProbe(probes.svg);
  removeTextMeasureProbe(probes.html);
}

function removeTextMeasureProbe(probe: Element | null): void {
  try {
    probe?.remove();
  } catch {
    // Disposal is best-effort for a realm that may already be tearing down.
  }
}

export function renderSvgElement(
  source: string,
  options?: SvgBindingOptions | string
): SVGSVGElement {
  if (typeof DOMParser === "undefined" || typeof document === "undefined") {
    throw new Error("renderSvgElement() requires a browser DOM.");
  }

  const svgText = renderSvg(source, options);
  assertSafeSvgForDom(svgText);
  const parsed = new DOMParser().parseFromString(svgText, "image/svg+xml");
  const parseError = parsed.querySelector("parsererror");
  if (parseError) {
    throw new Error(parseError.textContent || "Merman rendered invalid SVG.");
  }

  const svg = parsed.documentElement;
  if (svg.localName !== "svg") {
    throw new Error("Merman render output did not contain an SVG root element.");
  }
  return document.importNode(svg, true) as unknown as SVGSVGElement;
}

export function renderSvgToElement(
  target: Element,
  source: string,
  options?: SvgBindingOptions | string
): SVGSVGElement {
  const svg = renderSvgElement(source, options);
  target.replaceChildren(svg);
  return svg;
}

export function parseJson(source: string, options?: SvgBindingOptions | string): string {
  return getMerman().parseJson(source, encodeOptions(options));
}

export function parseObject<T = unknown>(source: string, options?: SvgBindingOptions | string): T {
  return JSON.parse(parseJson(source, options)) as T;
}

export function layoutJson(source: string, options?: SvgBindingOptions | string): string {
  return getMerman().layoutJson(source, encodeOptions(options));
}

export function layoutObject<T = unknown>(source: string, options?: SvgBindingOptions | string): T {
  return JSON.parse(layoutJson(source, options)) as T;
}
