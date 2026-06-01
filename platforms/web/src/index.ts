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
}

export interface BindingOptions {
  version?: number;
  parse?: ParseOptions;
  layout?: LayoutOptions;
  svg?: SvgOptions;
}

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

export function isThemeName(theme: string): theme is ThemeName {
  return (SUPPORTED_THEMES as readonly string[]).includes(theme);
}

export function normalizeThemeName(theme: string | null | undefined): ThemeName {
  return theme && isThemeName(theme) ? theme : "default";
}

export interface ValidationResult {
  valid: boolean;
  error?: string;
  code: number;
  code_name: string;
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

export function renderSvg(source: string, options?: BindingOptions | string): string {
  return getMerman().renderSvg(source, encodeOptions(options));
}

export function renderSvgElement(
  source: string,
  options?: BindingOptions | string
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
  options?: BindingOptions | string
): SVGSVGElement {
  const svg = renderSvgElement(source, options);
  target.replaceChildren(svg);
  return svg;
}

export function renderAscii(source: string, options?: BindingOptions | string): string {
  return getMerman().renderAscii(source, encodeOptions(options));
}

export function parseJson(source: string, options?: BindingOptions | string): string {
  return getMerman().parseJson(source, encodeOptions(options));
}

export function parseObject<T = unknown>(source: string, options?: BindingOptions | string): T {
  return JSON.parse(parseJson(source, options)) as T;
}

export function layoutJson(source: string, options?: BindingOptions | string): string {
  return getMerman().layoutJson(source, encodeOptions(options));
}

export function layoutObject<T = unknown>(source: string, options?: BindingOptions | string): T {
  return JSON.parse(layoutJson(source, options)) as T;
}

export function validate(source: string, options?: BindingOptions | string): ValidationResult {
  return getMerman().validate(source, encodeOptions(options));
}

export function supportedDiagrams(): string[] {
  return getMerman().supportedDiagrams();
}

export function asciiSupportedDiagrams(): string[] {
  return getMerman().asciiSupportedDiagrams();
}

export function themes(): ThemeName[] {
  return getMerman().themes().map(normalizeThemeName);
}

export function abiVersion(): number {
  return getMerman().abiVersion();
}

export function packageVersion(): string {
  return getMerman().packageVersion();
}

export function encodeOptions(options?: BindingOptions | string): string | undefined {
  if (options === undefined) {
    return undefined;
  }
  return typeof options === "string" ? options : JSON.stringify(options);
}
