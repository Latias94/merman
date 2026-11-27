import { loadInstalledNativeTransport, nodeLoaderLocalLayout } from "./native-loader.mjs";
import {
  MermanEngine,
  createNodeEngine as createEngineWithTransport,
} from "./engine.mjs";

export { MermanEngine };

export function createNodeEngine(options) {
  return createEngineWithTransport(options, {
    loadTransport: loadInstalledNativeTransport,
    localLayout: nodeLoaderLocalLayout(),
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
