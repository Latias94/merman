import {
  asciiSupportedDiagrams,
  initMerman,
  layoutJson,
  parseJson,
  SUPPORTED_THEMES,
  renderAscii,
  renderSvg,
  supportedDiagrams,
  themes,
  validate as validateDiagram,
} from "@merman/web";
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

export interface MermanWasm {
  init(): Promise<void>;
  render_svg(
    code: string,
    theme: string,
    configJson?: string,
    pipeline?: SvgPipeline
  ): string;
  render_ascii(code: string, theme?: string, configJson?: string): string | null;
  parse_json(code: string, theme?: string, configJson?: string): string;
  layout_json(code: string, theme?: string, configJson?: string): string;
  get_supported_diagrams(): string[];
  get_themes(): string[];
  get_ascii_supported_diagrams(): string[];
  validate(code: string): ValidationResult;
}

let wasmModule: MermanWasm | null = null;
let loadingPromise: Promise<MermanWasm> | null = null;
let warmupConfigSignature: string | null = null;
let warmupPromise: Promise<void> | null = null;

const WARMUP_SOURCE = "flowchart TD\n  warmupA[Warmup] --> warmupB[Ready]";

export async function loadWasm(): Promise<MermanWasm> {
  if (wasmModule) {
    return wasmModule;
  }
  if (loadingPromise) {
    return loadingPromise;
  }

  loadingPromise = (async () => {
    await initMerman();
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

export async function prewarmWasmRenderer(
  theme = "default",
  configJson = DEFAULT_MERMAID_CONFIG,
  pipeline?: SvgPipeline
): Promise<void> {
  const wasm = await loadWasm();
  const configSignature = [theme, configJson, pipeline ?? "parity"].join("\0");

  if (warmupConfigSignature === configSignature) return;
  if (warmupPromise) {
    await warmupPromise.catch(() => undefined);
    if (warmupConfigSignature === configSignature) return;
  }

  warmupPromise = Promise.resolve()
    .then(() => {
      wasm.render_svg(WARMUP_SOURCE, theme, configJson, pipeline);
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
      pipeline?: SvgPipeline
    ): string {
      return renderSvg(
        sourceWithConfig(code, theme, configJson),
        pipeline ? { svg: { pipeline } } : undefined
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
      configJson = DEFAULT_MERMAID_CONFIG
    ): string {
      return parseJson(sourceWithConfig(code, theme, configJson));
    },

    layout_json(
      code: string,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG
    ): string {
      return layoutJson(sourceWithConfig(code, theme, configJson));
    },

    get_supported_diagrams(): string[] {
      return supportedDiagrams();
    },

    get_themes(): string[] {
      return themes();
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
