import { digestJson, stableJson } from "../stable-json.mjs";

const SCALE_POINTS = [1, 2, 4, 10, 32, 100];
const OVERLAY_IDS = ["empty", "version-only", "real-resource-override"];
const TRANSPORTS = ["napi", "node-wasm"];
const REVISIONS = ["base", "head"];
const RSS_METHOD = "lane-local-retained/fresh-process-envelope-v4";
const CONFIRMATION_CRITICAL_Z = 2.2414027264652865;
const CONFIRMATION_POWER_Z = 0.8416212327266186;
const DECISION_STATUSES = [
  "confirmed-improvement",
  "rejected",
  "regressed",
  "inconclusive",
  "contract-failure",
];
const SHA256 = /^sha256:[0-9a-f]{64}$/;
const COMMIT = /^[0-9a-f]{40}$/;

export function validateRequestOverlayManifest(value) {
  assertObject(value, "request-overlay manifest");
  if (value.schema_version !== 2) fail("manifest schema_version must be 2");
  if (
    value.lane_id !== "binding-request-overlay-node-owner" ||
    value.owner !== "merman-bindings-core"
  ) {
    fail("manifest identity is not the U6 bindings owner lane");
  }
  if (value.transport_admission !== "not-evaluated") {
    fail("the owner lane must not claim Node transport admission");
  }
  assertEqual(value.timing_contract, {
    clock: "process.hrtime.bigint",
    operation: "executeSync",
    request_json: "pre-encoded-before-timing",
    base_options_json: "pre-encoded-before-timing",
    evidence_projection: "excluded-from-timing",
    engine_disposal: "excluded-from-timing",
    sample_fields: ["construct_ns", "batch_ns", "total_ns"],
  }, "manifest timing_contract");

  assertObject(value.operation, "manifest operation");
  if (
    value.operation.operation_id !== "semantic-json" ||
    value.operation.source !== "info\nshowInfo\n" ||
    value.operation.uri !== null
  ) {
    fail("manifest operation must be the fixed info semantic workload");
  }
  assertObject(value.base_options, "manifest base_options");
  if (
    value.base_options.version !== 1 ||
    value.base_options.runtime_policy !== "deterministic" ||
    value.base_options.resources?.profile !== "trusted-native"
  ) {
    fail("manifest base_options must select deterministic trusted-native behavior");
  }

  if (!Array.isArray(value.overlays) || value.overlays.length !== OVERLAY_IDS.length) {
    fail("manifest must contain exactly three request-overlay cases");
  }
  assertEqual(value.overlays.map((entry) => entry?.id), OVERLAY_IDS, "manifest overlay IDs");
  if (value.overlays[0].request_options !== null) {
    fail("the empty overlay must omit options_json");
  }
  for (const overlay of value.overlays.slice(1)) {
    assertObject(overlay.request_options, `manifest overlay ${overlay.id}`);
    if (overlay.request_options.version !== 1) {
      fail(`manifest overlay ${overlay.id} must use schema version 1`);
    }
  }

  assertObject(value.success_contract, "manifest success_contract");
  if (
    value.success_contract.version !== 1 ||
    value.success_contract.ok !== true ||
    value.success_contract.result?.operation_id !== "semantic-json" ||
    value.success_contract.result?.media_type !== "application/json" ||
    value.success_contract.result?.data !== '{"type":"info","showInfo":true}' ||
    value.success_contract.result?.metadata_json !==
      '{"version":1,"operation_id":"semantic-json","media_type":"application/json","runtime_policy":"deterministic","byte_length":31}'
  ) {
    fail("manifest success contract is not the exact info semantic result");
  }
  assertObject(value.error_probe, "manifest error_probe");
  if (
    value.error_probe.id !== "forbidden-runtime-policy" ||
    value.error_probe.request_options?.version !== 1 ||
    value.error_probe.request_options?.runtime_policy !== "native" ||
    value.error_probe.expected?.ok !== false ||
    value.error_probe.expected?.error?.code !== 3 ||
    value.error_probe.expected?.error?.code_name !== "MERMAN_OPTIONS_JSON_ERROR"
  ) {
    fail("manifest error probe is not the fixed request-scope policy rejection");
  }
  assertObject(value.resource_limit_probe, "manifest resource_limit_probe");
  if (
    value.resource_limit_probe.id !== "real-overlay-max-source-bytes" ||
    value.resource_limit_probe.source_prefix !== "info\n" ||
    value.resource_limit_probe.padding_character !== "x" ||
    value.resource_limit_probe.padding_bytes !== 4096 ||
    value.resource_limit_probe.overlay_id !== "real-resource-override" ||
    value.resource_limit_probe.expected?.ok !== false ||
    value.resource_limit_probe.expected?.error?.code !== 10 ||
    value.resource_limit_probe.expected?.error?.code_name !==
      "MERMAN_RESOURCE_LIMIT_EXCEEDED" ||
    value.resource_limit_probe.expected?.error?.message !==
      "resource limit exceeded during source: max_source_bytes actual=4101 max=4096"
  ) {
    fail("manifest resource-limit probe differs from the fixed real-overlay contract");
  }

  assertObject(value.scaling, "manifest scaling");
  assertEqual(value.scaling.batch_sizes, SCALE_POINTS, "manifest batch sizes");
  assertEqual(value.scaling.base_size_units, SCALE_POINTS, "manifest base-size points");
  if (
    !Number.isSafeInteger(value.scaling.base_size_bytes_per_unit) ||
    value.scaling.base_size_bytes_per_unit < 1 ||
    value.scaling.overlay_id !== "version-only"
  ) {
    fail("manifest scaling contract is invalid");
  }

  validateSamplingDefaults(value.sampling_defaults);
  validateSamplingLimits(value.sampling_limits);
  assertObject(value.statistics, "manifest statistics");
  if (
    value.statistics.method !== "deterministic-paired-bootstrap-mc-bounded-v2" ||
    value.statistics.confidence_level !== 0.95 ||
    value.statistics.bootstrap_seed !== 84_960_128_876_878 ||
    value.statistics.bootstrap_resamples !== 10_000 ||
    value.statistics.monte_carlo_failure_probability !== 0.001 ||
    value.statistics.multiplicity_adjustment !== "bonferroni" ||
    value.statistics.aa_family_size !== REVISIONS.length * TRANSPORTS.length * 4 ||
    value.statistics.confirmation_family_size !== TRANSPORTS.length * 2
  ) {
    fail("manifest statistics differ from the registered simultaneous bootstrap contract");
  }
  assertObject(value.rss_contract, "manifest rss_contract");
  assertEqual(value.rss_contract.dimensions, [
    { id: "batch-reused", measurement: "batch", lifecycle: "reused" },
    { id: "base-size-cold", measurement: "base-size", lifecycle: "cold" },
    { id: "base-size-reused", measurement: "base-size", lifecycle: "reused" },
  ], "manifest RSS dimensions");
  if (
    value.rss_contract.method !== RSS_METHOD ||
    value.rss_contract.curve_transform !== "ols-log-scale-log1p-growth-v1" ||
    value.rss_contract.curve_metric !==
      "lane-local-sampled-current-growth-bytes" ||
    value.rss_contract.process_envelope_metric !==
      "fresh-process-current-or-max-growth-v1" ||
    value.rss_contract.maximum_slope_upper_bound !== 1 ||
    value.rss_contract.maximum_absolute_growth_bytes !== 64 * 1024 * 1024 ||
    value.rss_contract.maximum_head_regression_bytes !== 1024 * 1024 ||
    value.rss_contract.maximum_process_envelope_growth_bytes !== 64 * 1024 * 1024 ||
    value.rss_contract.maximum_head_process_envelope_regression_bytes !== 1024 * 1024 ||
    value.rss_contract.maximum_startup_history_gap_bytes !== 1024 * 1024 ||
    value.rss_contract.maximum_lane_history_gap_bytes !== 1024 * 1024 ||
    value.rss_contract.upper_bound_method !== "observed-process-maximum-v1"
  ) {
    fail("manifest RSS contract differs from the registered owner bounds");
  }
  assertObject(value.decision_rule, "manifest decision_rule");
  if (
    value.decision_rule.primary_overlay_id !== "version-only" ||
    value.decision_rule.primary_engine_lifecycle !== "reused" ||
    value.decision_rule.primary_batch_size !== 1 ||
    value.decision_rule.minimum_relative_effect !== 0.1 ||
    value.decision_rule.minimum_absolute_effect_ns !== 50_000 ||
    value.decision_rule.one_sided_confidence !== 0.95 ||
    value.decision_rule.power !== 0.8
  ) {
    fail("manifest decision rule differs from the registered ordinary admission gate");
  }
  return structuredClone(value);
}

export function prepareRequestOverlayInputs(manifestValue) {
  const manifest = validateRequestOverlayManifest(manifestValue);
  const requestJsonByOverlay = Object.fromEntries(
    manifest.overlays.map((overlay) => [
      overlay.id,
      encodeRequest(manifest.operation, overlay.request_options),
    ]),
  );
  const baseJsonByUnits = Object.fromEntries(
    manifest.scaling.base_size_units.map((units) => {
      const options = structuredClone(manifest.base_options);
      options.site_config = {
        u6_request_overlay_padding: "x".repeat(
          units * manifest.scaling.base_size_bytes_per_unit,
        ),
      };
      return [String(units), JSON.stringify(options)];
    }),
  );
  return {
    manifest,
    manifest_digest: digestJson(manifest),
    request_json_by_overlay: requestJsonByOverlay,
    error_request_json: encodeRequest(
      manifest.operation,
      manifest.error_probe.request_options,
    ),
    resource_limit_request_json: encodeRequest(
      {
        ...manifest.operation,
        source:
          manifest.resource_limit_probe.source_prefix +
          manifest.resource_limit_probe.padding_character.repeat(
            manifest.resource_limit_probe.padding_bytes,
          ),
      },
      manifest.overlays.find(
        (overlay) => overlay.id === manifest.resource_limit_probe.overlay_id,
      ).request_options,
    ),
    base_json_by_units: baseJsonByUnits,
  };
}

