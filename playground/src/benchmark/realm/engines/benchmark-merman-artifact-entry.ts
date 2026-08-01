publishEngineSentinel("benchmark-merman");

export { benchmarkEngineAdapter } from "./merman.ts";

function publishEngineSentinel(id: string): void {
  Object.defineProperty(globalThis, "__mermanEngineArtifact", {
    configurable: false,
    enumerable: false,
    writable: false,
    value: Object.freeze({ id, version: null, evaluatedAt: performance.now() }),
  });
}
