import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  prepareRequestOverlayInputs,
  summarizeTimingSamples,
  validateRequestOverlayWorkerResult,
  validateSampling,
} from "./request-overlay-contract.mjs";
import { stableJson } from "../stable-json.mjs";

const requireArtifact = createRequire(import.meta.url);
const parseJson = JSON.parse.bind(JSON);
const stringifyJson = JSON.stringify.bind(JSON);
const toNumber = Number;
const isSafeInteger = Number.isSafeInteger.bind(Number);
const isArray = Array.isArray.bind(Array);
const maximum = Math.max.bind(Math);
const writeStdout = process.stdout.write.bind(process.stdout);
const writeStderr = process.stderr.write.bind(process.stderr);
const REUSED_SEMANTIC_PROBE_ITERATIONS = 32;

if (isMainModule()) {
  try {
    const input = readInvocation();
    const result = runRequestOverlayWorker(input);
    writeStdout(`${stringifyJson(result)}\n`);
  } catch (error) {
    writeStderr(`${error instanceof Error ? error.stack ?? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}

export function runRequestOverlayWorker(
  input,
  {
    loadEngineConstructor = loadRawEngineConstructor,
    collectGarbage,
    now,
  } = {},
) {
  validateInvocation(input);
  validateSampling(input.sampling, input.manifest);
  const trustedRuntime = captureTrustedRuntime();
  if (!trustedRuntime.collectGarbage && collectGarbage === undefined) {
    throw new Error("request-overlay workers require Node --expose-gc");
  }
  const trustedCollectGarbage = collectGarbage ?? trustedRuntime.collectGarbage;
  const trustedNow = now ?? trustedRuntime.now;
  const prepared = prepareRequestOverlayInputs(input.manifest);
  trustedCollectGarbage();
  const rssTracker = createRssTracker({
    readMemoryUsage: trustedRuntime.readMemoryUsage,
    readResourceUsage: trustedRuntime.readResourceUsage,
  });
  const artifactLoadLaneId = "lifecycle:artifact-load";
  rssTracker.beginLane(artifactLoadLaneId);
  const Engine = loadEngineConstructor(input.artifact_path, input.transport);
  rssTracker.observe(artifactLoadLaneId);
  trustedCollectGarbage();
  const semanticEvidence = collectSemanticEvidence({
    Engine,
    prepared,
    expectedRuntimeCatalogDigest: input.artifact_identity.runtime_catalog_digest,
    rssTracker,
  });
  const measurements = collectMeasurements({
    Engine,
    prepared,
    sampling: input.sampling,
    collectGarbage: trustedCollectGarbage,
    now: trustedNow,
    byteLength: trustedRuntime.byteLength,
    rssTracker,
  });
  semanticEvidence.probe_passed = semanticEvidence.passed;
  semanticEvidence.measurement_passed = measurements.semantic_contract_passed;
  semanticEvidence.passed =
    semanticEvidence.probe_passed && semanticEvidence.measurement_passed;
  trustedCollectGarbage();
  const rss = rssTracker.finish();
  const result = {
    schema_version: 2,
    lane_id: prepared.manifest.lane_id,
    artifact_key: input.artifact_key,
    revision: input.revision,
    transport: input.transport,
    manifest_digest: prepared.manifest_digest,
    process: {
      pid: trustedRuntime.identity.pid,
      invocation_nonce: input.invocation_nonce,
      parent_invocation_id: input.parent_invocation_id,
      node: trustedRuntime.identity.node,
      platform: trustedRuntime.identity.platform,
      arch: trustedRuntime.identity.arch,
      gc_mode: "exposed-double-before-lane-and-sample",
      clock: "process.hrtime.bigint",
    },
    artifact: input.artifact_identity,
    semantic_evidence: semanticEvidence,
    measurements,
    rss,
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
  observeRss = () => {},
  validateRaw = () => {},
}) {
  collectGarbage();
  const totalStart = now();
  const constructStart = now();
  const engine = new Engine(baseJson);
  const constructEnd = now();
  const raws = new Array(batchSize);
  try {
    const batchStart = now();
    for (let index = 0; index < batchSize; index += 1) {
      raws[index] = engine.executeSync(requestJson);
    }
    const batchEnd = now();
    const totalEnd = now();
    observeRss();
    for (const raw of raws) validateRaw(raw);
    return {
      raw: raws.at(-1),
      sample: {
        construct_ns: elapsedNs(constructStart, constructEnd),
        batch_ns: elapsedNs(batchStart, batchEnd),
        total_ns: elapsedNs(totalStart, totalEnd),
      },
    };
  } finally {
    disposeEngine(engine);
  }
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
  observeRss = () => {},
  validateRaw = () => {},
}) {
  const engine = new Engine(baseJson);
  try {
    let raw;
    for (let iteration = 0; iteration < warmupIterations; iteration += 1) {
      for (let index = 0; index < batchSize; index += 1) {
        raw = engine.executeSync(requestJson);
        validateRaw(raw);
      }
    }
    const timings = [];
    for (let iteration = 0; iteration < samples; iteration += 1) {
      collectGarbage();
      const raws = new Array(batchSize);
      const totalStart = now();
      const batchStart = now();
      for (let index = 0; index < batchSize; index += 1) {
        raws[index] = engine.executeSync(requestJson);
      }
      const batchEnd = now();
      const totalEnd = now();
      observeRss();
      for (const measuredRaw of raws) validateRaw(measuredRaw);
      raw = raws.at(-1);
      timings.push({
        construct_ns: 0,
        batch_ns: elapsedNs(batchStart, batchEnd),
        total_ns: elapsedNs(totalStart, totalEnd),
      });
    }
    return { raw, samples: timings };
  } finally {
    disposeEngine(engine);
  }
}

function collectMeasurements({
  Engine,
  prepared,
  sampling,
  collectGarbage,
  now,
  byteLength,
  rssTracker,
}) {
  const baseJson = prepared.base_json_by_units["1"];
  const overlays = [];
  for (const overlay of prepared.manifest.overlays) {
    const requestJson = prepared.request_json_by_overlay[overlay.id];
    const coldLaneId = `measurement:overlay:${overlay.id}:cold`;
    beginMeasurementLane(rssTracker, coldLaneId, collectGarbage);
    overlays.push(coldLane({
      Engine,
      baseJson,
      requestJson,
      overlayId: overlay.id,
      batchSize: 1,
      sampleCount: sampling.cold_samples,
      collectGarbage,
      now,
      expected: prepared.manifest.success_contract,
      observeRss: () => rssTracker.observe(coldLaneId),
    }));
    const reusedLaneId = `measurement:overlay:${overlay.id}:reused`;
    beginMeasurementLane(rssTracker, reusedLaneId, collectGarbage);
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
      expected: prepared.manifest.success_contract,
      observeRss: () => rssTracker.observe(reusedLaneId),
    }));
  }

  const scalingRequestJson =
    prepared.request_json_by_overlay[prepared.manifest.scaling.overlay_id];
  const batchScaling = prepared.manifest.scaling.batch_sizes.map((batchSize) => {
    const laneId = `measurement:batch:${batchSize}:reused`;
    beginMeasurementLane(rssTracker, laneId, collectGarbage);
    return {
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
        expected: prepared.manifest.success_contract,
        observeRss: () => rssTracker.observe(laneId),
      }),
      batch_size: batchSize,
    };
  });

  const baseSizeScaling = [];
  for (const units of prepared.manifest.scaling.base_size_units) {
    const scaledBaseJson = prepared.base_json_by_units[String(units)];
    const coldLaneId = `measurement:base-size:${units}:cold`;
    beginMeasurementLane(rssTracker, coldLaneId, collectGarbage);
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
        expected: prepared.manifest.success_contract,
        observeRss: () => rssTracker.observe(coldLaneId),
      }),
      base_size_units: units,
      base_options_bytes: byteLength(scaledBaseJson),
    });
    const reusedLaneId = `measurement:base-size:${units}:reused`;
    beginMeasurementLane(rssTracker, reusedLaneId, collectGarbage);
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
        expected: prepared.manifest.success_contract,
        observeRss: () => rssTracker.observe(reusedLaneId),
      }),
      base_size_units: units,
      base_options_bytes: byteLength(scaledBaseJson),
    });
  }
  const measurementLanes = [
    ...overlays,
    ...batchScaling,
    ...baseSizeScaling,
  ];
  return {
    semantic_contract_passed:
      measurementLanes.every((lane) => lane.semantic_contract.passed),
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
  expected,
  observeRss,
}) {
  const samples = [];
  let raw;
  const semanticContract = createMeasurementSemanticTracker(expected);
  for (let iteration = 0; iteration < sampleCount; iteration += 1) {
    const measured = measureColdSample({
      Engine,
      baseJson,
      requestJson,
      batchSize,
      collectGarbage,
      now,
      observeRss,
      validateRaw: semanticContract.observe,
    });
    raw = measured.raw;
    samples.push(measured.sample);
  }
  assertWireRaw(raw, overlayId);
  return {
    overlay_id: overlayId,
    engine_lifecycle: "cold",
    logical_operations_per_sample: batchSize,
    samples,
    summary: summarizeTimingSamples(samples, batchSize),
    semantic_contract: semanticContract.finish(),
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
  expected,
  observeRss,
}) {
  const semanticContract = createMeasurementSemanticTracker(expected);
  const measured = measureReusedSamples({
    Engine,
    baseJson,
    requestJson,
    batchSize,
    warmupIterations,
    samples: sampleCount,
    collectGarbage,
    now,
    observeRss,
    validateRaw: semanticContract.observe,
  });
  assertWireRaw(measured.raw, overlayId);
  return {
    overlay_id: overlayId,
    engine_lifecycle: "reused",
    logical_operations_per_sample: batchSize,
    samples: measured.samples,
    summary: summarizeTimingSamples(measured.samples, batchSize),
    semantic_contract: semanticContract.finish(),
  };
}

function collectSemanticEvidence({
  Engine,
  prepared,
  expectedRuntimeCatalogDigest,
  rssTracker,
}) {
  const runtimeCatalogLaneId = "semantic:runtime-catalog";
  rssTracker.beginLane(runtimeCatalogLaneId);
  const runtimeCatalogDigest = runtimeCatalogProbe(
    Engine,
    prepared.base_json_by_units["1"],
    () => rssTracker.observe(runtimeCatalogLaneId),
  );
  if (runtimeCatalogDigest !== expectedRuntimeCatalogDigest) {
    throw new Error("loaded artifact runtime catalog differs from its historical receipt");
  }
  const probes = [];
  const reusedProbes = [];
  const baseOne = prepared.base_json_by_units["1"];
  for (const overlay of prepared.manifest.overlays) {
    const laneId = `semantic:overlay:${overlay.id}`;
    rssTracker.beginLane(laneId);
    const requestJson = prepared.request_json_by_overlay[overlay.id];
    probes.push(successProbe({
      Engine,
      baseJson: baseOne,
      requestJson,
      id: `overlay:${overlay.id}`,
      expected: prepared.manifest.success_contract,
      observeRss: () => rssTracker.observe(laneId),
    }));
    reusedProbes.push(reusedSuccessProbe({
      Engine,
      baseJson: baseOne,
      requestJson,
      id: `overlay:${overlay.id}`,
      expected: prepared.manifest.success_contract,
      iterations: REUSED_SEMANTIC_PROBE_ITERATIONS,
      observeRss: () => rssTracker.observe(laneId),
    }));
  }
  const versionOnly =
    prepared.request_json_by_overlay[prepared.manifest.scaling.overlay_id];
  for (const units of prepared.manifest.scaling.base_size_units) {
    const laneId = `semantic:base-size:${units}`;
    rssTracker.beginLane(laneId);
    probes.push(successProbe({
      Engine,
      baseJson: prepared.base_json_by_units[String(units)],
      requestJson: versionOnly,
      id: `base-size:${units}`,
      expected: prepared.manifest.success_contract,
      observeRss: () => rssTracker.observe(laneId),
    }));
  }
  const errorLaneId = `semantic:error:${prepared.manifest.error_probe.id}`;
  rssTracker.beginLane(errorLaneId);
  const error = rawProbe({
    Engine,
    baseJson: baseOne,
    requestJson: prepared.error_request_json,
    observeRss: () => rssTracker.observe(errorLaneId),
  });
  const errorMatches = envelopeMatches(
    error.envelope,
    prepared.manifest.error_probe.expected,
  );
  const resourceLimitLaneId =
    `semantic:resource-limit:${prepared.manifest.resource_limit_probe.id}`;
  rssTracker.beginLane(resourceLimitLaneId);
  const resourceLimit = rawProbe({
    Engine,
    baseJson: baseOne,
    requestJson: prepared.resource_limit_request_json,
    observeRss: () => rssTracker.observe(resourceLimitLaneId),
  });
  const resourceLimitMatches = envelopeMatches(
    resourceLimit.envelope,
    prepared.manifest.resource_limit_probe.expected,
  );
  const passed =
    probes.every((probe) => probe.matches_contract) &&
    reusedProbes.every((probe) => probe.passed) &&
    errorMatches &&
    resourceLimitMatches;
  return {
    passed,
    runtime_catalog_digest: runtimeCatalogDigest,
    success_probes: probes,
    reused_success_probes: reusedProbes,
    error_probe: {
      id: prepared.manifest.error_probe.id,
      raw_sha256: digest(error.raw),
      envelope: error.envelope,
      matches_contract: errorMatches,
    },
    resource_limit_probe: {
      id: prepared.manifest.resource_limit_probe.id,
      raw_sha256: digest(resourceLimit.raw),
      envelope: resourceLimit.envelope,
      matches_contract: resourceLimitMatches,
    },
  };
}

function runtimeCatalogProbe(Engine, baseJson, observeRss) {
  const engine = new Engine(baseJson);
  try {
    if (typeof engine.runtimeCatalogJson !== "function") {
      throw new Error("raw artifact engine does not expose runtimeCatalogJson()");
    }
    const raw = engine.runtimeCatalogJson();
    observeRss();
    if (typeof raw !== "string") throw new Error("runtimeCatalogJson() returned a non-string");
    return digestJsonValue(parseJson(raw));
  } finally {
    disposeEngine(engine);
  }
}

function successProbe({ Engine, baseJson, requestJson, id, expected, observeRss }) {
  const probe = rawProbe({ Engine, baseJson, requestJson, observeRss });
  return {
    id,
    raw_sha256: digest(probe.raw),
    envelope: probe.envelope,
    matches_contract: envelopeMatches(probe.envelope, expected),
  };
}

function reusedSuccessProbe({
  Engine,
  baseJson,
  requestJson,
  id,
  expected,
  iterations,
  observeRss,
}) {
  const engine = new Engine(baseJson);
  const responseDigests = [];
  let matchingObservations = 0;
  let firstMismatch = null;
  try {
    for (let iteration = 0; iteration < iterations; iteration += 1) {
      const raw = engine.executeSync(requestJson);
      if (typeof raw !== "string") {
        throw new Error("executeSync returned a non-string wire result");
      }
      const envelope = parseEnvelope(raw);
      const rawSha256 = digest(raw);
      responseDigests.push(rawSha256);
      if (envelopeMatches(envelope, expected)) matchingObservations += 1;
      else if (firstMismatch === null) {
        firstMismatch = { iteration, raw_sha256: rawSha256, envelope };
      }
    }
    observeRss();
  } finally {
    disposeEngine(engine);
  }
  const uniqueResponseDigests = [...new Set(responseDigests)].sort();
  return {
    id,
    iterations,
    matching_observations: matchingObservations,
    response_sha256: responseDigests,
    response_sequence_sha256: digest(stableJson(responseDigests)),
    unique_response_sha256: uniqueResponseDigests,
    first_mismatch: firstMismatch,
    passed:
      matchingObservations === iterations && uniqueResponseDigests.length === 1,
  };
}

function rawProbe({ Engine, baseJson, requestJson, observeRss }) {
  const engine = new Engine(baseJson);
  try {
    const raw = engine.executeSync(requestJson);
    observeRss();
    if (typeof raw !== "string") throw new Error("executeSync returned a non-string wire result");
    return { raw, envelope: parseEnvelope(raw) };
  } finally {
    disposeEngine(engine);
  }
}

function assertWireRaw(raw, label) {
  if (typeof raw !== "string") {
    throw new Error(`${label} measurement returned a non-string wire result`);
  }
  parseEnvelope(raw);
}

function createMeasurementSemanticTracker(expected) {
  let observationCount = 0;
  let matchingObservations = 0;
  let firstMismatch = null;
  const responseDigests = [];
  return {
    observe(raw) {
      const envelope = parseEnvelope(raw);
      const rawSha256 = digest(raw);
      responseDigests.push(rawSha256);
      const matches = envelopeMatches(envelope, expected);
      if (matches) matchingObservations += 1;
      else if (firstMismatch === null) {
        firstMismatch = {
          observation_index: observationCount,
          raw_sha256: rawSha256,
          envelope,
        };
      }
      observationCount += 1;
    },
    finish() {
      const uniqueResponseDigests = [...new Set(responseDigests)].sort();
      const semanticMatches = matchingObservations === observationCount;
      const wireDeterministic = uniqueResponseDigests.length === 1;
      return {
        observation_count: observationCount,
        matching_observations: matchingObservations,
        response_sequence_sha256: digest(stableJson(responseDigests)),
        unique_response_sha256: uniqueResponseDigests,
        wire_deterministic: wireDeterministic,
        first_mismatch: firstMismatch,
        passed: semanticMatches && wireDeterministic,
      };
    },
  };
}

function envelopeMatches(actual, expected) {
  return stableJson(actual) === stableJson(expected);
}

function parseEnvelope(raw) {
  try {
    const value = parseJson(raw);
    if (!value || typeof value !== "object" || isArray(value)) throw new Error();
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

function captureTrustedRuntime() {
  const hrtime = process.hrtime;
  const memoryUsage = process.memoryUsage;
  const resourceUsage = process.resourceUsage;
  const garbageCollector = globalThis.gc;
  const byteLength = Buffer.byteLength;
  return Object.freeze({
    now: hrtime.bigint.bind(hrtime),
    readMemoryUsage: memoryUsage.bind(process),
    readResourceUsage: resourceUsage.bind(process),
    collectGarbage: typeof garbageCollector === "function"
      ? () => collectGarbageTwice(garbageCollector.bind(globalThis))
      : null,
    byteLength: byteLength.bind(Buffer),
    identity: Object.freeze({
      pid: process.pid,
      node: process.version,
      platform: process.platform,
      arch: process.arch,
    }),
  });
}

function createRssTracker({ readMemoryUsage, readResourceUsage }) {
  const baseline = readRssSample({ readMemoryUsage, readResourceUsage });
  let peakSampledCurrentRssBytes = baseline.currentRssBytes;
  let peakProcessMaxRssBytes = baseline.processMaxRssBytes;
  let finished = false;
  const lanes = [];
  const lanesById = new Map();

  const sample = () => {
    const value = readRssSample({ readMemoryUsage, readResourceUsage });
    peakSampledCurrentRssBytes = maximum(
      peakSampledCurrentRssBytes,
      value.currentRssBytes,
    );
    peakProcessMaxRssBytes = maximum(
      peakProcessMaxRssBytes,
      value.processMaxRssBytes,
    );
    return value;
  };

  return {
    beginLane(laneId) {
      if (finished) throw new Error("cannot begin an RSS lane after tracker finalization");
      if (typeof laneId !== "string" || laneId.length === 0 || lanesById.has(laneId)) {
        throw new Error("request-overlay RSS lane IDs must be unique non-empty strings");
      }
      const laneBaseline = sample();
      const lane = {
        lane_id: laneId,
        observation_count: 0,
        baseline_current_rss_bytes: laneBaseline.currentRssBytes,
        baseline_process_max_rss_bytes: laneBaseline.processMaxRssBytes,
        baseline_history_gap_bytes: maximum(
          0,
          laneBaseline.processMaxRssBytes - laneBaseline.currentRssBytes,
        ),
        peak_sampled_current_rss_bytes: laneBaseline.currentRssBytes,
        peak_process_max_rss_bytes: laneBaseline.processMaxRssBytes,
      };
      lanes.push(lane);
      lanesById.set(laneId, lane);
    },
    observe(laneId) {
      if (finished) throw new Error("cannot observe RSS after tracker finalization");
      const lane = lanesById.get(laneId);
      if (!lane) throw new Error(`cannot observe unknown RSS lane ${laneId}`);
      const value = sample();
      lane.observation_count += 1;
      lane.peak_sampled_current_rss_bytes = maximum(
        lane.peak_sampled_current_rss_bytes,
        value.currentRssBytes,
      );
      lane.peak_process_max_rss_bytes = maximum(
        lane.peak_process_max_rss_bytes,
        value.processMaxRssBytes,
      );
    },
    finish() {
      if (finished) throw new Error("request-overlay RSS tracker was finalized twice");
      finished = true;
      const final = sample();
      const sampledCurrentPeakGrowthBytes = maximum(
        0,
        peakSampledCurrentRssBytes - baseline.currentRssBytes,
      );
      const processMaxPeakGrowthBytes = maximum(
        0,
        peakProcessMaxRssBytes - baseline.processMaxRssBytes,
      );
      return {
        method: "lane-local-retained/fresh-process-envelope-v4",
        baseline_current_rss_bytes: baseline.currentRssBytes,
        baseline_process_max_rss_bytes: baseline.processMaxRssBytes,
        baseline_history_gap_bytes: maximum(
          0,
          baseline.processMaxRssBytes - baseline.currentRssBytes,
        ),
        final_current_rss_bytes: final.currentRssBytes,
        final_process_max_rss_bytes: final.processMaxRssBytes,
        peak_sampled_current_rss_bytes: peakSampledCurrentRssBytes,
        peak_process_max_rss_bytes: peakProcessMaxRssBytes,
        sampled_current_peak_growth_bytes: sampledCurrentPeakGrowthBytes,
        process_max_peak_growth_bytes: processMaxPeakGrowthBytes,
        fresh_process_envelope_growth_bytes: maximum(
          sampledCurrentPeakGrowthBytes,
          processMaxPeakGrowthBytes,
        ),
        lanes: lanes.map((lane) => {
          const sampledCurrentGrowthBytes = maximum(
            0,
            lane.peak_sampled_current_rss_bytes - lane.baseline_current_rss_bytes,
          );
          const processMaxGrowthBytes = maximum(
            0,
            lane.peak_process_max_rss_bytes - lane.baseline_process_max_rss_bytes,
          );
          return {
            ...lane,
            sampled_current_growth_bytes: sampledCurrentGrowthBytes,
            process_max_growth_bytes: processMaxGrowthBytes,
            operation_peak_growth_bytes: maximum(
              sampledCurrentGrowthBytes,
              processMaxGrowthBytes,
            ),
          };
        }),
      };
    },
  };
}

function beginMeasurementLane(rssTracker, laneId, collectGarbage) {
  collectGarbage();
  rssTracker.beginLane(laneId);
}

function readRssSample({ readMemoryUsage, readResourceUsage }) {
  const currentRssBytes = readMemoryUsage().rss;
  const maxRssKilobytes = readResourceUsage().maxRSS;
  const processMaxRssBytes = maxRssKilobytes * 1024;
  if (
    !isSafeInteger(currentRssBytes) ||
    currentRssBytes < 1 ||
    !isSafeInteger(maxRssKilobytes) ||
    maxRssKilobytes < 1 ||
    !isSafeInteger(processMaxRssBytes) ||
    processMaxRssBytes < currentRssBytes
  ) {
    throw new Error("trusted Node RSS readers returned invalid values");
  }
  return { currentRssBytes, processMaxRssBytes };
}

export function collectGarbageTwice(garbageCollector = globalThis.gc) {
  if (typeof garbageCollector !== "function") {
    throw new Error("request-overlay workers require Node --expose-gc");
  }
  garbageCollector();
  garbageCollector();
}

function disposeEngine(engine) {
  engine?.dispose?.();
}

function elapsedNs(start, end) {
  const value = toNumber(end - start);
  if (!isSafeInteger(value) || value < 0) {
    throw new Error("hrtime duration exceeds the safe nanosecond range");
  }
  return value;
}

function digest(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}

function digestJsonValue(value) {
  return `sha256:${createHash("sha256").update(stableJson(value)).digest("hex")}`;
}

function validateInvocation(input) {
  if (!input || typeof input !== "object" || isArray(input)) {
    throw new Error("request-overlay worker invocation must be an object");
  }
  if (
    input.schema_version !== 2 ||
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
  return parseJson(bytes.slice(0, -1));
}

function isMainModule() {
  return process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}
