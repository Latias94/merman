import {
  ALL_LATENCY_METRICS,
  EDITOR_ARTIFACT_MODES,
  EDITOR_ARTIFACT_VARIANTS,
  MAX_PRIMARY_LATENCY_REGRESSION_MS,
  MAX_PRIMARY_LATENCY_REGRESSION_RATIO,
  PRIMARY_LATENCY_METRICS,
  assertExactRecord,
  deepFreeze,
  expectNonNegativeFinite,
  expectPositiveInteger,
  expectString,
  isRecord,
} from "./contract-shared.mjs";
import { validateEquivalenceComparison } from "./contract-equivalence.mjs";

export function summarizeEditorArtifactRuns(runs) {
  if (!Array.isArray(runs) || runs.length === 0) {
    throw new TypeError(
      "Editor artifact evidence requires at least one raw run.",
    );
  }

  const summaries = Object.fromEntries(
    EDITOR_ARTIFACT_VARIANTS.map((variant) => {
      const variantRuns = runs.filter((run) => run.variant === variant);
      if (variantRuns.length === 0) {
        throw new TypeError(
          `Editor artifact evidence is missing ${variant} runs.`,
        );
      }
      for (const run of variantRuns) validateRun(run);

      const modes = Object.fromEntries(
        EDITOR_ARTIFACT_MODES.map((mode) => [
          mode,
          summarizeMode(variantRuns.map((run) => run[mode])),
        ]),
      );
      const memoryScopes = new Set(
        variantRuns.flatMap((run) =>
          EDITOR_ARTIFACT_MODES.map((mode) => run[mode].peakMemory.scope),
        ),
      );
      if (memoryScopes.size !== 1) {
        throw new TypeError(
          `${variant} runs must use one peak-memory measurement scope.`,
        );
      }

      return [
        variant,
        Object.freeze({
          modes: Object.freeze(modes),
          peakMemoryBytes: Math.max(
            ...variantRuns.flatMap((run) =>
              EDITOR_ARTIFACT_MODES.map((mode) => run[mode].peakMemory.bytes),
            ),
          ),
          peakMemoryScope: [...memoryScopes][0],
          runCount: variantRuns.length,
        }),
      ];
    }),
  );

  if (summaries.full.peakMemoryScope !== summaries.editor.peakMemoryScope) {
    throw new TypeError(
      "Full and editor evidence must use the same peak-memory measurement scope.",
    );
  }
  return Object.freeze(summaries);
}

