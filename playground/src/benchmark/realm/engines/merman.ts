import mermanWasmUrl from "@mermanjs/web/pkg/merman_wasm_bg.wasm?url";
import type {
  MermanWasmModule,
  SvgBindingOptions,
} from "@mermanjs/web";

import {
  diagramFontStack,
  type DiagramFont,
} from "../../../lib/diagram-font.ts";
import { sourceWithConfig } from "../../../lib/mermaid-config.ts";
import { assertRealmSourceBudget } from "../../../runtime/realm/channel-protocol.ts";
import {
  BenchmarkEngineError,
  runBenchmarkEngineStage,
  runObservedBenchmarkEngineStage,
  type BenchmarkEngineAdapter,
} from "../engine.ts";

export const benchmarkEngineAdapter: BenchmarkEngineAdapter = {
  async initialize({ mark, payload }) {
    mark("engine_import_start");
    const webPromise = import("@mermanjs/web");
    const shimPromise = import("@mermanjs/web/pkg/merman_wasm.js");
    const enginePromise = runObservedBenchmarkEngineStage(
      "engine-import",
      async () => {
        const [web, module] = await Promise.all([webPromise, shimPromise]);
        return { web, module: module as MermanWasmModule };
      },
      () => mark("engine_import_end")
    );

    mark("resource_acquire_start");
    const resourcePromise = runObservedBenchmarkEngineStage(
      "resource-acquire",
      async () => {
        const response = await fetch(
          new URL(mermanWasmUrl, window.location.href),
          { cache: "default" }
        );
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
    const { web, module } = engineResult.value;
    const wasm = resourceResult.value;
    mark("initialize_start");
    let measurement: ReturnType<
      typeof web.createBrowserTextMeasurementSession
    > | null = null;
    let configuredSource: string;
    let options: string;
    let version: string;
    try {
      await runBenchmarkEngineStage("initialize", () => module.default(wasm));
      measurement = await runBenchmarkEngineStage("initialize", () =>
        web.createBrowserTextMeasurementSession()
      );
      configuredSource = sourceWithConfig(
        payload.source,
        payload.theme,
        payload.configJson
      );
      assertRealmSourceBudget(configuredSource);
      options = bindingOptions(payload.diagramFont);
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

function bindingOptions(diagramFont: DiagramFont): string {
  const fontFamily = diagramFontStack(diagramFont);
  const options: SvgBindingOptions = {
    site_config: {
      fontFamily,
      themeVariables: { fontFamily },
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
