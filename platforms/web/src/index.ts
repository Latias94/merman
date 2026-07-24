import {
  createMermanRuntimeState,
  currentMermanRuntimeState,
  type MermanRuntimeState,
  withMermanRuntimeState,
} from "./runtime-state.js";
import { assertSafeSvgForDom } from "./svg-safety.js";
import {
  validatePackedSemanticTokens,
  validateSemanticTokenDescriptor,
} from "./editor-semantic-tokens.js";
import {
  MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION,
} from "./generated/text-measurement-abi.js";

import {
  isAsciiDiagramType,
  isDiagramType,
  isHostThemePresetName,
  isThemeName,
} from "./public-catalog.js";
import type {
  AsciiCapability,
  AsciiCapabilityEvidence,
  AsciiDiagramType,
  AsciiSupportLevel,
  DiagramFamilyCapability,
  DiagramType,
  HostThemePresetName,
  LintRuleCatalogEntry,
  LintRuleCatalogResponse,
  RuntimeCapabilities,
  TextMeasurementCapabilities,
  ThemeName,
} from "./public-catalog.js";
import type {
  AnalysisFactsResult,
  AnalysisResult,
  AsciiBindingOptions,
  BrowserEditorSession,
  BrowserTextMeasurementSession,
  CommonBindingOptions,
  DiagramDetectionFacts,
  EditorCodeAction,
  EditorCompletionList,
  EditorDiagnosticsResult,
  EditorDocumentSymbol,
  EditorHover,
  EditorLocation,
  EditorPosition,
  EditorPrepareRename,
  EditorSemanticTokenDescriptor,
  EditorSymbolInformation,
  EditorWorkspaceEdit,
  HostTextMeasureRequest,
  HostTextMeasureResult,
  HostTextMetricsResult,
  HostTextMeasurer,
  MermanInitInput,
  MermanWasmModule,
  RuntimeCatalog,
  ResourceOptions,
  SvgBindingOptions,
  UnavailableDiagramDetectionFacts,
  ValidationResult,
  WasmEditorSessionBinding,
} from "./public-types.js";

export {
  BINDING_STATUS_CODE_NAMES,
  SYSTEM_ADAPTER_IDS,
  TEXT_MEASUREMENT_PROVIDER_IDS,
  WEB_CAPABILITIES,
  WEB_CAPABILITY_IDS,
  WEB_OUTPUT_IDS,
  WEB_OUTPUTS,
  SUPPORTED_ASCII_DIAGRAMS,
  SUPPORTED_DIAGRAMS,
  SUPPORTED_HOST_THEME_PRESETS,
  SUPPORTED_THEMES,
  isAsciiDiagramType,
  isBindingErrorPayload,
  isBindingStatusCodeName,
  isDiagramType,
  isHostThemePresetName,
  isThemeName,
  normalizeHostThemePresetName,
  normalizeThemeName,
} from "./public-catalog.js";
export type * from "./public-catalog.js";
export type * from "./public-types.js";
export {
  MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION,
} from "./generated/text-measurement-abi.js";
export {
  RESOURCE_LIMIT_IDS,
  RESOURCE_PROFILES,
  rawResourceOptionsJson,
  resourceOptions,
  resourceOptionsJson,
} from "./generated/resource-contract.js";
export type {
  RawResourceOptions,
  ResourceLimitId,
  ResourceLimitOverrides,
  ResourceOptions,
  ResourceProfile,
} from "./generated/resource-contract.js";
export {
  SEMANTIC_TOKEN_DESCRIPTOR,
  SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
  SEMANTIC_TOKEN_MODIFIER_LSP_NAMES,
  SEMANTIC_TOKEN_RECORD_WIDTH,
  SEMANTIC_TOKEN_TYPE_LSP_NAMES,
  SEMANTIC_TOKEN_VALID_MODIFIER_MASK,
  SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX,
} from "./generated/token-descriptor.js";
export type {
  SemanticTokenModifierIndex,
  SemanticTokenTypeCode,
} from "./generated/token-descriptor.js";
export { assertSafeSvgForDom } from "./svg-safety.js";

const defaultRuntimeState = createMermanRuntimeState(defaultLoader);

export function initMerman(init?: MermanInitInput): Promise<MermanWasmModule> {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  if (state.wasmModule) {
    return Promise.resolve(state.wasmModule);
  }
  if (state.initPromise) {
    return state.initPromise;
  }
  state.initPromise = doInit(state, init).catch((error) => {
    state.initPromise = null;
    throw error;
  });
  return state.initPromise;
}

async function doInit(
  state: MermanRuntimeState,
  init?: MermanInitInput
): Promise<MermanWasmModule> {
  const loader = typeof init === "function" ? init : init?.loader;
  const wasm = typeof init === "function" ? undefined : init?.wasm;
  const module = loader ? await loader() : await state.defaultLoader();
  if (wasm === undefined) {
    await module.default();
  } else {
    await module.default({ module_or_path: wasm });
  }
  state.wasmModule = module;
  return module;
}

async function defaultLoader(): Promise<MermanWasmModule> {
  throw new Error(
    "The shared @mermanjs/web implementation has no WASM artifact. Import one browser package entry such as @mermanjs/web, @mermanjs/web-analysis, @mermanjs/web-editor, or @mermanjs/web-ascii."
  );
}

