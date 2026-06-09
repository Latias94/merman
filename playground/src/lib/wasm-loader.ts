import {
  asciiSupportedDiagrams,
  initMerman,
  layoutJson,
  parseJson,
  SUPPORTED_THEMES,
  renderAscii,
  renderSvg,
  supportedDiagrams,
  supportedThemes,
  validate as validateDiagram,
  type HostThemePresetName,
  type MermanWasmModule,
  type SvgBindingOptions,
} from "@mermanjs/web";
import mermanWasmUrl from "@mermanjs/web/pkg/merman_wasm_bg.wasm?url";
import {
  DEFAULT_MERMAID_CONFIG,
  sourceWithConfig,
} from "@/src/lib/mermaid-config";

export { SUPPORTED_THEMES };

export interface ValidationResult {
  valid: boolean;
  error?: string;
}

export type SvgPipeline = "parity" | "readable" | "resvg-safe";
export type HostThemePreset = HostThemePresetName;

export interface WasmRenderOptions {
  pipeline?: SvgPipeline;
  hostThemePreset?: HostThemePreset;
}

export interface MermanWasm {
  init(): Promise<void>;
  render_svg(
    code: string,
    theme: string,
    configJson?: string,
    options?: WasmRenderOptions
  ): string;
  render_ascii(code: string, theme?: string, configJson?: string): string | null;
  parse_json(
    code: string,
    theme?: string,
    configJson?: string,
    options?: WasmRenderOptions
  ): string;
  layout_json(
    code: string,
    theme?: string,
    configJson?: string,
    options?: WasmRenderOptions
  ): string;
  get_supported_diagrams(): string[];
  get_supported_themes(): string[];
  get_ascii_supported_diagrams(): string[];
  validate(code: string): ValidationResult;
}

let wasmModule: MermanWasm | null = null;
let loadingPromise: Promise<MermanWasm> | null = null;
let warmupConfigSignature: string | null = null;
let warmupPromise: Promise<void> | null = null;

const WARMUP_SOURCE = "flowchart TD\n  warmupA[Warmup] --> warmupB[Ready]";
const WASM_CACHE_NAME = "merman-playground-wasm-v1";

export async function loadWasm(): Promise<MermanWasm> {
  if (wasmModule) {
    return wasmModule;
  }
  if (loadingPromise) {
    return loadingPromise;
  }

  loadingPromise = (async () => {
    await initMerman({
      loader: loadWasmModule,
      wasm: await loadCachedWasmResponse(),
    });
    wasmModule = createWasmAdapter();
    return wasmModule;
  })().catch((error) => {
    loadingPromise = null;
    wasmModule = null;
    warmupConfigSignature = null;
    warmupPromise = null;
    throw error;
  });

  return loadingPromise;
}

async function loadWasmModule(): Promise<MermanWasmModule> {
  return (await import("@mermanjs/web/pkg/merman_wasm.js")) as MermanWasmModule;
}

async function loadCachedWasmResponse(): Promise<Response | undefined> {
  if (typeof window === "undefined" || !("caches" in window)) {
    return undefined;
  }

  const wasmUrl = new URL(mermanWasmUrl, window.location.href).href;

  try {
    const cache = await window.caches.open(WASM_CACHE_NAME);
    const cached = await cache.match(wasmUrl);
    if (cached) {
      return cached;
    }

    const response = await fetch(wasmUrl, { cache: "force-cache" });
    if (!response.ok) {
      return response;
    }

    await cache.put(wasmUrl, response.clone());
    pruneStaleWasmCacheEntries(cache, wasmUrl);
    return response;
  } catch {
    return undefined;
  }
}

function pruneStaleWasmCacheEntries(cache: Cache, currentUrl: string) {
  void cache.keys().then((requests) =>
    Promise.all(
      requests
        .filter((request) => request.url !== currentUrl)
        .map((request) => cache.delete(request))
    )
  );
}

export async function prewarmWasmRenderer(
  theme = "default",
  configJson = DEFAULT_MERMAID_CONFIG,
  options?: WasmRenderOptions
): Promise<void> {
  const wasm = await loadWasm();
  const configSignature = [
    theme,
    configJson,
    options?.pipeline ?? "parity",
    options?.hostThemePreset ?? "none",
  ].join("\0");

  if (warmupConfigSignature === configSignature) return;
  if (warmupPromise) {
    await warmupPromise.catch(() => undefined);
    if (warmupConfigSignature === configSignature) return;
  }

  warmupPromise = Promise.resolve()
    .then(() => {
      wasm.render_svg(WARMUP_SOURCE, theme, configJson, options);
      if (wasmModule === wasm) {
        warmupConfigSignature = configSignature;
      }
    })
    .finally(() => {
      warmupPromise = null;
    });

  await warmupPromise;
}

export function isWasmLoaded(): boolean {
  return wasmModule !== null;
}

export function getWasm(): MermanWasm {
  if (!wasmModule) {
    throw new Error("WASM module not loaded. Call loadWasm() first.");
  }
  return wasmModule;
}

function createWasmAdapter(): MermanWasm {
  return {
    async init() {},

    render_svg(
      code: string,
      theme: string,
      configJson = DEFAULT_MERMAID_CONFIG,
      options?: WasmRenderOptions
    ): string {
      const sourceTheme = options?.hostThemePreset ? "default" : theme;
      return renderSvg(
        sourceWithConfig(code, sourceTheme, configJson),
        bindingOptionsForRender(options)
      );
    },

    render_ascii(
      code: string,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG
    ): string | null {
      try {
        return renderAscii(sourceWithConfig(code, theme, configJson));
      } catch {
        return null;
      }
    },

    parse_json(
      code: string,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG,
      options?: WasmRenderOptions
    ): string {
      const sourceTheme = options?.hostThemePreset ? "default" : theme;
      return parseJson(
        sourceWithConfig(code, sourceTheme, configJson),
        bindingOptionsForRender(options)
      );
    },

    layout_json(
      code: string,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG,
      options?: WasmRenderOptions
    ): string {
      const sourceTheme = options?.hostThemePreset ? "default" : theme;
      return layoutJson(
        sourceWithConfig(code, sourceTheme, configJson),
        bindingOptionsForRender(options)
      );
    },

    get_supported_diagrams(): string[] {
      return supportedDiagrams();
    },

    get_supported_themes(): string[] {
      return supportedThemes();
    },

    get_ascii_supported_diagrams(): string[] {
      return asciiSupportedDiagrams();
    },

    validate(code: string): ValidationResult {
      const result = validateDiagram(code);
      return {
        valid: result.valid,
        error: result.error,
      };
    },
  };
}

function bindingOptionsForRender(
  options: WasmRenderOptions | undefined
): SvgBindingOptions | undefined {
  if (!options?.pipeline && !options?.hostThemePreset) {
    return undefined;
  }

  const bindingOptions: SvgBindingOptions = {};
  if (options.hostThemePreset) {
    bindingOptions.host_theme = { preset: options.hostThemePreset };
  }
  if (options.pipeline) {
    bindingOptions.svg = { pipeline: options.pipeline };
  }
  return bindingOptions;
}
