import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { digestJson, stableJson } from "../stable-json.mjs";
import {
  computeCorpusDigest,
  computeInputDigest,
  loadCorpus,
  validateBenchmarkWorkloads,
} from "./corpus.mjs";
import {
  equivalentTransportOutcome,
  svgTransportEvidence,
} from "./svg-signature.mjs";
import { summarize } from "./stats.mjs";

export {
  computeCorpusDigest,
  computeInputDigest,
  validateBenchmarkWorkloads,
} from "./corpus.mjs";

const REQUIRED_TOOLS = [
  "node",
  "npm",
  "rustc",
  "cargo",
  "napi",
  "napi_derive",
  "napi_build",
  "napi_cli",
];
const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");
const trustedCorpusManifest = path.join(nodeRoot, "benchmark", "corpus.json");
const buildDescriptor = readJson(path.join(nodeRoot, "candidate-builds.json"));
const packageDescriptor = readJson(path.join(nodeRoot, "package-surfaces.json"));
const TARGET_CONTRACTS = loadTargetContracts();
const INITIAL_TARGETS = Object.keys(TARGET_CONTRACTS).sort();
const ROOT_PACKAGE_NAME = packageDescriptor.root.name;
const WASM_PACKAGE_NAME = "@mermanjs/node-wasm-candidate";
const SHA256_DIGEST = /^sha256:[0-9a-f]{64}$/;
const EXPECTED_FAILURE_STATUS_CODES = new Set([
  "MERMAN_INVALID_ARGUMENT",
  "MERMAN_UTF8_ERROR",
  "MERMAN_OPTIONS_JSON_ERROR",
  "MERMAN_NO_DIAGRAM",
  "MERMAN_PARSE_ERROR",
  "MERMAN_RENDER_ERROR",
  "MERMAN_RESOURCE_LIMIT_EXCEEDED",
]);
const FATAL_FAILURE_STATUS_CODES = new Set([
  "MERMAN_PANIC",
  "MERMAN_INTERNAL_ERROR",
]);

function loadTargetContracts() {
  const contracts = {};
  for (const buildTarget of buildDescriptor.candidates?.napi?.targets ?? []) {
    const packageTarget = packageDescriptor.targets?.find(
      (item) => item.target === buildTarget.id,
    );
    if (!packageTarget || contracts[buildTarget.id]) {
      throw new Error(`Node target descriptors disagree for ${buildTarget.id}.`);
    }
    const manifest = readJson(path.join(nodeRoot, packageTarget.directory, "package.json"));
    const platform = singleString(manifest.os, `${buildTarget.id} package OS`);
    const arch = singleString(manifest.cpu, `${buildTarget.id} package CPU`);
    const manifestLibc = manifest.libc === undefined
      ? null
      : singleString(manifest.libc, `${buildTarget.id} package libc`);
    contracts[buildTarget.id] = Object.freeze({
      platform,
      arch,
      libc: manifestLibc === "glibc" ? "gnu" : manifestLibc,
      rust_target: buildTarget.rust_target,
      package_name: packageTarget.name,
    });
  }
  if (
    Object.keys(contracts).length === 0 ||
    Object.keys(contracts).length !== packageDescriptor.targets?.length
  ) {
    throw new Error("Node build and package target descriptors must define the same target matrix.");
  }
  return Object.freeze(contracts);
}

function singleString(value, label) {
  if (!Array.isArray(value) || value.length !== 1 || typeof value[0] !== "string") {
    throw new Error(`${label} must contain exactly one string.`);
  }
  return value[0];
}

export function validateComparisonReport(
  report,
  { trustedCorpus = loadCorpus(trustedCorpusManifest) } = {},
) {
  const trusted = trustedCorpusEvidence(trustedCorpus);
  assertObject(report, "report");
  if (report.schema_version !== 2) throw new Error("report schema_version must be 2.");
  validateProvenance(report.provenance);
  assertObject(report.input, "input");
  if (!SHA256_DIGEST.test(report.input.digest ?? "")) {
    throw new Error("input digest must be a sha256 digest.");
  }
  if (!SHA256_DIGEST.test(report.input.corpus_digest ?? "")) {
    throw new Error("input corpus_digest must be a sha256 digest.");
  }
  if (report.input.binding_options?.runtime_policy !== "deterministic") {
    throw new Error("comparison input must select deterministic runtime policy.");
  }
  assertObject(report.input.operation_options, "comparison input operation options");
  const profile = report.input.binding_options?.resources?.profile;
  if (typeof profile !== "string" || profile.length === 0) {
    throw new Error("comparison input must name one shared resource profile.");
  }
  const reportedWorkloads = validateBenchmarkWorkloads(
    report.input.workloads,
    "comparison input workloads",
  );
  if (
    report.input.corpus !== trusted.reportPath ||
    report.input.corpus_digest !== trusted.corpusDigest ||
    stableJson(report.input.binding_options) !== stableJson(trusted.bindingOptions) ||
    stableJson(report.input.operation_options) !== stableJson(trusted.operationOptions) ||
    stableJson(reportedWorkloads) !== stableJson(trusted.workloads)
  ) {
    throw new Error("comparison input does not match the trusted corpus manifest.");
  }
  const expectedInputDigest = computeInputDigest({
    corpusDigest: trusted.corpusDigest,
    bindingOptions: trusted.bindingOptions,
    operationOptions: trusted.operationOptions,
    workloads: trusted.workloads,
  });
  if (report.input.digest !== expectedInputDigest) {
    throw new Error("input digest does not match the recorded corpus and benchmark options.");
  }
  if (!Array.isArray(report.candidates) || report.candidates.length !== 2) {
    throw new Error("comparison must contain exactly the node-wasm and napi candidates.");
  }
  const ids = report.candidates.map((candidate) => candidate.id).sort();
  if (stableJson(ids) !== stableJson(["napi", "node-wasm"])) {
    throw new Error("comparison must contain exactly the node-wasm and napi candidates.");
  }
  if (report.input.cases !== trusted.cases.length) {
    throw new Error("comparison input case count does not match the trusted corpus manifest.");
  }
  validateSampling(report.sampling);
  for (const candidate of report.candidates) {
    validateCandidate(
      candidate,
      report.input.digest,
      report.input.cases,
      report.sampling,
      report.provenance,
      trusted.workloads,
      trusted.paths,
    );
  }
  validateCrossCandidateEvidence(report.candidates);
  validateWorkloadComparison(report.workload_comparison, report.candidates);
  const sourceDigests = new Set(
    report.candidates.map((candidate) => candidate.build_receipt.source_digest),
  );
  if (sourceDigests.size !== 1) {
    throw new Error("candidate build receipts must share one source digest.");
  }
  const bindingContractDigests = new Set(
    report.candidates.map((candidate) => candidate.build_receipt.binding_contract_digest),
  );
  if (bindingContractDigests.size !== 1) {
    throw new Error("candidate build receipts must share one bindings-contract digest.");
  }
  const cargoLockDigests = new Set(
    report.candidates.map((candidate) => candidate.build_receipt.cargo_lock_digest),
  );
  if (cargoLockDigests.size !== 1) {
    throw new Error("candidate build receipts must share one Cargo lock digest.");
  }
  const capabilityRecipeDigests = new Set(
    report.candidates.map((candidate) => candidate.build_receipt.capability_recipe_digest),
  );
  if (capabilityRecipeDigests.size !== 1) {
    throw new Error("candidate build receipts must share one capability-recipe digest.");
  }
  const runtimeCatalogDigests = new Set(
    report.candidates.map((candidate) => candidate.build_receipt.runtime_catalog_digest),
  );
  if (runtimeCatalogDigests.size !== 1) {
    throw new Error("candidate build receipts must share one runtime-catalog digest.");
  }
  validateDecision(report.decision, report.candidates);
  return report;
}

