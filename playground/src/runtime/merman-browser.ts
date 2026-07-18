import {
  asciiCapabilities,
  asciiSupportedDiagrams,
  assertSafeSvgForDom,
  bindingCapabilities,
  createBrowserTextMeasurementSession,
  detectDiagramFacts,
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
  isMermanInitialized,
  layoutJson,
  layoutJsonWithTextMeasurer,
  packageVersion,
  parseJson,
  renderAscii,
  renderSvg,
  renderSvgWithTextMeasurer,
  selectedRegistryProfile,
  supportedDiagrams,
  supportedThemes,
  validate,
  type DiagramDetectionFacts,
  type HostTextMeasurer,
  type MermanWasmModule,
  type SvgBindingOptions,
} from "@mermanjs/web";
import mermanWasmUrl from "@mermanjs/web/pkg/merman_wasm_bg.wasm?url";

import { diagramFontStack } from "../lib/diagram-font.ts";
import {
  DEFAULT_MERMAID_CONFIG,
  sourceWithConfig,
} from "../lib/mermaid-config.ts";
import type {
  MermanDomainFacade,
  MermanRenderOptions,
  MermanRuntimeDependencies,
  MermanSession,
} from "./merman-core.ts";

const PLAYGROUND_DOCUMENT_URI = "file:///merman/playground.mmd";
const UNAVAILABLE_DIAGRAM_DETECTION: DiagramDetectionFacts = Object.freeze({
  status: "unavailable",
  diagramType: null,
  syntaxId: null,
  effectiveLayoutId: null,
});

export const mermanBrowserDependencies: MermanRuntimeDependencies = {
  createSession,
  fetchWasm: ({ cache, signal }) =>
    fetch(new URL(mermanWasmUrl, window.location.href), { cache, signal }),
  async initialize({ module, wasm }) {
    await initMerman({
      loader: async () => module as MermanWasmModule,
      wasm,
    });
  },
  isInitialized: isMermanInitialized,
  isRetryableInitializationError: (error) =>
    error instanceof WebAssembly.CompileError,
  loadModule: async () =>
    (await import("@mermanjs/web/pkg/merman_wasm.js")) as MermanWasmModule,
};

function createSession(): MermanSession {
  const measurement = createBrowserTextMeasurementSession();

  return {
    facade: createFacade(measurement.measure),
    dispose: () => measurement.dispose(),
  };
}

function createFacade(measureText: HostTextMeasurer): MermanDomainFacade {
  return {
    packageVersion: packageVersion(),

    bindingCapabilities,

    detectDiagram(
      code,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG,
      options
    ) {
      try {
        return detectDiagramFacts(
          configuredSource(code, theme, configJson, options),
          bindingOptionsForRender(options)
        );
      } catch {
        return UNAVAILABLE_DIAGRAM_DETECTION;
      }
    },

    editorCodeActions(code) {
      return editorCodeActions(code, undefined, PLAYGROUND_DOCUMENT_URI);
    },

    editorCompletions(code, position) {
      return editorCompletions(code, position, PLAYGROUND_DOCUMENT_URI);
    },

    editorDefinition(code, position) {
      return editorDefinition(code, position, PLAYGROUND_DOCUMENT_URI);
    },

    editorDiagnostics(code) {
      return editorDiagnostics(code, undefined, PLAYGROUND_DOCUMENT_URI);
    },

    editorDocumentSymbols(code) {
      return editorDocumentSymbols(code, PLAYGROUND_DOCUMENT_URI);
    },

    editorHover(code, position) {
      return editorHover(code, position, PLAYGROUND_DOCUMENT_URI);
    },

    editorPrepareRename(code, position) {
      return editorPrepareRename(code, position, PLAYGROUND_DOCUMENT_URI);
    },

    editorReferences(code, position, includeDeclaration) {
      return editorReferences(
        code,
        position,
        includeDeclaration,
        PLAYGROUND_DOCUMENT_URI
      );
    },

    editorRename(code, position, newName) {
      return editorRename(
        code,
        position,
        newName,
        PLAYGROUND_DOCUMENT_URI
      );
    },

    editorSemanticTokenLegend,

    editorSemanticTokens(code) {
      return editorSemanticTokens(code, PLAYGROUND_DOCUMENT_URI);
    },

    getAsciiCapabilities: asciiCapabilities,
    getAsciiSupportedDiagrams: asciiSupportedDiagrams,
    getSupportedDiagrams: supportedDiagrams,
    getThemes: supportedThemes,

    layoutJson(
      code,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG,
      options
    ) {
      const source = configuredSource(code, theme, configJson, options);
      const bindingOptions = bindingOptionsForRender(options);
      return options?.textMeasurementMode === "browser"
        ? layoutJsonWithTextMeasurer(source, measureText, bindingOptions)
        : layoutJson(source, bindingOptions);
    },

    parseJson(
      code,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG,
      options
    ) {
      return parseJson(
        configuredSource(code, theme, configJson, options),
        bindingOptionsForRender(options)
      );
    },

    registryProfile: selectedRegistryProfile,

    render(
      code,
      theme,
      configJson = DEFAULT_MERMAID_CONFIG,
      options
    ) {
      const startedAt = performance.now();
      try {
        const source = configuredSource(code, theme, configJson, options);
        const bindingOptions = bindingOptionsForRender(options);
        const svg =
          options?.textMeasurementMode === "browser"
            ? renderSvgWithTextMeasurer(source, measureText, bindingOptions)
            : renderSvg(source, bindingOptions);
        assertSafeSvgForDom(svg);
        return {
          error: null,
          renderTime: performance.now() - startedAt,
          svg,
        };
      } catch (error) {
        return {
          error: error instanceof Error ? error.message : String(error),
          renderTime: 0,
          svg: null,
        };
      }
    },

    renderAscii(
      code,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG
    ) {
      try {
        return renderAscii(sourceWithConfig(code, theme, configJson));
      } catch {
        return null;
      }
    },

    validate,
  };
}

function configuredSource(
  code: string,
  theme: string,
  configJson: string,
  options: MermanRenderOptions | undefined
): string {
  return sourceWithConfig(
    code,
    options?.hostThemePreset ? "default" : theme,
    configJson
  );
}

function bindingOptionsForRender(
  options: MermanRenderOptions | undefined
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
      fontFamily,
      themeVariables: { fontFamily },
    };
  }
  if (options?.pipeline) {
    bindingOptions.svg = { pipeline: options.pipeline };
  }
  return bindingOptions;
}
