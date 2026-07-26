import {
  asciiCapabilities,
  asciiSupportedDiagrams,
  assertSafeSvgForDom,
  createBrowserTextMeasurementSession,
  detectDiagramFacts,
  initMerman,
  isMermanInitialized,
  layoutJson,
  layoutJsonWithTextMeasurer,
  loadMermanWasmModule,
  MERMAN_WASM_URL,
  packageVersion,
  parseJson,
  renderAscii,
  renderSvg,
  renderSvgWithTextMeasurer,
  runtimeCatalog,
  supportedDiagrams,
  supportedThemes,
  UNAVAILABLE_DIAGRAM_DETECTION,
  validate,
  type HostTextMeasurer,
  type MermanWasmModule,
} from "@mermanjs/web";

import {
  DEFAULT_MERMAID_CONFIG,
  sourceWithConfig,
} from "../lib/mermaid-config.ts";
import { configuredMermanOperationInput } from "./merman-operation-input.ts";
import { projectError } from "./error-projection.ts";
import type {
  MermanDomainFacade,
  MermanRuntimeDependencies,
  MermanSession,
} from "./merman-core.ts";

export const mermanBrowserDependencies: MermanRuntimeDependencies = {
  createSession,
  fetchWasm: ({ cache, signal }) =>
    fetch(new URL(MERMAN_WASM_URL, window.location.href), { cache, signal }),
  async initialize({ module, wasm }) {
    await initMerman({
      // The generated shim is rebuilt independently; Web contract checks validate its full shape.
      loader: async () => module as unknown as MermanWasmModule,
      wasm,
    });
  },
  isInitialized: isMermanInitialized,
  isRetryableInitializationError: (error) =>
    error instanceof WebAssembly.CompileError ||
    error instanceof WebAssembly.LinkError,
  loadModule: loadMermanWasmModule,
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

    runtimeCatalog,

    detectDiagram(
      code,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG,
      options,
    ) {
      try {
        const input = configuredMermanOperationInput(
          code,
          theme,
          configJson,
          options,
        );
        return detectDiagramFacts(
          input.source,
          input.bindingOptions,
        );
      } catch {
        return UNAVAILABLE_DIAGRAM_DETECTION;
      }
    },

    getAsciiCapabilities: asciiCapabilities,
    getAsciiSupportedDiagrams: asciiSupportedDiagrams,
    getSupportedDiagrams: supportedDiagrams,
    getThemes: supportedThemes,

    layoutJson(
      code,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG,
      options,
    ) {
      const input = configuredMermanOperationInput(
        code,
        theme,
        configJson,
        options,
      );
      return options?.textMeasurementMode === "browser"
        ? layoutJsonWithTextMeasurer(
            input.source,
            measureText,
            input.bindingOptions,
          )
        : layoutJson(input.source, input.bindingOptions);
    },

    parseJson(
      code,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG,
      options,
    ) {
      const input = configuredMermanOperationInput(
        code,
        theme,
        configJson,
        options,
      );
      return parseJson(input.source, input.bindingOptions);
    },

    render(code, theme, configJson = DEFAULT_MERMAID_CONFIG, options) {
      const startedAt = performance.now();
      try {
        const input = configuredMermanOperationInput(
          code,
          theme,
          configJson,
          options,
        );
        const svg =
          options?.textMeasurementMode === "browser"
            ? renderSvgWithTextMeasurer(
                input.source,
                measureText,
                input.bindingOptions,
              )
            : renderSvg(input.source, input.bindingOptions);
        assertSafeSvgForDom(svg);
        return {
          error: null,
          renderTime: performance.now() - startedAt,
          status: "success",
          svg,
        };
      } catch (error) {
        return {
          error: projectError(error),
          renderTime: 0,
          status: "failure",
          svg: null,
        };
      }
    },

    renderAscii(code, theme = "default", configJson = DEFAULT_MERMAID_CONFIG) {
      try {
        return {
          ascii: renderAscii(sourceWithConfig(code, theme, configJson)),
          error: null,
          status: "success",
        };
      } catch (error) {
        return {
          ascii: null,
          error: projectError(error),
          status: "failure",
        };
      }
    },

    validate,
  };
}