function trustedCorpusEvidence(corpus) {
  assertObject(corpus, "trusted corpus");
  if (typeof corpus.manifestPath !== "string" || corpus.manifestPath.length === 0) {
    throw new Error("trusted corpus manifest path is required.");
  }
  const corpusDigest = computeCorpusDigest(corpus.cases);
  const workloads = validateBenchmarkWorkloads(
    corpus.workloads,
    "trusted corpus workloads",
  );
  assertObject(corpus.bindingOptions, "trusted corpus binding options");
  assertObject(corpus.operationOptions, "trusted corpus operation options");
  const digest = computeInputDigest({
    corpusDigest,
    bindingOptions: corpus.bindingOptions,
    operationOptions: corpus.operationOptions,
    workloads,
  });
  if (
    (corpus.corpusDigest !== undefined && corpus.corpusDigest !== corpusDigest) ||
    (corpus.digest !== undefined && corpus.digest !== digest)
  ) {
    throw new Error("trusted corpus contains a stale recorded digest.");
  }
  return {
    cases: corpus.cases,
    paths: corpus.cases.map((item) => item.path).sort(),
    bindingOptions: corpus.bindingOptions,
    operationOptions: corpus.operationOptions,
    workloads,
    corpusDigest,
    digest,
    reportPath: path
      .relative(repositoryRoot, path.resolve(corpus.manifestPath))
      .split(path.sep)
      .join("/"),
  };
}

function validateProvenance(provenance) {
  assertObject(provenance, "provenance");
  if (
    typeof provenance.measured_at_utc !== "string" ||
    !Number.isFinite(Date.parse(provenance.measured_at_utc))
  ) {
    throw new Error("provenance measured_at_utc is required.");
  }
  if (typeof provenance.timezone !== "string" || provenance.timezone.length === 0) {
    throw new Error("provenance timezone is required.");
  }
  if (!SHA256_DIGEST.test(provenance.harness_digest ?? "")) {
    throw new Error("provenance harness digest is required.");
  }
  assertObject(provenance.machine, "provenance.machine");
  for (const key of ["hostname", "os", "release", "arch", "cpu"]) {
    if (typeof provenance.machine[key] !== "string" || provenance.machine[key].length === 0) {
      throw new Error(`provenance.machine.${key} is required.`);
    }
  }
  for (const key of ["logical_cpus", "total_memory_bytes"]) {
    if (!Number.isFinite(provenance.machine[key]) || provenance.machine[key] < 1) {
      throw new Error(`provenance.machine.${key} is required.`);
    }
  }
  assertObject(provenance.tools, "provenance.tools");
  for (const key of REQUIRED_TOOLS) {
    if (typeof provenance.tools[key] !== "string" || provenance.tools[key].length === 0) {
      throw new Error(`provenance tool ${key} is required.`);
    }
  }
  if (!/^[0-9a-f]{40}$/.test(provenance.commit ?? "")) {
    throw new Error("provenance commit must be the full Git commit id.");
  }
}

