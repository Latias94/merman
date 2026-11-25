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
  if (directSvgAddon && (typeof executeSvg !== "function" || typeof executeSvgSync !== "function")) {
    throw new MermanInvalidTransportError(`${label} does not implement the direct SVG contract.`);
  }
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
  if (directSvgAddon) {
    transport.executeSvg = (source, optionsJson, signal, timeoutMs) =>
      executeSvg.call(currentEngine(), source, optionsJson, signal, timeoutMs);
    transport.executeSvgSync = (source, optionsJson, timeoutMs) =>
      executeSvgSync.call(currentEngine(), source, optionsJson, timeoutMs);
    const { executeSvgWithId, executeSvgWithIdSync } = engine;
    if (typeof executeSvgWithId === "function" && typeof executeSvgWithIdSync === "function") {
      transport.executeSvgWithId = (source, diagramId, signal, timeoutMs) =>
        executeSvgWithId.call(currentEngine(), source, diagramId, signal, timeoutMs);
      transport.executeSvgWithIdSync = (source, diagramId, timeoutMs) =>
        executeSvgWithIdSync.call(currentEngine(), source, diagramId, timeoutMs);
    }
  }
  return transport;
}
