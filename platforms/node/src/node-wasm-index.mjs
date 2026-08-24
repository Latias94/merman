import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

import {
  MermanEngine,
  createNodeEngine as createEngineWithTransport,
} from "./engine.mjs";
import { loadNodeWasmTransport } from "./candidates/wasm.mjs";

const wasmBindingPath = fileURLToPath(
  new URL("../artifact/merman_node.cjs", import.meta.url),
);
const requireFromPackage = createRequire(import.meta.url);

export { MermanEngine };

export function createNodeEngine(options) {
  return createEngineWithTransport(options, {
    loadTransport: (optionsJson) =>
      loadNodeWasmTransport(optionsJson, {
        modulePath: wasmBindingPath,
        loadModule: () => Promise.resolve(requireFromPackage(wasmBindingPath)),
      }),
  });
}

export {
  MermanDisposedError,
  MermanError,
  MermanInvalidTransportError,
  MermanLifecycleError,
  MermanMissingPlatformPackageError,
  MermanNativeLoadError,
  MermanOperationError,
  MermanQueueSaturatedError,
  MermanUnsupportedTargetError,
} from "./errors.mjs";