function validateCandidate(
  candidate,
  inputDigest,
  inputCases,
  sampling,
  provenance,
  workloads,
  trustedPaths,
) {
  assertObject(candidate, "candidate");
  validateBuildReceipt(candidate.build_receipt, candidate.id, provenance.commit);
  if (!SHA256_DIGEST.test(candidate.input_digest ?? "")) {
    throw new Error(`${candidate.id} input digest is required.`);
  }
  if (candidate.input_digest !== inputDigest) {
    throw new Error(`${candidate.id} input digest does not match the shared comparison input digest.`);
  }
  if (
    !Number.isSafeInteger(candidate.corpus?.cases) ||
    !Number.isSafeInteger(candidate.corpus?.matched) ||
    !Number.isSafeInteger(candidate.corpus?.mismatched)
  ) {
    throw new Error(`${candidate.id} corpus result is incomplete.`);
  }
  if (candidate.corpus.cases !== inputCases) {
    throw new Error(`${candidate.id} corpus case count does not match the comparison input.`);
  }
  if (
    !Number.isSafeInteger(candidate.corpus.successful) ||
    !Number.isSafeInteger(candidate.corpus.failed) ||
    candidate.corpus.successful + candidate.corpus.failed !== candidate.corpus.cases
  ) {
    throw new Error(`${candidate.id} corpus success/failure counts are incomplete.`);
  }
  if (
    candidate.corpus.matched + candidate.corpus.mismatched !== candidate.corpus.cases
  ) {
    throw new Error(`${candidate.id} corpus parity counts do not cover every case.`);
  }
  if (
    !Number.isSafeInteger(candidate.corpus.geometry_svg_mismatches) ||
    candidate.corpus.geometry_svg_mismatches < 0 ||
    candidate.corpus.geometry_svg_mismatches > candidate.corpus.successful
  ) {
    throw new Error(`${candidate.id} SVG geometry mismatch count is invalid.`);
  }
  if (
    !Number.isSafeInteger(candidate.corpus.raw_svg_byte_mismatches) ||
    candidate.corpus.raw_svg_byte_mismatches < 0 ||
    candidate.corpus.raw_svg_byte_mismatches > candidate.corpus.matched
  ) {
    throw new Error(`${candidate.id} raw SVG byte mismatch count is invalid.`);
  }
  validateMismatchPaths(
    candidate.corpus.mismatch_paths,
    candidate.corpus.mismatched,
    `${candidate.id} contract`,
  );
  validateMismatchPaths(
    candidate.corpus.geometry_mismatch_paths,
    candidate.corpus.geometry_svg_mismatches,
    `${candidate.id} geometry`,
  );
  if (
    !Array.isArray(candidate.corpus.outcomes) ||
    candidate.corpus.outcomes.length !== candidate.corpus.cases
  ) {
    throw new Error(`${candidate.id} raw corpus outcomes are required.`);
  }
  const outcomePaths = new Set();
  const outcomeByPath = new Map();
  for (const outcome of candidate.corpus.outcomes) {
    if (
      !outcome ||
      typeof outcome.path !== "string" ||
      outcome.path.length === 0 ||
      typeof outcome.ok !== "boolean" ||
      outcomePaths.has(outcome.path)
    ) {
      throw new Error(`${candidate.id} raw corpus outcomes contain an invalid path or result.`);
    }
    if (outcome.ok) {
      if (
        outcome.operation_id !== "svg" ||
        outcome.media_type !== "image/svg+xml" ||
        !SHA256_DIGEST.test(outcome.sha256 ?? "") ||
        !SHA256_DIGEST.test(outcome.svg_structure_sha256 ?? "") ||
        !SHA256_DIGEST.test(outcome.svg_geometry_sha256 ?? "") ||
        !Number.isSafeInteger(outcome.bytes) ||
        outcome.bytes < 1
      ) {
        throw new Error(`${candidate.id} successful corpus outcome lacks output evidence.`);
      }
    } else if (!hasTypedFailureEvidence(outcome)) {
      throw new Error(`${candidate.id} failed corpus outcome lacks typed error evidence.`);
    }
    validateOperationEvidence(
      outcome.semantic,
      `${candidate.id} semantic outcome`,
      { operationId: "semantic-json", mediaType: "application/json" },
    );
    outcomePaths.add(outcome.path);
    outcomeByPath.set(outcome.path, outcome);
  }
  if (stableJson([...outcomePaths].sort()) !== stableJson(trustedPaths)) {
    throw new Error(`${candidate.id} corpus outcome paths do not match the trusted corpus manifest.`);
  }
  if (candidate.cold_process?.isolated_processes !== true) {
    throw new Error(`${candidate.id} cold samples must use an isolated process per sample.`);
  }
  if (candidate.cold_process.workload_id !== "cold_svg") {
    throw new Error(`${candidate.id} cold samples must use the declared cold_svg workload.`);
  }
  if (
    candidate.cold_process.timing_scope !==
      "parent-dispatch-through-worker-raw-svg-result" ||
    candidate.cold_process.operation_timing_scope !==
      "worker-engine-init-through-first-svg-operation-result" ||
    candidate.cold_process.evidence_excluded !== true
  ) {
    throw new Error(`${candidate.id} cold timing scope must exclude SVG evidence projection.`);
  }
  const coldRepresentative = validateWorkloadRepresentative(
    candidate.cold_process.representative,
    workloads.cold_svg,
    `${candidate.id} cold workload`,
  );
  validateSamples(candidate.cold_process?.samples_ms, `${candidate.id} cold process`);
  if (
    !Array.isArray(candidate.cold_process?.samples) ||
    candidate.cold_process.samples.length !== candidate.cold_process.samples_ms.length ||
    candidate.cold_process.samples.length !== sampling.cold_processes
  ) {
    throw new Error(`${candidate.id} raw cold-process samples are required.`);
  }
  for (let index = 0; index < candidate.cold_process.samples.length; index += 1) {
    const sample = candidate.cold_process.samples[index];
    if (
      !sample ||
      sample.elapsed_ms !== candidate.cold_process.samples_ms[index] ||
      sample.elapsed_ms < 0 ||
      !Number.isFinite(sample.operation_ms) ||
      sample.operation_ms < 0 ||
      !Number.isFinite(sample.baseline_rss_bytes) ||
      sample.baseline_rss_bytes < 0 ||
      !Number.isFinite(sample.peak_rss_bytes) ||
      sample.peak_rss_bytes < sample.baseline_rss_bytes ||
      !matchesBenchmarkWorkloadOutcome(sample.outcome, workloads.cold_svg)
    ) {
      throw new Error(`${candidate.id} raw cold-process sample ${index} is incomplete.`);
    }
  }
  validateStableBenchmarkWorkloadOutcomes(
    candidate.cold_process.samples.map((sample) => sample.outcome),
    `${candidate.id} cold workload`,
    coldRepresentative,
  );
  validateSummary(
    candidate.cold_process.summary,
    candidate.cold_process.samples_ms,
    `${candidate.id} cold process`,
  );
  validateSamples(candidate.warm_latency?.samples_ms, `${candidate.id} warm latency`);
  if (
    !Array.isArray(candidate.warm_latency?.samples) ||
    candidate.warm_latency.samples.length !== candidate.warm_latency.samples_ms.length ||
    candidate.warm_latency.samples.length !== inputCases * sampling.measured_iterations
  ) {
    throw new Error(`${candidate.id} raw warm-latency samples are required.`);
  }
  const warmKeys = new Set();
  const successfulSvgSamplesMs = [];
  for (let index = 0; index < candidate.warm_latency.samples.length; index += 1) {
    const sample = candidate.warm_latency.samples[index];
    const key = `${sample?.iteration}\0${sample?.path}`;
    const corpusOutcome = outcomeByPath.get(sample?.path);
    if (
      !sample ||
      sample.elapsed_ms !== candidate.warm_latency.samples_ms[index] ||
      sample.elapsed_ms < 0 ||
      !Number.isSafeInteger(sample.iteration) ||
      sample.iteration < 0 ||
      sample.iteration >= sampling.measured_iterations ||
      typeof sample.path !== "string" ||
      sample.path.length === 0 ||
      warmKeys.has(key) ||
      !corpusOutcome ||
      !sample.outcome ||
      typeof sample.outcome.ok !== "boolean" ||
      !matchesTimedOutcome(sample.outcome, corpusOutcome)
    ) {
      throw new Error(`${candidate.id} raw warm-latency sample ${index} is incomplete.`);
    }
    if (sample.outcome.ok) successfulSvgSamplesMs.push(sample.elapsed_ms);
    warmKeys.add(key);
  }
  validateSummary(
    candidate.warm_latency.summary,
    candidate.warm_latency.samples_ms,
    `${candidate.id} warm latency`,
  );
  const successfulSvg = candidate.warm_latency.successful_svg;
  if (
    stableJson(successfulSvg?.samples_ms) !== stableJson(successfulSvgSamplesMs)
  ) {
    throw new Error(`${candidate.id} successful SVG latency must be derived from raw warm samples.`);
  }
  validateSamples(successfulSvg.samples_ms, `${candidate.id} successful SVG latency`);
  validateSummary(
    successfulSvg.summary,
    successfulSvg.samples_ms,
    `${candidate.id} successful SVG latency`,
  );
  if (candidate.rss?.method !== "process.resourceUsage.maxRSS") {
    throw new Error(`${candidate.id} RSS measurement method is required.`);
  }
  for (const key of ["peak_bytes", "baseline_bytes"]) {
    if (!Number.isFinite(candidate.rss?.[key]) || candidate.rss[key] < 0) {
      throw new Error(`${candidate.id} RSS ${key} is required.`);
    }
  }
  if (candidate.rss.peak_bytes < candidate.rss.baseline_bytes) {
    throw new Error(`${candidate.id} RSS peak must not be below its baseline.`);
  }
  validateFootprint(candidate.footprint, candidate.id, candidate.build_receipt);
  validateQueueLifecycle(candidate.queue_lifecycle, candidate.id);
  if (candidate.concurrency?.workload_id !== "concurrency_svg") {
    throw new Error(`${candidate.id} concurrency must use the declared concurrency_svg workload.`);
  }
  if (
    candidate.concurrency.timing_scope !== "warmed-engine-raw-svg-operation-batch" ||
    candidate.concurrency.evidence_excluded !== true
  ) {
    throw new Error(`${candidate.id} concurrency timing scope must exclude SVG evidence projection.`);
  }
  const concurrencyRepresentative = validateWorkloadRepresentative(
    candidate.concurrency.representative,
    workloads.concurrency_svg,
    `${candidate.id} concurrency workload`,
  );
  if (!Number.isSafeInteger(candidate.concurrency?.workers) || candidate.concurrency.workers < 1) {
    throw new Error(`${candidate.id} concurrency worker count is required.`);
  }
  if (
    !Number.isSafeInteger(candidate.concurrency?.requests_per_batch) ||
    candidate.concurrency.requests_per_batch < 1 ||
    candidate.concurrency.requests_per_batch !== candidate.concurrency.workers
  ) {
    throw new Error(`${candidate.id} concurrency batch size is required.`);
  }
  validateSamples(candidate.concurrency?.batch_samples_ms, `${candidate.id} concurrency batch`);
  if (candidate.concurrency.batch_samples_ms.length !== sampling.concurrency_iterations) {
    throw new Error(`${candidate.id} concurrency sample count does not match the sampling plan.`);
  }
  if (
    !Array.isArray(candidate.concurrency.samples) ||
    candidate.concurrency.samples.length !== sampling.concurrency_iterations
  ) {
    throw new Error(`${candidate.id} raw concurrency samples are required.`);
  }
  for (let index = 0; index < candidate.concurrency.samples.length; index += 1) {
    const sample = candidate.concurrency.samples[index];
    if (
      !sample ||
      sample.iteration !== index ||
      sample.elapsed_ms !== candidate.concurrency.batch_samples_ms[index] ||
      !Array.isArray(sample.outcomes) ||
      sample.outcomes.length !== candidate.concurrency.workers
    ) {
      throw new Error(`${candidate.id} raw concurrency sample ${index} is incomplete.`);
    }
    for (let outcomeIndex = 0; outcomeIndex < sample.outcomes.length; outcomeIndex += 1) {
      if (
        !matchesBenchmarkWorkloadOutcome(
          sample.outcomes[outcomeIndex],
          workloads.concurrency_svg,
        )
      ) {
        throw new Error(`${candidate.id} concurrency sample ${index} contains a failed outcome.`);
      }
    }
  }
  validateStableBenchmarkWorkloadOutcomes(
    candidate.concurrency.samples.flatMap((sample) => sample.outcomes),
    `${candidate.id} concurrency workload`,
    concurrencyRepresentative,
  );
  validateSummary(
    candidate.concurrency.summary,
    candidate.concurrency.batch_samples_ms,
    `${candidate.id} concurrency batch`,
  );
  validateErrorBehavior(candidate.error_behavior, candidate.id);
  if (!Array.isArray(candidate.target_results)) {
    throw new Error(`${candidate.id} target_results must be an array.`);
  }
  const targets = new Set();
  for (const result of candidate.target_results) {
    if (
      !result ||
      typeof result.target !== "string" ||
      result.target.length === 0 ||
      targets.has(result.target) ||
      typeof result.runtime_passed !== "boolean" ||
      typeof result.install_passed !== "boolean" ||
      !TARGET_CONTRACTS[result.target]
    ) {
      throw new Error(`${candidate.id} target result is invalid.`);
    }
    if (result.evidence !== undefined) {
      validateTargetEvidence(result, candidate, provenance);
    }
    targets.add(result.target);
  }
}

