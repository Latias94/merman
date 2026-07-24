import { digestJson, stableJson } from "../stable-json.mjs";

const REQUIRED_TOOLS = [
  "node",
  "rustc",
  "cargo",
  "napi",
  "napi_derive",
  "napi_build",
  "napi_cli",
];
const INITIAL_TARGETS = [
  "darwin-arm64",
  "darwin-x64",
  "linux-x64-gnu",
  "linux-x64-musl",
  "win32-x64-msvc",
];
const SHA256_DIGEST = /^sha256:[0-9a-f]{64}$/;

export function computeInputDigest({ cases, bindingOptions, formatOptions }) {
  const normalizedCases = [...cases]
    .map(({ path, source }) => ({ path, source }))
    .sort((left, right) => left.path.localeCompare(right.path));
  return digestJson({
    cases: normalizedCases,
    binding_options: bindingOptions,
    format_options: formatOptions,
  });
}

export function validateComparisonReport(report) {
  assertObject(report, "report");
  if (report.schema_version !== 1) throw new Error("report schema_version must be 1.");
  validateProvenance(report.provenance);
  assertObject(report.input, "input");
  if (!SHA256_DIGEST.test(report.input.digest ?? "")) {
    throw new Error("input digest must be a sha256 digest.");
  }
  if (report.input.binding_options?.runtime_policy !== "deterministic") {
    throw new Error("comparison input must select deterministic runtime policy.");
  }
  const profile = report.input.binding_options?.resources?.profile;
  if (typeof profile !== "string" || profile.length === 0) {
    throw new Error("comparison input must name one shared resource profile.");
  }
  if (!Array.isArray(report.candidates) || report.candidates.length !== 2) {
    throw new Error("comparison must contain exactly the node-wasm and napi candidates.");
  }
  const ids = report.candidates.map((candidate) => candidate.id).sort();
  if (stableJson(ids) !== stableJson(["napi", "node-wasm"])) {
    throw new Error("comparison must contain exactly the node-wasm and napi candidates.");
  }
  for (const candidate of report.candidates) {
    validateCandidate(candidate, report.input.digest, report.provenance.commit);
  }
  const bindingContractDigests = new Set(
    report.candidates.map((candidate) => candidate.build_receipt.binding_contract_digest),
  );
  if (bindingContractDigests.size !== 1) {
    throw new Error("candidate build receipts must share one bindings-contract digest.");
  }
  const capabilityRecipeDigests = new Set(
    report.candidates.map((candidate) => candidate.build_receipt.capability_recipe_digest),
  );
  if (capabilityRecipeDigests.size !== 1) {
    throw new Error("candidate build receipts must share one capability-recipe digest.");
  }
  validateDecision(report.decision, report.candidates);
  return report;
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

function validateCandidate(candidate, inputDigest, commit) {
  assertObject(candidate, "candidate");
  validateBuildReceipt(candidate.build_receipt, candidate.id, commit);
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
        !SHA256_DIGEST.test(outcome.sha256 ?? "") ||
        !SHA256_DIGEST.test(outcome.svg_structure_sha256 ?? "") ||
        !SHA256_DIGEST.test(outcome.svg_geometry_sha256 ?? "") ||
        !Number.isFinite(outcome.bytes)
      ) {
        throw new Error(`${candidate.id} successful corpus outcome lacks output evidence.`);
      }
    } else if (typeof outcome.kind !== "string" || outcome.kind.length === 0) {
      throw new Error(`${candidate.id} failed corpus outcome lacks a typed error kind.`);
    }
    validateOperationEvidence(outcome.semantic, `${candidate.id} semantic outcome`);
    outcomePaths.add(outcome.path);
  }
  if (candidate.cold_process?.isolated_processes !== true) {
    throw new Error(`${candidate.id} cold samples must use an isolated process per sample.`);
  }
  validateSamples(candidate.cold_process?.samples_ms, `${candidate.id} cold process`);
  if (
    !Array.isArray(candidate.cold_process?.samples) ||
    candidate.cold_process.samples.length !== candidate.cold_process.samples_ms.length
  ) {
    throw new Error(`${candidate.id} raw cold-process samples are required.`);
  }
  for (let index = 0; index < candidate.cold_process.samples.length; index += 1) {
    const sample = candidate.cold_process.samples[index];
    if (
      !sample ||
      sample.elapsed_ms !== candidate.cold_process.samples_ms[index] ||
      !Number.isFinite(sample.operation_ms) ||
      !Number.isFinite(sample.baseline_rss_bytes) ||
      !Number.isFinite(sample.peak_rss_bytes)
    ) {
      throw new Error(`${candidate.id} raw cold-process sample ${index} is incomplete.`);
    }
  }
  validateSamples(candidate.warm_latency?.samples_ms, `${candidate.id} warm latency`);
  if (
    !Array.isArray(candidate.warm_latency?.samples) ||
    candidate.warm_latency.samples.length !== candidate.warm_latency.samples_ms.length
  ) {
    throw new Error(`${candidate.id} raw warm-latency samples are required.`);
  }
  for (let index = 0; index < candidate.warm_latency.samples.length; index += 1) {
    const sample = candidate.warm_latency.samples[index];
    if (
      !sample ||
      sample.elapsed_ms !== candidate.warm_latency.samples_ms[index] ||
      !Number.isSafeInteger(sample.iteration) ||
      sample.iteration < 0 ||
      typeof sample.path !== "string" ||
      sample.path.length === 0 ||
      !sample.outcome ||
      typeof sample.outcome.ok !== "boolean"
    ) {
      throw new Error(`${candidate.id} raw warm-latency sample ${index} is incomplete.`);
    }
  }
  if (typeof candidate.rss?.method !== "string" || candidate.rss.method.length === 0) {
    throw new Error(`${candidate.id} RSS measurement method is required.`);
  }
  for (const key of ["peak_bytes", "baseline_bytes"]) {
    if (!Number.isFinite(candidate.rss?.[key])) throw new Error(`${candidate.id} RSS ${key} is required.`);
  }
  validateFootprint(candidate.footprint, candidate.id);
  for (const key of [
    "saturation_passed",
    "dispose_passed",
    "non_preemptive_abort_passed",
  ]) {
    if (typeof candidate.queue_lifecycle?.[key] !== "boolean") {
      throw new Error(`${candidate.id} queue/lifecycle ${key} is required.`);
    }
  }
  if (!Number.isSafeInteger(candidate.concurrency?.workers) || candidate.concurrency.workers < 1) {
    throw new Error(`${candidate.id} concurrency worker count is required.`);
  }
  if (
    !Number.isSafeInteger(candidate.concurrency?.requests_per_batch) ||
    candidate.concurrency.requests_per_batch < 1
  ) {
    throw new Error(`${candidate.id} concurrency batch size is required.`);
  }
  validateSamples(candidate.concurrency?.batch_samples_ms, `${candidate.id} concurrency batch`);
  if (candidate.error_behavior?.unknown_operation?.kind !== "unknown-operation") {
    throw new Error(`${candidate.id} must preserve the typed unknown-operation error.`);
  }
  if (
    candidate.error_behavior?.missing_capability?.kind !== "missing-capability" ||
    candidate.error_behavior?.missing_capability?.capability_id !== "png"
  ) {
    throw new Error(`${candidate.id} must preserve the typed missing-capability error.`);
  }
  if (candidate.error_behavior?.text_measurement_callback_rejected !== true) {
    throw new Error(`${candidate.id} must reject a JavaScript text measurement callback.`);
  }
  if (
    candidate.error_behavior?.unknown_operation?.unexpected_success === true ||
    candidate.error_behavior?.missing_capability?.unexpected_success === true
  ) {
    throw new Error(`${candidate.id} error probe unexpectedly succeeded.`);
  }
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
      typeof result.install_passed !== "boolean"
    ) {
      throw new Error(`${candidate.id} target result is invalid.`);
    }
    targets.add(result.target);
  }
}

