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
    const baseRequest = JSON.parse(options.requestJson);
    const deadlineRequestJson = JSON.stringify({
      ...baseRequest,
      operation_control: { timeout_ms: 0 },
    });
    assertCancellationResponse(
      await executeCandidate(
        engine,
        options.candidate,
        deadlineRequestJson,
        { timeoutMs: 0 },
      ),
      expectation,
      "deadline_exceeded",
      `${options.candidate} deadline`,
      "admission",
    );

    if (options.candidate === "napi") {
      await assertNapiCancellationBridge(engine, expectation);
    }

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

function executeCandidate(engine, candidate, requestJson, { signal, timeoutMs } = {}) {
  if (candidate === "napi") return engine.execute(requestJson, signal, timeoutMs);
  if (signal !== undefined) {
    throw new Error("The Node-targeted WASM candidate cannot observe a mid-call AbortSignal.");
  }
  return engine.execute(requestJson, timeoutMs);
}

function operationError(
  responseJson,
  expectation,
  label,
  { allowedCancellationReasons = [] } = {},
) {
  let cause;
  try {
    decodeWireResponse(responseJson, expectation, { allowedCancellationReasons });
  } catch (error) {
    cause = error;
  }
  if (cause instanceof MermanOperationError) return cause;
  if (cause !== undefined) throw cause;
  throw new Error(`${label} unexpectedly succeeded.`);
}

function assertCancellationResponse(
  responseJson,
  expectation,
  expectedReason,
  label,
  expectedPhase = null,
) {
  const cause = operationError(responseJson, expectation, label, {
    allowedCancellationReasons: [expectedReason],
  });
  const phase = cause.cancellationDetails?.phase;
  if (
    cause.codeName !== "MERMAN_CANCELLED" ||
    cause.cancellationDetails?.reason !== expectedReason ||
    typeof phase !== "string" ||
    phase.length === 0 ||
    (expectedPhase !== null && phase !== expectedPhase)
  ) {
    throw new Error(`${label} did not preserve canonical cancellation details.`);
  }
  return cause.cancellationDetails;
}

async function assertNapiCancellationBridge(engine, expectation) {
  // This real-addon smoke proves the AbortSignal-to-OperationControl bridge and canonical error
  // envelope. The deterministic after-start lifecycle case belongs to the public API contract
  // test; guessing libuv worker start from a timer would make this build probe flaky.
  const requestJson = JSON.stringify({
    operation_id: "semantic-json",
    source: largeFlowchartSource(8_000),
    uri: null,
  });
  const controller = new AbortController();
  const pending = engine.execute(requestJson, controller.signal);
  controller.abort();
  assertCancellationResponse(
    await pending,
    expectation,
    "requested",
    "napi AbortSignal bridge",
  );
}

function largeFlowchartSource(edgeCount) {
  const lines = ["flowchart TD"];
  for (let index = 0; index < edgeCount; index += 1) {
    lines.push(`N${index} --> N${index + 1}`);
  }
  return lines.join("\n");
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
