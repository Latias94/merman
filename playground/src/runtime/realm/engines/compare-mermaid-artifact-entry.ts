import { MERMAID_JS_VERSION } from "../../mermaid-requirements.ts";

publishEngineSentinel("compare-mermaid", MERMAID_JS_VERSION);

export { renderWithMermaid } from "./mermaid.ts";

function publishEngineSentinel(id: string, version: string): void {
  Object.defineProperty(globalThis, "__mermanEngineArtifact", {
    configurable: false,
    enumerable: false,
    writable: false,
    value: Object.freeze({ id, version, evaluatedAt: performance.now() }),
  });
}