function validateBuildReceipt(receipt, candidateId, commit) {
  assertObject(receipt, `${candidateId} build receipt`);
  for (const key of [
    "receipt_digest",
    "source_digest",
    "binding_contract_digest",
    "capability_recipe_digest",
    "input_digest",
    "artifact_digest",
  ]) {
    if (!SHA256_DIGEST.test(receipt[key] ?? "")) {
      throw new Error(`${candidateId} build receipt ${key} must be a sha256 digest.`);
    }
  }
  if (receipt.commit !== commit) {
    throw new Error(`${candidateId} build receipt commit must match the benchmark commit.`);
  }
}

function validateFootprint(footprint, candidateId) {
  assertObject(footprint, `${candidateId} footprint`);
  for (const key of ["packed_bytes", "unpacked_bytes", "installed_bytes"]) {
    if (!Number.isFinite(footprint[key]) || footprint[key] < 0) {
      throw new Error(`${candidateId} footprint ${key} is required.`);
    }
  }
  if (!Number.isSafeInteger(footprint.package_count) || footprint.package_count < 1) {
    throw new Error(`${candidateId} footprint package count is required.`);
  }
  if (footprint.runtime_api_passed !== true) {
    throw new Error(`${candidateId} installed package API probe is required.`);
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
  if (!Array.isArray(footprint.packages) || footprint.packages.length !== footprint.package_count) {
    throw new Error(`${candidateId} package contents are required.`);
  }
  for (const packageResult of footprint.packages) {
    if (
      !packageResult ||
      typeof packageResult.filename !== "string" ||
      packageResult.filename.length === 0 ||
      !Number.isFinite(packageResult.size) ||
      !Number.isFinite(packageResult.unpacked_size) ||
      !Array.isArray(packageResult.files)
    ) {
      throw new Error(`${candidateId} package contents are invalid.`);
    }
    for (const file of packageResult.files) {
      if (
        !file ||
        typeof file.path !== "string" ||
        file.path.length === 0 ||
        !Number.isFinite(file.bytes) ||
        file.bytes < 0
      ) {
        throw new Error(`${candidateId} package content entry is invalid.`);
      }
    }
  }
  if (!Array.isArray(footprint.installed_files) || footprint.installed_files.length === 0) {
    throw new Error(`${candidateId} installed package contents are required.`);
  }
  for (const file of footprint.installed_files) {
    if (
      !file ||
      typeof file.path !== "string" ||
      file.path.length === 0 ||
      !Number.isFinite(file.bytes) ||
      file.bytes < 0
    ) {
      throw new Error(`${candidateId} installed package content entry is invalid.`);
    }
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
      (result) => result.runtime_passed !== true || result.install_passed !== true,
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
  if (selected.corpus.mismatched !== 0 || selected.corpus.matched !== selected.corpus.cases) {
    throw new Error("selected candidate must pass the complete corpus before admission.");
  }
  if (
    Object.values(selected.queue_lifecycle).some((passed) => passed !== true)
  ) {
    throw new Error("selected candidate must pass every queue and lifecycle probe before admission.");
  }
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

function validateOperationEvidence(evidence, label) {
  assertObject(evidence, label);
  if (typeof evidence.ok !== "boolean") throw new Error(`${label} must record success or failure.`);
  if (evidence.ok) {
    if (
      typeof evidence.operation_id !== "string" ||
      typeof evidence.media_type !== "string" ||
      !SHA256_DIGEST.test(evidence.sha256 ?? "")
    ) {
      throw new Error(`${label} lacks successful output evidence.`);
    }
  } else if (typeof evidence.kind !== "string" || evidence.kind.length === 0) {
    throw new Error(`${label} lacks a typed error kind.`);
  }
}

function validateSamples(samples, label) {
  if (!Array.isArray(samples) || samples.length === 0 || samples.some((value) => !Number.isFinite(value))) {
    throw new Error(`${label} samples are required.`);
  }
}

function assertObject(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object.`);
  }
}
