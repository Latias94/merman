import { loadInstalledNativeTransport } from "./native-loader.mjs";
import {
  MermanEngine,
  createNodeEngine as createEngineWithTransport,
} from "./engine.mjs";

export { MermanEngine };

export function createNodeEngine(options) {
  return createEngineWithTransport(options, {
    loadTransport: loadInstalledNativeTransport,
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