export function requestOverlayRssLanePlan(manifestValue, samplingValue) {
  const manifest = validateRequestOverlayManifest(manifestValue);
  const sampling = validateSampling(samplingValue, manifest);
  return buildRssLanePlan(manifest, sampling);
}

export function validateSampling(value, manifestValue) {
  const manifest = validateRequestOverlayManifest(manifestValue);
  assertObject(value, "sampling");
  for (const key of [
    "aa_pairs",
    "maximum_confirmation_pairs",
    "confirmation_pairs",
    "cold_samples",
    "warmup_iterations",
    "reused_samples",
  ]) {
    if (!Number.isSafeInteger(value[key]) || value[key] < 1) {
      fail(`sampling.${key} must be a positive integer`);
    }
  }
  if (
    value.aa_pairs < 8 ||
    value.aa_pairs > 32 ||
    value.aa_pairs % 2 !== 0 ||
    value.maximum_confirmation_pairs !== manifest.sampling_defaults.maximum_confirmation_pairs ||
    value.confirmation_pairs < 8 ||
    value.confirmation_pairs > value.maximum_confirmation_pairs ||
    value.confirmation_pairs % 2 !== 0
  ) {
    fail("sampling pair counts violate the registered balanced 8..32 budget");
  }
  const limits = manifest.sampling_limits;
  if (
    value.cold_samples > limits.maximum_cold_samples ||
    value.warmup_iterations > limits.maximum_warmup_iterations ||
    value.reused_samples > limits.maximum_reused_samples ||
    estimatedWorkerLogicalOperations(value) >
      limits.maximum_logical_operations_per_worker
  ) {
    fail("sampling workload exceeds the registered worker budget");
  }
  return value;
}

export function explorationCells({ aaPairs }) {
  validateEvenPairCount(aaPairs, "A/A pairs");
  const cells = [];
  for (let pairIndex = 0; pairIndex < aaPairs; pairIndex += 1) {
    for (const calibrationRevision of REVISIONS) {
      for (const transport of TRANSPORTS) {
        const order = pairIndex % 2 === 0 ? ["a", "b"] : ["b", "a"];
        for (const [orderPosition, role] of order.entries()) {
          cells.push({
            id: `aa:${calibrationRevision}:${transport}:${pairIndex}:${orderPosition}`,
            phase: "aa",
            calibration_revision: calibrationRevision,
            transport,
            pair_index: pairIndex,
            order,
            order_position: orderPosition,
            role,
            artifact_key: `${calibrationRevision}:${transport}`,
          });
        }
      }
    }
  }
  return cells;
}

export function confirmationCells({ confirmationPairs }) {
  validateEvenPairCount(confirmationPairs, "confirmation pairs");
  const cells = [];
  for (let pairIndex = 0; pairIndex < confirmationPairs; pairIndex += 1) {
    for (const transport of TRANSPORTS) {
      const order = pairIndex % 2 === 0 ? ["base", "head"] : ["head", "base"];
      for (const [orderPosition, role] of order.entries()) {
        cells.push({
          id: `ab-ba:${transport}:${pairIndex}:${orderPosition}`,
          phase: "ab_ba",
          calibration_revision: null,
          transport,
          pair_index: pairIndex,
          order,
          order_position: orderPosition,
          role,
          artifact_key: `${role}:${transport}`,
        });
      }
    }
  }
  return cells;
}

export function summarizeTimingSamples(samples, logicalOperationsPerSample) {
  if (
    !Array.isArray(samples) ||
    samples.length === 0 ||
    !Number.isSafeInteger(logicalOperationsPerSample) ||
    logicalOperationsPerSample < 1
  ) {
    fail("timing samples require a non-empty distribution and logical operation count");
  }
  for (const sample of samples) validateTimingSample(sample);
  return {
    count: samples.length,
    construct_ns: summarizeNumbers(samples.map((sample) => sample.construct_ns)),
    batch_ns: summarizeNumbers(samples.map((sample) => sample.batch_ns)),
    total_ns: summarizeNumbers(samples.map((sample) => sample.total_ns)),
    per_operation_batch_ns: summarizeNumbers(
      samples.map((sample) => sample.batch_ns / logicalOperationsPerSample),
    ),
  };
}

export function validateRequestOverlayWorkerResult(
  value,
  { artifactKey, artifactIdentity, manifest: manifestValue, sampling },
) {
  const manifest = validateRequestOverlayManifest(manifestValue);
  assertObject(value, "worker result");
  if (
    value.schema_version !== 2 ||
    value.lane_id !== manifest.lane_id ||
    value.artifact_key !== artifactKey ||
    value.manifest_digest !== digestJson(manifest)
  ) {
    fail("worker result identity does not match its invocation");
  }
  const [revision, transport] = artifactKey.split(":");
  if (value.revision !== revision || value.transport !== transport) {
    fail("worker result revision or transport is inconsistent");
  }
  assertObject(value.process, "worker process");
  if (
    !Number.isSafeInteger(value.process.pid) ||
    value.process.pid < 1 ||
    typeof value.process.invocation_nonce !== "string" ||
    !/^[0-9a-f]{32}$/.test(value.process.invocation_nonce) ||
    typeof value.process.parent_invocation_id !== "string" ||
    !/^[0-9a-f]{32}$/.test(value.process.parent_invocation_id) ||
    value.process.gc_mode !== "exposed-double-before-lane-and-sample" ||
    value.process.clock !== "process.hrtime.bigint"
  ) {
    fail("worker process evidence is invalid");
  }
  assertEqual(value.artifact, artifactIdentity, "worker artifact identity");
  validateRss(value.rss, manifest, sampling);
  const measurementSemanticPassed = validateMeasurements(
    value.measurements,
    manifest,
    sampling,
  );
  validateSemanticEvidence(
    value.semantic_evidence,
    manifest,
    artifactIdentity,
    measurementSemanticPassed,
  );
  return value;
}

export function projectSemanticEvidence(cells) {
  return {
    passed: cells.every((cell) => cell.result.semantic_evidence.passed === true),
    cells: cells.map((cell) => ({
      cell_id: cell.id,
      artifact_key: cell.artifact_key,
      success_digest: digestJson(cell.result.semantic_evidence.success_probes),
      reused_success_digest:
        digestJson(cell.result.semantic_evidence.reused_success_probes),
      measurement_digest: digestJson({
        passed: cell.result.measurements.semantic_contract_passed,
        overlays: cell.result.measurements.overlays.map(
          (lane) => lane.semantic_contract,
        ),
        batch_scaling: cell.result.measurements.batch_scaling.map(
          (lane) => lane.semantic_contract,
        ),
        base_size_scaling: cell.result.measurements.base_size_scaling.map(
          (lane) => lane.semantic_contract,
        ),
      }),
      error_digest: digestJson(cell.result.semantic_evidence.error_probe),
      resource_limit_digest: digestJson(cell.result.semantic_evidence.resource_limit_probe),
      runtime_catalog_digest: cell.result.semantic_evidence.runtime_catalog_digest,
    })),
  };
}

export function projectRss(cells) {
  return artifactKeys().map((artifactKey) => ({
    artifact_key: artifactKey,
    method: RSS_METHOD,
    processes: cells
      .filter((cell) => cell.artifact_key === artifactKey)
      .map((cell) => ({
        cell_id: cell.id,
        baseline_current_rss_bytes: cell.result.rss.baseline_current_rss_bytes,
        baseline_process_max_rss_bytes:
          cell.result.rss.baseline_process_max_rss_bytes,
        baseline_history_gap_bytes: cell.result.rss.baseline_history_gap_bytes,
        final_current_rss_bytes: cell.result.rss.final_current_rss_bytes,
        final_process_max_rss_bytes: cell.result.rss.final_process_max_rss_bytes,
        peak_sampled_current_rss_bytes:
          cell.result.rss.peak_sampled_current_rss_bytes,
        peak_process_max_rss_bytes: cell.result.rss.peak_process_max_rss_bytes,
        sampled_current_peak_growth_bytes:
          cell.result.rss.sampled_current_peak_growth_bytes,
        process_max_peak_growth_bytes:
          cell.result.rss.process_max_peak_growth_bytes,
        fresh_process_envelope_growth_bytes:
          cell.result.rss.fresh_process_envelope_growth_bytes,
        lanes: structuredClone(cell.result.rss.lanes),
      })),
  }));
}

export function qualifyNoise(cells, manifestValue, maximumConfirmationPairs = 32) {
  const manifest = validateRequestOverlayManifest(manifestValue);
  validateEvenPairCount(maximumConfirmationPairs, "maximum confirmation pairs");
  const powerContract = confirmationPowerContract(manifest);
  const byArtifact = Object.fromEntries(
    REVISIONS.flatMap((calibrationRevision) => TRANSPORTS.map((transport) => {
      const pairs = pairedPrimaryObservations(cells, "aa", transport, calibrationRevision);
      const r = pairs.map(({ left, right }) => Math.log(right / left));
      const d = pairs.map(({ left, right }) => right - left);
      const relativeMde = Math.log(1 + manifest.decision_rule.minimum_relative_effect);
      const absoluteMde = manifest.decision_rule.minimum_absolute_effect_ns;
      const requiredRelative = requiredPairs(
        sampleStandardDeviation(r),
        relativeMde,
        powerContract,
      );
      const requiredAbsolute = requiredPairs(
        sampleStandardDeviation(d),
        absoluteMde,
        powerContract,
      );
      const required = nextEven(Math.max(8, requiredRelative, requiredAbsolute));
      const identityBounds = pairedBootstrapBounds(
        pairs.map((pair) => pair.left),
        pairs.map((pair) => pair.right),
        {
          interval: "two-sided",
          seedLabel: `aa:${calibrationRevision}:${transport}:identity`,
          statistics: manifest.statistics,
          familySize: manifest.statistics.aa_family_size,
        },
      );
      const orderBounds = pairedBootstrapBounds(
        pairs.map((pair) => pair.first),
        pairs.map((pair) => pair.second),
        {
          interval: "two-sided",
          seedLabel: `aa:${calibrationRevision}:${transport}:order`,
          statistics: manifest.statistics,
          familySize: manifest.statistics.aa_family_size,
        },
      );
      const stable =
        includesZero(identityBounds.log_ratio) &&
        withinEquivalence(identityBounds.log_ratio, relativeMde) &&
        includesZero(identityBounds.absolute_ns) &&
        withinEquivalence(identityBounds.absolute_ns, absoluteMde) &&
        includesZero(orderBounds.log_ratio) &&
        withinEquivalence(orderBounds.log_ratio, relativeMde) &&
        includesZero(orderBounds.absolute_ns) &&
        withinEquivalence(orderBounds.absolute_ns, absoluteMde) &&
        required <= maximumConfirmationPairs;
      return [`${calibrationRevision}:${transport}`, {
        calibration_revision: calibrationRevision,
        transport,
        pairs: pairs.map((pair, index) => ({
          pair_index: pair.pair_index,
          order: pair.order,
          left_ns: pair.left,
          right_ns: pair.right,
          first_ns: pair.first,
          second_ns: pair.second,
          r: r[index],
          d_ns: d[index],
        })),
        identity_bounds: identityBounds,
        order_effect_bounds: orderBounds,
        required_relative_pairs: requiredRelative,
        required_absolute_pairs: requiredAbsolute,
        required_pairs: required,
        stable,
      }];
    })),
  );
  const requiredPairsAcrossTransports = Math.max(
    ...Object.values(byArtifact).map((entry) => entry.required_pairs),
  );
  return {
    method: manifest.statistics.method,
    simultaneous_confidence_level: manifest.statistics.confidence_level,
    multiplicity_adjustment: manifest.statistics.multiplicity_adjustment,
    power_contract: powerContract,
    by_artifact: byArtifact,
    required_confirmation_pairs: requiredPairsAcrossTransports,
    within_budget: requiredPairsAcrossTransports <= maximumConfirmationPairs,
    stable: Object.values(byArtifact).every((entry) => entry.stable),
  };
}

