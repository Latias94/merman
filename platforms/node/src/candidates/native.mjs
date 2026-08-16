import {
  MermanInvalidTransportError,
  decodeWireCreationError,
  validateTransportIdentityExport,
} from "../errors.mjs";
import {
  assertRuntimePackageVersion,
  loadNativeBinding,
  nodeLoaderPackageVersion,
} from "../native-loader.mjs";
import { wrapCandidateEngine } from "./wrap-engine.mjs";

export async function loadNativeTransport(optionsJson, loaderOptions = {}) {
  const binding = loadNativeBinding(loaderOptions);
  const NativeEngine = binding?.NativeEngine ?? binding?.default?.NativeEngine;
  if (typeof NativeEngine !== "function") {
    throw new MermanInvalidTransportError(
      "The target-specific Merman package does not export NativeEngine.",
    );
  }
  validateTransportIdentityExport(binding?.transportIdentityJson, {
    expectedPackageVersion: nodeLoaderPackageVersion(),
    expectedTransport: "napi",
    label: "The target-specific Merman package",
  });
  let transport;
  try {
    transport = wrapCandidateEngine(
      new NativeEngine(optionsJson),
      "The target-specific Merman addon",
      { forwardsAbortSignal: true },
    );
  } catch (cause) {
    throw decodeWireCreationError(cause, "The target-specific Merman addon");
  }
  try {
    const runtimeCatalogJson = assertRuntimePackageVersion(
      transport.runtimeCatalogJson(),
    );
    return {
      ...transport,
      runtimeCatalogJson: () => runtimeCatalogJson,
    };
  } catch (cause) {
    try {
      await transport.dispose();
    } catch {
      // Preserve the package-version failure that made the transport unusable.
    }
    throw cause;
  }
}