function validateCrossCandidateEvidence(candidates) {
  const [left, right] = candidates;
  const rightByPath = new Map(right.corpus.outcomes.map((outcome) => [outcome.path, outcome]));
  const leftPaths = new Set(left.corpus.outcomes.map((outcome) => outcome.path));
  const mismatchPaths = [];
  const geometryMismatchPaths = [];
  let matched = 0;
  let rawSvgByteMismatches = 0;

  for (const outcome of left.corpus.outcomes) {
    const other = rightByPath.get(outcome.path);
    if (!other || !equivalentTransportOutcome(outcome, other)) {
      mismatchPaths.push(outcome.path);
      continue;
    }
    matched += 1;
    if (outcome.ok) {
      if (outcome.svg_geometry_sha256 !== other.svg_geometry_sha256) {
        geometryMismatchPaths.push(outcome.path);
      }
      if (outcome.sha256 !== other.sha256) rawSvgByteMismatches += 1;
    }
  }
  for (const outcome of right.corpus.outcomes) {
    if (!leftPaths.has(outcome.path)) mismatchPaths.push(outcome.path);
  }

  const expected = {
    matched,
    mismatched: new Set(mismatchPaths).size,
    mismatch_paths: [...new Set(mismatchPaths)].sort(),
    geometry_svg_mismatches: new Set(geometryMismatchPaths).size,
    geometry_mismatch_paths: [...new Set(geometryMismatchPaths)].sort(),
    raw_svg_byte_mismatches: rawSvgByteMismatches,
  };
  for (const candidate of candidates) {
    const successful = candidate.corpus.outcomes.filter((outcome) => outcome.ok).length;
    const failed = candidate.corpus.outcomes.length - successful;
    if (
      candidate.corpus.results_digest !== digestJson(candidate.corpus.outcomes) ||
      candidate.corpus.successful !== successful ||
      candidate.corpus.failed !== failed
    ) {
      throw new Error(`${candidate.id} raw corpus outcomes do not match their recorded summary.`);
    }
    for (const key of [
      "matched",
      "mismatched",
      "geometry_svg_mismatches",
      "raw_svg_byte_mismatches",
    ]) {
      if (candidate.corpus[key] !== expected[key]) {
        throw new Error(`${candidate.id} raw corpus outcomes do not match cross-candidate parity.`);
      }
    }
    for (const key of ["mismatch_paths", "geometry_mismatch_paths"]) {
      if (stableJson(candidate.corpus[key]) !== stableJson(expected[key])) {
        throw new Error(`${candidate.id} raw corpus outcomes do not match cross-candidate parity paths.`);
      }
    }
  }
}

export function computeWorkloadComparison(candidates) {
  const byId = new Map(candidates.map((candidate) => [candidate.id, candidate]));
  const wasm = byId.get("node-wasm");
  const napi = byId.get("napi");
  return {
    cold_svg: compareWorkloadOutcomes(
      wasm.cold_process.samples[0].outcome,
      napi.cold_process.samples[0].outcome,
    ),
    concurrency_svg: compareWorkloadOutcomes(
      wasm.concurrency.samples[0].outcomes[0],
      napi.concurrency.samples[0].outcomes[0],
    ),
  };
}

function validateWorkloadComparison(recorded, candidates) {
  const expected = computeWorkloadComparison(candidates);
  for (const [id, comparison] of Object.entries(expected)) {
    if (!comparison.structure_matched) {
      throw new Error(`${id} SVG structure differs across Node transports.`);
    }
  }
  if (stableJson(recorded) !== stableJson(expected)) {
    throw new Error("workload comparison does not match the recorded transport outcomes.");
  }
}

function compareWorkloadOutcomes(left, right) {
  return {
    structure_matched:
      left.svg_structure_sha256 === right.svg_structure_sha256,
    geometry_matched:
      left.svg_geometry_sha256 === right.svg_geometry_sha256,
    raw_svg_matched: left.sha256 === right.sha256,
    bytes_matched: left.bytes === right.bytes,
  };
}

function validateBuildReceipt(receipt, candidateId, commit) {
  assertObject(receipt, `${candidateId} build receipt`);
  if (receipt.candidate !== candidateId) {
    throw new Error(`${candidateId} build receipt candidate is invalid.`);
  }
  if (candidateId === "napi") {
    const target = TARGET_CONTRACTS[receipt.target];
    if (
      !target ||
      receipt.rust_target !== target.rust_target ||
      receipt.wasm_pack_target !== null
    ) {
      throw new Error(`${candidateId} build receipt target configuration is invalid.`);
    }
  } else if (
    candidateId !== "node-wasm" ||
    receipt.target !== null ||
    receipt.rust_target !== "wasm32-unknown-unknown" ||
    receipt.wasm_pack_target !== "nodejs"
  ) {
    throw new Error(`${candidateId} build receipt target configuration is invalid.`);
  }
  for (const key of [
    "receipt_digest",
    "source_digest",
    "cargo_lock_digest",
    "binding_contract_digest",
    "dependency_closure_digest",
    "capability_recipe_digest",
    "runtime_catalog_digest",
    "input_digest",
    "artifact_digest",
  ]) {
    if (!SHA256_DIGEST.test(receipt[key] ?? "")) {
      throw new Error(`${candidateId} build receipt ${key} must be a sha256 digest.`);
    }
  }
  const expectedRuntimePaths = candidateId === "napi"
    ? ["merman.node"]
    : ["merman_node.js", "merman_node_bg.wasm", "package.json"];
  if (
    !Array.isArray(receipt.runtime_artifacts) ||
    stableJson(receipt.runtime_artifacts.map((artifact) => artifact?.path)) !==
      stableJson(expectedRuntimePaths) ||
    receipt.runtime_artifacts.some(
      (artifact) =>
        !Number.isSafeInteger(artifact?.bytes) ||
        artifact.bytes < 1 ||
        !SHA256_DIGEST.test(artifact?.sha256 ?? ""),
    ) ||
    receipt.runtime_artifacts[0].sha256 !== receipt.artifact_digest
  ) {
    throw new Error(`${candidateId} build receipt runtime artifact set is invalid.`);
  }
  if (receipt.commit !== commit) {
    throw new Error(`${candidateId} build receipt commit must match the benchmark commit.`);
  }
}

