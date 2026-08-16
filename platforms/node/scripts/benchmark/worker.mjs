import { performance } from "node:perf_hooks";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { svgTransportEvidence } from "./svg-signature.mjs";
import { digestJson } from "../stable-json.mjs";

const SMOKE_SOURCE = "flowchart TD\nA-->B";
const PRODUCT_EXPORTS = [
  "MermanDisposedError",
  "MermanEngine",
  "MermanError",
  "MermanInvalidTransportError",
  "MermanLifecycleError",
  "MermanMissingPlatformPackageError",
  "MermanOperationError",
  "MermanQueueSaturatedError",
  "MermanUnsupportedTargetError",
  "createNodeEngine",
];

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main();
}

async function main() {
  try {
    const inputPath = process.argv[2];
    if (!inputPath) throw new Error("benchmark worker requires an input JSON path");
    const input = JSON.parse(readFileSync(inputPath, "utf8"));
    const output =
      input.mode === "cold"
        ? await runCold(input)
        : input.mode === "concurrency"
          ? await runConcurrency(input)
          : input.mode === "shutdown"
            ? await runShutdown(input)
            : await runWarm(input);
    process.stdout.write(`${JSON.stringify(output)}\n`);
  } catch (error) {
    console.error(error instanceof Error ? error.stack ?? error.message : String(error));
    process.exitCode = 1;
  }
}

async function runCold(input) {
  const baselineBytes = process.memoryUsage().rss;
  const started = performance.now();
  const engine = await createProductEngine(
    input,
    { bindingOptions: input.bindingOptions, concurrency: 1, maxQueue: 1 },
  );
  const execution = await executeSvg(engine, input.workload, input.operationOptions);
  const operationMs = performance.now() - started;
  const result = rawExecutionResult(input.workload, execution);
  await engine.dispose();
  return {
    operation_ms: operationMs,
    baseline_rss_bytes: baselineBytes,
    peak_rss_bytes: maxRssBytes(),
    result,
  };
}

async function runConcurrency(input) {
  const engine = await createProductEngine(
    input,
    {
      bindingOptions: input.bindingOptions,
      concurrency: input.concurrency,
      maxQueue: input.maxQueue,
    },
  );
  const batch = Array.from({ length: input.concurrency }, () => input.workload);
  for (let iteration = 0; iteration < input.warmupIterations; iteration += 1) {
    await Promise.all(
      batch.map((item) => executeSvg(engine, item, input.operationOptions)),
    );
  }
  const batchSamplesMs = [];
  const samples = [];
  for (let iteration = 0; iteration < input.concurrencyIterations; iteration += 1) {
    const started = performance.now();
    const executions = await Promise.all(
      batch.map((item) => executeSvg(engine, item, input.operationOptions)),
    );
    const elapsedMs = performance.now() - started;
    batchSamplesMs.push(elapsedMs);
    samples.push({
      iteration,
      elapsed_ms: elapsedMs,
      results: executions.map((execution, index) =>
        rawExecutionResult(batch[index], execution)),
    });
  }
  await engine.dispose();
  return { batch_samples_ms: batchSamplesMs, samples };
}

async function runShutdown(input) {
  const engine = await createProductEngine(
    input,
    { bindingOptions: input.bindingOptions, concurrency: 1, maxQueue: 1 },
  );
  const item = { path: "process-shutdown-smoke.mmd", source: SMOKE_SOURCE };
  const outcome = renderOutcome(
    item,
    await executeSvg(engine, item, input.operationOptions),
  );
  if (!outcome.ok) {
    throw new Error(`process-shutdown probe failed to render: ${outcome.message ?? outcome.kind}`);
  }
  // Deliberately do not call dispose(): successful worker exit proves an idle engine does not
  // retain event-loop handles that would hang a one-shot SSG process.
  return {
    process_shutdown_passed: true,
    evidence: {
      render_succeeded: true,
      dispose_called: false,
    },
  };
}

async function runWarm(input) {
  const baselineBytes = process.memoryUsage().rss;
  const engine = await createProductEngine(
    input,
    {
      bindingOptions: input.bindingOptions,
      concurrency: input.concurrency,
      maxQueue: input.maxQueue,
    },
  );

  for (let iteration = 0; iteration < input.warmupIterations; iteration += 1) {
    for (const item of input.cases) {
      await executeSvg(engine, item, input.operationOptions);
    }
  }

  const samplesMs = [];
  const samples = [];
  for (let iteration = 0; iteration < input.iterations; iteration += 1) {
    for (const item of input.cases) {
      const { elapsedMs, outcome } = await measureWarmSample({
        execute: () => executeSvg(engine, item, input.operationOptions),
        project: (execution) => timedRenderOutcome(item, execution),
      });
      samplesMs.push(elapsedMs);
      samples.push({
        iteration,
        path: item.path,
        elapsed_ms: elapsedMs,
        outcome,
      });
    }
  }

  const outcomes = [];
  for (const item of input.cases) {
    outcomes.push(await corpusOutcome(engine, item, input.operationOptions));
  }
  const errorBehavior = await probeTypedErrors(engine, input);
  await engine.dispose();

  const queueLifecycle = await probeQueueLifecycle(input);
  return {
    outcomes,
    samples_ms: samplesMs,
    samples,
    baseline_rss_bytes: baselineBytes,
    peak_rss_bytes: maxRssBytes(),
    queue_lifecycle: queueLifecycle,
    error_behavior: errorBehavior,
  };
}

