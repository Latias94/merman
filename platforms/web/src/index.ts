export interface ParseOptions {
  suppress_errors?: boolean;
}

export interface LayoutOptions {
  viewport_width?: number;
  viewport_height?: number;
  text_measurer?: "vendored" | "deterministic";
  math_renderer?: "none" | "ratex";
}

export interface SvgOptions {
  diagram_id?: string;
  pipeline?: "parity" | "readable" | "resvg-safe" | "resvg_safe";
  scoped_css?: string;
  css_override_policy?: "preserve" | "strip-existing-important" | "strip_existing_important";
  root_background_color?: string;
  drop_native_duplicate_fallbacks?: boolean;
}

export type MermaidSiteConfig = Record<string, unknown>;

export interface CommonBindingOptions {
  version?: number;
  site_config?: MermaidSiteConfig;
  parse?: ParseOptions;
}

export type AsciiBindingOptions = CommonBindingOptions;

export interface SvgBindingOptions extends CommonBindingOptions {
  layout?: LayoutOptions;
  svg?: SvgOptions;
}

export type BindingOptions = SvgBindingOptions;

export const SUPPORTED_THEMES = [
  "default",
  "base",
  "dark",
  "forest",
  "neutral",
  "neo",
  "neo-dark",
  "redux",
  "redux-dark",
  "redux-color",
  "redux-dark-color",
] as const;

export type ThemeName = (typeof SUPPORTED_THEMES)[number];

export const SUPPORTED_DIAGRAMS = [
  "architecture",
  "block",
  "c4",
  "class",
  "er",
  "flowchart",
  "gantt",
  "gitgraph",
  "info",
  "journey",
  "kanban",
  "mindmap",
  "packet",
  "pie",
  "quadrantchart",
  "radar",
  "requirement",
  "sankey",
  "sequence",
  "state",
  "timeline",
  "treemap",
  "xychart",
  "zenuml",
] as const;

export type DiagramType = (typeof SUPPORTED_DIAGRAMS)[number];

export const BINDING_STATUS_CODE_NAMES = [
  "MERMAN_OK",
  "MERMAN_INVALID_ARGUMENT",
  "MERMAN_UTF8_ERROR",
  "MERMAN_OPTIONS_JSON_ERROR",
  "MERMAN_NO_DIAGRAM",
  "MERMAN_PARSE_ERROR",
  "MERMAN_RENDER_ERROR",
  "MERMAN_UNSUPPORTED_FORMAT",
  "MERMAN_PANIC",
  "MERMAN_INTERNAL_ERROR",
] as const;

export type BindingStatusCodeName = (typeof BINDING_STATUS_CODE_NAMES)[number];

export interface BindingErrorPayload {
  version: number;
  ok: false;
  code: number;
  code_name: BindingStatusCodeName | string;
  message: string;
}

export function isThemeName(theme: string): theme is ThemeName {
  return (SUPPORTED_THEMES as readonly string[]).includes(theme);
}

export function isDiagramType(diagram: string): diagram is DiagramType {
  return (SUPPORTED_DIAGRAMS as readonly string[]).includes(diagram);
}

export function isBindingStatusCodeName(
  codeName: string
): codeName is BindingStatusCodeName {
  return (BINDING_STATUS_CODE_NAMES as readonly string[]).includes(codeName);
}

export function isBindingErrorPayload(error: unknown): error is BindingErrorPayload {
  if (!error || typeof error !== "object") {
    return false;
  }
  const payload = error as Record<string, unknown>;
  return (
    payload.ok === false &&
    typeof payload.version === "number" &&
    typeof payload.code === "number" &&
    typeof payload.code_name === "string" &&
    typeof payload.message === "string"
  );
}

export function normalizeThemeName(theme: string | null | undefined): ThemeName {
  return theme && isThemeName(theme) ? theme : "default";
}

export interface ValidationResult {
  valid: boolean;
  error?: string;
  code: number;
  code_name: BindingStatusCodeName;
}