export function classifyRssGate(cells, manifestValue) {
  const manifest = validateRequestOverlayManifest(manifestValue);
  const contract = manifest.rss_contract;
  const byTransport = Object.fromEntries(TRANSPORTS.map((transport) => {
    const pairs = pairedRssCurves(cells, transport, manifest);
    const dimensions = Object.fromEntries(contract.dimensions.map((dimension) => {
      const samples = pairs.map((pair) => {
        const baseCurve = rssCurve(pair.base, dimension);
        const headCurve = rssCurve(pair.head, dimension);
        return {
          pair_index: pair.pair_index,
          order: pair.order,
          base_growth_bytes: baseCurve,
          head_growth_bytes: headCurve,
          head_slope: log1pScaleSlope(SCALE_POINTS, headCurve),
          head_absolute_growth_bytes: Math.max(...headCurve),
          head_regression_bytes: Math.max(
            ...headCurve.map((value, index) => value - baseCurve[index]),
          ),
        };
      });
      const slopeUpper = observedProcessUpper(
        samples.map((sample) => sample.head_slope),
        contract,
      );
      const absoluteUpper = observedProcessUpper(
        samples.map((sample) => sample.head_absolute_growth_bytes),
        contract,
      );
      const regressionUpper = observedProcessUpper(
        samples.map((sample) => sample.head_regression_bytes),
        contract,
      );
      const qualified =
        slopeUpper.upper <= contract.maximum_slope_upper_bound &&
        absoluteUpper.upper <= contract.maximum_absolute_growth_bytes &&
        regressionUpper.upper <= contract.maximum_head_regression_bytes;
      return [dimension.id, {
        dimension: structuredClone(dimension),
        samples,
        slope_upper_bound: slopeUpper,
        absolute_growth_upper_bound_bytes: absoluteUpper,
        head_regression_upper_bound_bytes: regressionUpper,
        qualified,
      }];
    }));
    const processSamples = pairs.map((pair) => ({
      pair_index: pair.pair_index,
      order: pair.order,
      base_growth_bytes: pair.base.rss.fresh_process_envelope_growth_bytes,
      head_growth_bytes: pair.head.rss.fresh_process_envelope_growth_bytes,
      base_startup_history_gap_bytes: pair.base.rss.baseline_history_gap_bytes,
      head_startup_history_gap_bytes: pair.head.rss.baseline_history_gap_bytes,
      base_lane_history_gap_bytes: maximumMeasurementLaneHistoryGap(pair.base),
      head_lane_history_gap_bytes: maximumMeasurementLaneHistoryGap(pair.head),
      head_regression_bytes:
        pair.head.rss.fresh_process_envelope_growth_bytes -
        pair.base.rss.fresh_process_envelope_growth_bytes,
    }));
    const processEnvelope = {
      samples: processSamples,
      absolute_growth_upper_bound_bytes: observedProcessUpper(
        processSamples.map((sample) => sample.head_growth_bytes),
        contract,
      ),
      head_regression_upper_bound_bytes: observedProcessUpper(
        processSamples.map((sample) => sample.head_regression_bytes),
        contract,
      ),
      startup_history_gap_upper_bound_bytes: observedProcessUpper(
        processSamples.flatMap((sample) => [
          sample.base_startup_history_gap_bytes,
          sample.head_startup_history_gap_bytes,
        ]),
        contract,
      ),
      lane_history_gap_upper_bound_bytes: observedProcessUpper(
        processSamples.flatMap((sample) => [
          sample.base_lane_history_gap_bytes,
          sample.head_lane_history_gap_bytes,
        ]),
        contract,
      ),
    };
    processEnvelope.qualified =
      processEnvelope.absolute_growth_upper_bound_bytes.upper <=
        contract.maximum_process_envelope_growth_bytes &&
      processEnvelope.head_regression_upper_bound_bytes.upper <=
        contract.maximum_head_process_envelope_regression_bytes &&
      processEnvelope.startup_history_gap_upper_bound_bytes.upper <=
        contract.maximum_startup_history_gap_bytes &&
      processEnvelope.lane_history_gap_upper_bound_bytes.upper <=
        contract.maximum_lane_history_gap_bytes;
    return [transport, {
      dimensions,
      process_envelope: processEnvelope,
      qualified:
        Object.values(dimensions).every((dimension) => dimension.qualified) &&
        processEnvelope.qualified,
    }];
  }));
  const qualified = Object.values(byTransport).every((transport) => transport.qualified);
  return {
    method: contract.method,
    curve_transform: contract.curve_transform,
    curve_metric: contract.curve_metric,
    process_envelope_metric: contract.process_envelope_metric,
    upper_bound_method: contract.upper_bound_method,
    limits: {
      maximum_slope_upper_bound: contract.maximum_slope_upper_bound,
      maximum_absolute_growth_bytes: contract.maximum_absolute_growth_bytes,
      maximum_head_regression_bytes: contract.maximum_head_regression_bytes,
      maximum_process_envelope_growth_bytes:
        contract.maximum_process_envelope_growth_bytes,
      maximum_head_process_envelope_regression_bytes:
        contract.maximum_head_process_envelope_regression_bytes,
      maximum_startup_history_gap_bytes:
        contract.maximum_startup_history_gap_bytes,
      maximum_lane_history_gap_bytes:
        contract.maximum_lane_history_gap_bytes,
    },
    by_transport: byTransport,
    qualified,
  };
}

function maximumMeasurementLaneHistoryGap(workerResult) {
  const gaps = workerResult.rss.lanes
    .filter((lane) => lane.lane_id.startsWith("measurement:"))
    .map((lane) => lane.baseline_history_gap_bytes);
  if (gaps.length === 0) fail("worker RSS evidence lacks measurement lane history gaps");
  return Math.max(...gaps);
}

export function classifyDecision(cells, noise, manifestValue) {
  const manifest = validateRequestOverlayManifest(manifestValue);
  const semantic = classifySemanticGate(cells);
  if (!semantic.base_qualified) {
    fail("base artifacts do not satisfy the fixed request-overlay semantic contract");
  }
  const rss = classifyRssGate(cells, manifest);
  const byTransport = Object.fromEntries(
    TRANSPORTS.map((transport) => {
      const pairs = pairedPrimaryObservations(cells, "ab_ba", transport);
      const bounds = pairedBootstrapBounds(
        pairs.map((pair) => pair.base),
        pairs.map((pair) => pair.head),
        {
          interval: "one-sided",
          seedLabel: `confirmation:${transport}`,
          statistics: manifest.statistics,
          familySize: manifest.statistics.confirmation_family_size,
        },
      );
      const rBound = bounds.log_ratio;
      const dBound = bounds.absolute_ns;
      const relativeThreshold = Math.log(1 + manifest.decision_rule.minimum_relative_effect);
      const absoluteThreshold = manifest.decision_rule.minimum_absolute_effect_ns;
      const improvement = rBound.upper < -relativeThreshold && dBound.upper < -absoluteThreshold;
      const nonImprovement =
        rBound.lower >= -relativeThreshold || dBound.lower >= -absoluteThreshold;
      const regression = rBound.lower > relativeThreshold && dBound.lower > absoluteThreshold;
      return [transport, {
        pairs: pairs.map((pair, index) => ({
          pair_index: pair.pair_index,
          order: pair.order,
          base_ns: pair.base,
          head_ns: pair.head,
          r: Math.log(pair.head / pair.base),
          d_ns: pair.head - pair.base,
        })),
        bounds,
        confirmed_improvement: improvement,
        confirmed_non_improvement: nonImprovement,
        confirmed_regression: regression,
      }];
    }),
  );
  let status = "inconclusive";
  const reasons = [];
  if (!semantic.head_qualified) {
    status = "rejected";
    reasons.push("At least one head artifact failed a mandatory semantic owner gate.");
  } else if (!noise.stable || !noise.within_budget) {
    reasons.push("A/A noise did not qualify within the registered 32-pair budget.");
  } else if (Object.values(byTransport).some((entry) => entry.confirmed_regression)) {
    status = "regressed";
    reasons.push("At least one transport cleared both ordinary regression thresholds.");
  } else if (Object.values(byTransport).some((entry) => entry.confirmed_non_improvement)) {
    status = "rejected";
    reasons.push("At least one transport disconfirmed an ordinary improvement threshold.");
  } else if (Object.values(byTransport).every((entry) => entry.confirmed_improvement)) {
    if (rss.qualified) {
      status = "confirmed-improvement";
      reasons.push(
        "Both transports cleared the ordinary latency gates and every RSS owner bound.",
      );
    } else {
      reasons.push("Latency improved, but at least one RSS curve exceeded its owner bound.");
    }
  } else {
    reasons.push("At least one paired interval crossed an ordinary improvement threshold.");
  }
  return {
    status,
    transport_admission: "not-evaluated",
    primary_metric: "version-only reused executeSync batch_ns per logical operation",
    semantic,
    noise,
    confirmation: { by_transport: byTransport },
    rss,
    reasons,
  };
}

