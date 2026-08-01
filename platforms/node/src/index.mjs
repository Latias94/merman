import { loadNativeTransport } from "./candidates/native.mjs";
import {
  MermanNodeEngine,
  createNodeEngine as createEngineWithTransport,
} from "./engine.mjs";

export { MermanNodeEngine };

export function createNodeEngine(options) {
  return createEngineWithTransport(options, { loadTransport: loadNativeTransport });
}

export {
  MermanDisposedError,
  MermanError,
  MermanLifecycleError,
  MermanMissingPlatformPackageError,
  MermanOperationError,
  MermanQueueSaturatedError,
  MermanUnsupportedTargetError,
} from "./errors.mjs";
