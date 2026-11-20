import { MermanDisposedError, MermanInvalidTransportError } from "../errors.mjs";

export function wrapCandidateEngine(
  engine,
  label,
  { forwardsAbortSignal = false, directSvgAddon = false } = {},
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
  const executeSvg = directSvgAddon ? engine.executeSvg : undefined;
  const executeSvgSync = directSvgAddon ? engine.executeSvgSync : undefined;
  const transport = {
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
  if (typeof executeSvg === "function" && typeof executeSvgSync === "function") {
    transport.executeSvg = (requestJson, signal, timeoutMs) =>
      executeSvg.call(currentEngine(), requestJson, signal, timeoutMs);
    transport.executeSvgSync = (requestJson, timeoutMs) =>
      executeSvgSync.call(currentEngine(), requestJson, timeoutMs);
  }
  return transport;
}