function classifySemanticGate(cells) {
  const byArtifact = Object.fromEntries(artifactKeys().map((artifactKey) => {
    const artifactCells = cells.filter((cell) => cell.artifact_key === artifactKey);
    const failedCellIds = artifactCells
      .filter((cell) => cell.result.semantic_evidence.passed !== true)
      .map((cell) => cell.id);
    const rawIdentityDigests = [
      ...new Set(artifactCells.map((cell) => semanticRawIdentityDigest(cell.result))),
    ].sort();
    return [artifactKey, {
      cell_count: artifactCells.length,
      failed_cell_ids: failedCellIds,
      raw_identity_digests: rawIdentityDigests,
      fresh_process_deterministic: rawIdentityDigests.length === 1,
      qualified:
        artifactCells.length > 0 &&
        failedCellIds.length === 0 &&
        rawIdentityDigests.length === 1,
    }];
  }));
  const crossRevision = Object.fromEntries(TRANSPORTS.map((transport) => {
    const baseDigest = byArtifact[`base:${transport}`].raw_identity_digests[0] ?? null;
    const headDigest = byArtifact[`head:${transport}`].raw_identity_digests[0] ?? null;
    return [transport, {
      base_digest: baseDigest,
      head_digest: headDigest,
      equal: baseDigest !== null && baseDigest === headDigest,
    }];
  }));
  return {
    by_artifact: byArtifact,
    cross_revision: crossRevision,
    base_qualified: TRANSPORTS.every(
      (transport) => byArtifact[`base:${transport}`].qualified,
    ),
    head_qualified: TRANSPORTS.every(
      (transport) =>
        byArtifact[`head:${transport}`].qualified && crossRevision[transport].equal,
    ),
  };
}

function semanticRawIdentityDigest(workerResult) {
  const semantic = workerResult.semantic_evidence;
  return digestJson({
    success: semantic.success_probes.map((probe) => ({
      id: probe.id,
      raw_sha256: probe.raw_sha256,
    })),
    reused_success: semantic.reused_success_probes.map((probe) => ({
      id: probe.id,
      response_sequence_sha256: probe.response_sequence_sha256,
      unique_response_sha256: probe.unique_response_sha256,
    })),
    error: semantic.error_probe.raw_sha256,
    resource_limit: semantic.resource_limit_probe.raw_sha256,
    measurements: {
      overlays: measurementWireIdentity(workerResult.measurements.overlays),
      batch_scaling: measurementWireIdentity(workerResult.measurements.batch_scaling),
      base_size_scaling: measurementWireIdentity(
        workerResult.measurements.base_size_scaling,
      ),
    },
  });
}

function measurementWireIdentity(lanes) {
  return lanes.map((lane) => ({
    overlay_id: lane.overlay_id,
    engine_lifecycle: lane.engine_lifecycle,
    logical_operations_per_sample: lane.logical_operations_per_sample,
    batch_size: lane.batch_size ?? null,
    base_size_units: lane.base_size_units ?? null,
    response_sequence_sha256: lane.semantic_contract.response_sequence_sha256,
    unique_response_sha256: lane.semantic_contract.unique_response_sha256,
  }));
}

export function validateRequestOverlayReport(report, { trustedManifest }) {
  const manifest = validateRequestOverlayManifest(trustedManifest);
  assertObject(report, "request-overlay report");
  if (
    report.schema_version !== 2 ||
    report.report_kind !== "merman-node-request-overlay-owner-v2" ||
    report.owner !== "merman-bindings-core"
  ) {
    fail("report identity is invalid");
  }
  assertObject(report.scope, "report scope");
  if (
    report.scope.lane_id !== manifest.lane_id ||
    report.scope.transport_admission !== "not-evaluated" ||
    report.scope.operation_id !== "semantic-json" ||
    report.scope.timing_clock !== "process.hrtime.bigint"
  ) {
    fail("report scope differs from the owner-lane contract");
  }
  validateProvenance(report.provenance);
  validateRevisions(report.revisions);
  assertObject(report.input, "report input");
  if (
    report.input.manifest_digest !== digestJson(manifest) ||
    stableJson(report.input.manifest) !== stableJson(manifest)
  ) {
    fail("report input is not bound to the trusted manifest");
  }
  validateSampling(report.sampling, manifest);
  validateRecordedCommand(report.provenance.command, report.sampling);
  validateArtifacts(report.artifacts, report.revisions);
  validateCells(report.cells, report, manifest);
  assertEqual(report.semantic_evidence, projectSemanticEvidence(report.cells), "report semantic evidence");
  assertEqual(report.rss, projectRss(report.cells), "report RSS projection");
  for (const cell of report.cells) {
    if (cell.result.process.parent_invocation_id !== report.provenance.invocation_id) {
      fail("report cell does not belong to the recorded single invocation");
    }
  }
  assertObject(report.decision, "report decision");
  if (
    !DECISION_STATUSES.includes(report.decision.status) ||
    report.decision.transport_admission !== "not-evaluated" ||
    !Array.isArray(report.decision.reasons) ||
    report.decision.reasons.length === 0
  ) {
    fail("report decision is outside the owner-lane decision contract");
  }
  const expectedNoise = qualifyNoise(
    report.cells,
    manifest,
    report.sampling.maximum_confirmation_pairs,
  );
  const expectedConfirmationPairs = Math.min(
    report.sampling.maximum_confirmation_pairs,
    Math.max(8, expectedNoise.required_confirmation_pairs),
  );
  if (report.sampling.confirmation_pairs !== expectedConfirmationPairs) {
    fail("report confirmation budget was not derived from its A/A evidence");
  }
  assertEqual(
    report.decision,
    classifyDecision(report.cells, expectedNoise, manifest),
    "report decision",
  );
  return report;
}

export function artifactKeys() {
  return REVISIONS.flatMap((revision) =>
    TRANSPORTS.map((transport) => `${revision}:${transport}`),
  );
}

function validateMeasurements(value, manifest, sampling) {
  assertObject(value, "worker measurements");
  if (typeof value.semantic_contract_passed !== "boolean") {
    fail("worker measurement semantic aggregate is invalid");
  }
  const semanticContracts = [];
  if (!Array.isArray(value.overlays) || value.overlays.length !== OVERLAY_IDS.length * 2) {
    fail("worker overlay measurements have incomplete lifecycle coverage");
  }
  for (const overlayId of OVERLAY_IDS) {
    for (const lifecycle of ["cold", "reused"]) {
      const lane = value.overlays.find(
        (entry) => entry.overlay_id === overlayId && entry.engine_lifecycle === lifecycle,
      );
      validateLane(
        lane,
        lifecycle === "cold" ? sampling.cold_samples : sampling.reused_samples,
        lifecycle,
        1,
        sampling.warmup_iterations,
        manifest.success_contract,
      );
      semanticContracts.push(lane.semantic_contract);
    }
  }
  if (!Array.isArray(value.batch_scaling) || value.batch_scaling.length !== SCALE_POINTS.length) {
    fail("worker batch scaling does not contain six points");
  }
  for (const batchSize of SCALE_POINTS) {
    const lane = value.batch_scaling.find((entry) => entry.batch_size === batchSize);
    if (lane?.overlay_id !== manifest.scaling.overlay_id) {
      fail(`worker batch ${batchSize} does not use the registered overlay`);
    }
    validateLane(
      lane,
      sampling.reused_samples,
      "reused",
      batchSize,
      sampling.warmup_iterations,
      manifest.success_contract,
    );
    semanticContracts.push(lane.semantic_contract);
  }
  if (!Array.isArray(value.base_size_scaling) || value.base_size_scaling.length !== SCALE_POINTS.length * 2) {
    fail("worker base-size scaling does not contain both lifecycles at six points");
  }
  for (const units of SCALE_POINTS) {
    for (const lifecycle of ["cold", "reused"]) {
      const lane = value.base_size_scaling.find(
        (entry) => entry.base_size_units === units && entry.engine_lifecycle === lifecycle,
      );
      if (
        lane?.overlay_id !== manifest.scaling.overlay_id ||
        !Number.isSafeInteger(lane.base_options_bytes) ||
        lane.base_options_bytes < 1
      ) {
        fail(`worker base-size ${units}/${lifecycle} contract is invalid`);
      }
      validateLane(
        lane,
        lifecycle === "cold" ? sampling.cold_samples : sampling.reused_samples,
        lifecycle,
        1,
        sampling.warmup_iterations,
        manifest.success_contract,
      );
      semanticContracts.push(lane.semantic_contract);
    }
  }
  const semanticPassed = semanticContracts.every((contract) => contract.passed);
  if (value.semantic_contract_passed !== semanticPassed) {
    fail("worker measurement semantic aggregate is inconsistent");
  }
  return semanticPassed;
}

function validateLane(
  lane,
  expectedCount,
  lifecycle,
  logicalOperations,
  warmupIterations,
  expectedEnvelope,
) {
  assertObject(lane, "measurement lane");
  if (
    lane.engine_lifecycle !== lifecycle ||
    lane.logical_operations_per_sample !== logicalOperations ||
    !Array.isArray(lane.samples) ||
    lane.samples.length !== expectedCount
  ) {
    fail("measurement lane identity or sample count is invalid");
  }
  for (const sample of lane.samples) {
    validateTimingSample(sample);
    if (lifecycle === "cold" && sample.construct_ns < 1) {
      fail("cold samples must retain engine construction time");
    }
    if (lifecycle === "reused" && sample.construct_ns !== 0) {
      fail("reused samples must exclude engine construction");
    }
  }
  assertEqual(
    lane.summary,
    summarizeTimingSamples(lane.samples, logicalOperations),
    "measurement lane summary",
  );
  const expectedSemanticObservations = logicalOperations * (
    expectedCount + (lifecycle === "reused" ? warmupIterations : 0)
  );
  validateMeasurementSemanticContract(
    lane.semantic_contract,
    expectedSemanticObservations,
    expectedEnvelope,
  );
}

