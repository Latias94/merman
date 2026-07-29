import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  prepareRequestOverlayInputs,
  summarizeTimingSamples,
  validateRequestOverlayWorkerResult,
} from "./request-overlay-contract.mjs";
import { stableJson } from "../stable-json.mjs";

const requireArtifact = createRequire(import.meta.url);

if (isMainModule()) {
  try {
    const input = readInvocation();
    const result = runRequestOverlayWorker(input);
    process.stdout.write(`${JSON.stringify(result)}\n`);
  } catch (error) {
    console.error(error instanceof Error ? error.stack ?? error.message : String(error));
    process.exitCode = 1;
  }
}

export function runRequestOverlayWorker(
  input,
  {
    loadEngineConstructor = loadRawEngineConstructor,
    collectGarbage = collectGarbageTwice,
    now = () => process.hrtime.bigint(),
  } = {},
) {
  validateInvocation(input);
  if (typeof globalThis.gc !== "function" && collectGarbage === collectGarbageTwice) {
    throw new Error("request-overlay workers require Node --expose-gc");
  }
  const prepared = prepareRequestOverlayInputs(input.manifest);
  const Engine = loadEngineConstructor(input.artifact_path, input.transport);
  collectGarbage();
  const baselineCurrentRssBytes = process.memoryUsage().rss;
  const baselinePeakRssBytes = maxRssBytes();
  const semanticEvidence = collectSemanticEvidence({
    Engine,
    prepared,
    expectedRuntimeCatalogDigest: input.artifact_identity.runtime_catalog_digest,
  });
  const measurements = collectMeasurements({
    Engine,
    prepared,
    sampling: input.sampling,
    collectGarbage,
    now,
  });
  collectGarbage();
  const finalCurrentRssBytes = process.memoryUsage().rss;
  const peakRssBytes = maxRssBytes();
  const result = {
    schema_version: 1,
    lane_id: prepared.manifest.lane_id,
    artifact_key: input.artifact_key,
    revision: input.revision,
    transport: input.transport,
    manifest_digest: prepared.manifest_digest,
    process: {
      pid: process.pid,
      invocation_nonce: input.invocation_nonce,
      parent_invocation_id: input.parent_invocation_id,
      node: process.version,
      platform: process.platform,
      arch: process.arch,
      gc_mode: "exposed-double-before-sample",
      clock: "process.hrtime.bigint",
    },
    artifact: input.artifact_identity,
    semantic_evidence: semanticEvidence,
    measurements,
    rss: {
      method: "process.memoryUsage.rss/process.resourceUsage.maxRSS",
      baseline_current_rss_bytes: baselineCurrentRssBytes,
      baseline_peak_rss_bytes: baselinePeakRssBytes,
      final_current_rss_bytes: finalCurrentRssBytes,
      peak_rss_bytes: peakRssBytes,
      operation_peak_growth_bytes: Math.max(0, peakRssBytes - baselinePeakRssBytes),
    },
  };
  return validateRequestOverlayWorkerResult(result, {
    artifactKey: input.artifact_key,
    artifactIdentity: input.artifact_identity,
    manifest: prepared.manifest,
    sampling: input.sampling,
  });
}

export function measureColdSample({
  Engine,
  baseJson,
  requestJson,
  batchSize,
  collectGarbage,
  now,
}) {
  collectGarbage();
  const totalStart = now();
  const constructStart = now();
  const engine = new Engine(baseJson);
  const constructEnd = now();
  const batchStart = now();
  let raw;
  for (let index = 0; index < batchSize; index += 1) raw = engine.executeSync(requestJson);
  const batchEnd = now();
  const totalEnd = now();
  disposeEngine(engine);
  return {
    raw,
    sample: {
      construct_ns: elapsedNs(constructStart, constructEnd),
      batch_ns: elapsedNs(batchStart, batchEnd),
      total_ns: elapsedNs(totalStart, totalEnd),
    },
  };
}

export function measureReusedSamples({
  Engine,
  baseJson,
  requestJson,
  batchSize,
  warmupIterations,
  samples,
  collectGarbage,
  now,
}) {
  const engine = new Engine(baseJson);
  let raw;
  for (let iteration = 0; iteration < warmupIterations; iteration += 1) {
    for (let index = 0; index < batchSize; index += 1) raw = engine.executeSync(requestJson);
  }
  const timings = [];
  for (let iteration = 0; iteration < samples; iteration += 1) {
    collectGarbage();
    const totalStart = now();
    const batchStart = now();
    for (let index = 0; index < batchSize; index += 1) raw = engine.executeSync(requestJson);
    const batchEnd = now();
    const totalEnd = now();
    timings.push({
      construct_ns: 0,
      batch_ns: elapsedNs(batchStart, batchEnd),
      total_ns: elapsedNs(totalStart, totalEnd),
    });
  }
  disposeEngine(engine);
  return { raw, samples: timings };
}

