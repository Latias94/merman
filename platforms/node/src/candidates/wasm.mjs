import path from "node:path";
import { pathToFileURL } from "node:url";

import {
  MermanInvalidTransportError,
  decodeWireCreationError,
  validateTransportIdentityExport,
} from "../errors.mjs";
import {
  assertRuntimePackageVersion,
  nodeLoaderPackageVersion,
} from "../native-loader.mjs";
import { wrapCandidateEngine } from "./wrap-engine.mjs";

export async function loadNodeWasmTransport(
  optionsJson,
  {
    modulePath = process.env.MERMAN_NODE_WASM_BINDING,
    loadModule = (specifier) => import(specifier),
  } = {},
) {
  if (!modulePath) {
    throw new MermanInvalidTransportError(
      "MERMAN_NODE_WASM_BINDING must point to the explicit Node-targeted WASM candidate.",
    );
  }
  if (modulePath.includes("@mermanjs/web") || modulePath.includes("platforms/web")) {
    throw new MermanInvalidTransportError(
      "Browser WASM packages cannot be used as the Node-targeted WASM candidate.",
    );
  }
  const specifier = /^[a-zA-Z][a-zA-Z+.-]*:/.test(modulePath)
    ? modulePath
    : pathToFileURL(path.resolve(modulePath)).href;
  const binding = await loadModule(specifier);
  const WasmEngine = binding?.WasmEngine ?? binding?.default?.WasmEngine;
  if (typeof WasmEngine !== "function") {
    throw new MermanInvalidTransportError(
      "The Node-targeted WASM candidate does not export WasmEngine.",
    );
  }
  validateTransportIdentityExport(binding?.transportIdentityJson, {
    expectedPackageVersion: nodeLoaderPackageVersion(),
    expectedTransport: "wasm",
    label: "The Node-targeted WASM candidate",
  });
  let transport;
  try {
    transport = wrapCandidateEngine(
      new WasmEngine(optionsJson),
      "The Node-targeted WASM candidate",
    );
  } catch (cause) {
    throw decodeWireCreationError(cause, "The Node-targeted WASM candidate");
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
