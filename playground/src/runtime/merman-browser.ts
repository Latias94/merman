import {
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
  presentationCatalog,
  renderAscii,
  renderSvg,
  renderSvgWithTextMeasurer,
  runtimeCatalog,
  supportedDiagrams,
  supportedThemes,
  svgPlanJson,
  UNAVAILABLE_DIAGRAM_DETECTION,
  validate,
  type HostTextMeasurer,
  type MermanWasmModule,
} from "@mermanjs/web";

import { projectError } from "./error-projection.ts";
import { projectSafeInlineSvg } from "./render-artifact.ts";
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

    presentationCatalog,
    runtimeCatalog,

    detectDiagram(input) {
      if (input.configurationError) return UNAVAILABLE_DIAGRAM_DETECTION;
      try {
        return detectDiagramFacts(
          input.configuredSource,
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

    layoutJson(input) {
      assertConfiguredOperation(input);
      return input.textMeasurementMode === "browser"
        ? layoutJsonWithTextMeasurer(
            input.configuredSource,
            measureText,
            input.bindingOptions,
          )
        : layoutJson(input.configuredSource, input.bindingOptions);
    },

    parseJson(input) {
      assertConfiguredOperation(input);
      return parseJson(input.configuredSource, input.bindingOptions);
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
      let svg: string;
      try {
        svg =
          input.textMeasurementMode === "browser"
            ? renderSvgWithTextMeasurer(
                input.configuredSource,
                measureText,
                input.bindingOptions,
              )
            : renderSvg(input.configuredSource, input.bindingOptions);
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
          artifact: projectSafeInlineSvg(svg),
          error: null,
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
          ascii: renderAscii(input.configuredSource),
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

    svgPlan(input) {
      assertConfiguredOperation(input);
      return svgPlanJson(input.configuredSource, input.bindingOptions);
    },

    validate,
  };
}

function assertConfiguredOperation(
  input: Parameters<MermanDomainFacade["render"]>[0]
): void {
  if (input.configurationError) throw input.configurationError;
}