function validateFootprint(footprint, candidateId, buildReceipt, target = buildReceipt?.target) {
  assertObject(footprint, `${candidateId} footprint`);
  for (const key of ["packed_bytes", "unpacked_bytes", "installed_bytes"]) {
    if (!Number.isFinite(footprint[key]) || footprint[key] < 0) {
      throw new Error(`${candidateId} footprint ${key} is required.`);
    }
  }
  if (!Number.isSafeInteger(footprint.package_count) || footprint.package_count < 1) {
    throw new Error(`${candidateId} footprint package count is required.`);
  }
  for (const key of [
    "runtime_api_passed",
    "runtime_catalog_passed",
    "generic_operation_passed",
    "svg_plan_operation_passed",
    "svg_operation_passed",
    "request_options_passed",
    "browser_fallback_absent",
  ]) {
    if (footprint[key] !== true) {
      throw new Error(`${candidateId} footprint ${key} must be true.`);
    }
  }
  if (!new Set([
    "explicit-package-pair",
    "root-optional-dependency",
    "single-package",
  ]).has(footprint.install_method)) {
    throw new Error(`${candidateId} footprint install method is invalid.`);
  }
  if (typeof footprint.target_install_passed !== "boolean") {
    throw new Error(`${candidateId} target install evidence is required.`);
  }
  if (candidateId === "napi") {
    if (
      footprint.install_method !== "root-optional-dependency" ||
      footprint.optional_platform_package_passed !== true
    ) {
      throw new Error(
        "napi footprint must load the target package through the root optional dependency.",
      );
    }
  } else if (
    footprint.install_method !== "single-package" ||
    footprint.optional_platform_package_passed !== null
  ) {
    throw new Error("node-wasm footprint must use its explicit single-package candidate.");
  }
  if (!Array.isArray(footprint.packages) || footprint.packages.length !== footprint.package_count) {
    throw new Error(`${candidateId} package contents are required.`);
  }
  let packedBytes = 0;
  let unpackedBytes = 0;
  for (const packageResult of footprint.packages) {
    if (
      !packageResult ||
      typeof packageResult.name !== "string" ||
      packageResult.name.length === 0 ||
      typeof packageResult.version !== "string" ||
      packageResult.version.length === 0 ||
      typeof packageResult.filename !== "string" ||
      packageResult.filename.length === 0 ||
      !Number.isSafeInteger(packageResult.size) ||
      packageResult.size < 1 ||
      !Number.isSafeInteger(packageResult.unpacked_size) ||
      packageResult.unpacked_size < 1 ||
      !Array.isArray(packageResult.files)
    ) {
      throw new Error(`${candidateId} package contents are invalid.`);
    }
    const packagePaths = new Set();
    let packageBytes = 0;
    for (const file of packageResult.files) {
      if (
        !file ||
        typeof file.path !== "string" ||
        file.path.length === 0 ||
        packagePaths.has(file.path) ||
        !Number.isSafeInteger(file.bytes) ||
        file.bytes < 0
      ) {
        throw new Error(`${candidateId} package content entry is invalid.`);
      }
      packagePaths.add(file.path);
      packageBytes += file.bytes;
    }
    if (packageBytes !== packageResult.unpacked_size) {
      throw new Error(`${candidateId} package unpacked size does not match its file contents.`);
    }
    packedBytes += packageResult.size;
    unpackedBytes += packageResult.unpacked_size;
  }
  if (
    footprint.packed_bytes !== packedBytes ||
    footprint.unpacked_bytes !== unpackedBytes
  ) {
    throw new Error(`${candidateId} package footprint totals do not match their package contents.`);
  }
  if (!Array.isArray(footprint.installed_files) || footprint.installed_files.length === 0) {
    throw new Error(`${candidateId} installed package contents are required.`);
  }
  const installedPaths = new Set();
  let installedBytes = 0;
  for (const file of footprint.installed_files) {
    if (
      !file ||
      typeof file.path !== "string" ||
      file.path.length === 0 ||
      installedPaths.has(file.path) ||
      !Number.isSafeInteger(file.bytes) ||
      file.bytes < 0
    ) {
      throw new Error(`${candidateId} installed package content entry is invalid.`);
    }
    installedPaths.add(file.path);
    installedBytes += file.bytes;
  }
  if (footprint.installed_bytes !== installedBytes) {
    throw new Error(`${candidateId} installed footprint total does not match its file contents.`);
  }
  validateInstallationEvidence(
    footprint.installation_evidence,
    footprint,
    candidateId,
    buildReceipt,
    target,
  );
  validateInstalledRuntimeProbe(footprint.runtime_probe, candidateId, buildReceipt);
}

function validateInstallationEvidence(evidence, footprint, candidateId, buildReceipt, target) {
  assertObject(evidence, `${candidateId} installation evidence`);
  const packageNames = new Set();
  for (const packageResult of footprint.packages) {
    if (packageNames.has(packageResult.name)) {
      throw new Error(`${candidateId} package evidence contains duplicate package names.`);
    }
    packageNames.add(packageResult.name);
  }
  const installedFiles = new Map(
    footprint.installed_files.map((file) => [file.path, file]),
  );
  validateInstalledPackage(
    evidence.root_package,
    candidateId === "napi" ? ROOT_PACKAGE_NAME : WASM_PACKAGE_NAME,
    footprint.packages,
    `${candidateId} root package`,
  );
  if (
    typeof evidence.product_entrypoint !== "string" ||
    evidence.product_entrypoint.length === 0 ||
    !installedFiles.has(evidence.product_entrypoint)
  ) {
    throw new Error(`${candidateId} installation evidence lacks its product entrypoint.`);
  }
  if (!Array.isArray(evidence.loaded_artifacts) || evidence.loaded_artifacts.length === 0) {
    throw new Error(`${candidateId} installation evidence lacks loaded runtime artifacts.`);
  }
  const loadedPaths = new Set();
  for (const artifact of evidence.loaded_artifacts) {
    if (
      !artifact ||
      typeof artifact.path !== "string" ||
      artifact.path.length === 0 ||
      loadedPaths.has(artifact.path) ||
      !Number.isFinite(artifact.bytes) ||
      artifact.bytes < 1 ||
      !SHA256_DIGEST.test(artifact.sha256 ?? "") ||
      installedFiles.get(artifact.path)?.bytes !== artifact.bytes
    ) {
      throw new Error(`${candidateId} loaded runtime artifact evidence is invalid.`);
    }
    loadedPaths.add(artifact.path);
  }
  validateInstallResolution(evidence, footprint, candidateId, target);

  if (candidateId === "napi") {
    const targetContract = TARGET_CONTRACTS[target];
    if (!targetContract) throw new Error("napi installation evidence has an unknown target.");
    validateInstalledPackage(
      evidence.target_package,
      targetContract.package_name,
      footprint.packages,
      "napi target package",
    );
    const expectedEntrypoint = `${ROOT_PACKAGE_NAME}/dist/index.mjs`;
    const expectedArtifact = `${targetContract.package_name}/merman.node`;
    const [loadedArtifact] = evidence.loaded_artifacts;
    const rootManifest = evidence.root_package.manifest;
    const targetManifest = evidence.target_package.manifest;
    const expectedLibc = targetContract.libc === "gnu" ? "glibc" : targetContract.libc;
    if (
      footprint.package_count !== 2 ||
      evidence.product_entrypoint !== expectedEntrypoint ||
      evidence.loaded_artifacts.length !== 1 ||
      loadedArtifact.path !== expectedArtifact ||
      loadedArtifact.sha256 !== buildReceipt?.artifact_digest ||
      loadedArtifact.bytes !== buildReceipt?.runtime_artifacts?.[0]?.bytes ||
      rootManifest.optionalDependencies?.[targetContract.package_name] !==
        evidence.target_package.version ||
      evidence.root_package.version !== evidence.target_package.version ||
      targetManifest.main !== "./merman.node" ||
      stableJson(targetManifest.os) !== stableJson([targetContract.platform]) ||
      stableJson(targetManifest.cpu) !== stableJson([targetContract.arch]) ||
      (targetContract.libc !== null &&
        stableJson(targetManifest.libc) !== stableJson([expectedLibc])) ||
      (targetContract.libc === null && targetManifest.libc !== undefined)
    ) {
      throw new Error("napi installation evidence does not bind the target package and loaded artifact.");
    }
    return;
  }

  if (evidence.target_package !== null) {
    throw new Error("node-wasm installation evidence must not contain a target package.");
  }
  const expectedEntrypoint = `${WASM_PACKAGE_NAME}/index.mjs`;
  const loadedByPath = new Map(evidence.loaded_artifacts.map((artifact) => [artifact.path, artifact]));
  const expectedArtifacts = buildReceipt.runtime_artifacts.map((artifact) => ({
    ...artifact,
    path: `${WASM_PACKAGE_NAME}/artifact/${artifact.path}`,
  }));
  if (
    footprint.package_count !== 1 ||
    evidence.product_entrypoint !== expectedEntrypoint ||
    stableJson([...loadedByPath.keys()].sort()) !==
      stableJson(expectedArtifacts.map((artifact) => artifact.path).sort()) ||
    expectedArtifacts.some(
      (artifact) =>
        loadedByPath.get(artifact.path)?.sha256 !== artifact.sha256 ||
        loadedByPath.get(artifact.path)?.bytes !== artifact.bytes,
    )
  ) {
    throw new Error("node-wasm installation evidence does not bind the installed WASM artifacts.");
  }
}