function validateMeasurementSemanticContract(value, expectedCount, expectedEnvelope) {
  assertObject(value, "measurement semantic contract");
  if (
    value.observation_count !== expectedCount ||
    !Number.isSafeInteger(value.matching_observations) ||
    value.matching_observations < 0 ||
    value.matching_observations > expectedCount ||
    !SHA256.test(value.response_sequence_sha256 ?? "") ||
    !Array.isArray(value.unique_response_sha256) ||
    value.unique_response_sha256.length === 0 ||
    value.unique_response_sha256.some((digest) => !SHA256.test(digest)) ||
    stableJson(value.unique_response_sha256) !==
      stableJson([...new Set(value.unique_response_sha256)].sort()) ||
    typeof value.wire_deterministic !== "boolean" ||
    typeof value.passed !== "boolean"
  ) {
    fail("measurement semantic coverage is invalid");
  }
  const semanticMatches = value.matching_observations === expectedCount;
  const wireDeterministic = value.unique_response_sha256.length === 1;
  if (value.wire_deterministic !== wireDeterministic) {
    fail("measurement wire determinism classification is invalid");
  }
  const passed = semanticMatches && wireDeterministic;
  if (value.passed !== passed) {
    fail("measurement semantic classification is invalid");
  }
  if (semanticMatches) {
    if (value.first_mismatch !== null) {
      fail("measurement semantic contract records a spurious mismatch");
    }
    return;
  }
  assertObject(value.first_mismatch, "measurement first mismatch");
  if (
    !Number.isSafeInteger(value.first_mismatch.observation_index) ||
    value.first_mismatch.observation_index < 0 ||
    value.first_mismatch.observation_index >= expectedCount ||
    !SHA256.test(value.first_mismatch.raw_sha256 ?? "") ||
    stableJson(value.first_mismatch.envelope) === stableJson(expectedEnvelope)
  ) {
    fail("measurement first mismatch is invalid");
  }
}

function validateTimingSample(sample) {
  assertObject(sample, "timing sample");
  for (const key of ["construct_ns", "batch_ns", "total_ns"]) {
    if (!Number.isSafeInteger(sample[key]) || sample[key] < 0) {
      fail(`timing sample ${key} must be a non-negative safe integer`);
    }
  }
  if (sample.batch_ns < 1 || sample.total_ns < sample.construct_ns + sample.batch_ns) {
    fail("timing sample boundaries are internally inconsistent");
  }
}

function validateSemanticEvidence(
  value,
  manifest,
  artifactIdentity,
  measurementSemanticPassed,
) {
  assertObject(value, "worker semantic evidence");
  if (
    typeof value.passed !== "boolean" ||
    typeof value.probe_passed !== "boolean" ||
    typeof value.measurement_passed !== "boolean" ||
    !Array.isArray(value.success_probes) ||
    !Array.isArray(value.reused_success_probes)
  ) {
    fail("worker semantic evidence is malformed");
  }
  if (value.runtime_catalog_digest !== artifactIdentity.runtime_catalog_digest) {
    fail("worker runtime catalog does not match its receipt-bound artifact identity");
  }
  const expectedIds = [
    ...OVERLAY_IDS.map((id) => `overlay:${id}`),
    ...SCALE_POINTS.map((units) => `base-size:${units}`),
  ];
  assertEqual(value.success_probes.map((probe) => probe?.id), expectedIds, "semantic probe IDs");
  for (const probe of value.success_probes) {
    assertDigest(probe.raw_sha256, `semantic probe ${probe.id} digest`);
    const matches = stableJson(probe.envelope) === stableJson(manifest.success_contract);
    if (probe.matches_contract !== matches) {
      fail(`semantic probe ${probe.id} match classification is invalid`);
    }
  }
  assertEqual(
    value.reused_success_probes.map((probe) => probe?.id),
    OVERLAY_IDS.map((id) => `overlay:${id}`),
    "reused semantic probe IDs",
  );
  for (const probe of value.reused_success_probes) {
    validateReusedSemanticProbe(probe, manifest.success_contract);
  }
  assertDigest(value.error_probe?.raw_sha256, "semantic error probe digest");
  const errorMatches =
    stableJson(value.error_probe?.envelope) === stableJson(manifest.error_probe.expected);
  if (value.error_probe?.matches_contract !== errorMatches) {
    fail("semantic error probe match classification is invalid");
  }
  assertDigest(
    value.resource_limit_probe?.raw_sha256,
    "semantic resource-limit probe digest",
  );
  const resourceMatches =
    stableJson(value.resource_limit_probe?.envelope) ===
    stableJson(manifest.resource_limit_probe.expected);
  if (value.resource_limit_probe?.matches_contract !== resourceMatches) {
    fail("semantic resource-limit probe match classification is invalid");
  }
  const probePassed =
    value.success_probes.every((probe) => probe.matches_contract) &&
    value.reused_success_probes.every((probe) => probe.passed) &&
    errorMatches &&
    resourceMatches;
  if (
    value.probe_passed !== probePassed ||
    value.measurement_passed !== measurementSemanticPassed ||
    value.passed !== (probePassed && measurementSemanticPassed)
  ) {
    fail("worker semantic aggregate classification is invalid");
  }
}

function validateReusedSemanticProbe(probe, expectedEnvelope) {
  assertObject(probe, "reused semantic probe");
  if (
    probe.iterations !== 32 ||
    !Number.isSafeInteger(probe.matching_observations) ||
    probe.matching_observations < 0 ||
    probe.matching_observations > probe.iterations ||
    !Array.isArray(probe.response_sha256) ||
    probe.response_sha256.length !== probe.iterations ||
    !Array.isArray(probe.unique_response_sha256)
  ) {
    fail(`reused semantic probe ${probe.id} coverage is invalid`);
  }
  for (const digest of probe.response_sha256) {
    assertDigest(digest, `reused semantic probe ${probe.id} response digest`);
  }
  assertSortedUniqueStrings(
    probe.unique_response_sha256,
    `reused semantic probe ${probe.id} unique response digests`,
  );
  assertEqual(
    probe.unique_response_sha256,
    [...new Set(probe.response_sha256)].sort(),
    `reused semantic probe ${probe.id} unique response digest projection`,
  );
  if (probe.response_sequence_sha256 !== digestJson(probe.response_sha256)) {
    fail(`reused semantic probe ${probe.id} sequence digest is invalid`);
  }
  if (probe.matching_observations === probe.iterations) {
    if (probe.first_mismatch !== null) {
      fail(`reused semantic probe ${probe.id} records a spurious mismatch`);
    }
  } else {
    assertObject(probe.first_mismatch, `reused semantic probe ${probe.id} first mismatch`);
    const mismatch = probe.first_mismatch;
    if (
      !Number.isSafeInteger(mismatch.iteration) ||
      mismatch.iteration < 0 ||
      mismatch.iteration >= probe.iterations ||
      mismatch.raw_sha256 !== probe.response_sha256[mismatch.iteration] ||
      stableJson(mismatch.envelope) === stableJson(expectedEnvelope)
    ) {
      fail(`reused semantic probe ${probe.id} first mismatch is invalid`);
    }
  }
  const passed =
    probe.matching_observations === probe.iterations &&
    probe.unique_response_sha256.length === 1;
  if (probe.passed !== passed) {
    fail(`reused semantic probe ${probe.id} aggregate classification is invalid`);
  }
}

function validateCells(cells, report, manifest) {
  if (!Array.isArray(cells)) fail("report cells must be an array");
  const expected = [
    ...explorationCells({ aaPairs: report.sampling.aa_pairs }),
    ...confirmationCells({ confirmationPairs: report.sampling.confirmation_pairs }),
  ];
  if (cells.length !== expected.length) fail("report cell coverage is incomplete");
  const artifacts = new Map(report.artifacts.map((artifact) => [artifact.key, artifact]));
  const nonces = new Set();
  for (let index = 0; index < expected.length; index += 1) {
    const cell = cells[index];
    const identity = expected[index];
    for (const key of [
      "id",
      "phase",
      "transport",
      "pair_index",
      "order_position",
      "role",
      "artifact_key",
    ]) {
      if (cell?.[key] !== identity[key]) fail(`report cell ${index} ${key} is invalid`);
    }
    if (cell?.calibration_revision !== identity.calibration_revision) {
      fail(`report cell ${index} calibration_revision is invalid`);
    }
    assertEqual(cell.order, identity.order, `report cell ${index} order`);
    validateRequestOverlayWorkerResult(cell.result, {
      artifactKey: cell.artifact_key,
      artifactIdentity: projectWorkerArtifact(artifacts.get(cell.artifact_key)),
      manifest,
      sampling: report.sampling,
    });
    const nonce = cell.result.process.invocation_nonce;
    if (nonces.has(nonce)) fail("each report cell must come from a fresh worker process");
    nonces.add(nonce);
  }
}

function validateArtifacts(values, revisions) {
  if (!Array.isArray(values) || values.length !== artifactKeys().length) {
    fail("report must bind exactly four artifacts");
  }
  assertEqual(values.map((entry) => entry?.key), artifactKeys(), "report artifact keys");
  for (const value of values) validateArtifactIdentity(value);
  for (const revision of REVISIONS) {
    const side = values.filter((value) => value.revision === revision);
    if (side.length !== 2 || side.some((value) => value.commit !== revisions[revision])) {
      fail(`${revision} artifact commits do not match report revisions`);
    }
    if (side.some((value) => value.commit_tree !== revisions[`${revision}_tree`])) {
      fail(`${revision} artifact trees do not match report revisions`);
    }
    for (const key of [
      "commit_tree",
      "source_digest",
      "cargo_lock_digest",
      "binding_contract_digest",
      "build_environment_digest",
      "capability_recipe_digest",
      "runtime_catalog_digest",
    ]) {
      if (new Set(side.map((value) => value[key])).size !== 1) {
        fail(`${revision} transports disagree on ${key}`);
      }
    }
  }
}