function collectMeasurements({ Engine, prepared, sampling, collectGarbage, now }) {
  const baseJson = prepared.base_json_by_units["1"];
  const overlays = [];
  for (const overlay of prepared.manifest.overlays) {
    const requestJson = prepared.request_json_by_overlay[overlay.id];
    overlays.push(coldLane({
      Engine,
      baseJson,
      requestJson,
      overlayId: overlay.id,
      batchSize: 1,
      sampleCount: sampling.cold_samples,
      collectGarbage,
      now,
    }));
    overlays.push(reusedLane({
      Engine,
      baseJson,
      requestJson,
      overlayId: overlay.id,
      batchSize: 1,
      warmupIterations: sampling.warmup_iterations,
      sampleCount: sampling.reused_samples,
      collectGarbage,
      now,
    }));
  }

  const scalingRequestJson =
    prepared.request_json_by_overlay[prepared.manifest.scaling.overlay_id];
  const batchScaling = prepared.manifest.scaling.batch_sizes.map((batchSize) => ({
    ...reusedLane({
      Engine,
      baseJson,
      requestJson: scalingRequestJson,
      overlayId: prepared.manifest.scaling.overlay_id,
      batchSize,
      warmupIterations: sampling.warmup_iterations,
      sampleCount: sampling.reused_samples,
      collectGarbage,
      now,
    }),
    batch_size: batchSize,
  }));

  const baseSizeScaling = [];
  for (const units of prepared.manifest.scaling.base_size_units) {
    const scaledBaseJson = prepared.base_json_by_units[String(units)];
    baseSizeScaling.push({
      ...coldLane({
        Engine,
        baseJson: scaledBaseJson,
        requestJson: scalingRequestJson,
        overlayId: prepared.manifest.scaling.overlay_id,
        batchSize: 1,
        sampleCount: sampling.cold_samples,
        collectGarbage,
        now,
      }),
      base_size_units: units,
      base_options_bytes: Buffer.byteLength(scaledBaseJson),
    });
    baseSizeScaling.push({
      ...reusedLane({
        Engine,
        baseJson: scaledBaseJson,
        requestJson: scalingRequestJson,
        overlayId: prepared.manifest.scaling.overlay_id,
        batchSize: 1,
        warmupIterations: sampling.warmup_iterations,
        sampleCount: sampling.reused_samples,
        collectGarbage,
        now,
      }),
      base_size_units: units,
      base_options_bytes: Buffer.byteLength(scaledBaseJson),
    });
  }
  return {
    overlays,
    batch_scaling: batchScaling,
    base_size_scaling: baseSizeScaling,
  };
}

function coldLane({
  Engine,
  baseJson,
  requestJson,
  overlayId,
  batchSize,
  sampleCount,
  collectGarbage,
  now,
}) {
  const samples = [];
  let raw;
  for (let iteration = 0; iteration < sampleCount; iteration += 1) {
    const measured = measureColdSample({
      Engine,
      baseJson,
      requestJson,
      batchSize,
      collectGarbage,
      now,
    });
    raw = measured.raw;
    samples.push(measured.sample);
  }
  assertSuccessRaw(raw, overlayId);
  return {
    overlay_id: overlayId,
    engine_lifecycle: "cold",
    logical_operations_per_sample: batchSize,
    samples,
    summary: summarizeTimingSamples(samples, batchSize),
  };
}

function reusedLane({
  Engine,
  baseJson,
  requestJson,
  overlayId,
  batchSize,
  warmupIterations,
  sampleCount,
  collectGarbage,
  now,
}) {
  const measured = measureReusedSamples({
    Engine,
    baseJson,
    requestJson,
    batchSize,
    warmupIterations,
    samples: sampleCount,
    collectGarbage,
    now,
  });
  assertSuccessRaw(measured.raw, overlayId);
  return {
    overlay_id: overlayId,
    engine_lifecycle: "reused",
    logical_operations_per_sample: batchSize,
    samples: measured.samples,
    summary: summarizeTimingSamples(measured.samples, batchSize),
  };
}