export function decideEditorArtifact(summaries, equivalence) {
  validateSummary(summaries?.full, "full");
  validateSummary(summaries?.editor, "editor");
  equivalence = validateEquivalenceComparison(equivalence);
  if (summaries.full.peakMemoryScope !== summaries.editor.peakMemoryScope) {
    throw new TypeError(
      "Full and editor summaries must use the same peak-memory measurement scope.",
    );
  }

  const full = summaries.full;
  const editor = summaries.editor;
  const semanticEquivalence = Object.freeze({
    passes: equivalence.exact,
    familyCount: equivalence.familyCount,
    queryCount: equivalence.queryCount,
    cellCount: equivalence.cellCount,
    mismatchCount: equivalence.mismatches.length,
    fullAggregateSha256: equivalence.variants.full.aggregateSha256,
    editorAggregateSha256: equivalence.variants.editor.aggregateSha256,
  });
  const coldBytes = Object.freeze({
    editor: editor.modes.cold.totalTransferBytes,
    full: full.modes.cold.totalTransferBytes,
    passes:
      editor.modes.cold.totalTransferBytes < full.modes.cold.totalTransferBytes,
  });
  const peakMemory = Object.freeze({
    editor: editor.peakMemoryBytes,
    full: full.peakMemoryBytes,
    passes: editor.peakMemoryBytes <= full.peakMemoryBytes,
    scope: full.peakMemoryScope,
  });
  const primaryLatencies = Object.freeze(
    EDITOR_ARTIFACT_MODES.flatMap((mode) =>
      PRIMARY_LATENCY_METRICS.map((metric) => {
        const fullMs = full.modes[mode][metric];
        const editorMs = editor.modes[mode][metric];
        const regressionMs = editorMs - fullMs;
        const regressionRatio =
          fullMs === 0 ? (regressionMs > 0 ? null : 0) : regressionMs / fullMs;
        const ratioExceedsLimit =
          regressionRatio === null
            ? regressionMs > 0
            : regressionRatio > MAX_PRIMARY_LATENCY_REGRESSION_RATIO;
        const passes = !(
          regressionMs > MAX_PRIMARY_LATENCY_REGRESSION_MS && ratioExceedsLimit
        );
        return Object.freeze({
          editorMs,
          fullMs,
          metric,
          mode,
          passes,
          regressionMs,
          regressionRatio,
        });
      }),
    ),
  );
  const latencyPasses = primaryLatencies.every((metric) => metric.passes);
  const editorEligible =
    semanticEquivalence.passes &&
    coldBytes.passes &&
    peakMemory.passes &&
    latencyPasses;
  const reasons = [];
  if (!semanticEquivalence.passes) {
    reasons.push(
      `Editor semantic equivalence failed with ${semanticEquivalence.mismatchCount} matrix mismatch(es): ${equivalence.mismatches.slice(0, 3).join(", ")}.`,
    );
  }
  if (!coldBytes.passes) {
    reasons.push(
      `Editor cold transfer ${coldBytes.editor} bytes is not lower than full ${coldBytes.full} bytes.`,
    );
  }
  if (!peakMemory.passes) {
    reasons.push(
      `Editor peak memory ${peakMemory.editor} bytes exceeds full ${peakMemory.full} bytes.`,
    );
  }
  for (const latency of primaryLatencies.filter((metric) => !metric.passes)) {
    reasons.push(
      `Editor ${latency.mode} ${latency.metric} regresses by ${latency.regressionMs.toFixed(3)} ms (${formatRatio(latency.regressionRatio)}).`,
    );
  }
  if (editorEligible) {
    reasons.push(
      `Editor is exactly equivalent across ${semanticEquivalence.familyCount} families and ${semanticEquivalence.queryCount} queries, lowers cold total transfer, does not increase peak memory, and keeps every primary latency within the joint 5% and 20 ms regression limit.`,
    );
  }

  return deepFreeze({
    criteria: Object.freeze({
      semanticEquivalence,
      coldBytes,
      peakMemory,
      primaryLatencies,
    }),
    editorEligible,
    reasons: Object.freeze(reasons),
    selected: editorEligible ? "editor" : "full",
  });
}

function summarizeMode(samples) {
  const summary = {
    totalTransferBytes: median(
      samples.map((sample) => sample.totalTransferBytes),
    ),
  };
  for (const metric of ALL_LATENCY_METRICS) {
    summary[metric] = median(samples.map((sample) => sample[metric]));
  }
  return Object.freeze(summary);
}

export function validateRun(run) {
  const value = assertExactRecord(
    run,
    ["block", "cold", "position", "variant", "warm"],
    "raw measurement run",
  );
  if (!EDITOR_ARTIFACT_VARIANTS.includes(value.variant)) {
    throw new TypeError(
      `Unknown editor artifact variant ${String(value.variant)}.`,
    );
  }
  expectPositiveInteger(value.block, "run block");
  if (value.position !== 1 && value.position !== 2) {
    throw new TypeError("Run position must be 1 or 2.");
  }
  for (const mode of EDITOR_ARTIFACT_MODES) validateMode(value[mode], mode);
}