function validateArtifactIdentity(value) {
  assertObject(value, "artifact identity");
  if (
    value.key !== `${value.revision}:${value.transport}` ||
    !REVISIONS.includes(value.revision) ||
    !TRANSPORTS.includes(value.transport) ||
    !COMMIT.test(value.commit ?? "") ||
    !COMMIT.test(value.commit_tree ?? "") ||
    !Number.isSafeInteger(value.artifact_bytes) ||
    value.artifact_bytes < 1 ||
    typeof value.artifact_path_in_receipt !== "string" ||
    value.artifact_path_in_receipt.length === 0 ||
    typeof value.rust_target !== "string" ||
    value.rust_target.length === 0 ||
    !Array.isArray(value.cargo_features) ||
    value.cargo_features.length === 0 ||
    !Array.isArray(value.capability_ids) ||
    value.capability_ids.length === 0
  ) {
    fail("artifact identity fields are invalid");
  }
  if (
    (value.transport === "napi" &&
      (typeof value.target !== "string" || value.target.length === 0 || value.wasm_pack_target !== null)) ||
    (value.transport === "node-wasm" &&
      (value.target !== null || value.wasm_pack_target !== "nodejs"))
  ) {
    fail("artifact target metadata is inconsistent with its transport");
  }
  assertSortedUniqueStrings(value.cargo_features, `artifact ${value.key ?? `${value.revision}:${value.transport}`} features`);
  assertSortedUniqueStrings(value.capability_ids, `artifact ${value.key ?? `${value.revision}:${value.transport}`} capabilities`);
  assertObject(value.build_tools, "artifact build tools");
  for (const key of ["cargo", "node", "rustc", "transport_builder"]) {
    if (typeof value.build_tools[key] !== "string" || value.build_tools[key].length === 0) {
      fail(`artifact build tool ${key} is missing`);
    }
  }
  for (const key of [
    "artifact_sha256",
    "receipt_digest",
    "source_digest",
    "cargo_lock_digest",
    "binding_contract_digest",
    "build_environment_digest",
    "dependency_closure_digest",
    "capability_recipe_digest",
    "input_digest",
    "runtime_catalog_digest",
  ]) {
    assertDigest(value[key], `artifact ${value.key} ${key}`);
  }
}

function projectWorkerArtifact(artifact) {
  if (!artifact) fail("worker references an unknown artifact");
  const { key: _key, ...identity } = artifact;
  return identity;
}

function pairedPrimaryObservations(cells, phase, transport, calibrationRevision = null) {
  const selected = cells.filter((cell) =>
    cell.phase === phase &&
    cell.transport === transport &&
    cell.calibration_revision === calibrationRevision,
  );
  const pairs = [];
  for (const pairIndex of [...new Set(selected.map((cell) => cell.pair_index))].sort((a, b) => a - b)) {
    const pairCells = selected.filter((cell) => cell.pair_index === pairIndex);
    if (pairCells.length !== 2) fail(`${phase}/${transport}/${pairIndex} is not a complete pair`);
    const byRole = new Map(pairCells.map((cell) => [cell.role, primaryEstimate(cell.result)]));
    if (phase === "aa") {
      const order = pairCells[0].order;
      const left = byRole.get("a");
      const right = byRole.get("b");
      pairs.push({
        pair_index: pairIndex,
        order,
        left,
        right,
        first: order[0] === "a" ? left : right,
        second: order[1] === "b" ? right : left,
      });
    } else {
      pairs.push({
        pair_index: pairIndex,
        order: pairCells[0].order,
        base: byRole.get("base"),
        head: byRole.get("head"),
      });
    }
  }
  if (pairs.some((pair) => Object.values(pair).some((value) => value === undefined))) {
    fail(`${phase}/${transport} pair roles are incomplete`);
  }
  return pairs;
}

function pairedRssCurves(cells, transport, manifest) {
  const selected = cells.filter(
    (cell) => cell.phase === "ab_ba" && cell.transport === transport,
  );
  const pairs = [];
  for (const pairIndex of [...new Set(selected.map((cell) => cell.pair_index))]
    .sort((left, right) => left - right)) {
    const pairCells = selected.filter((cell) => cell.pair_index === pairIndex);
    if (pairCells.length !== 2) {
      fail(`RSS confirmation/${transport}/${pairIndex} is not a complete pair`);
    }
    const byRole = new Map(pairCells.map((cell) => [cell.role, cell.result]));
    const base = byRole.get("base");
    const head = byRole.get("head");
    if (!base || !head) {
      fail(`RSS confirmation/${transport}/${pairIndex} pair roles are incomplete`);
    }
    for (const dimension of manifest.rss_contract.dimensions) {
      rssCurve(base, dimension);
      rssCurve(head, dimension);
    }
    pairs.push({ pair_index: pairIndex, order: pairCells[0].order, base, head });
  }
  if (pairs.length < 8) fail(`RSS confirmation/${transport} requires at least eight pairs`);
  return pairs;
}

function rssCurve(workerResult, dimension) {
  return SCALE_POINTS.map((scale) => {
    const laneId = rssScaleLaneId(dimension, scale);
    const lane = workerResult.rss.lanes.find((entry) => entry.lane_id === laneId);
    if (!lane) fail(`worker RSS evidence lacks ${laneId}`);
    if (!Number.isSafeInteger(lane.sampled_current_growth_bytes) ||
      lane.sampled_current_growth_bytes < 0) {
      fail(`worker RSS evidence has an invalid ${laneId} growth value`);
    }
    return lane.sampled_current_growth_bytes;
  });
}

function rssScaleLaneId(dimension, scale) {
  if (dimension.measurement === "batch") {
    return `measurement:batch:${scale}:${dimension.lifecycle}`;
  }
  if (dimension.measurement === "base-size") {
    return `measurement:base-size:${scale}:${dimension.lifecycle}`;
  }
  fail(`unknown RSS scale dimension ${dimension.id}`);
}

function log1pScaleSlope(scales, growthBytes) {
  const x = scales.map((scale) => Math.log(scale));
  const y = growthBytes.map((value) => Math.log1p(value));
  const xMean = mean(x);
  const yMean = mean(y);
  const denominator = x.reduce((sum, value) => sum + (value - xMean) ** 2, 0);
  if (!(denominator > 0)) fail("RSS scale slope requires distinct positive scales");
  return x.reduce(
    (sum, value, index) => sum + (value - xMean) * (y[index] - yMean),
    0,
  ) / denominator;
}

function observedProcessUpper(values, contract) {
  if (
    !Array.isArray(values) ||
    values.length < 8 ||
    values.some((value) => !Number.isFinite(value))
  ) {
    fail("RSS owner bound requires at least eight finite process observations");
  }
  return {
    estimate: mean(values),
    lower: Math.min(...values),
    upper: Math.max(...values),
    observation_count: values.length,
    upper_bound_method: contract.upper_bound_method,
  };
}

function primaryEstimate(workerResult) {
  const lane = workerResult.measurements.overlays.find(
    (entry) => entry.overlay_id === "version-only" && entry.engine_lifecycle === "reused",
  );
  if (!lane) fail("worker result lacks the primary version-only reused lane");
  return lane.summary.per_operation_batch_ns.p50_ns;
}

function validateRevisions(value) {
  assertObject(value, "report revisions");
  if (
    !COMMIT.test(value.base ?? "") ||
    !COMMIT.test(value.base_tree ?? "") ||
    !COMMIT.test(value.head ?? "") ||
    !COMMIT.test(value.head_tree ?? "") ||
    value.base === value.head ||
    value.relationship !== "head^1==base" ||
    value.verified !== true
  ) {
    fail("report revisions do not prove the adjacent first-parent relationship");
  }
}