function validateInstallResolution(evidence, footprint, candidateId, target) {
  const manifest = evidence.install_manifest;
  const lock = evidence.package_lock;
  assertObject(manifest, `${candidateId} install manifest`);
  assertObject(lock, `${candidateId} install lockfile`);
  assertObject(manifest.dependencies, `${candidateId} install dependencies`);
  assertObject(lock.packages, `${candidateId} install lockfile packages`);
  const rootName = candidateId === "napi" ? ROOT_PACKAGE_NAME : WASM_PACKAGE_NAME;
  const rootPackage = footprint.packages.find((item) => item.name === rootName);
  const rootReference = `file:../tarballs/${rootPackage?.filename}`;
  const expectedDependencies = { [rootName]: rootReference };
  const lockRoot = lock.packages[""];
  const installedRoot = lock.packages[`node_modules/${rootName}`];
  if (
    manifest.name !== "merman-node-footprint-probe" ||
    manifest.version !== "0.0.0" ||
    manifest.private !== true ||
    stableJson(manifest.dependencies) !== stableJson(expectedDependencies) ||
    lock.lockfileVersion !== 3 ||
    stableJson(lockRoot?.dependencies) !== stableJson(expectedDependencies) ||
    installedRoot?.version !== rootPackage?.version ||
    installedRoot?.resolved !== rootReference
  ) {
    throw new Error(`${candidateId} install manifest or lockfile root edge is invalid.`);
  }

  if (candidateId === "node-wasm") {
    if (manifest.overrides !== undefined) {
      throw new Error("node-wasm install manifest must not contain dependency overrides.");
    }
    return;
  }

  const targetContract = TARGET_CONTRACTS[target];
  const targetPackage = footprint.packages.find(
    (item) => item.name === targetContract?.package_name,
  );
  const targetReference = `file:../tarballs/${targetPackage?.filename}`;
  const installedTarget = lock.packages[`node_modules/${targetContract?.package_name}`];
  if (
    stableJson(manifest.overrides) !==
      stableJson({ [targetContract.package_name]: targetReference }) ||
    manifest.dependencies[targetContract.package_name] !== undefined ||
    installedRoot.optionalDependencies?.[targetContract.package_name] !==
      targetPackage?.version ||
    installedTarget?.version !== targetPackage?.version ||
    installedTarget?.resolved !== targetReference ||
    installedTarget?.optional !== true
  ) {
    throw new Error(
      "napi install evidence does not prove resolution through the root optional dependency.",
    );
  }
}

function validateInstalledPackage(value, expectedName, packages, label) {
  assertObject(value, label);
  assertObject(value.manifest, `${label} manifest`);
  const packed = packages.find((item) => item.name === value.name);
  if (
    value.name !== expectedName ||
    typeof value.version !== "string" ||
    value.version.length === 0 ||
    value.manifest.name !== value.name ||
    value.manifest.version !== value.version ||
    packed?.version !== value.version
  ) {
    throw new Error(`${label} identity is invalid.`);
  }
}

function validateInstalledRuntimeProbe(probe, candidateId, buildReceipt) {
  assertObject(probe, `${candidateId} installed runtime probe`);
  if (
    !SHA256_DIGEST.test(probe.runtime_catalog_digest ?? "") ||
    probe.runtime_catalog_digest !== buildReceipt?.runtime_catalog_digest
  ) {
    throw new Error(`${candidateId} installed runtime catalog does not match its build receipt.`);
  }
  const semantic = probe.semantic_operation;
  if (
    semantic?.operation_id !== "semantic-json" ||
    semantic?.media_type !== "application/json" ||
    !SHA256_DIGEST.test(semantic?.result_digest ?? "") ||
    !Number.isSafeInteger(semantic?.bytes) ||
    semantic.bytes < 1
  ) {
    throw new Error(`${candidateId} installed semantic operation evidence is invalid.`);
  }
  const svgPlan = probe.svg_plan_operation;
  if (
    svgPlan?.operation_id !== "svg-plan-json" ||
    svgPlan?.media_type !== "application/json" ||
    !SHA256_DIGEST.test(svgPlan?.result_digest ?? "") ||
    svgPlan?.planned_operation_id !== "svg" ||
    typeof svgPlan?.ready !== "boolean" ||
    !Number.isSafeInteger(svgPlan?.bytes) ||
    svgPlan.bytes < 1
  ) {
    throw new Error(`${candidateId} installed SVG capability-plan evidence is invalid.`);
  }
  const svg = probe.svg_operation;
  if (
    svg?.operation_id !== "svg" ||
    svg?.media_type !== "image/svg+xml" ||
    !SHA256_DIGEST.test(svg?.output_digest ?? "") ||
    !SHA256_DIGEST.test(svg?.structure_sha256 ?? "") ||
    !SHA256_DIGEST.test(svg?.geometry_sha256 ?? "") ||
    !Number.isSafeInteger(svg?.bytes) ||
    svg.bytes < 1
  ) {
    throw new Error(`${candidateId} installed SVG operation evidence is invalid.`);
  }
  if (probe.request_options_error?.code_name !== "MERMAN_RESOURCE_LIMIT_EXCEEDED") {
    throw new Error(`${candidateId} installed request-options evidence is invalid.`);
  }
}