export async function measureWarmSample({
  execute,
  project,
  now = () => performance.now(),
}) {
  const started = now();
  const execution = await execute();
  const elapsedMs = now() - started;
  const outcome = project(execution);
  return { elapsedMs, outcome };
}

async function probeTypedErrors(engine, input) {
  const source = "flowchart TD\nA";
  const unknownOperation = await operationError(engine, {
    operationId: "bitmap",
    source,
  });
  const missingCapability = await operationError(engine, {
    operationId: "png",
    source,
  });
  return {
    unknown_operation: unknownOperation,
    missing_capability: missingCapability,
    text_measurement_callback_rejected: await probeTextMeasurementPolicy(input),
  };
}

async function probeTextMeasurementPolicy(input) {
  try {
    const unexpected = await createProductEngine(
      input,
      {
        bindingOptions: {
          ...input.bindingOptions,
          textMeasurement: () => ({ width: 1, height: 1 }),
        },
      },
    );
    await unexpected.dispose();
    return false;
  } catch (error) {
    return error instanceof TypeError && /text measurement callbacks are not supported/i.test(error.message);
  }
}

async function operationError(engine, request) {
  try {
    await engine.executeOperation(request);
    return { kind: null, capability_id: null, unexpected_success: true };
  } catch (error) {
    return {
      kind: error?.kind ?? "generic",
      capability_id: error?.capabilityId ?? null,
      code_name: error?.codeName ?? error?.code ?? null,
    };
  }
}

async function probeQueueLifecycle(input) {
  const source = SMOKE_SOURCE;
  const optionsJson = JSON.stringify(input.operationOptions);
  const saturationEngine = await createProductEngine(
    input,
    { bindingOptions: input.bindingOptions, concurrency: 1, maxQueue: 1 },
  );
  const active = saturationEngine.renderSvg(source, {
    optionsJson,
  });
  const queued = saturationEngine.renderSvg(source, {
    optionsJson,
  });
  const saturated = saturationEngine.renderSvg(source, {
    optionsJson,
  });
  const saturatedSettlement = await lifecycleSettlement(saturated);
  const [activeSettlement, queuedSettlement] = await Promise.all([
    lifecycleSettlement(active),
    lifecycleSettlement(queued),
  ]);
  const saturationDisposeSettlement = await lifecycleSettlement(saturationEngine.dispose());
  const saturationPassed =
    saturatedSettlement.status === "rejected" &&
    saturatedSettlement.error.code === "MERMAN_QUEUE_SATURATED" &&
    activeSettlement.status === "fulfilled" &&
    queuedSettlement.status === "fulfilled" &&
    saturationDisposeSettlement.status === "fulfilled";

  const disposeEngine = await createProductEngine(
    input,
    { bindingOptions: input.bindingOptions, concurrency: 1, maxQueue: 1 },
  );
  const disposingActive = disposeEngine.renderSvg(source, {
    optionsJson,
  });
  const disposingQueued = disposeEngine.renderSvg(source, {
    optionsJson,
  });
  const disposing = disposeEngine.dispose();
  const [disposingQueuedSettlement, disposingActiveSettlement, disposeSettlement] =
    await Promise.all([
      lifecycleSettlement(disposingQueued),
      lifecycleSettlement(disposingActive),
      lifecycleSettlement(disposing),
    ]);
  const disposePassed =
    disposingQueuedSettlement.status === "rejected" &&
    disposingQueuedSettlement.error.code === "MERMAN_ENGINE_DISPOSED" &&
    disposingActiveSettlement.status === "fulfilled" &&
    disposeSettlement.status === "fulfilled";

  const abortEngine = await createProductEngine(
    input,
    { bindingOptions: input.bindingOptions, concurrency: 1, maxQueue: 1 },
  );
  const queuedController = new AbortController();
  const executing = abortEngine.renderSvg(source, {
    optionsJson,
  });
  const queuedAbort = abortEngine.renderSvg(source, {
    optionsJson,
    signal: queuedController.signal,
  });
  const executionResult = lifecycleSettlement(executing);
  const queuedAbortResult = lifecycleSettlement(queuedAbort);
  const disposeAfterExecuting = executing.then(
    () => abortEngine.dispose(),
    () => abortEngine.dispose(),
  );
  queuedController.abort();
  const [executionSettlement, queuedAbortSettlement, abortDisposeSettlement] = await Promise.all([
    executionResult,
    queuedAbortResult,
    lifecycleSettlement(disposeAfterExecuting),
  ]);
  const queuedAbortPassed =
    queuedAbortSettlement.status === "rejected" &&
    queuedAbortSettlement.error.name === "AbortError";
  const abortDisposed = abortDisposeSettlement.status === "fulfilled";

  return {
    saturation_passed: saturationPassed,
    dispose_passed: disposePassed,
    queued_abort_passed: queuedAbortPassed && abortDisposed,
    evidence: {
      saturation: {
        active: activeSettlement,
        queued: queuedSettlement,
        saturated: saturatedSettlement,
        dispose: saturationDisposeSettlement,
      },
      disposal: {
        active: disposingActiveSettlement,
        queued: disposingQueuedSettlement,
        dispose: disposeSettlement,
      },
      abort: {
        executing: executionSettlement,
        queued: queuedAbortSettlement,
        dispose: abortDisposeSettlement,
      },
    },
  };
}

