import type {
  MermanWasmModule,
  SvgBindingOptions,
} from "@mermanjs/web";
import { createBrowserTextMeasurementSession } from "../../../../../platforms/web/packages/full/dist/runtime-render.js";

import {
  diagramFontStack,
  type DiagramFont,
} from "../../../lib/diagram-font.ts";
import { sourceWithConfig } from "../../../lib/mermaid-config.ts";
import {
  assertRealmSourceBudget,
  type RealmViewport,
} from "../../../runtime/realm/channel-protocol.ts";
import {
  BenchmarkEngineError,
  runBenchmarkEngineStage,
  runObservedBenchmarkEngineStage,
  type BenchmarkEngineAdapter,
} from "../engine.ts";

export const benchmarkEngineAdapter: BenchmarkEngineAdapter = {
  async initialize({ mark, payload, resourceUrl }) {
    const wasmUrl = validateMermanWasmUrl(resourceUrl);
    mark("engine_import_start");
    const modulePromise = import(
      "../../../../../platforms/web/packages/full/artifacts/wasm/merman_wasm.js"
    );
    const enginePromise = runObservedBenchmarkEngineStage(
      "engine-import",
      async () => {
        const module = await modulePromise;
        return module as unknown as MermanWasmModule;
      },
      () => mark("engine_import_end")
    );

    mark("resource_acquire_start");
    const resourcePromise = runObservedBenchmarkEngineStage(
      "resource-acquire",
      async () => {
        const response = await fetch(wasmUrl, { cache: "default" });
        validateWasmResponse(response);
        return response.arrayBuffer();
      },
      () => mark("resource_acquire_end")
    );

    const [engineResult, resourceResult] = await Promise.allSettled([
      enginePromise,
      resourcePromise,
    ]);
    if (engineResult.status === "rejected") throw engineResult.reason;
    if (resourceResult.status === "rejected") throw resourceResult.reason;
    const module = engineResult.value;
    const wasmResponse = new Response(resourceResult.value, {
      headers: { "content-type": "application/wasm" },
    });
    mark("initialize_start");
    let measurement: ReturnType<
      typeof createBrowserTextMeasurementSession
    > | null = null;
    let configuredSource: string;
    let options: string;
    let version: string;
    try {
      await runBenchmarkEngineStage("initialize", () =>
        module.default({ module_or_path: wasmResponse })
      );
      measurement = await runBenchmarkEngineStage("initialize", () =>
        createBrowserTextMeasurementSession()
      );
      configuredSource = sourceWithConfig(
        payload.source,
        payload.theme,
        payload.configJson
      );
      assertRealmSourceBudget(configuredSource);
      options = bindingOptions(payload.diagramFont, payload.viewport);
      version = module.packageVersion();
    } catch (error) {
      measurement?.dispose();
      throw error;
    } finally {
      mark("initialize_end");
    }

    let disposed = false;
    return {
      version,
      dispose() {
        if (disposed) return;
        disposed = true;
        measurement.dispose();
      },
      render() {
        if (disposed) {
          throw new BenchmarkEngineError(
            "disposed",
            "Merman benchmark session is disposed."
          );
        }
        return runBenchmarkEngineStage("render", () => {
          const render = module.renderSvgWithTextMeasurer;
          if (!render) {
            throw new Error(
              "Merman WASM does not expose renderSvgWithTextMeasurer()."
            );
          }
          return render(configuredSource, options, measurement.measure);
        });
      },
    };
  },
};

function validateMermanWasmUrl(value: string | null): URL {
  if (!value) {
    throw new BenchmarkEngineError(
      "resource-acquire",
      "Merman benchmark has no WASM resource URL."
    );
  }
  const url = new URL(value, window.location.href);
  if (
    url.origin !== window.location.origin ||
    !/\/merman_wasm_bg(?:-[\w-]+)?\.wasm$/u.test(url.pathname)
  ) {
    throw new BenchmarkEngineError(
      "resource-acquire",
      "Merman benchmark WASM resource URL is invalid."
    );
  }
  return url;
}

function bindingOptions(
  diagramFont: DiagramFont,
  viewport: RealmViewport
): string {
  const fontFamily = diagramFontStack(diagramFont);
  const screenAvailableWidth = window.screen.availWidth;
  const options: SvgBindingOptions = {
    version: 2,
    site_config: {
      fontFamily,
      themeVariables: { fontFamily },
    },
    layout: {
      container_width: viewport.width,
      container_height: viewport.height,
      ...(Number.isFinite(screenAvailableWidth) && screenAvailableWidth > 0
        ? { screen_available_width: screenAvailableWidth }
        : {}),
    },
  };
  return JSON.stringify(options);
}

function validateWasmResponse(response: Response): void {
  if (!response.ok) {
    throw new Error(`WASM request failed with HTTP ${response.status}.`);
  }
  const contentType = response.headers.get("content-type") ?? "";
  if (!/^application\/wasm(?:\s*;|$)/i.test(contentType)) {
    throw new Error(
      `WASM response must use application/wasm, received ${contentType || "none"}.`
    );
  }
}