export interface MermanWasmModule {
  default: (input?: unknown) => Promise<unknown>;
  abiVersion: () => number;
  packageVersion: () => string;
  renderSvg: (source: string, optionsJson?: string | null) => string;
  renderAscii: (source: string, optionsJson?: string | null) => string;
  parseJson: (source: string, optionsJson?: string | null) => string;
  layoutJson: (source: string, optionsJson?: string | null) => string;
  validate: (source: string, optionsJson?: string | null) => ValidationResult;
  asciiSupportedDiagrams: () => string[];
  supportedDiagrams: () => string[];
  themes: () => string[];
}

export type MermanWasmLoader = () => Promise<MermanWasmModule>;

export interface MermanInitOptions {
  loader?: MermanWasmLoader;
  wasm?: unknown;
}

export type MermanInitInput = MermanWasmLoader | MermanInitOptions;

let wasmModule: MermanWasmModule | null = null;
let initPromise: Promise<MermanWasmModule> | null = null;
let supportedDiagramsCache: DiagramType[] | null = null;
let asciiSupportedDiagramsCache: DiagramType[] | null = null;
let themesCache: ThemeName[] | null = null;

export function initMerman(init?: MermanInitInput): Promise<MermanWasmModule> {
  if (wasmModule) {
    return Promise.resolve(wasmModule);
  }
  if (initPromise) {
    return initPromise;
  }
  initPromise = doInit(init).catch((error) => {
    initPromise = null;
    throw error;
  });
  return initPromise;
}

async function doInit(init?: MermanInitInput): Promise<MermanWasmModule> {
  const loader = typeof init === "function" ? init : init?.loader;
  const wasm = typeof init === "function" ? undefined : init?.wasm;
  const module = loader ? await loader() : await defaultLoader();
  await module.default(wasm);
  wasmModule = module;
  return module;
}

async function defaultLoader(): Promise<MermanWasmModule> {
  return (await import("../pkg/merman_wasm.js")) as MermanWasmModule;
}

export function getMerman(): MermanWasmModule {
  if (!wasmModule) {
    throw new Error("Merman WASM is not initialized. Call initMerman() first.");
  }
  return wasmModule;
}

export function isMermanInitialized(): boolean {
  return wasmModule !== null;
}

export function renderSvg(source: string, options?: SvgBindingOptions | string): string {
  return getMerman().renderSvg(source, encodeOptions(options));
}

export function renderSvgElement(
  source: string,
  options?: SvgBindingOptions | string
): SVGSVGElement {
  if (typeof DOMParser === "undefined" || typeof document === "undefined") {
    throw new Error("renderSvgElement() requires a browser DOM.");
  }

  const svgText = renderSvg(source, options);
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

export function validate(source: string, options?: SvgBindingOptions | string): ValidationResult {
  return getMerman().validate(source, encodeOptions(options));
}

export function supportedDiagrams(): DiagramType[] {
  supportedDiagramsCache ??= getMerman().supportedDiagrams().map(assertDiagramType);
  return [...supportedDiagramsCache];
}

export function asciiSupportedDiagrams(): DiagramType[] {
  asciiSupportedDiagramsCache ??= getMerman()
    .asciiSupportedDiagrams()
    .map(assertDiagramType);
  return [...asciiSupportedDiagramsCache];
}

export function themes(): ThemeName[] {
  themesCache ??= getMerman().themes().map(assertThemeName);
  return [...themesCache];
}

export function abiVersion(): number {
  return getMerman().abiVersion();
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

function assertDiagramType(diagram: string): DiagramType {
  if (isDiagramType(diagram)) {
    return diagram;
  }
  throw new Error(`Merman WASM returned unknown diagram type: ${diagram}`);
}

function assertThemeName(theme: string): ThemeName {
  if (isThemeName(theme)) {
    return theme;
  }
  throw new Error(`Merman WASM returned unknown theme: ${theme}`);
}
