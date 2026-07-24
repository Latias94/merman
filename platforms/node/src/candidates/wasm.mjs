import path from "node:path";
import { pathToFileURL } from "node:url";

import {
  MermanInvalidTransportError,
  decodeWireCreationError,
} from "../errors.mjs";
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
  try {
    return wrapCandidateEngine(
      new WasmEngine(optionsJson),
      "The Node-targeted WASM candidate",
    );
  } catch (cause) {
    throw decodeWireCreationError(cause, "The Node-targeted WASM candidate");
  }
}