function validateMode(mode, label) {
  const value = assertExactRecord(
    mode,
    [
      "firstDiagnosticsMs",
      "mainCompileInitializeMs",
      "mainFirstResultMs",
      "network",
      "peakMemory",
      "totalTransferBytes",
      "workerCompileInitializeMs",
      "workerReadyMs",
    ],
    `${label} measurement`,
  );
  expectNonNegativeFinite(
    value.totalTransferBytes,
    `${label} totalTransferBytes`,
  );
  for (const metric of ALL_LATENCY_METRICS) {
    expectNonNegativeFinite(value[metric], `${label} ${metric}`);
  }
  const peakMemory = assertExactRecord(
    value.peakMemory,
    ["bytes", "samples", "scope"],
    `${label} peakMemory`,
  );
  expectNonNegativeFinite(peakMemory.bytes, `${label} peakMemory bytes`);
  expectString(peakMemory.scope, `${label} peakMemory scope`);
  if (!Array.isArray(peakMemory.samples) || peakMemory.samples.length === 0) {
    throw new TypeError(`${label} peakMemory samples must be non-empty.`);
  }
  for (const [index, sampleValue] of peakMemory.samples.entries()) {
    const sample = assertExactRecord(
      sampleValue,
      ["atMs", "bytes"],
      `${label} peakMemory sample ${index}`,
    );
    expectNonNegativeFinite(sample.atMs, `${label} sample ${index} atMs`);
    expectNonNegativeFinite(sample.bytes, `${label} sample ${index} bytes`);
  }
  const sampledPeak = Math.max(
    ...peakMemory.samples.map((sample) => sample.bytes),
  );
  if (sampledPeak !== peakMemory.bytes) {
    throw new TypeError(
      `${label} peakMemory bytes must equal the sampled maximum.`,
    );
  }

  const network = assertExactRecord(
    value.network,
    ["bodyBytes", "requests"],
    `${label} network`,
  );
  expectNonNegativeFinite(network.bodyBytes, `${label} network bodyBytes`);
  if (!Array.isArray(network.requests)) {
    throw new TypeError(`${label} network requests must be an array.`);
  }
  let requestBytes = 0;
  for (const [index, requestValue] of network.requests.entries()) {
    const request = assertExactRecord(
      requestValue,
      [
        "bodyBytes",
        "cacheControl",
        "contentEncoding",
        "finishedWallTimeMs",
        "method",
        "pathname",
      ],
      `${label} network request ${index}`,
    );
    expectNonNegativeFinite(
      request.bodyBytes,
      `${label} network request ${index} bodyBytes`,
    );
    requestBytes += request.bodyBytes;
    for (const field of [
      "cacheControl",
      "contentEncoding",
      "method",
      "pathname",
    ]) {
      expectString(
        request[field],
        `${label} network request ${index} ${field}`,
      );
    }
    if (request.finishedWallTimeMs !== null) {
      expectNonNegativeFinite(
        request.finishedWallTimeMs,
        `${label} network request ${index} finishedWallTimeMs`,
      );
    }
  }
  if (requestBytes !== network.bodyBytes) {
    throw new TypeError(`${label} network bodyBytes does not match requests.`);
  }
  if (value.totalTransferBytes !== network.bodyBytes) {
    throw new TypeError(
      `${label} totalTransferBytes does not match network bodyBytes.`,
    );
  }
}

function validateSummary(summary, label) {
  if (!isRecord(summary)) {
    throw new TypeError(`${label} summary must be an object.`);
  }
  expectPositiveInteger(summary.runCount, `${label} runCount`);
  expectNonNegativeFinite(summary.peakMemoryBytes, `${label} peakMemoryBytes`);
  expectString(summary.peakMemoryScope, `${label} peakMemoryScope`);
  if (!isRecord(summary.modes)) {
    throw new TypeError(`${label} modes must be an object.`);
  }
  for (const mode of EDITOR_ARTIFACT_MODES) {
    const value = summary.modes[mode];
    if (!isRecord(value)) {
      throw new TypeError(`${label} ${mode} must be an object.`);
    }
    expectNonNegativeFinite(value.totalTransferBytes, `${label} ${mode} bytes`);
    for (const metric of ALL_LATENCY_METRICS) {
      expectNonNegativeFinite(value[metric], `${label} ${mode} ${metric}`);
    }
  }
}

function median(values) {
  if (values.length === 0) {
    throw new TypeError("Cannot calculate an empty median.");
  }
  for (const value of values) expectNonNegativeFinite(value, "median value");
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

function formatRatio(value) {
  return value === null ? "unbounded" : `${(value * 100).toFixed(2)}%`;
}
