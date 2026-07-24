import { MermanDisposedError, MermanInvalidTransportError } from "../errors.mjs";

export function wrapCandidateEngine(engine, label) {
  if (typeof engine?.execute !== "function" || typeof engine?.executeSync !== "function") {
    throw new MermanInvalidTransportError(`${label} does not implement the operation contract.`);
  }
  let ownedEngine = engine;
  const currentEngine = () => {
    if (!ownedEngine) throw new MermanDisposedError();
    return ownedEngine;
  };
  return {
    execute: (requestJson) => currentEngine().execute(requestJson),
    executeSync: (requestJson) => currentEngine().executeSync(requestJson),
    dispose: () => {
      const disposing = ownedEngine;
      ownedEngine = null;
      return disposing?.dispose?.();
    },
  };
}