function validateRss(value, manifest, sampling) {
  assertObject(value, "worker RSS");
  assertExactKeys(value, [
    "method",
    "baseline_current_rss_bytes",
    "baseline_process_max_rss_bytes",
    "baseline_history_gap_bytes",
    "final_current_rss_bytes",
    "final_process_max_rss_bytes",
    "peak_sampled_current_rss_bytes",
    "peak_process_max_rss_bytes",
    "sampled_current_peak_growth_bytes",
    "process_max_peak_growth_bytes",
    "fresh_process_envelope_growth_bytes",
    "lanes",
  ], "worker RSS");
  if (value.method !== RSS_METHOD) {
    fail("worker RSS evidence is invalid");
  }
  for (const key of [
    "baseline_current_rss_bytes",
    "baseline_process_max_rss_bytes",
    "final_current_rss_bytes",
    "final_process_max_rss_bytes",
    "peak_sampled_current_rss_bytes",
    "peak_process_max_rss_bytes",
  ]) {
    validatePositiveSafeInteger(value[key], `worker RSS ${key}`);
  }
  for (const key of [
    "sampled_current_peak_growth_bytes",
    "process_max_peak_growth_bytes",
    "baseline_history_gap_bytes",
    "fresh_process_envelope_growth_bytes",
  ]) {
    validateNonNegativeSafeInteger(value[key], `worker RSS ${key}`);
  }
  if (
    value.baseline_process_max_rss_bytes < value.baseline_current_rss_bytes ||
    value.baseline_history_gap_bytes !==
      value.baseline_process_max_rss_bytes - value.baseline_current_rss_bytes ||
    value.final_process_max_rss_bytes < value.final_current_rss_bytes ||
    value.final_process_max_rss_bytes < value.baseline_process_max_rss_bytes ||
    value.peak_process_max_rss_bytes < value.peak_sampled_current_rss_bytes
  ) {
    fail("worker RSS evidence is invalid");
  }

  const expectedPlan = requestOverlayRssLanePlan(manifest, sampling);
  if (!Array.isArray(value.lanes) || value.lanes.length !== expectedPlan.length) {
    fail("worker RSS lane coverage is incomplete");
  }
  let precedingProcessMaximum = value.baseline_process_max_rss_bytes;
  for (let index = 0; index < expectedPlan.length; index += 1) {
    const lane = value.lanes[index];
    const expected = expectedPlan[index];
    assertObject(lane, `worker RSS lane ${index}`);
    assertExactKeys(lane, [
      "lane_id",
      "observation_count",
      "baseline_current_rss_bytes",
      "baseline_process_max_rss_bytes",
      "baseline_history_gap_bytes",
      "peak_sampled_current_rss_bytes",
      "peak_process_max_rss_bytes",
      "sampled_current_growth_bytes",
      "process_max_growth_bytes",
      "operation_peak_growth_bytes",
    ], `worker RSS lane ${index}`);
    if (
      lane.lane_id !== expected.lane_id ||
      lane.observation_count !== expected.observation_count
    ) {
      fail(`worker RSS lane ${index} identity or observation count is invalid`);
    }
    validatePositiveSafeInteger(
      lane.baseline_current_rss_bytes,
      `worker RSS lane ${lane.lane_id} sampled current baseline`,
    );
    validatePositiveSafeInteger(
      lane.baseline_process_max_rss_bytes,
      `worker RSS lane ${lane.lane_id} process maximum baseline`,
    );
    validatePositiveSafeInteger(
      lane.peak_sampled_current_rss_bytes,
      `worker RSS lane ${lane.lane_id} sampled current peak`,
    );
    validatePositiveSafeInteger(
      lane.peak_process_max_rss_bytes,
      `worker RSS lane ${lane.lane_id} process maximum`,
    );
    for (const key of [
      "baseline_history_gap_bytes",
      "sampled_current_growth_bytes",
      "process_max_growth_bytes",
      "operation_peak_growth_bytes",
    ]) {
      validateNonNegativeSafeInteger(
        lane[key],
        `worker RSS lane ${lane.lane_id} ${key}`,
      );
    }
    const sampledCurrentGrowth = Math.max(
      0,
      lane.peak_sampled_current_rss_bytes - lane.baseline_current_rss_bytes,
    );
    const processMaxGrowth = Math.max(
      0,
      lane.peak_process_max_rss_bytes - lane.baseline_process_max_rss_bytes,
    );
    if (
      lane.baseline_process_max_rss_bytes < lane.baseline_current_rss_bytes ||
      lane.baseline_history_gap_bytes !==
        lane.baseline_process_max_rss_bytes - lane.baseline_current_rss_bytes ||
      lane.baseline_process_max_rss_bytes < precedingProcessMaximum ||
      lane.peak_sampled_current_rss_bytes < lane.baseline_current_rss_bytes ||
      lane.peak_process_max_rss_bytes < lane.baseline_process_max_rss_bytes ||
      lane.peak_process_max_rss_bytes < lane.peak_sampled_current_rss_bytes ||
      lane.sampled_current_growth_bytes !== sampledCurrentGrowth ||
      lane.process_max_growth_bytes !== processMaxGrowth ||
      lane.operation_peak_growth_bytes !==
        Math.max(sampledCurrentGrowth, processMaxGrowth)
    ) {
      fail(`worker RSS lane ${lane.lane_id} derived evidence is invalid`);
    }
    precedingProcessMaximum = lane.peak_process_max_rss_bytes;
  }
  if (value.final_process_max_rss_bytes < precedingProcessMaximum) {
    fail("worker RSS final process maximum predates a larger lane maximum");
  }

  const peakSampledCurrent = Math.max(
    value.baseline_current_rss_bytes,
    value.final_current_rss_bytes,
    ...value.lanes.map((lane) => lane.peak_sampled_current_rss_bytes),
  );
  const peakProcessMaximum = Math.max(
    value.baseline_process_max_rss_bytes,
    value.final_process_max_rss_bytes,
    ...value.lanes.map((lane) => lane.peak_process_max_rss_bytes),
  );
  const sampledCurrentGrowth = Math.max(
    0,
    peakSampledCurrent - value.baseline_current_rss_bytes,
  );
  const processMaxGrowth = Math.max(
    0,
    peakProcessMaximum - value.baseline_process_max_rss_bytes,
  );
  if (
    value.peak_sampled_current_rss_bytes !== peakSampledCurrent ||
    value.peak_process_max_rss_bytes !== peakProcessMaximum ||
    value.sampled_current_peak_growth_bytes !== sampledCurrentGrowth ||
    value.process_max_peak_growth_bytes !== processMaxGrowth ||
    value.fresh_process_envelope_growth_bytes !==
      Math.max(sampledCurrentGrowth, processMaxGrowth)
  ) {
    fail("worker RSS aggregate evidence is invalid");
  }
}

function buildRssLanePlan(manifest, sampling) {
  return [
    { lane_id: "lifecycle:artifact-load", observation_count: 1 },
    { lane_id: "semantic:runtime-catalog", observation_count: 1 },
    ...manifest.overlays.map((overlay) => ({
      lane_id: `semantic:overlay:${overlay.id}`,
      observation_count: 2,
    })),
    ...manifest.scaling.base_size_units.map((units) => ({
      lane_id: `semantic:base-size:${units}`,
      observation_count: 1,
    })),
    {
      lane_id: `semantic:error:${manifest.error_probe.id}`,
      observation_count: 1,
    },
    {
      lane_id: `semantic:resource-limit:${manifest.resource_limit_probe.id}`,
      observation_count: 1,
    },
    ...manifest.overlays.flatMap((overlay) => [
      {
        lane_id: `measurement:overlay:${overlay.id}:cold`,
        observation_count: sampling.cold_samples,
      },
      {
        lane_id: `measurement:overlay:${overlay.id}:reused`,
        observation_count: sampling.reused_samples,
      },
    ]),
    ...manifest.scaling.batch_sizes.map((batchSize) => ({
      lane_id: `measurement:batch:${batchSize}:reused`,
      observation_count: sampling.reused_samples,
    })),
    ...manifest.scaling.base_size_units.flatMap((units) => [
      {
        lane_id: `measurement:base-size:${units}:cold`,
        observation_count: sampling.cold_samples,
      },
      {
        lane_id: `measurement:base-size:${units}:reused`,
        observation_count: sampling.reused_samples,
      },
    ]),
  ];
}

function validateProvenance(value) {
  assertObject(value, "report provenance");
  if (
    typeof value.measured_at_utc !== "string" ||
    Number.isNaN(Date.parse(value.measured_at_utc)) ||
    typeof value.invocation_id !== "string" ||
    !/^[0-9a-f]{32}$/.test(value.invocation_id) ||
    typeof value.node !== "string" ||
    typeof value.platform !== "string" ||
    typeof value.arch !== "string" ||
    typeof value.timezone !== "string" ||
    typeof value.hostname !== "string" ||
    typeof value.release !== "string" ||
    typeof value.cpu !== "string" ||
    !Number.isSafeInteger(value.logical_cpus) ||
    value.logical_cpus < 1 ||
    !Number.isSafeInteger(value.total_memory_bytes) ||
    value.total_memory_bytes < 1 ||
    !Array.isArray(value.command) ||
    value.command.length < 2 ||
    value.command.some((item) => typeof item !== "string" || item.length === 0)
  ) {
    fail("report provenance is incomplete");
  }
  assertDigest(value.harness_digest, "report harness digest");
}

function validateRecordedCommand(command, sampling) {
  if (!command[1].endsWith("request-overlay-run.mjs")) {
    fail("report command does not invoke the request-overlay runner");
  }
  const flags = new Map();
  for (let index = 2; index < command.length; index += 2) {
    const flag = command[index];
    const value = command[index + 1];
    if (!flag?.startsWith("--") || value === undefined || flags.has(flag)) {
      fail("report command has malformed or duplicate arguments");
    }
    flags.set(flag, value);
  }
  const expectedFlags = [
    "--base-napi",
    "--base-wasm",
    "--head-napi",
    "--head-wasm",
    "--output",
    "--aa-pairs",
    "--cold-samples",
    "--warmup-iterations",
    "--reused-samples",
  ];
  if (stableJson([...flags.keys()]) !== stableJson(expectedFlags)) {
    fail("report command does not retain the complete canonical invocation");
  }
  for (const [flag, expected] of [
    ["--aa-pairs", sampling.aa_pairs],
    ["--cold-samples", sampling.cold_samples],
    ["--warmup-iterations", sampling.warmup_iterations],
    ["--reused-samples", sampling.reused_samples],
  ]) {
    if (flags.get(flag) !== String(expected)) {
      fail(`report command ${flag} disagrees with sampling`);
    }
  }
  if (!flags.get("--output").endsWith(".json")) {
    fail("report command output is not a JSON report path");
  }
}

function validateSamplingDefaults(value) {
  assertObject(value, "manifest sampling_defaults");
  for (const key of [
    "aa_pairs",
    "maximum_confirmation_pairs",
    "cold_samples",
    "warmup_iterations",
    "reused_samples",
  ]) {
    if (!Number.isSafeInteger(value[key]) || value[key] < 1) {
      fail(`manifest sampling_defaults.${key} must be positive`);
    }
  }
  if (
    value.aa_pairs < 8 ||
    value.aa_pairs > 32 ||
    value.aa_pairs % 2 !== 0 ||
    value.maximum_confirmation_pairs !== 32
  ) {
    fail("manifest A/A and confirmation budgets must be balanced within 8..32");
  }
}

function validateSamplingLimits(value) {
  assertObject(value, "manifest sampling_limits");
  if (
    value.maximum_cold_samples !== 20 ||
    value.maximum_warmup_iterations !== 100 ||
    value.maximum_reused_samples !== 100 ||
    value.maximum_logical_operations_per_worker !== 50_000 ||
    value.worker_timeout_ms !== 120_000
  ) {
    fail("manifest sampling limits differ from the registered worker budget");
  }
}

function estimatedWorkerLogicalOperations(sampling) {
  const semanticOperations = OVERLAY_IDS.length * (1 + 32) + SCALE_POINTS.length + 2;
  const coldOperations =
    (OVERLAY_IDS.length + SCALE_POINTS.length) * sampling.cold_samples;
  const reusedScale = sampling.warmup_iterations + sampling.reused_samples;
  const reusedOperations =
    (OVERLAY_IDS.length + SCALE_POINTS.length +
      SCALE_POINTS.reduce((sum, scale) => sum + scale, 0)) * reusedScale;
  return semanticOperations + coldOperations + reusedOperations;
}

