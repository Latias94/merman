import { MERMAID_JS_VERSION } from "../../../runtime/mermaid-requirements.ts";

publishEngineSentinel("benchmark-mermaid", MERMAID_JS_VERSION);

export { benchmarkEngineAdapter } from "./mermaid.ts";

function publishEngineSentinel(id: string, version: string): void {
  Object.defineProperty(globalThis, "__mermanEngineArtifact", {
    configurable: false,
    enumerable: false,
    writable: false,
    value: Object.freeze({ id, version, evaluatedAt: performance.now() }),
  });
}
