import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

import {
  MermanEngine,
  createNodeEngine as createEngineWithTransport,
} from "./engine.mjs";
import { loadNodeWasmTransport } from "./candidates/wasm.mjs";
import { nodeLoaderLocalLayout } from "./native-loader.mjs";

const wasmBindingPath = fileURLToPath(
  new URL("../artifact/merman_node.cjs", import.meta.url),
);
const requireFromPackage = createRequire(import.meta.url);
const localLayout = nodeLoaderLocalLayout("@mermanjs/node-wasm");

export { MermanEngine };

export function createNodeEngine(options) {
  return createEngineWithTransport(options, {
    localLayout,
    loadTransport: (optionsJson, iconPacksJson) =>
      loadNodeWasmTransport(optionsJson, {
        modulePath: wasmBindingPath,
        loadModule: () => Promise.resolve(requireFromPackage(wasmBindingPath)),
        localLayout,
      }, iconPacksJson),
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
