import { performance } from "node:perf_hooks";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

import { svgTransportEvidence } from "./svg-signature.mjs";
import { digestJson } from "../stable-json.mjs";

const SMOKE_SOURCE = "flowchart TD\nA-->B";

try {
  const inputPath = process.argv[2];
  if (!inputPath) throw new Error("benchmark worker requires an input JSON path");
  const input = JSON.parse(readFileSync(inputPath, "utf8"));
  const output =
    input.mode === "cold"
      ? await runCold(input)
      : input.mode === "shutdown"
        ? await runShutdown(input)
        : await runWarm(input);
  process.stdout.write(`${JSON.stringify(output)}\n`);
} catch (error) {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
}

async function runCold(input) {
  const baselineBytes = process.memoryUsage().rss;
  const started = performance.now();
  const engine = await createProductEngine(
    input,
    { bindingOptions: input.bindingOptions, concurrency: 1, maxQueue: 1 },
  );
  const outcome = await renderOutcome(engine, input.cases[0], input.operationOptions);
  const operationMs = performance.now() - started;
  await engine.dispose();
  return {
    operation_ms: operationMs,
    baseline_rss_bytes: baselineBytes,
    peak_rss_bytes: maxRssBytes(),
    outcome,
  };
}

async function runShutdown(input) {
  const engine = await createProductEngine(
    input,
    { bindingOptions: input.bindingOptions, concurrency: 1, maxQueue: 1 },
  );
  const outcome = await renderOutcome(
    engine,
    { path: "process-shutdown-smoke.mmd", source: SMOKE_SOURCE },
    input.operationOptions,
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
      await renderOutcome(engine, item, input.operationOptions);
    }
  }

  const samplesMs = [];
  const samples = [];
  for (let iteration = 0; iteration < input.iterations; iteration += 1) {
    for (const item of input.cases) {
      const started = performance.now();
      const outcome = await renderOutcome(engine, item, input.operationOptions);
      const elapsedMs = performance.now() - started;
      samplesMs.push(elapsedMs);
      samples.push({
        iteration,
        path: item.path,
        elapsed_ms: elapsedMs,
        outcome,
      });
    }
  }

  const concurrencySamplesMs = [];
  const concurrencySamples = [];
  const batch = Array.from(
    { length: input.concurrency },
    (_, index) => input.cases[index % input.cases.length],
  );
  for (let iteration = 0; iteration < input.concurrencyIterations; iteration += 1) {
    const started = performance.now();
    const batchOutcomes = await Promise.all(
      batch.map((item) => renderOutcome(engine, item, input.operationOptions)),
    );
    const elapsedMs = performance.now() - started;
    concurrencySamplesMs.push(elapsedMs);
    concurrencySamples.push({ iteration, elapsed_ms: elapsedMs, outcomes: batchOutcomes });
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
    concurrency_samples_ms: concurrencySamplesMs,
    concurrency_samples: concurrencySamples,
    baseline_rss_bytes: baselineBytes,
    peak_rss_bytes: maxRssBytes(),
    queue_lifecycle: queueLifecycle,
    error_behavior: errorBehavior,
  };
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
  const executingController = new AbortController();
  const queuedController = new AbortController();
  const executing = abortEngine.renderSvg(source, {
    optionsJson,
    signal: executingController.signal,
  });
  const queuedAbort = abortEngine.renderSvg(source, {
    optionsJson,
    signal: queuedController.signal,
  });
  const executionResult = lifecycleSettlement(executing);
  const queuedAbortResult = lifecycleSettlement(queuedAbort);
  executingController.abort();
  queuedController.abort();
  const [executionSettlement, queuedAbortSettlement, abortDisposeSettlement] = await Promise.all([
    executionResult,
    queuedAbortResult,
    lifecycleSettlement(executing.finally(() => abortEngine.dispose())),
  ]);
  const queuedAbortPassed =
    queuedAbortSettlement.status === "rejected" &&
    queuedAbortSettlement.error.name === "AbortError";
  const executionPassed = executionSettlement.status === "fulfilled";
  const abortDisposed = abortDisposeSettlement.status === "fulfilled";

  return {
    saturation_passed: saturationPassed,
    dispose_passed: disposePassed,
    queued_abort_passed: queuedAbortPassed && abortDisposed,
    non_preemptive_abort_passed: executionPassed && abortDisposed,
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
  if (typeof facade.createNodeEngine !== "function") {
    throw new Error(`${input.candidate} product entrypoint does not export createNodeEngine().`);
  }
  return facade.createNodeEngine(options);
}

async function renderOutcome(engine, item, operationOptions) {
  try {
    const result = await engine.executeOperation({
      operationId: "svg",
      source: item.source,
      optionsJson: JSON.stringify(operationOptions),
    });
    return {
      path: item.path,
      ok: true,
      operation_id: result.operation_id,
      media_type: result.media_type,
      sha256: digest(result.data),
      bytes: Buffer.byteLength(result.data),
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

async function corpusOutcome(engine, item, operationOptions) {
  const semantic = await semanticOutcome(engine, item, operationOptions);
  try {
    const result = await engine.executeOperation({
      operationId: "svg",
      source: item.source,
      optionsJson: JSON.stringify(operationOptions),
    });
    const svgEvidence = svgTransportEvidence(result.data);
    return {
      path: item.path,
      ok: true,
      operation_id: result.operation_id,
      media_type: result.media_type,
      sha256: digest(result.data),
      svg_structure_sha256: svgEvidence.structure_sha256,
      svg_geometry_sha256: svgEvidence.geometry_sha256,
      bytes: Buffer.byteLength(result.data),
      semantic,
    };
  } catch (error) {
    return {
      path: item.path,
      ok: false,
      ...errorEvidence(error),
      semantic,
    };
  }
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