export function getMerman(): MermanWasmModule {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  if (!state.wasmModule) {
    throw new Error("Merman WASM is not initialized. Call initMerman() first.");
  }
  return state.wasmModule;
}

export function isMermanInitialized(): boolean {
  return currentMermanRuntimeState(defaultRuntimeState).wasmModule !== null;
}

export function renderSvg(source: string, options?: SvgBindingOptions | string): string {
  return getMerman().renderSvg(source, encodeOptions(options));
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
  probe.textContent = text;
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

export function renderAscii(source: string, options?: AsciiBindingOptions | string): string {
  return getMerman().renderAscii(source, encodeOptions(options));
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

export function analyze(source: string, options?: SvgBindingOptions | string): AnalysisResult {
  const merman = getMerman();
  const encodedOptions = encodeOptions(options);
  const analysis =
    merman.analyze?.(source, encodedOptions) ?? merman.analyzeJson?.(source, encodedOptions);
  if (!analysis) {
    throw new Error("Merman analyze() is not available in this artifact.");
  }
  return analysis;
}

export function analyzeJson(
  source: string,
  options?: SvgBindingOptions | string
): AnalysisResult {
  return analyze(source, options);
}

export function analysisFacts(
  source: string,
  options?: SvgBindingOptions | string
): AnalysisFactsResult {
  const merman = getMerman();
  const facts = merman.analysisFacts;
  if (!facts) {
    throw new Error("Merman analysisFacts() is not available in this artifact.");
  }
  return facts(source, encodeOptions(options));
}

export const UNAVAILABLE_DIAGRAM_DETECTION: UnavailableDiagramDetectionFacts = Object.freeze({
  status: "unavailable",
  validity: "unknown",
  diagramType: null,
  syntaxId: null,
  effectiveLayoutId: null,
});

export function detectDiagramFacts(
  source: string,
  options?: SvgBindingOptions | string
): DiagramDetectionFacts {
  try {
    const facts: unknown = analysisFacts(source, options);
    if (
      !isRecord(facts) ||
      facts.version !== 1 ||
      typeof facts.valid !== "boolean"
    ) {
      return UNAVAILABLE_DIAGRAM_DETECTION;
    }

    const diagrams = facts.diagrams;
    if (!Array.isArray(diagrams) || diagrams.length !== 1 || !isRecord(diagrams[0])) {
      return UNAVAILABLE_DIAGRAM_DETECTION;
    }

    const syntax = diagrams[0].syntax;
    if (!isRecord(syntax)) {
      return UNAVAILABLE_DIAGRAM_DETECTION;
    }

    const syntaxId = syntax.diagram_type;
    const effectiveLayoutId = syntax.effective_layout;
    if (
      typeof syntaxId !== "string" ||
      syntaxId.trim().length === 0 ||
      typeof effectiveLayoutId !== "string" ||
      effectiveLayoutId.trim().length === 0
    ) {
      return UNAVAILABLE_DIAGRAM_DETECTION;
    }

    const diagramType = diagramMetadataBySyntaxId().get(syntaxId);
    if (diagramType == null) {
      return UNAVAILABLE_DIAGRAM_DETECTION;
    }

    return Object.freeze({
      status: "available",
      validity: facts.valid ? "valid" : "recoverable-invalid",
      diagramType,
      syntaxId,
      effectiveLayoutId,
    });
  } catch {
    return UNAVAILABLE_DIAGRAM_DETECTION;
  }
}

export function analyzeDocument(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): AnalysisResult {
  const analyzeDocument = getMerman().analyzeDocument;
  if (!analyzeDocument) {
    throw new Error("Merman analyzeDocument() is not available in this artifact.");
  }
  return analyzeDocument(source, encodeOptions(options), uri);
}

export function analyzeDocumentFacts(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): AnalysisFactsResult {
  const analyzeDocumentFacts = getMerman().analyzeDocumentFacts;
  if (!analyzeDocumentFacts) {
    throw new Error("Merman analyzeDocumentFacts() is not available in this artifact.");
  }
  return analyzeDocumentFacts(source, encodeOptions(options), uri);
}

export function validate(source: string, options?: SvgBindingOptions | string): ValidationResult {
  return getMerman().validate(source, encodeOptions(options));
}

export function createEditorSession(
  source: string,
  version: number,
  uri?: string,
  options?: SvgBindingOptions | string
): BrowserEditorSession {
  const runtimeState = currentMermanRuntimeState(defaultRuntimeState);
  const EditorSession = requireEditorLanguage(
    "createEditorSession",
    getMerman().EditorSession
  );
  const native = new EditorSession(source, version, uri, encodeOptions(options));
  return new BrowserEditorSessionImpl(native, runtimeState);
}

class BrowserEditorSessionImpl implements BrowserEditorSession {
  private native: WasmEditorSessionBinding | null;

  constructor(
    native: WasmEditorSessionBinding,
    private readonly runtimeState: MermanRuntimeState
  ) {
    this.native = native;
  }

  get version(): number {
    return this.withNative((native) => native.version);
  }

  get uri(): string {
    return this.withNative((native) => native.uri);
  }

  update(source: string, version: number): void {
    this.withNative((native) => native.update(source, version));
  }

  diagnostics(): EditorDiagnosticsResult {
    return this.withNative((native) => native.diagnostics());
  }

  diagramDetection(): DiagramDetectionFacts {
    return this.withNative((native) =>
      validateEditorDiagramDetection(native.diagramDetection())
    );
  }

  codeActions(): EditorCodeAction[] {
    return this.withNative((native) => native.codeActions());
  }

  completions(position: EditorPosition): EditorCompletionList {
    return this.withNative((native) =>
      native.completions(position.line, position.character)
    );
  }

  hover(position: EditorPosition): EditorHover | null {
    return this.withNative((native) => native.hover(position.line, position.character));
  }

  documentSymbols(): EditorDocumentSymbol[] {
    return this.withNative((native) => native.documentSymbols());
  }

  workspaceSymbols(query: string): EditorSymbolInformation[] {
    return this.withNative((native) => native.workspaceSymbols(query));
  }

  definition(position: EditorPosition): EditorLocation | null {
    return this.withNative((native) =>
      native.definition(position.line, position.character)
    );
  }

  references(
    position: EditorPosition,
    includeDeclaration = true
  ): EditorLocation[] {
    return this.withNative((native) =>
      native.references(position.line, position.character, includeDeclaration)
    );
  }

  prepareRename(position: EditorPosition): EditorPrepareRename | null {
    return this.withNative((native) =>
      native.prepareRename(position.line, position.character)
    );
  }

  rename(
    position: EditorPosition,
    newName: string
  ): EditorWorkspaceEdit | null {
    return this.withNative((native) =>
      native.rename(position.line, position.character, newName)
    );
  }

  semanticTokens(): Uint32Array {
    return this.withNative((native) => {
      cachedEditorSemanticTokenDescriptor();
      return validatePackedSemanticTokens(native.semanticTokens());
    });
  }

  dispose(): void {
    const native = this.native;
    if (!native) return;
    this.native = null;
    withMermanRuntimeState(this.runtimeState, () => native.free());
  }

  private withNative<T>(run: (native: WasmEditorSessionBinding) => T): T {
    const native = this.native;
    if (!native) {
      throw new Error("Merman editor session is disposed.");
    }
    return withMermanRuntimeState(this.runtimeState, () => run(native));
  }
}

export function editorDiagnostics(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): EditorDiagnosticsResult {
  const diagnostics = requireEditorLanguage("editorDiagnostics", getMerman().editorDiagnostics);
  return diagnostics(source, encodeOptions(options), uri);
}

export function editorDiagramDetection(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): DiagramDetectionFacts {
  const detection = requireEditorLanguage(
    "editorDiagramDetection",
    getMerman().editorDiagramDetection
  );
  return validateEditorDiagramDetection(detection(source, encodeOptions(options), uri));
}

export function editorCodeActions(
  source: string,
  options?: SvgBindingOptions | string,
  uri?: string
): EditorCodeAction[] {
  const codeActions = requireEditorLanguage("editorCodeActions", getMerman().editorCodeActions);
  return codeActions(source, encodeOptions(options), uri);
}

export function editorCompletions(
  source: string,
  position: EditorPosition,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorCompletionList {
  const completions = requireEditorLanguage("editorCompletions", getMerman().editorCompletions);
  return completions(source, position.line, position.character, uri, encodeOptions(options));
}

export function editorHover(
  source: string,
  position: EditorPosition,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorHover | null {
  const hover = requireEditorLanguage("editorHover", getMerman().editorHover);
  return hover(source, position.line, position.character, uri, encodeOptions(options));
}

export function editorDocumentSymbols(
  source: string,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorDocumentSymbol[] {
  const documentSymbols = requireEditorLanguage(
    "editorDocumentSymbols",
    getMerman().editorDocumentSymbols
  );
  return documentSymbols(source, uri, encodeOptions(options));
}

export function editorWorkspaceSymbols(
  source: string,
  query: string,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorSymbolInformation[] {
  const workspaceSymbols = requireEditorLanguage(
    "editorWorkspaceSymbols",
    getMerman().editorWorkspaceSymbols
  );
  return workspaceSymbols(source, query, uri, encodeOptions(options));
}

export function editorDefinition(
  source: string,
  position: EditorPosition,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorLocation | null {
  const definition = requireEditorLanguage("editorDefinition", getMerman().editorDefinition);
  return definition(source, position.line, position.character, uri, encodeOptions(options));
}

export function editorReferences(
  source: string,
  position: EditorPosition,
  includeDeclaration = true,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorLocation[] {
  const refs = requireEditorLanguage("editorReferences", getMerman().editorReferences);
  return refs(source, position.line, position.character, includeDeclaration, uri, encodeOptions(options));
}

export function editorPrepareRename(
  source: string,
  position: EditorPosition,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorPrepareRename | null {
  const prepare = requireEditorLanguage("editorPrepareRename", getMerman().editorPrepareRename);
  return prepare(source, position.line, position.character, uri, encodeOptions(options));
}

export function editorRename(
  source: string,
  position: EditorPosition,
  newName: string,
  uri?: string,
  options?: SvgBindingOptions | string
): EditorWorkspaceEdit | null {
  const rename = requireEditorLanguage("editorRename", getMerman().editorRename);
  return rename(source, position.line, position.character, newName, uri, encodeOptions(options));
}

export function editorSemanticTokenDescriptor(): EditorSemanticTokenDescriptor {
  return cloneSemanticTokenDescriptor(cachedEditorSemanticTokenDescriptor());
}

function cachedEditorSemanticTokenDescriptor(): EditorSemanticTokenDescriptor {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  if (state.editorSemanticTokenDescriptorCache) {
    return state.editorSemanticTokenDescriptorCache;
  }
  const descriptor = requireEditorLanguage(
    "editorSemanticTokenDescriptor",
    getMerman().editorSemanticTokenDescriptor
  );
  state.editorSemanticTokenDescriptorCache = validateSemanticTokenDescriptor(descriptor());
  return state.editorSemanticTokenDescriptorCache;
}

function cloneSemanticTokenDescriptor(
  descriptor: EditorSemanticTokenDescriptor
): EditorSemanticTokenDescriptor {
  return {
    ...descriptor,
    renamePolicies: [...descriptor.renamePolicies],
    tokenTypes: descriptor.tokenTypes.map((tokenType) => ({ ...tokenType })),
    modifiers: descriptor.modifiers.map((modifier) => ({ ...modifier })),
    packed: {
      ...descriptor.packed,
      fieldOrder: [...descriptor.packed.fieldOrder],
    },
    overlayPrecedence: descriptor.overlayPrecedence.map((entry) => ({ ...entry })),
    tokenTypeLspNames: [...descriptor.tokenTypeLspNames],
    modifierLspNames: [...descriptor.modifierLspNames],
  } as unknown as EditorSemanticTokenDescriptor;
}

export function editorSemanticTokens(
  source: string,
  uri?: string,
  options?: SvgBindingOptions | string
): Uint32Array {
  cachedEditorSemanticTokenDescriptor();
  const tokens = requireEditorLanguage("editorSemanticTokens", getMerman().editorSemanticTokens);
  return validatePackedSemanticTokens(tokens(source, uri, encodeOptions(options)));
}

export function runtimeCatalog(): RuntimeCatalog {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  state.runtimeCatalogCache ??= normalizeRuntimeCatalog(getMerman().runtimeCatalog());
  return structuredCloneValue(state.runtimeCatalogCache);
}

export function supportedDiagrams(): DiagramType[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  state.supportedDiagramsCache ??= getMerman().supportedDiagrams().map(assertDiagramType);
  return [...state.supportedDiagramsCache];
}

export function diagramFamilyCapabilities(): DiagramFamilyCapability[] {
  return cachedDiagramFamilyCapabilities().map((capability) => ({ ...capability }));
}

function cachedDiagramFamilyCapabilities(): readonly DiagramFamilyCapability[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  state.diagramFamilyCapabilitiesCache ??= getMerman()
    .diagramFamilyCapabilities()
    .map(normalizeDiagramFamilyCapability);
  return state.diagramFamilyCapabilitiesCache;
}

function diagramMetadataBySyntaxId(): ReadonlyMap<string, DiagramType | null> {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  if (state.diagramMetadataBySyntaxIdCache) {
    return state.diagramMetadataBySyntaxIdCache;
  }

  const index = new Map<string, DiagramType | null>();
  for (const capability of cachedDiagramFamilyCapabilities()) {
    const syntaxId = capability.diagram_type;
    if (index.has(syntaxId)) {
      index.set(syntaxId, null);
    } else {
      index.set(syntaxId, capability.metadata_id);
    }
  }
  state.diagramMetadataBySyntaxIdCache = index;
  return index;
}

export function lintRuleCatalog(): LintRuleCatalogEntry[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  const response = getMerman().lintRuleCatalog?.();
  if (!response) {
    throw new Error("Merman lintRuleCatalog() is not available in this artifact.");
  }
  state.lintRuleCatalogCache ??= normalizeLintRuleCatalogResponse(response);
  return state.lintRuleCatalogCache.map((rule) => ({
    ...rule,
    evidence: [...rule.evidence],
  }));
}

export function asciiSupportedDiagrams(): AsciiDiagramType[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  state.asciiSupportedDiagramsCache ??= getMerman()
    .asciiSupportedDiagrams()
    .map(assertAsciiDiagramType);
  return [...state.asciiSupportedDiagramsCache];
}

export function asciiCapabilities(): AsciiCapability[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  state.asciiCapabilitiesCache ??= getMerman().asciiCapabilities().map(normalizeAsciiCapability);
  return state.asciiCapabilitiesCache.map((capability) => ({
    ...capability,
    supported_semantics: [...capability.supported_semantics],
    limits: [...capability.limits],
    evidence: capability.evidence.map((evidence) => ({ ...evidence })),
  }));
}

export function supportedThemes(): ThemeName[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  state.supportedThemesCache ??= getMerman().supportedThemes().map(assertThemeName);
  return [...state.supportedThemesCache];
}

export function supportedHostThemePresets(): HostThemePresetName[] {
  const state = currentMermanRuntimeState(defaultRuntimeState);
  state.supportedHostThemePresetsCache ??= getMerman()
    .supportedHostThemePresets()
    .map(assertHostThemePresetName);
  return [...state.supportedHostThemePresetsCache];
}

export function transportApiVersion(): number {
  return assertSafeIntegerField(
    getMerman().transportApiVersion(),
    "Web transport API version",
    1
  );
}

export function packageVersion(): string {
  return getMerman().packageVersion();
}

export function encodeOptions(
  options?: CommonBindingOptions | string
): string | undefined {
  if (options === undefined) {
    return undefined;
  }
  return typeof options === "string" ? options : JSON.stringify(options);
}

export function withResourceOptions<T extends CommonBindingOptions>(
  options: T,
  resources: ResourceOptions,
): T {
  const result: CommonBindingOptions = { ...options };
  let hasWrapper = false;
  for (const key of ["analysis", "merman"] as const) {
    if (Object.prototype.hasOwnProperty.call(options, key)) {
      hasWrapper = true;
      const wrapper = options[key];
      if (wrapper !== undefined && isRecord(wrapper)) {
        result[key] = { ...wrapper, resources };
      }
    }
  }
  if (!hasWrapper) {
    result.resources = resources;
  }
  return result as T;
}

function assertDiagramType(diagram: string): DiagramType {
  if (isDiagramType(diagram)) {
    return diagram;
  }
  throw new Error(`Merman WASM returned unknown diagram type: ${diagram}`);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function assertAsciiDiagramType(diagram: string): AsciiDiagramType {
  if (isAsciiDiagramType(diagram)) {
    return diagram;
  }
  throw new Error(`Merman WASM returned unknown ASCII diagram type: ${diagram}`);
}

function normalizeDiagramFamilyCapability(
  capability: DiagramFamilyCapability
): DiagramFamilyCapability {
  if (!capability || typeof capability !== "object") {
    throw new Error("Merman WASM returned an invalid diagram family capability.");
  }
  if (typeof capability.diagram_type !== "string") {
    throw new Error("Merman WASM returned an invalid diagram family capability.");
  }
  const metadataId =
    capability.metadata_id === undefined || capability.metadata_id === null
      ? null
      : assertDiagramType(String(capability.metadata_id));
  return {
    diagram_type: capability.diagram_type,
    logical_family_kind: assertStringField(
      capability.logical_family_kind,
      "diagram logical family kind"
    ),
    metadata_id: metadataId,
    render_model_kind: assertNullableStringField(
      capability.render_model_kind,
      "diagram render model kind"
    ),
    has_detector: assertBooleanField(capability.has_detector, "diagram detector capability"),
    has_semantic_parser: assertBooleanField(
      capability.has_semantic_parser,
      "diagram semantic parser capability"
    ),
    has_editor_parser: assertBooleanField(
      capability.has_editor_parser,
      "diagram editor parser capability"
    ),
    has_combined_parser: assertBooleanField(
      capability.has_combined_parser,
      "diagram combined parser capability"
    ),
    has_render_parser: assertBooleanField(
      capability.has_render_parser,
      "diagram render parser capability"
    ),
    has_header: assertBooleanField(capability.has_header, "diagram header capability"),
    config_namespace: assertNullableStringField(
      capability.config_namespace,
      "diagram config namespace"
    ),
  };
}

function normalizeLintRuleCatalogEntry(
  rule: LintRuleCatalogEntry
): LintRuleCatalogEntry {
  if (!rule || typeof rule !== "object") {
    throw new Error("Merman WASM returned an invalid lint rule catalog entry.");
  }
  return {
    id: assertStringField(rule.id, "lint rule id"),
    description: assertStringField(rule.description, "lint rule description"),
    evidence: assertStringArray(rule.evidence, "lint rule evidence"),
    default_severity: assertCatalogValue(rule.default_severity, [
      "error",
      "warning",
      "info",
      "hint",
    ]),
    category: assertCatalogValue(rule.category, [
      "parse",
      "semantic",
      "config",
      "resource",
      "compatibility",
      "layout",
      "render",
      "internal",
    ]),
    default_enabled: Boolean(rule.default_enabled),
    default_profile: assertCatalogValue(rule.default_profile, [
      "core",
      "recommended",
      "strict",
    ]),
    origin: assertCatalogValue(rule.origin, [
      "mermaid_syntax",
      "mermaid_compatibility",
      "merman_authoring",
      "merman_resource_policy",
      "merman_internal",
    ]),
    configurable: Boolean(rule.configurable),
    fixable: Boolean(rule.fixable),
  };
}

function normalizeLintRuleCatalogResponse(
  response: LintRuleCatalogResponse
): LintRuleCatalogEntry[] {
  if (!response || typeof response !== "object") {
    throw new Error("Merman WASM returned an invalid lint rule catalog response.");
  }
  if (response.version !== 1) {
    throw new Error(
      `Merman WASM returned unsupported lint rule catalog version: ${String(response.version)}.`
    );
  }
  if (!Array.isArray(response.rules)) {
    throw new Error("Merman WASM returned a lint rule catalog response without rules.");
  }
  return response.rules.map(normalizeLintRuleCatalogEntry);
}

function assertStringField(value: unknown, label: string): string {
  if (typeof value === "string") {
    return value;
  }
  throw new Error(`Merman WASM returned an invalid ${label}.`);
}

function assertNullableStringField(value: unknown, label: string): string | null {
  if (value === undefined || value === null) {
    return null;
  }
  return assertStringField(value, label);
}

function assertBooleanField(value: unknown, label: string): boolean {
  if (typeof value === "boolean") {
    return value;
  }
  throw new Error(`Merman WASM returned an invalid ${label}.`);
}

function assertStringArray(value: unknown, label: string): string[] {
  if (Array.isArray(value) && value.every((item) => typeof item === "string")) {
    return [...value];
  }
  throw new Error(`Merman WASM returned invalid ${label}.`);
}

function assertCatalogValue<const T extends string>(
  value: unknown,
  allowed: readonly T[]
): T {
  if (typeof value === "string" && (allowed as readonly string[]).includes(value)) {
    return value as T;
  }
  throw new Error(`Merman WASM returned an invalid lint rule catalog value: ${String(value)}`);
}

function normalizeAsciiCapability(capability: AsciiCapability): AsciiCapability {
  if (!capability || typeof capability !== "object") {
    throw new Error("Merman WASM returned an invalid ASCII capability.");
  }
  if (typeof capability.diagram_type !== "string") {
    throw new Error("Merman WASM returned an invalid ASCII capability.");
  }

  const supportLevel = normalizeAsciiSupportLevel(capability.support_level);
  const evidence = Array.isArray(capability.evidence)
    ? capability.evidence.map(normalizeAsciiCapabilityEvidence)
    : [];

  return {
    diagram_type: capability.diagram_type,
    display_name:
      typeof capability.display_name === "string"
        ? capability.display_name
        : capability.diagram_type,
    support_level: supportLevel,
    summary_fallback: Boolean(capability.summary_fallback),
    supported_semantics: Array.isArray(capability.supported_semantics)
      ? capability.supported_semantics.map(String)
      : [],
    limits: Array.isArray(capability.limits) ? capability.limits.map(String) : [],
    evidence,
  };
}

function normalizeAsciiCapabilityEvidence(
  evidence: AsciiCapabilityEvidence
): AsciiCapabilityEvidence {
  return {
    kind: typeof evidence.kind === "string" ? evidence.kind : "support_matrix",
    source: typeof evidence.source === "string" ? evidence.source : "",
    note: typeof evidence.note === "string" ? evidence.note : "",
  };
}

function normalizeAsciiSupportLevel(level: unknown): AsciiSupportLevel {
  return level === "full" ||
    level === "partial" ||
    level === "summary" ||
    level === "unsupported"
    ? level
    : "unsupported";
}

function assertThemeName(theme: string): ThemeName {
  if (isThemeName(theme)) {
    return theme;
  }
  throw new Error(`Merman WASM returned unknown theme: ${theme}`);
}

function assertHostThemePresetName(preset: string): HostThemePresetName {
  if (isHostThemePresetName(preset)) {
    return preset;
  }
  throw new Error(`Merman WASM returned unknown host theme preset: ${preset}`);
}

function normalizeRuntimeCatalog(value: unknown): RuntimeCatalog {
  if (!isRecord(value) || value.schema_version !== 1) {
    throw new Error("Merman WASM returned an unsupported runtime catalog schema.");
  }
  assertExactRecordKeys(
    value,
    [
      "capabilities",
      "package_version",
      "registry",
      "resources",
      "schema_version",
      "transport_api_version",
    ],
    "Merman WASM runtime catalog"
  );
  const catalogTransportApiVersion = assertSafeIntegerField(
    value.transport_api_version,
    "runtime transport API version",
    1
  );
  if (catalogTransportApiVersion !== transportApiVersion()) {
    throw new Error(
      "Merman WASM runtime catalog transport API does not match the loaded module."
    );
  }
  if (
    typeof value.package_version !== "string" ||
    value.package_version.length === 0 ||
    value.package_version !== packageVersion()
  ) {
    throw new Error("Merman WASM runtime catalog package version does not match the loaded module.");
  }
  if (!isRecord(value.registry)) {
    throw new Error("Merman WASM returned an invalid runtime registry catalog.");
  }
  assertExactRecordKeys(
    value.registry,
    ["diagram_family_count"],
    "Merman WASM runtime registry catalog"
  );
  const diagramFamilyCount = assertSafeIntegerField(
    value.registry.diagram_family_count,
    "runtime registry diagram family count",
    0
  );
  if (!isRecord(value.resources)) {
    throw new Error("Merman WASM returned an invalid runtime resource contract.");
  }
  return {
    schema_version: 1,
    transport_api_version: catalogTransportApiVersion,
    package_version: value.package_version,
    capabilities: normalizeRuntimeCapabilities(value.capabilities),
    registry: { diagram_family_count: diagramFamilyCount },
    resources: normalizeRuntimeResourceContract(value.resources),
  };
}

function normalizeRuntimeCapabilities(value: unknown): RuntimeCapabilities {
  if (!isRecord(value)) {
    throw new Error("Merman WASM returned an invalid runtime capability report.");
  }
  assertExactRecordKeys(value, [
    "capability_ids",
    "output_ids",
    "operation_ids",
    "system_adapter_ids",
    "text_measurement",
  ], "Merman WASM runtime capability report");

  const capabilityIds = normalizeSortedIdentifierIds(value.capability_ids, "runtime capability IDs");
  const capabilitySet = new Set(capabilityIds);
  const operationIds = normalizeSortedIdentifierIds(
    value.operation_ids,
    "runtime binding operation IDs"
  );
  const operationSet = new Set(operationIds);
  const outputIds = normalizeSortedIdentifierIds(value.output_ids, "runtime output IDs");
  for (const outputId of outputIds) {
    if (!operationSet.has(outputId)) {
      throw new Error(
        `Merman WASM runtime output ${outputId} is absent from runtime binding operation IDs.`
      );
    }
  }

  const systemAdapterIds = normalizeSortedIdentifierIds(
    value.system_adapter_ids,
    "system adapter IDs"
  );
  for (const adapterId of systemAdapterIds) {
    if (!capabilitySet.has(adapterId)) {
      throw new Error(
        `Merman WASM system adapter ${adapterId} is absent from runtime capability IDs.`
      );
    }
  }
  if (systemAdapterIds.length !== 0) {
    throw new Error("Merman browser WASM must not expose native system adapters.");
  }

  const textMeasurement = normalizeTextMeasurementCapabilities(value.text_measurement);
  const svgAvailable = capabilitySet.has("svg");
  if (svgAvailable !== (textMeasurement !== null)) {
    throw new Error(
      "Merman WASM text measurement must be present exactly when SVG is available."
    );
  }

  return {
    capability_ids: capabilityIds,
    output_ids: outputIds,
    operation_ids: operationIds,
    system_adapter_ids: systemAdapterIds,
    text_measurement: textMeasurement,
  };
}

function normalizeRuntimeResourceContract(
  value: Record<string, unknown>
): RuntimeCatalog["resources"] {
  assertExactRecordKeys(
    value,
    [
      "schema_version",
      "general_binding_default_profile",
      "cli_default_profile",
      "limits",
      "profiles",
    ],
    "Merman WASM runtime resource contract"
  );
  const resourceSchemaVersion = assertSafeIntegerField(
    value.schema_version,
    "runtime resource schema version",
    1
  );
  if (
    typeof value.general_binding_default_profile !== "string" ||
    value.general_binding_default_profile.length === 0 ||
    typeof value.cli_default_profile !== "string" ||
    value.cli_default_profile.length === 0 ||
    !Array.isArray(value.limits) ||
    !Array.isArray(value.profiles)
  ) {
    throw new Error("Merman WASM returned an invalid runtime resource contract.");
  }
  const limits = value.limits.map((limit) => {
    if (!isRecord(limit)) {
      throw new Error("Merman WASM returned an invalid runtime resource limit.");
    }
    assertExactRecordKeys(
      limit,
      ["id", "phase", "description", "overridable", "hard_cap"],
      "Merman WASM runtime resource limit"
    );
    if (
      typeof limit.id !== "string" ||
      typeof limit.phase !== "string" ||
      typeof limit.description !== "string" ||
      typeof limit.overridable !== "boolean" ||
      typeof limit.hard_cap !== "boolean"
    ) {
      throw new Error("Merman WASM returned an invalid runtime resource limit.");
    }
    return {
      id: limit.id,
      phase: limit.phase,
      description: limit.description,
      overridable: limit.overridable,
      hard_cap: limit.hard_cap,
    };
  });
  const profiles = value.profiles.map((profile) => {
    if (!isRecord(profile)) {
      throw new Error("Merman WASM returned an invalid runtime resource profile.");
    }
    assertExactRecordKeys(
      profile,
      ["id", "purpose", "trust_assumption", "recommended_binding_default", "limits"],
      "Merman WASM runtime resource profile"
    );
    if (
      typeof profile.id !== "string" ||
      typeof profile.purpose !== "string" ||
      typeof profile.trust_assumption !== "string" ||
      typeof profile.recommended_binding_default !== "boolean" ||
      !(isRecord(profile.limits) || profile.limits instanceof Map)
    ) {
      throw new Error("Merman WASM returned an invalid runtime resource profile.");
    }
    const profileLimits: Record<string, number | null> = {};
    for (const [id, limit] of normalizeStringMapEntries(
      profile.limits,
      `resource profile ${profile.id} limits`
    )) {
      profileLimits[id] =
        limit === null || limit === undefined
          ? null
          : assertSafeIntegerField(limit, `resource profile limit ${id}`, 0);
    }
    return {
      id: profile.id,
      purpose: profile.purpose,
      trust_assumption: profile.trust_assumption,
      recommended_binding_default: profile.recommended_binding_default,
      limits: profileLimits,
    };
  });
  return {
    schema_version: resourceSchemaVersion,
    general_binding_default_profile: value.general_binding_default_profile,
    cli_default_profile: value.cli_default_profile,
    limits,
    profiles,
  };
}

function structuredCloneValue<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function normalizeTextMeasurementCapabilities(value: unknown): TextMeasurementCapabilities | null {
  if (value === null || value === undefined) {
    return null;
  }
  if (!isRecord(value)) {
    throw new Error("Merman WASM returned invalid text measurement capabilities.");
  }
  assertExactRecordKeys(
    value,
    ["protocol_version", "provider_ids"],
    "Merman WASM text measurement capabilities"
  );
  if (
    !Number.isSafeInteger(value.protocol_version) ||
    value.protocol_version !== MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION
  ) {
    throw new Error("Merman WASM returned an unsupported text measurement protocol.");
  }
  const providerIds = normalizeSortedIdentifierIds(
    value.provider_ids,
    "text measurement provider IDs"
  );
  if (!providerIds.includes("vendored")) {
    throw new Error("Merman WASM text measurement must include vendored support.");
  }
  return {
    protocol_version: value.protocol_version,
    provider_ids: providerIds,
  };
}

function normalizeStringMapEntries(
  value: unknown,
  label: string
): [string, unknown][] {
  const entries = value instanceof Map
    ? [...value.entries()]
    : isRecord(value)
      ? Object.entries(value)
      : null;
  if (entries === null || entries.some(([key]) => typeof key !== "string" || key.length === 0)) {
    throw new Error(`Merman WASM returned invalid ${label}.`);
  }
  return entries as [string, unknown][];
}

function normalizeSortedIdentifierIds(value: unknown, label: string): string[] {
  if (!Array.isArray(value)) {
    throw new Error(`Merman WASM returned invalid ${label}.`);
  }
  const identifiers = value.map((entry) => assertRuntimeIdentifier(entry, label));
  for (let index = 1; index < identifiers.length; index += 1) {
    if (identifiers[index - 1] >= identifiers[index]) {
      throw new Error(`Merman WASM ${label} must be sorted and unique.`);
    }
  }
  return identifiers;
}

function assertRuntimeIdentifier(value: unknown, label: string): string {
  if (
    typeof value === "string" &&
    /^[a-z0-9][a-z0-9-]*$/.test(value)
  ) {
    return value;
  }
  throw new Error(`Merman WASM returned an invalid ${label}.`);
}

function assertSafeIntegerField(value: unknown, label: string, minimum: number): number {
  if (typeof value === "number" && Number.isSafeInteger(value) && value >= minimum) {
    return value;
  }
  throw new Error(`Merman WASM returned an invalid ${label}.`);
}

function assertExactRecordKeys(
  value: Record<string, unknown>,
  expected: readonly string[],
  label: string
): void {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (
    actual.length !== wanted.length ||
    actual.some((key, index) => key !== wanted[index])
  ) {
    throw new Error(`${label} has an unsupported shape.`);
  }
}

function requireEditorLanguage<T>(
  apiName: string,
  binding: T | undefined
): T {
  if (
    !runtimeCatalog().capabilities.capability_ids.includes("editor") ||
    binding === undefined
  ) {
    throw new Error(`Merman ${apiName}() is not available in this artifact.`);
  }
  return binding;
}

function hasEditorLanguageBindings(merman: MermanWasmModule): boolean {
  return (
    typeof merman.EditorSession === "function" &&
    typeof merman.editorDiagnostics === "function" &&
    typeof merman.editorDiagramDetection === "function" &&
    typeof merman.editorCodeActions === "function" &&
    typeof merman.editorCompletions === "function" &&
    typeof merman.editorHover === "function" &&
    typeof merman.editorDocumentSymbols === "function" &&
    typeof merman.editorWorkspaceSymbols === "function" &&
    typeof merman.editorDefinition === "function" &&
    typeof merman.editorReferences === "function" &&
    typeof merman.editorPrepareRename === "function" &&
    typeof merman.editorRename === "function" &&
    typeof merman.editorSemanticTokenDescriptor === "function" &&
    typeof merman.editorSemanticTokens === "function"
  );
}

function validateEditorDiagramDetection(value: unknown): DiagramDetectionFacts {
  if (!isRecord(value)) {
    throw new Error("Merman returned an invalid editor diagram detection result.");
  }
  if (
    value.status === "unavailable" &&
    value.validity === "unknown" &&
    value.diagramType === null &&
    value.syntaxId === null &&
    value.effectiveLayoutId === null
  ) {
    return UNAVAILABLE_DIAGRAM_DETECTION;
  }
  if (
    value.status !== "available" ||
    (value.validity !== "valid" && value.validity !== "recoverable-invalid") ||
    typeof value.diagramType !== "string" ||
    !isDiagramType(value.diagramType) ||
    typeof value.syntaxId !== "string" ||
    value.syntaxId.trim().length === 0 ||
    typeof value.effectiveLayoutId !== "string" ||
    value.effectiveLayoutId.trim().length === 0
  ) {
    throw new Error("Merman returned an invalid editor diagram detection result.");
  }
  return Object.freeze({
    status: value.status,
    validity: value.validity,
    diagramType: value.diagramType,
    syntaxId: value.syntaxId,
    effectiveLayoutId: value.effectiveLayoutId,
  });
}