function collectSemanticEvidence({ Engine, prepared, expectedRuntimeCatalogDigest }) {
  const runtimeCatalogDigest = runtimeCatalogProbe(
    Engine,
    prepared.base_json_by_units["1"],
  );
  if (runtimeCatalogDigest !== expectedRuntimeCatalogDigest) {
    throw new Error("loaded artifact runtime catalog differs from its historical receipt");
  }
  const probes = [];
  const baseOne = prepared.base_json_by_units["1"];
  for (const overlay of prepared.manifest.overlays) {
    probes.push(successProbe({
      Engine,
      baseJson: baseOne,
      requestJson: prepared.request_json_by_overlay[overlay.id],
      id: `overlay:${overlay.id}`,
      expected: prepared.manifest.success_contract,
    }));
  }
  const versionOnly =
    prepared.request_json_by_overlay[prepared.manifest.scaling.overlay_id];
  for (const units of prepared.manifest.scaling.base_size_units) {
    probes.push(successProbe({
      Engine,
      baseJson: prepared.base_json_by_units[String(units)],
      requestJson: versionOnly,
      id: `base-size:${units}`,
      expected: prepared.manifest.success_contract,
    }));
  }
  const error = rawProbe({
    Engine,
    baseJson: baseOne,
    requestJson: prepared.error_request_json,
  });
  assertEnvelope(error.envelope, prepared.manifest.error_probe.expected, "fixed error probe");
  return {
    passed: true,
    runtime_catalog_digest: runtimeCatalogDigest,
    success_probes: probes,
    error_probe: {
      id: prepared.manifest.error_probe.id,
      raw_sha256: digest(error.raw),
      envelope: error.envelope,
    },
  };
}

function runtimeCatalogProbe(Engine, baseJson) {
  const engine = new Engine(baseJson);
  try {
    if (typeof engine.runtimeCatalogJson !== "function") {
      throw new Error("raw artifact engine does not expose runtimeCatalogJson()");
    }
    const raw = engine.runtimeCatalogJson();
    if (typeof raw !== "string") throw new Error("runtimeCatalogJson() returned a non-string");
    return digestJsonValue(JSON.parse(raw));
  } finally {
    disposeEngine(engine);
  }
}

function successProbe({ Engine, baseJson, requestJson, id, expected }) {
  const probe = rawProbe({ Engine, baseJson, requestJson });
  assertEnvelope(probe.envelope, expected, id);
  return { id, raw_sha256: digest(probe.raw), envelope: probe.envelope };
}

function rawProbe({ Engine, baseJson, requestJson }) {
  const engine = new Engine(baseJson);
  try {
    const raw = engine.executeSync(requestJson);
    if (typeof raw !== "string") throw new Error("executeSync returned a non-string wire result");
    return { raw, envelope: parseEnvelope(raw) };
  } finally {
    disposeEngine(engine);
  }
}

function assertSuccessRaw(raw, label) {
  const envelope = parseEnvelope(raw);
  if (envelope.ok !== true || envelope.result?.data !== '{"type":"info","showInfo":true}') {
    throw new Error(`${label} measurement no longer returns the fixed info semantic result`);
  }
}

function assertEnvelope(actual, expected, label) {
  if (stableJson(actual) !== stableJson(expected)) {
    throw new Error(`${label} differs from the exact request-overlay contract`);
  }
}

function parseEnvelope(raw) {
  try {
    const value = JSON.parse(raw);
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error();
    return value;
  } catch {
    throw new Error("executeSync returned invalid wire JSON");
  }
}

function loadRawEngineConstructor(artifact, transport) {
  const binding = requireArtifact(path.resolve(artifact));
  const Engine = transport === "napi"
    ? binding?.NativeEngine ?? binding?.default?.NativeEngine
    : binding?.WasmEngine ?? binding?.default?.WasmEngine;
  if (typeof Engine !== "function") {
    throw new Error(`${transport} artifact does not export its raw engine constructor`);
  }
  return Engine;
}

export function collectGarbageTwice() {
  globalThis.gc();
  globalThis.gc();
}

function disposeEngine(engine) {
  engine?.dispose?.();
}

function elapsedNs(start, end) {
  const value = Number(end - start);
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error("hrtime duration exceeds the safe nanosecond range");
  }
  return value;
}

function maxRssBytes() {
  return process.resourceUsage().maxRSS * 1024;
}

function digest(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}

function digestJsonValue(value) {
  return `sha256:${createHash("sha256").update(stableJson(value)).digest("hex")}`;
}

function validateInvocation(input) {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw new Error("request-overlay worker invocation must be an object");
  }
  if (
    input.schema_version !== 1 ||
    input.artifact_key !== `${input.revision}:${input.transport}` ||
    !new Set(["base", "head"]).has(input.revision) ||
    !new Set(["napi", "node-wasm"]).has(input.transport) ||
    typeof input.artifact_path !== "string" ||
    input.artifact_path.length === 0 ||
    !/^[0-9a-f]{32}$/.test(input.invocation_nonce ?? "") ||
    !/^[0-9a-f]{32}$/.test(input.parent_invocation_id ?? "")
  ) {
    throw new Error("request-overlay worker invocation identity is invalid");
  }
}

function readInvocation() {
  const bytes = readFileSync(0, "utf8");
  if (!bytes.endsWith("\n") || bytes.slice(0, -1).includes("\n")) {
    throw new Error("request-overlay worker expects one newline-terminated JSON object");
  }
  return JSON.parse(bytes.slice(0, -1));
}

function isMainModule() {
  return process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}
