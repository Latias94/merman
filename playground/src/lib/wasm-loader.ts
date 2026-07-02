import {
  asciiCapabilities,
  asciiSupportedDiagrams,
  bindingCapabilities,
  createBrowserTextMeasurer,
  editorCodeActions,
  editorCompletions,
  editorDefinition,
  editorDiagnostics,
  editorDocumentSymbols,
  editorHover,
  editorPrepareRename,
  editorReferences,
  editorRename,
  editorSemanticTokenLegend,
  editorSemanticTokens,
  initMerman,
  layoutJson,
  layoutJsonWithTextMeasurer,
  parseJson,
  SUPPORTED_THEMES,
  renderAscii,
  renderSvg,
  renderSvgWithTextMeasurer,
  supportedDiagrams,
  supportedThemes,
  selectedRegistryProfile,
  validate as validateDiagram,
  type AsciiBindingOptions,
  type AsciiCapability,
  type BindingCapabilities,
  type HostTextMeasurer,
  type HostThemePresetName,
  type MermanWasmModule,
  type RegistryProfile,
  type SvgBindingOptions,
  type EditorCodeAction,
  type EditorCompletionList,
  type EditorDiagnosticsResult,
  type EditorDocumentSymbol,
  type EditorHover,
  type EditorLocation,
  type EditorPosition,
  type EditorPrepareRename,
  type EditorSemanticToken,
  type EditorSemanticTokenLegend,
  type EditorWorkspaceEdit,
} from "@mermanjs/web";
import mermanWasmUrl from "@mermanjs/web/pkg/merman_wasm_bg.wasm?url";
import {
  DEFAULT_MERMAID_CONFIG,
  sourceWithConfig,
} from "@/src/lib/mermaid-config";
import {
  diagramFontStack,
  type DiagramFont,
} from "@/src/lib/diagram-font";

export { SUPPORTED_THEMES };
export type { AsciiCapability, BindingCapabilities, RegistryProfile };

export interface ValidationResult {
  valid: boolean;
  error?: string;
}

export type SvgPipeline = "parity" | "readable" | "resvg-safe";
export type HostThemePreset = HostThemePresetName;
export type TextMeasurementMode = "browser" | "headless";
export type { DiagramFont };

export interface WasmRenderOptions {
  pipeline?: SvgPipeline;
  hostThemePreset?: HostThemePreset;
  textMeasurementMode?: TextMeasurementMode;
  diagramFont?: DiagramFont;
}

export interface MermanWasm {
  init(): Promise<void>;
  binding_capabilities(): BindingCapabilities;
  selected_registry_profile(): RegistryProfile;
  render_svg(
    code: string,
    theme: string,
    configJson?: string,
    options?: WasmRenderOptions
  ): string;
  render_ascii(
    code: string,
    theme?: string,
    configJson?: string,
    options?: AsciiBindingOptions
  ): string | null;
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
  get_ascii_capabilities(): AsciiCapability[];
  validate(code: string): ValidationResult;
  editor_diagnostics(code: string): EditorDiagnosticsResult;
  editor_code_actions(code: string): EditorCodeAction[];
  editor_completions(code: string, position: EditorPosition): EditorCompletionList;
  editor_hover(code: string, position: EditorPosition): EditorHover | null;
  editor_document_symbols(code: string): EditorDocumentSymbol[];
  editor_definition(code: string, position: EditorPosition): EditorLocation | null;
  editor_references(
    code: string,
    position: EditorPosition,
    includeDeclaration: boolean
  ): EditorLocation[];
  editor_prepare_rename(
    code: string,
    position: EditorPosition
  ): EditorPrepareRename | null;
  editor_rename(
    code: string,
    position: EditorPosition,
    newName: string
  ): EditorWorkspaceEdit | null;
  editor_semantic_token_legend(): EditorSemanticTokenLegend;
  editor_semantic_tokens(code: string): EditorSemanticToken[];
}

let wasmModule: MermanWasm | null = null;
let loadingPromise: Promise<MermanWasm> | null = null;
let warmupConfigSignature: string | null = null;
let warmupPromise: Promise<void> | null = null;
let browserTextMeasurer: HostTextMeasurer | null = null;

