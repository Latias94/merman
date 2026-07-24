import { createRequire } from "node:module";
import { performance } from "node:perf_hooks";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

import { loadNativeTransport } from "../../src/candidates/native.mjs";
import { loadNodeWasmTransport } from "../../src/candidates/wasm.mjs";
import { createNodeEngine } from "../../src/engine.mjs";
import {
  MermanDisposedError,
  MermanQueueSaturatedError,
} from "../../src/errors.mjs";
import { svgTransportEvidence } from "./svg-signature.mjs";
import { digestJson } from "../stable-json.mjs";

const requireFromWorker = createRequire(import.meta.url);

try {
  const inputPath = process.argv[2];
  if (!inputPath) throw new Error("benchmark worker requires an input JSON path");
  const input = JSON.parse(readFileSync(inputPath, "utf8"));
  const output = input.mode === "cold" ? await runCold(input) : await runWarm(input);
  process.stdout.write(`${JSON.stringify(output)}\n`);
} catch (error) {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
}

async function runCold(input) {
  const baselineBytes = process.memoryUsage().rss;
  const loadTransport = candidateLoader(input);
  const started = performance.now();
  const engine = await createNodeEngine(
    { bindingOptions: input.bindingOptions, concurrency: 1, maxQueue: 1 },
    { loadTransport },
  );
  const outcome = await renderOutcome(engine, input.cases[0], input.formatOptions);
  const operationMs = performance.now() - started;
  await engine.dispose();
  return {
    operation_ms: operationMs,
    baseline_rss_bytes: baselineBytes,
    peak_rss_bytes: maxRssBytes(),
    outcome,
  };
}

async function runWarm(input) {
  const baselineBytes = process.memoryUsage().rss;
  const loadTransport = candidateLoader(input);
  const engine = await createNodeEngine(
    {
      bindingOptions: input.bindingOptions,
      concurrency: input.concurrency,
      maxQueue: input.maxQueue,
    },
    { loadTransport },
  );

  for (let iteration = 0; iteration < input.warmupIterations; iteration += 1) {
    for (const item of input.cases) {
      await renderOutcome(engine, item, input.formatOptions);
    }
  }

  const samplesMs = [];
  const samples = [];
  for (let iteration = 0; iteration < input.iterations; iteration += 1) {
    for (const item of input.cases) {
      const started = performance.now();
      const outcome = await renderOutcome(engine, item, input.formatOptions);
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
  const batch = Array.from(
    { length: input.concurrency },
    (_, index) => input.cases[index % input.cases.length],
  );
  for (let iteration = 0; iteration < input.concurrencyIterations; iteration += 1) {
    const started = performance.now();
    await Promise.all(batch.map((item) => renderOutcome(engine, item, input.formatOptions)));
    concurrencySamplesMs.push(performance.now() - started);
  }
  const outcomes = [];
  for (const item of input.cases) {
    outcomes.push(await corpusOutcome(engine, item, input.formatOptions));
  }
  const errorBehavior = await probeTypedErrors(engine, input, loadTransport);
  await engine.dispose();

  const queueLifecycle = await probeQueueLifecycle(input, loadTransport);
  return {
    outcomes,
    samples_ms: samplesMs,
    samples,
    concurrency_samples_ms: concurrencySamplesMs,
    baseline_rss_bytes: baselineBytes,
    peak_rss_bytes: maxRssBytes(),
    queue_lifecycle: queueLifecycle,
    error_behavior: errorBehavior,
  };
}

async function probeTypedErrors(engine, input, loadTransport) {
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
    text_measurement_callback_rejected: await probeTextMeasurementPolicy(input, loadTransport),
  };
}

async function probeTextMeasurementPolicy(input, loadTransport) {
  try {
    const unexpected = await createNodeEngine(
      {
        bindingOptions: {
          ...input.bindingOptions,
          textMeasurement: () => ({ width: 1, height: 1 }),
        },
      },
      { loadTransport },
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

async function probeQueueLifecycle(input, loadTransport) {
  const saturationEngine = await createNodeEngine(
    { bindingOptions: input.bindingOptions, concurrency: 1, maxQueue: 1 },
    { loadTransport },
  );
  const active = saturationEngine.renderSvg(input.cases[0].source, {
    formatOptions: input.formatOptions,
  });
  const queued = saturationEngine.renderSvg(input.cases[0].source, {
    formatOptions: input.formatOptions,
  });
  const saturated = saturationEngine.renderSvg(input.cases[0].source, {
    formatOptions: input.formatOptions,
  });
  const saturationPassed = await rejectsAs(saturated, MermanQueueSaturatedError);
  await Promise.allSettled([active, queued]);
  await saturationEngine.dispose();

  const disposeEngine = await createNodeEngine(
    { bindingOptions: input.bindingOptions, concurrency: 1, maxQueue: 1 },
    { loadTransport },
  );
  const disposingActive = disposeEngine.renderSvg(input.cases[0].source, {
    formatOptions: input.formatOptions,
  });
  const disposingQueued = disposeEngine.renderSvg(input.cases[0].source, {
    formatOptions: input.formatOptions,
  });
  const disposing = disposeEngine.dispose();
  const disposePassed = await rejectsAs(disposingQueued, MermanDisposedError);
  await Promise.allSettled([disposingActive, disposing]);

  const abortEngine = await createNodeEngine(
    { bindingOptions: input.bindingOptions, concurrency: 1, maxQueue: 1 },
    { loadTransport },
  );
  const controller = new AbortController();
  const executing = abortEngine.renderSvg(input.cases[0].source, {
    formatOptions: input.formatOptions,
    signal: controller.signal,
  });
  controller.abort();
  const executionResult = await executing.then(
    () => ({ aborted: false }),
    (error) => ({ aborted: error?.name === "AbortError" }),
  );
  await abortEngine.dispose();

  return {
    saturation_passed: saturationPassed,
    dispose_passed: disposePassed,
    non_preemptive_abort_passed: !executionResult.aborted,
  };
}

function candidateLoader(input) {
  if (input.candidate === "napi") {
    const binding = requireFromWorker(input.artifact);
    return (optionsJson) =>
      loadNativeTransport(optionsJson, { loadPackage: () => binding });
  }
  if (input.candidate === "node-wasm") {
    return (optionsJson) =>
      loadNodeWasmTransport(optionsJson, { modulePath: input.artifact });
  }
  throw new Error(`unknown benchmark candidate: ${input.candidate}`);
}

async function renderOutcome(engine, item, formatOptions) {
  try {
    const result = await engine.executeOperation({
      operationId: "svg",
      source: item.source,
      formatOptions,
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

async function corpusOutcome(engine, item, formatOptions) {
  const semantic = await semanticOutcome(engine, item);
  try {
    const result = await engine.executeOperation({
      operationId: "svg",
      source: item.source,
      formatOptions,
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

async function semanticOutcome(engine, item) {
  try {
    const result = await engine.executeOperation({
      operationId: "semantic-json",
      source: item.source,
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

async function rejectsAs(promise, ErrorType) {
  return promise.then(
    () => false,
    (error) => error instanceof ErrorType,
  );
}

function digest(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}

function maxRssBytes() {
  return process.resourceUsage().maxRSS * 1024;
}
