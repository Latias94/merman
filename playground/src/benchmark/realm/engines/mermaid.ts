import type { Mermaid, MermaidConfig } from "mermaid";

import {
  buildMermaidOperationInput,
} from "../../../lib/mermaid-config.ts";
import {
  MERMAID_JS_VERSION,
} from "../../../runtime/mermaid-requirements.ts";
import { mermaidExternalModuleRegistrar } from "../../../runtime/external-module-registrar.ts";
import { assertRealmSourceBudget } from "../../../runtime/realm/channel-protocol.ts";
import {
  BenchmarkEngineError,
  runBenchmarkEngineStage,
  type BenchmarkEngineAdapter,
} from "../engine.ts";

let renderSequence = 0;

export const benchmarkEngineAdapter: BenchmarkEngineAdapter = {
  async initialize({ mark, payload, resourceUrl }) {
    if (resourceUrl !== null) {
      throw new BenchmarkEngineError(
        "protocol",
        "Mermaid benchmark received an unexpected resource URL."
      );
    }
    mark("engine_import_start");
    let mermaid: Mermaid;
    try {
      mermaid = await runBenchmarkEngineStage("engine-import", async () =>
        (await import("mermaid")).default
      );
    } finally {
      mark("engine_import_end");
    }

    mark("register_start");
    try {
      await runBenchmarkEngineStage("register", () =>
        mermaidExternalModuleRegistrar.register(
          mermaid,
          payload.externalRequirements
        )
      );
    } finally {
      mark("register_end");
    }

    mark("initialize_start");
    let configuredSource: string;
    let version: string;
    try {
      const configured = await runBenchmarkEngineStage("initialize", async () => {
        const configured = buildMermaidOperationInput(
          payload.source, payload.theme, payload.configJson,
          { diagramFont: payload.diagramFont },
        );
        mermaid.initialize({
          ...configured.initializationConfig,
          startOnLoad: false,
          securityLevel: configured.initializationConfig.securityLevel ?? "loose",
        } as MermaidConfig);
        return configured;
      });
      configuredSource = configured.configuredSource;
      assertRealmSourceBudget(configuredSource);
      version = MERMAID_JS_VERSION;
    } finally {
      mark("initialize_end");
    }
    let disposed = false;
    return {
      version,
      dispose() {
        disposed = true;
      },
      async render() {
        if (disposed) {
          throw new BenchmarkEngineError(
            "disposed",
            "Mermaid benchmark session is disposed."
          );
        }
        return runBenchmarkEngineStage("render", async () =>
          (await mermaid.render(nextRenderId(), configuredSource)).svg
        );
      },
    };
  },
};

function nextRenderId(): string {
  renderSequence += 1;
  return `merman-benchmark-mermaid-${renderSequence}`;
}
