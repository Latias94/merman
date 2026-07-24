import {
  MermanInvalidTransportError,
  decodeWireCreationError,
} from "../errors.mjs";
import { loadNativeBinding } from "../native-loader.mjs";
import { wrapCandidateEngine } from "./wrap-engine.mjs";

export async function loadNativeTransport(optionsJson, loaderOptions = {}) {
  const binding = loadNativeBinding(loaderOptions);
  const NativeEngine = binding?.NativeEngine ?? binding?.default?.NativeEngine;
  if (typeof NativeEngine !== "function") {
    throw new MermanInvalidTransportError(
      "The target-specific Merman package does not export NativeEngine.",
    );
  }
  try {
    return wrapCandidateEngine(
      new NativeEngine(optionsJson),
      "The target-specific Merman addon",
    );
  } catch (cause) {
    throw decodeWireCreationError(cause, "The target-specific Merman addon");
  }
}
