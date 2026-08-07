import { createRequire } from "node:module";
import path from "node:path";

import {
  MermanOperationError,
  decodeWireInvocationError,
  decodeWireResponse,
  validateTransportIdentityJson,
} from "../src/errors.mjs";
import { BINDING_OPERATION_EXPECTATIONS } from "../src/generated/binding-contract.mjs";
import { nodeLoaderPackageVersion } from "../src/native-loader.mjs";

const requireFromProbe = createRequire(import.meta.url);

try {
  const options = parseArgs(process.argv.slice(2));
  const binding = requireFromProbe(path.resolve(options.artifact));
  const transportKind = options.candidate === "napi" ? "napi" : "wasm";
  if (typeof binding?.transportIdentityJson !== "function") {
    throw new Error(`${options.candidate} candidate does not export transportIdentityJson().`);
  }
  validateTransportIdentityJson(binding.transportIdentityJson(), {
    expectedPackageVersion: nodeLoaderPackageVersion(),
    expectedTransport: transportKind,
  });
  const Engine = options.candidate === "napi"
    ? binding?.NativeEngine ?? binding?.default?.NativeEngine
    : binding?.WasmEngine ?? binding?.default?.WasmEngine;
  if (typeof Engine !== "function") {
    throw new Error(`${options.candidate} candidate does not export its engine constructor.`);
  }
  const expectation = BINDING_OPERATION_EXPECTATIONS.find(
    ({ operation_id }) => operation_id === "semantic-json",
  );
  if (!expectation) throw new Error("generated operation contract lacks semantic-json.");

  const engine = new Engine(JSON.stringify({
    version: 2,
    runtime_policy: "deterministic",
    resources: { profile: "interactive" },
  }));
  let disposed = false;
  try {
    const result = decodeWireResponse(await engine.execute(options.requestJson), expectation);
    JSON.parse(result.data);
    engine.dispose();
    disposed = true;
    for (const [method, invoke] of [
      ["execute", () => engine.execute(options.requestJson)],
      ["executeSync", () => engine.executeSync(options.requestJson)],
      ["runtimeCatalogJson", () => engine.runtimeCatalogJson()],
      ["metadataJson", () => engine.metadataJson("supported-diagrams")],
    ]) {
      await assertDisposedFailure(options.candidate, method, invoke);
    }
    engine.dispose();
    process.stdout.write(`${JSON.stringify({
      semantic_json_bytes: Buffer.byteLength(result.data),
    })}\n`);
  } finally {
    if (!disposed) {
      try {
        engine.dispose?.();
      } catch {
        // Preserve the probe failure that made the candidate unusable.
      }
    }
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}

async function assertDisposedFailure(candidate, method, invoke) {
  let cause;
  try {
    await invoke();
  } catch (error) {
    cause = error;
  }
  const decoded = cause === undefined
    ? null
    : decodeWireInvocationError(cause, `${candidate} candidate ${method}`);
  if (
    !(decoded instanceof MermanOperationError) ||
    decoded.codeName !== "MERMAN_INVALID_ARGUMENT" ||
    decoded.kind !== "generic" ||
    !/disposed/i.test(decoded.message)
  ) {
    throw new Error(`${candidate} candidate ${method} did not fail closed after dispose.`);
  }
}

function parseArgs(args) {
  const artifact = valueAfter(args, "--artifact");
  const candidate = valueAfter(args, "--candidate");
  const requestJson = valueAfter(args, "--request-json");
  if (!artifact || !requestJson || !["napi", "node-wasm"].includes(candidate)) {
    throw new Error(
      "usage: probe-candidate-async.mjs --artifact <path> --candidate <napi|node-wasm> --request-json <json>",
    );
  }
  return { artifact, candidate, requestJson };
}

function valueAfter(args, flag) {
  const index = args.indexOf(flag);
  return index === -1 ? null : args[index + 1] ?? null;
}