async function createProductEngine(input, options) {
  if (typeof input.productModule !== "string" || input.productModule.length === 0) {
    throw new Error(`${input.candidate} benchmark lacks an installed product entrypoint.`);
  }
  const facade = await import(input.productModule);
  if (JSON.stringify(Object.keys(facade).sort()) !== JSON.stringify(PRODUCT_EXPORTS)) {
    throw new Error(`${input.candidate} product entrypoint does not export the Node facade.`);
  }
  return facade.createNodeEngine(options);
}

async function executeSvg(engine, item, operationOptions) {
  try {
    return {
      ok: true,
      result: await engine.executeOperation({
        operationId: "svg",
        source: item.source,
        optionsJson: JSON.stringify(operationOptions),
      }),
    };
  } catch (error) {
    return { ok: false, error };
  }
}

function renderOutcome(item, execution) {
  const outcome = timedRenderOutcome(item, execution);
  if (!outcome.ok) return outcome;
  try {
    const svgEvidence = svgTransportEvidence(execution.result.data);
    return {
      ...outcome,
      svg_structure_sha256: svgEvidence.structure_sha256,
      svg_geometry_sha256: svgEvidence.geometry_sha256,
    };
  } catch (error) {
    return {
      path: item.path,
      ok: false,
      code_name: error?.codeName ?? error?.code ?? null,
      kind: error?.kind ?? "generic",
      capability_id: error?.capabilityId ?? null,
      message: error instanceof Error ? error.message : String(error),
    };
  }
}

function timedRenderOutcome(item, execution) {
  if (!execution.ok) {
    return {
      path: item.path,
      ok: false,
      ...errorEvidence(execution.error),
    };
  }
  return {
    path: item.path,
    ok: true,
    operation_id: execution.result.operation_id,
    media_type: execution.result.media_type,
    sha256: digest(execution.result.data),
    bytes: Buffer.byteLength(execution.result.data),
  };
}

function rawExecutionResult(item, execution) {
  if (!execution.ok) {
    return {
      path: item.path,
      ok: false,
      ...errorEvidence(execution.error),
    };
  }
  return {
    path: item.path,
    ok: true,
    operation_id: execution.result.operation_id,
    media_type: execution.result.media_type,
    data: execution.result.data,
  };
}

async function corpusOutcome(engine, item, operationOptions) {
  const semantic = await semanticOutcome(engine, item, operationOptions);
  return {
    ...renderOutcome(item, await executeSvg(engine, item, operationOptions)),
    semantic,
  };
}

async function semanticOutcome(engine, item, operationOptions) {
  try {
    const result = await engine.executeOperation({
      operationId: "semantic-json",
      source: item.source,
      optionsJson: JSON.stringify(operationOptions),
    });
    return {
      ok: true,
      operation_id: result.operation_id,
      media_type: result.media_type,
      sha256: digestJson(JSON.parse(result.data)),
    };
  } catch (error) {
    return { ok: false, ...errorEvidence(error) };
  }
}

function errorEvidence(error) {
  return {
    code_name: error?.codeName ?? error?.code ?? null,
    kind: error?.kind ?? "generic",
    capability_id: error?.capabilityId ?? null,
    message: error instanceof Error ? error.message : String(error),
  };
}

async function lifecycleSettlement(promise) {
  try {
    await promise;
    return { status: "fulfilled" };
  } catch (error) {
    return {
      status: "rejected",
      error: {
        name: error?.name ?? null,
        code: error?.code ?? null,
        code_name: error?.codeName ?? null,
        kind: error?.kind ?? null,
      },
    };
  }
}

function digest(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}

function maxRssBytes() {
  return process.resourceUsage().maxRSS * 1024;
}