function validateTargetEvidence(result, candidate, reportProvenance) {
  const evidence = result.evidence;
  assertObject(evidence, `${candidate.id} ${result.target} target evidence`);
  assertExactKeys(evidence, [
    "schema_version",
    "digest",
    "host",
    "provenance",
    "build_receipt",
    "footprint",
    "queue_lifecycle",
    "error_behavior",
  ], `${candidate.id} ${result.target} target evidence`);
  if (evidence.schema_version !== 1) {
    throw new Error(`${candidate.id} ${result.target} target evidence schema is invalid.`);
  }
  const { digest, ...payload } = evidence;
  if (!SHA256_DIGEST.test(digest ?? "") || digest !== digestJson(payload)) {
    throw new Error(`${candidate.id} ${result.target} target evidence digest is invalid.`);
  }

  const target = TARGET_CONTRACTS[result.target];
  assertObject(evidence.host, `${candidate.id} ${result.target} host evidence`);
  assertExactKeys(
    evidence.host,
    ["platform", "arch", "libc", "resolved_target", "node"],
    `${candidate.id} ${result.target} host evidence`,
  );
  if (
    evidence.host.platform !== target.platform ||
    evidence.host.arch !== target.arch ||
    evidence.host.libc !== target.libc ||
    evidence.host.resolved_target !== result.target ||
    typeof evidence.host.node !== "string" ||
    evidence.host.node.length === 0
  ) {
    throw new Error(`${candidate.id} ${result.target} host evidence does not match the target.`);
  }
  validateProvenance(evidence.provenance);
  if (
    evidence.provenance.commit !== reportProvenance.commit ||
    evidence.provenance.harness_digest !== reportProvenance.harness_digest ||
    evidence.provenance.machine.os !== evidence.host.platform ||
    evidence.provenance.machine.arch !== evidence.host.arch ||
    evidence.provenance.tools.node !== evidence.host.node ||
    (result.node !== undefined && result.node !== evidence.host.node)
  ) {
    throw new Error(`${candidate.id} ${result.target} provenance does not match its host evidence.`);
  }

  validateBuildReceipt(
    evidence.build_receipt,
    candidate.id,
    evidence.provenance.commit,
  );
  if (
    candidate.id === "napi" &&
    (evidence.build_receipt.target !== result.target ||
      evidence.build_receipt.rust_target !== target.rust_target)
  ) {
    throw new Error(`napi ${result.target} build receipt does not match its target evidence.`);
  }
  for (const key of [
    "source_digest",
    "cargo_lock_digest",
    "binding_contract_digest",
    "capability_recipe_digest",
    "runtime_catalog_digest",
  ]) {
    if (evidence.build_receipt[key] !== candidate.build_receipt[key]) {
      throw new Error(`${candidate.id} ${result.target} build receipt ${key} is inconsistent.`);
    }
  }
  if (
    candidate.id === "node-wasm" &&
    stableJson(evidence.build_receipt) !== stableJson(candidate.build_receipt)
  ) {
    throw new Error(`node-wasm ${result.target} must reuse the measured WASM build receipt.`);
  }

  validateFootprint(
    evidence.footprint,
    candidate.id,
    evidence.build_receipt,
    result.target,
  );
  validateQueueLifecycle(
    evidence.queue_lifecycle,
    `${candidate.id} ${result.target}`,
  );
  validateErrorBehavior(
    evidence.error_behavior,
    `${candidate.id} ${result.target}`,
  );
  if (
    result.runtime_passed !== true ||
    result.install_passed !== true ||
    evidence.footprint.target_install_passed !== true
  ) {
    throw new Error(
      `${candidate.id} ${result.target} pass flags do not match their complete target evidence.`,
    );
  }
}

function validateQueueLifecycle(value, label) {
  assertObject(value, `${label} queue/lifecycle evidence`);
  for (const key of [
    "saturation_passed",
    "dispose_passed",
    "queued_abort_passed",
    "non_preemptive_abort_passed",
    "process_shutdown_passed",
  ]) {
    if (value[key] !== true) {
      throw new Error(`${label} queue/lifecycle ${key} must be true.`);
    }
  }
  const evidence = value.evidence;
  assertObject(evidence, `${label} raw queue/lifecycle evidence`);
  const saturation = evidence.saturation;
  const disposal = evidence.disposal;
  const abort = evidence.abort;
  const shutdown = evidence.shutdown;
  assertObject(saturation, `${label} saturation evidence`);
  assertObject(disposal, `${label} disposal evidence`);
  assertObject(abort, `${label} abort evidence`);
  assertObject(shutdown, `${label} shutdown evidence`);
  validateLifecycleSettlement(saturation.active, "fulfilled", `${label} saturation active`);
  validateLifecycleSettlement(saturation.queued, "fulfilled", `${label} saturation queued`);
  validateLifecycleSettlement(
    saturation.saturated,
    "rejected",
    `${label} saturated request`,
    { code: "MERMAN_QUEUE_SATURATED" },
  );
  validateLifecycleSettlement(saturation.dispose, "fulfilled", `${label} saturation dispose`);
  validateLifecycleSettlement(disposal.active, "fulfilled", `${label} disposal active`);
  validateLifecycleSettlement(
    disposal.queued,
    "rejected",
    `${label} disposal queued`,
    { code: "MERMAN_ENGINE_DISPOSED" },
  );
  validateLifecycleSettlement(disposal.dispose, "fulfilled", `${label} disposal completion`);
  validateLifecycleSettlement(abort.executing, "fulfilled", `${label} abort executing`);
  validateLifecycleSettlement(
    abort.queued,
    "rejected",
    `${label} abort queued`,
    { name: "AbortError" },
  );
  validateLifecycleSettlement(abort.dispose, "fulfilled", `${label} abort dispose`);
  if (shutdown.render_succeeded !== true || shutdown.dispose_called !== false) {
    throw new Error(`${label} shutdown evidence is invalid.`);
  }
}

function validateLifecycleSettlement(value, status, label, expectedError = null) {
  assertObject(value, label);
  if (value.status !== status) throw new Error(`${label} settlement is invalid.`);
  if (status === "fulfilled") {
    if (value.error !== undefined) throw new Error(`${label} unexpectedly records an error.`);
    return;
  }
  assertObject(value.error, `${label} error`);
  for (const [key, expected] of Object.entries(expectedError)) {
    if (value.error[key] !== expected) {
      throw new Error(`${label} error ${key} is invalid.`);
    }
  }
}

function validateErrorBehavior(value, label) {
  if (value?.unknown_operation?.kind !== "unknown-operation") {
    throw new Error(`${label} must preserve the typed unknown-operation error.`);
  }
  if (
    value?.missing_capability?.kind !== "missing-capability" ||
    value?.missing_capability?.capability_id !== "png"
  ) {
    throw new Error(`${label} must preserve the typed missing-capability error.`);
  }
  if (value?.text_measurement_callback_rejected !== true) {
    throw new Error(`${label} must reject a JavaScript text measurement callback.`);
  }
  if (
    value?.unknown_operation?.unexpected_success === true ||
    value?.missing_capability?.unexpected_success === true
  ) {
    throw new Error(`${label} error probe unexpectedly succeeded.`);
  }
}

