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

let wasmModule: MermanWasmModule | null = null;
let initPromise: Promise<MermanWasmModule> | null = null;

export function initMerman(loader?: MermanWasmLoader): Promise<MermanWasmModule> {
  if (wasmModule) {
    return Promise.resolve(wasmModule);
  }
  if (initPromise) {
    return initPromise;
  }
  initPromise = doInit(loader);
  return initPromise;
}

async function doInit(loader?: MermanWasmLoader): Promise<MermanWasmModule> {
  const module = loader ? await loader() : await defaultLoader();
  await module.default();
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

export function themes(): string[] {
  return getMerman().themes();
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
