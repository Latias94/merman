import {
  BINDING_OPTIONS_SCHEMA_VERSION,
  asciiCapabilities,
  asciiSupportedDiagrams,
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
  renderSvgResult,
  renderSvgResultWithTextMeasurer,
  resolveTheme,
  runtimeCatalog,
  supportedDiagrams,
  svgPlanJson,
  themeCatalog,
  exportThemePreset,
  UNAVAILABLE_DIAGRAM_DETECTION,
  validate,
  type HostTextMeasurer,
  type MermanWasmModule,
  type SvgRenderResult,
} from "@mermanjs/web";

import { projectError } from "./error-projection.ts";
import { encodeMermanOptions } from "./merman-operation-input.ts";
import { projectNavigableInlineSvg } from "./render-artifact.ts";
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
    themeCatalog,
    exportThemePreset,
    resolveTheme,

    detectDiagram(input) {
      if (input.configurationError) return UNAVAILABLE_DIAGRAM_DETECTION;
      try {
        return detectDiagramFacts(input.configuredSource, input.bindingOptionsJson);
      } catch {
        return UNAVAILABLE_DIAGRAM_DETECTION;
      }
    },

    getAsciiCapabilities: asciiCapabilities,
    getAsciiSupportedDiagrams: asciiSupportedDiagrams,
    getSupportedDiagrams: supportedDiagrams,

    layoutJson(input) {
      assertConfiguredOperation(input);
      return input.textMeasurementMode === "browser"
        ? layoutJsonWithTextMeasurer(
            input.configuredSource,
            measureText,
            input.bindingOptionsJson,
          )
        : layoutJson(input.configuredSource, input.bindingOptionsJson);
    },

    parseJson(input) {
      assertConfiguredOperation(input);
      return parseJson(input.configuredSource, input.bindingOptionsJson);
    },

    svgPlan(input) {
      assertConfiguredOperation(input);
      return svgPlanJson(input.configuredSource, input.bindingOptionsJson);
    },

    render(input) {
      const startedAt = performance.now();
      if (input.configurationError) {
        return {
          artifact: null,
          error: input.configurationError,
          renderTime: 0,
          stage: "render",
          status: "failure",
        };
      }
      let result: SvgRenderResult;
      try {
        result =
          input.textMeasurementMode === "browser"
            ? renderSvgResultWithTextMeasurer(
                input.configuredSource,
                measureText,
                input.bindingOptionsJson,
              )
            : renderSvgResult(input.configuredSource, input.bindingOptionsJson);
      } catch (error) {
        return {
          artifact: null,
          error: projectError(error),
          renderTime: 0,
          stage: "render",
          status: "failure",
        };
      }
      try {
        return {
          artifact: projectNavigableInlineSvg(result.svg),
          error: null,
          metadata: result.metadata,
          renderTime: performance.now() - startedAt,
          status: "success",
        };
      } catch (error) {
        return {
          artifact: null,
          error: projectError(error),
          renderTime: 0,
          stage: "svg-validation",
          status: "failure",
        };
      }
    },

    renderAscii(input) {
      if (input.configurationError) {
        return {
          ascii: null,
          error: input.configurationError,
          status: "failure",
        };
      }
      try {
        return {
          ascii: renderAscii(input.configuredSource, encodeMermanOptions({
            version: BINDING_OPTIONS_SCHEMA_VERSION,
            site_config: input.bindingOptions.site_config,
            fixed_unix_millis: input.bindingOptions.fixed_unix_millis,
            fixed_local_offset_minutes: input.bindingOptions.fixed_local_offset_minutes,
            theme: input.bindingOptions.theme,
          }, input.themeDocumentJson)),
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

function assertConfiguredOperation(
  input: Parameters<MermanDomainFacade["render"]>[0],
): void {
  if (input.configurationError) throw input.configurationError;
}
