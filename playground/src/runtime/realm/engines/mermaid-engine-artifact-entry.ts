import { MERMAID_JS_VERSION } from "../../mermaid-requirements.ts";

publishEngineSentinel(MERMAID_JS_VERSION);

export { renderWithMermaid } from "./mermaid.ts";
export { benchmarkEngineAdapter } from "../../../benchmark/realm/engines/mermaid.ts";

function publishEngineSentinel(version: string): void {
  Object.defineProperty(globalThis, "__mermanEngineArtifact", {
    configurable: false,
    enumerable: false,
    writable: false,
    value: Object.freeze({ id: "mermaid", version, evaluatedAt: performance.now() }),
  });
}