function validateEvenPairCount(value, label) {
  if (!Number.isSafeInteger(value) || value < 8 || value > 32 || value % 2 !== 0) {
    fail(`${label} must be an even integer in 8..32`);
  }
}

function summarizeNumbers(values) {
  if (values.some((value) => !Number.isFinite(value) || value < 0)) {
    fail("sample distribution contains an invalid number");
  }
  const sorted = [...values].sort((left, right) => left - right);
  return {
    min_ns: sorted[0],
    p50_ns: percentile(sorted, 0.5),
    p95_ns: percentile(sorted, 0.95),
    max_ns: sorted.at(-1),
    mean_ns: mean(sorted),
  };
}

function requiredPairs(sigma, mde, powerContract) {
  if (sigma === 0) return 8;
  const required = Math.ceil(
    (((powerContract.critical_z + powerContract.power_z) * sigma) / mde) ** 2,
  );
  return Number.isSafeInteger(required) && required < Number.MAX_SAFE_INTEGER
    ? required
    : Number.MAX_SAFE_INTEGER - 1;
}

function confirmationPowerContract(manifest) {
  const simultaneousConfidence = manifest.statistics.confidence_level;
  const familySize = manifest.statistics.confirmation_family_size;
  const componentConfidence = 1 - (1 - simultaneousConfidence) / familySize;
  return {
    method: "normal-approximation-v1",
    interval: "one-sided",
    target_power: manifest.decision_rule.power,
    simultaneous_confidence_level: simultaneousConfidence,
    component_confidence_level: componentConfidence,
    family_size: familySize,
    multiplicity_adjustment: manifest.statistics.multiplicity_adjustment,
    critical_z: CONFIRMATION_CRITICAL_Z,
    power_z: CONFIRMATION_POWER_Z,
  };
}

function nextEven(value) {
  return value % 2 === 0 ? value : value + 1;
}

export function pairedBootstrapBounds(
  baseValues,
  headValues,
  { interval, seedLabel, statistics, familySize },
) {
  if (
    !Array.isArray(baseValues) ||
    baseValues.length === 0 ||
    baseValues.length !== headValues?.length ||
    [...baseValues, ...headValues].some((value) => !Number.isFinite(value) || value <= 0)
  ) {
    fail("paired bootstrap requires equal non-empty positive finite samples");
  }
  if (!new Set(["one-sided", "two-sided"]).has(interval)) {
    fail("paired bootstrap interval must be one-sided or two-sided");
  }
  if (!Number.isSafeInteger(familySize) || familySize < 1) {
    fail("paired bootstrap family size must be a positive integer");
  }
  const logRatios = baseValues.map((base, index) => Math.log(headValues[index] / base));
  const absolute = baseValues.map((base, index) => headValues[index] - base);
  const confidenceLevel = statistics.confidence_level;
  const componentConfidence = 1 - (1 - confidenceLevel) / familySize;
  return {
    log_ratio: bootstrapMeanBounds(logRatios, {
      confidenceLevel: componentConfidence,
      interval,
      resamples: statistics.bootstrap_resamples,
      monteCarloFailureProbability:
        statistics.monte_carlo_failure_probability / (familySize * 4),
      seed: deriveSeed(statistics.bootstrap_seed, `${seedLabel}:log-ratio`),
    }),
    absolute_ns: bootstrapMeanBounds(absolute, {
      confidenceLevel: componentConfidence,
      interval,
      resamples: statistics.bootstrap_resamples,
      monteCarloFailureProbability:
        statistics.monte_carlo_failure_probability / (familySize * 4),
      seed: deriveSeed(statistics.bootstrap_seed, `${seedLabel}:absolute`),
    }),
    confidence_contract: {
      simultaneous_confidence_level: confidenceLevel,
      component_confidence_level: componentConfidence,
      family_size: familySize,
      multiplicity_adjustment: statistics.multiplicity_adjustment,
      interval,
      bootstrap_seed: statistics.bootstrap_seed,
      bootstrap_resamples: statistics.bootstrap_resamples,
      monte_carlo_failure_probability:
        statistics.monte_carlo_failure_probability,
      monte_carlo_failure_probability_per_bound:
        statistics.monte_carlo_failure_probability / (familySize * 4),
    },
  };
}

function bootstrapMeanBounds(
  values,
  {
    confidenceLevel,
    interval,
    resamples,
    seed,
    monteCarloFailureProbability,
  },
) {
  const estimate = mean(values);
  if (values.length === 1 || values.every((value) => value === values[0])) {
    return { estimate, lower: estimate, upper: estimate };
  }
  const random = seededRandom(seed);
  const means = [];
  for (let sample = 0; sample < resamples; sample += 1) {
    let total = 0;
    for (let index = 0; index < values.length; index += 1) {
      total += values[Math.floor(random() * values.length)];
    }
    means.push(total / values.length);
  }
  means.sort((left, right) => left - right);
  const alpha = 1 - confidenceLevel;
  const lowerProbability = interval === "one-sided" ? alpha : alpha / 2;
  const upperProbability = interval === "one-sided" ? confidenceLevel : 1 - alpha / 2;
  const lower = conservativeBootstrapLower(
    means,
    lowerProbability,
    monteCarloFailureProbability,
  );
  const upper = conservativeBootstrapUpper(
    means,
    upperProbability,
    monteCarloFailureProbability,
  );
  return {
    estimate,
    lower: lower.value,
    upper: upper.value,
    monte_carlo: {
      method: "exact-binomial-order-statistic-v1",
      failure_probability: monteCarloFailureProbability,
      lower_rank: lower.rank,
      upper_rank: upper.rank,
      resamples,
    },
  };
}

function conservativeBootstrapLower(sorted, probability, failureProbability) {
  const count = maximumBinomialLowerTailCount(
    sorted.length,
    probability,
    failureProbability,
  );
  if (count < 0) return { value: -Number.MAX_VALUE, rank: null };
  const rank = count + 1;
  return { value: sorted[rank - 1], rank };
}

function conservativeBootstrapUpper(sorted, probability, failureProbability) {
  const count = maximumBinomialLowerTailCount(
    sorted.length,
    1 - probability,
    failureProbability,
  );
  if (count < 0) return { value: Number.MAX_VALUE, rank: null };
  const rank = sorted.length - count;
  return { value: sorted[rank - 1], rank };
}

function maximumBinomialLowerTailCount(trials, probability, maximumCumulative) {
  if (
    !Number.isSafeInteger(trials) ||
    trials < 1 ||
    !(probability > 0 && probability < 1) ||
    !(maximumCumulative > 0 && maximumCumulative < 1)
  ) {
    fail("Monte Carlo order-statistic confidence contract is invalid");
  }
  let probabilityAtCount = Math.exp(trials * Math.log1p(-probability));
  let cumulative = probabilityAtCount;
  if (cumulative > maximumCumulative) return -1;
  let maximumCount = 0;
  for (let count = 0; count < trials; count += 1) {
    probabilityAtCount *=
      ((trials - count) / (count + 1)) *
      (probability / (1 - probability));
    if (cumulative + probabilityAtCount > maximumCumulative) break;
    cumulative += probabilityAtCount;
    maximumCount = count + 1;
  }
  return maximumCount;
}

function deriveSeed(baseSeed, label) {
  let hash = BigInt(baseSeed) ^ 0xcbf29ce484222325n;
  for (const character of label) {
    hash ^= BigInt(character.codePointAt(0));
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return Number((hash ^ (hash >> 32n)) & 0xffff_ffffn);
}

function seededRandom(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4_294_967_296;
  };
}

function withinEquivalence(bounds, margin) {
  return bounds.lower >= -margin && bounds.upper <= margin;
}

function includesZero(bounds) {
  return bounds.lower <= 0 && bounds.upper >= 0;
}

function sampleStandardDeviation(values) {
  return Math.sqrt(sampleVariance(values));
}

function sampleVariance(values) {
  if (values.length < 2) return 0;
  const center = mean(values);
  return values.reduce((sum, value) => sum + (value - center) ** 2, 0) / (values.length - 1);
}

function mean(values) {
  if (!Array.isArray(values) || values.length === 0) fail("cannot summarize an empty distribution");
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function percentile(sorted, probability) {
  const index = (sorted.length - 1) * probability;
  const lower = Math.floor(index);
  const upper = Math.ceil(index);
  if (lower === upper) return sorted[lower];
  return sorted[lower] + (sorted[upper] - sorted[lower]) * (index - lower);
}

function assertDigest(value, label) {
  if (!SHA256.test(value ?? "")) fail(`${label} must be a SHA-256 digest`);
}

function assertSortedUniqueStrings(value, label) {
  if (
    !Array.isArray(value) ||
    value.length === 0 ||
    value.some((item) => typeof item !== "string" || item.length === 0) ||
    stableJson(value) !== stableJson([...new Set(value)].sort())
  ) {
    fail(`${label} must be sorted unique strings`);
  }
}

function assertExactKeys(value, expectedKeys, label) {
  const actual = Object.keys(value).sort();
  const expected = [...expectedKeys].sort();
  if (stableJson(actual) !== stableJson(expected)) {
    fail(`${label} fields differ from the fixed contract`);
  }
}

function validatePositiveSafeInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 1) {
    fail(`${label} must be a positive safe integer`);
  }
}

function validateNonNegativeSafeInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) {
    fail(`${label} must be a non-negative safe integer`);
  }
}

function assertObject(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
}

function assertEqual(actual, expected, label) {
  if (stableJson(actual) !== stableJson(expected)) fail(`${label} differs from its fixed contract`);
}

function encodeRequest(operation, requestOptions) {
  const request = {
    operation_id: operation.operation_id,
    source: operation.source,
    uri: operation.uri,
  };
  if (requestOptions !== null) request.options_json = JSON.stringify(requestOptions);
  return JSON.stringify(request);
}

function fail(message) {
  throw new Error(message);
}