function validateDecision(decision, candidates) {
  assertObject(decision, "decision");
  if (!new Set(["inconclusive", "admitted", "rejected"]).has(decision.status)) {
    throw new Error("decision.status must be inconclusive, admitted, or rejected.");
  }
  if (!Array.isArray(decision.reasons) || decision.reasons.length === 0) {
    throw new Error("decision.reasons must explain the evidence decision.");
  }
  if (decision.status === "inconclusive") {
    if (decision.selected !== null) {
      throw new Error("an inconclusive decision cannot select a transport.");
    }
    return;
  }
  if (decision.status === "rejected") {
    if (decision.selected !== null) throw new Error("a rejected decision cannot select a transport.");
    return;
  }
  const selected = candidates.find((candidate) => candidate.id === decision.selected);
  if (!selected) throw new Error("admitted decision must select one measured candidate.");
  if (
    selected.target_results.length === 0 ||
    selected.target_results.some(
      (result) =>
        result.runtime_passed !== true ||
        result.install_passed !== true ||
        result.evidence === undefined,
    )
  ) {
    throw new Error(
      "selected targets require passing runtime and installation evidence before admission.",
    );
  }
  const measuredTargets = selected.target_results.map((result) => result.target).sort();
  if (JSON.stringify(measuredTargets) !== JSON.stringify([...INITIAL_TARGETS].sort())) {
    throw new Error("selected candidate must pass the complete initial target matrix before admission.");
  }
  if (
    selected.id === "napi" &&
    new Set(
      selected.target_results.map((result) => result.evidence.build_receipt.receipt_digest),
    ).size !== selected.target_results.length
  ) {
    throw new Error("napi admission requires one distinct build receipt per native target.");
  }
  if (selected.footprint.target_install_passed !== true) {
    throw new Error("selected candidate requires a passing product installation probe.");
  }
  if (
    selected.id === "napi" &&
    selected.footprint.install_method !== "root-optional-dependency"
  ) {
    throw new Error(
      "the selected napi candidate must resolve its platform package through the root optional dependency.",
    );
  }
  if (
    selected.corpus.successful < 1 ||
    selected.corpus.mismatched !== 0 ||
    selected.corpus.matched !== selected.corpus.cases
  ) {
    throw new Error("selected candidate must pass the complete corpus before admission.");
  }
  validateQueueLifecycle(selected.queue_lifecycle, selected.id);
}

function validateMismatchPaths(paths, expectedLength, label) {
  if (
    !Array.isArray(paths) ||
    paths.length !== expectedLength ||
    paths.some((path) => typeof path !== "string" || path.length === 0) ||
    new Set(paths).size !== paths.length
  ) {
    throw new Error(`${label} mismatch paths are invalid.`);
  }
}

function matchesBenchmarkWorkloadOutcome(outcome, workload) {
  return (
    outcome?.path === workload.path &&
    outcome.ok === true &&
    outcome.operation_id === "svg" &&
    outcome.media_type === "image/svg+xml" &&
    SHA256_DIGEST.test(outcome.sha256 ?? "") &&
    SHA256_DIGEST.test(outcome.svg_structure_sha256 ?? "") &&
    SHA256_DIGEST.test(outcome.svg_geometry_sha256 ?? "") &&
    Number.isSafeInteger(outcome.bytes) &&
    outcome.bytes > 0
  );
}

function validateStableBenchmarkWorkloadOutcomes(outcomes, label, representative) {
  const expected = workloadOutputEvidence(outcomes[0]);
  if (stableJson(expected) !== stableJson(representative)) {
    throw new Error(`${label} evidence does not match its representative raw SVG.`);
  }
  for (let index = 1; index < outcomes.length; index += 1) {
    if (stableJson(workloadOutputEvidence(outcomes[index])) !== stableJson(expected)) {
      throw new Error(`${label} SVG evidence drifted across repetitions.`);
    }
  }
}

function validateWorkloadRepresentative(value, workload, label) {
  assertObject(value, `${label} representative`);
  if (
    value.source_sha256 !== digest(workload.source) ||
    typeof value.raw_svg !== "string" ||
    value.raw_svg.length === 0
  ) {
    throw new Error(`${label} representative is not bound to its workload source.`);
  }
  const svgEvidence = svgTransportEvidence(value.raw_svg);
  return {
    sha256: digest(value.raw_svg),
    bytes: Buffer.byteLength(value.raw_svg),
    svg_structure_sha256: svgEvidence.structure_sha256,
    svg_geometry_sha256: svgEvidence.geometry_sha256,
  };
}

function workloadOutputEvidence(outcome) {
  return {
    sha256: outcome.sha256,
    bytes: outcome.bytes,
    svg_structure_sha256: outcome.svg_structure_sha256,
    svg_geometry_sha256: outcome.svg_geometry_sha256,
  };
}

function digest(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}

function matchesTimedOutcome(sample, corpus) {
  if (
    !sample ||
    !corpus ||
    sample.path !== corpus.path ||
    sample.ok !== corpus.ok
  ) return false;
  if (sample.ok) {
    return (
      sample.operation_id === "svg" &&
      sample.media_type === "image/svg+xml" &&
      sample.operation_id === corpus.operation_id &&
      sample.media_type === corpus.media_type &&
      sample.sha256 === corpus.sha256 &&
      sample.bytes === corpus.bytes
    );
  }
  return (
    sample.kind === corpus.kind &&
    (sample.code_name ?? null) === (corpus.code_name ?? null) &&
    (sample.capability_id ?? null) === (corpus.capability_id ?? null)
  );
}

function validateOperationEvidence(
  evidence,
  label,
  { operationId = null, mediaType = null } = {},
) {
  assertObject(evidence, label);
  if (typeof evidence.ok !== "boolean") throw new Error(`${label} must record success or failure.`);
  if (evidence.ok) {
    if (
      typeof evidence.operation_id !== "string" ||
      typeof evidence.media_type !== "string" ||
      (operationId !== null && evidence.operation_id !== operationId) ||
      (mediaType !== null && evidence.media_type !== mediaType) ||
      !SHA256_DIGEST.test(evidence.sha256 ?? "")
    ) {
      throw new Error(`${label} lacks successful output evidence.`);
    }
  } else if (!hasTypedFailureEvidence(evidence)) {
    throw new Error(`${label} lacks typed error evidence.`);
  }
}

function hasTypedFailureEvidence(evidence) {
  const codeName = evidence?.code_name;
  if (FATAL_FAILURE_STATUS_CODES.has(codeName)) return false;
  if (EXPECTED_FAILURE_STATUS_CODES.has(codeName)) return true;
  return (
    typeof evidence?.kind === "string" &&
    evidence.kind.length > 0 &&
    evidence.kind !== "generic"
  );
}

function validateSamples(samples, label) {
  if (
    !Array.isArray(samples) ||
    samples.length === 0 ||
    samples.some((value) => !Number.isFinite(value) || value < 0)
  ) {
    throw new Error(`${label} samples are required.`);
  }
}

function validateSampling(sampling) {
  assertObject(sampling, "sampling");
  for (const key of [
    "cold_processes",
    "warmup_iterations",
    "measured_iterations",
    "concurrency_iterations",
  ]) {
    if (!Number.isSafeInteger(sampling[key]) || sampling[key] < 1) {
      throw new Error(`sampling.${key} must be a positive integer.`);
    }
  }
}

function validateSummary(summary, samples, label) {
  assertObject(summary, `${label} summary`);
  if (stableJson(summary) !== stableJson(summarize(samples))) {
    throw new Error(`${label} summary does not match its raw samples.`);
  }
}

function assertObject(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object.`);
  }
}

function assertExactKeys(value, expected, label) {
  const actual = Object.keys(value).sort();
  const normalizedExpected = [...expected].sort();
  if (stableJson(actual) !== stableJson(normalizedExpected)) {
    throw new Error(`${label} fields are invalid.`);
  }
}

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}
