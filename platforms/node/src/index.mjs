import { loadNativeTransport } from "./candidates/native.mjs";
import {
  MermanEngine,
  createNodeEngine as createEngineWithTransport,
} from "./engine.mjs";

export { MermanEngine };

export function createNodeEngine(options) {
  return createEngineWithTransport(options, { loadTransport: loadNativeTransport });
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
