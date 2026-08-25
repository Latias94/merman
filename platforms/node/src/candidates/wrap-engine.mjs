import { MermanDisposedError, MermanInvalidTransportError } from "../errors.mjs";

export function wrapCandidateEngine(
  engine,
  label,
  { forwardsAbortSignal = false } = {},
) {
  if (
    typeof engine?.execute !== "function" ||
    typeof engine?.executeSync !== "function" ||
    typeof engine?.runtimeCatalogJson !== "function" ||
    typeof engine?.metadataJson !== "function"
  ) {
    throw new MermanInvalidTransportError(`${label} does not implement the operation contract.`);
  }
  let ownedEngine = engine;
  const currentEngine = () => {
    if (!ownedEngine) throw new MermanDisposedError();
    return ownedEngine;
  };
  return {
    execute: (requestJson, signal, timeoutMs) =>
      forwardsAbortSignal
        ? currentEngine().execute(requestJson, signal, timeoutMs)
        : currentEngine().execute(requestJson, timeoutMs),
    executeSync: (requestJson, timeoutMs) =>
      currentEngine().executeSync(requestJson, timeoutMs),
    runtimeCatalogJson: () => currentEngine().runtimeCatalogJson(),
    metadataJson: (id) => currentEngine().metadataJson(id),
    dispose: () => {
      const disposing = ownedEngine;
      ownedEngine = null;
      return disposing?.dispose?.();
    },
  };
}