const WARMUP_SOURCE = "flowchart TD\n  warmupA[Warmup] --> warmupB[Ready]";
const WASM_CACHE_NAME = "merman-playground-wasm-v1";
const PLAYGROUND_DOCUMENT_URI = "file:///merman/playground.mmd";

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
    options?.textMeasurementMode ?? "headless",
    options?.diagramFont ?? "system",
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

    binding_capabilities(): BindingCapabilities {
      return bindingCapabilities();
    },

    selected_registry_profile(): RegistryProfile {
      return selectedRegistryProfile();
    },

    render_svg(
      code: string,
      theme: string,
      configJson = DEFAULT_MERMAID_CONFIG,
      options?: WasmRenderOptions
    ): string {
      const sourceTheme = options?.hostThemePreset ? "default" : theme;
      const source = sourceWithConfig(code, sourceTheme, configJson);
      const bindingOptions = bindingOptionsForRender(options);
      if (options?.textMeasurementMode === "browser") {
        return renderSvgWithTextMeasurer(
          source,
          getBrowserTextMeasurer(),
          bindingOptions
        );
      }
      return renderSvg(source, bindingOptions);
    },

    render_ascii(
      code: string,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG,
      options?: AsciiBindingOptions
    ): string | null {
      try {
        return renderAscii(sourceWithConfig(code, theme, configJson), options);
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
      const source = sourceWithConfig(code, sourceTheme, configJson);
      const bindingOptions = bindingOptionsForRender(options);
      if (options?.textMeasurementMode === "browser") {
        return layoutJsonWithTextMeasurer(
          source,
          getBrowserTextMeasurer(),
          bindingOptions
        );
      }
      return layoutJson(
        source,
        bindingOptions
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

    get_ascii_capabilities(): AsciiCapability[] {
      return asciiCapabilities();
    },

    validate(code: string): ValidationResult {
      const result = validateDiagram(code);
      return {
        valid: result.valid,
        error: result.error,
      };
    },

    editor_diagnostics(code: string): EditorDiagnosticsResult {
      return editorDiagnostics(code, undefined, PLAYGROUND_DOCUMENT_URI);
    },

    editor_code_actions(code: string): EditorCodeAction[] {
      return editorCodeActions(code, undefined, PLAYGROUND_DOCUMENT_URI);
    },

    editor_completions(
      code: string,
      position: EditorPosition
    ): EditorCompletionList {
      return editorCompletions(code, position, PLAYGROUND_DOCUMENT_URI);
    },

    editor_hover(code: string, position: EditorPosition): EditorHover | null {
      return editorHover(code, position, PLAYGROUND_DOCUMENT_URI);
    },

    editor_document_symbols(code: string): EditorDocumentSymbol[] {
      return editorDocumentSymbols(code, PLAYGROUND_DOCUMENT_URI);
    },

    editor_definition(
      code: string,
      position: EditorPosition
    ): EditorLocation | null {
      return editorDefinition(code, position, PLAYGROUND_DOCUMENT_URI);
    },

    editor_references(
      code: string,
      position: EditorPosition,
      includeDeclaration: boolean
    ): EditorLocation[] {
      return editorReferences(
        code,
        position,
        includeDeclaration,
        PLAYGROUND_DOCUMENT_URI
      );
    },

    editor_prepare_rename(
      code: string,
      position: EditorPosition
    ): EditorPrepareRename | null {
      return editorPrepareRename(code, position, PLAYGROUND_DOCUMENT_URI);
    },

    editor_rename(
      code: string,
      position: EditorPosition,
      newName: string
    ): EditorWorkspaceEdit | null {
      return editorRename(code, position, newName, PLAYGROUND_DOCUMENT_URI);
    },

    editor_semantic_token_legend(): EditorSemanticTokenLegend {
      return editorSemanticTokenLegend();
    },

    editor_semantic_tokens(code: string): EditorSemanticToken[] {
      return editorSemanticTokens(code, PLAYGROUND_DOCUMENT_URI);
    },
  };
}

function bindingOptionsForRender(
  options: WasmRenderOptions | undefined
): SvgBindingOptions | undefined {
  const fontFamily = options?.diagramFont
    ? diagramFontStack(options.diagramFont)
    : undefined;
  if (!options?.pipeline && !options?.hostThemePreset && !fontFamily) {
    return undefined;
  }

  const bindingOptions: SvgBindingOptions = {};
  if (options?.hostThemePreset) {
    bindingOptions.host_theme = {
      preset: options.hostThemePreset,
      ...(fontFamily ? { font_family: fontFamily } : {}),
    };
  } else if (fontFamily) {
    bindingOptions.site_config = {
      fontFamily: fontFamily,
      themeVariables: {
        fontFamily: fontFamily,
      },
    };
  }
  if (options?.pipeline) {
    bindingOptions.svg = { pipeline: options.pipeline };
  }
  return bindingOptions;
}

function getBrowserTextMeasurer(): HostTextMeasurer {
  browserTextMeasurer ??= createBrowserTextMeasurer();
  return browserTextMeasurer;
}
