import { createHash } from "node:crypto";

import {
  EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION,
  EDITOR_ARTIFACT_FAMILY_COUNT,
  EDITOR_ARTIFACT_QUERY_KINDS,
  canonicalStringify,
} from "./equivalence-shared.mjs";

export {
  EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION,
  EDITOR_ARTIFACT_FAMILY_COUNT,
  EDITOR_ARTIFACT_QUERY_KINDS,
};

export const EDITOR_ARTIFACT_RECEIPT_SCHEMA_VERSION = 1;
export const DEFAULT_EDITOR_ARTIFACT_RECEIPT_PATH =
  "target/playground/editor-artifact-measurement/receipt-v1.json";

export const EDITOR_ARTIFACT_VARIANTS = Object.freeze(["full", "editor"]);
export const EDITOR_ARTIFACT_MODES = Object.freeze(["cold", "warm"]);
export const PRIMARY_LATENCY_METRICS = Object.freeze([
  "workerReadyMs",
  "firstDiagnosticsMs",
  "mainFirstResultMs",
]);

const SECONDARY_LATENCY_METRICS = Object.freeze([
  "mainCompileInitializeMs",
  "workerCompileInitializeMs",
]);
const ALL_LATENCY_METRICS = Object.freeze([
  ...PRIMARY_LATENCY_METRICS,
  ...SECONDARY_LATENCY_METRICS,
]);
const MAX_PRIMARY_LATENCY_REGRESSION_RATIO = 0.05;
const MAX_PRIMARY_LATENCY_REGRESSION_MS = 20;
const SHA256_PATTERN = /^[a-f0-9]{64}$/u;

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

export function compareEditorArtifactEquivalence(variants) {
  return compareEditorArtifactEquivalenceOwned(
    cloneOwnedValue(variants, "editor semantic-equivalence variants"),
  );
}

function compareEditorArtifactEquivalenceOwned(variants) {
  assertExactRecord(
    variants,
    EDITOR_ARTIFACT_VARIANTS,
    "editor semantic-equivalence variants",
  );
  const full = validateEquivalenceMatrix(variants.full, "full");
  const editor = validateEquivalenceMatrix(variants.editor, "editor");
  const mismatches = [];

  for (
    let familyIndex = 0;
    familyIndex < full.families.length;
    familyIndex += 1
  ) {
    const fullFamily = full.families[familyIndex];
    const editorFamily = editor.families[familyIndex];
    for (const field of [
      "diagramType",
      "baselineId",
      "fixture",
      "sourceSha256",
    ]) {
      if (fullFamily[field] !== editorFamily[field]) {
        mismatches.push(`families[${familyIndex}].${field}`);
      }
    }
    for (
      let queryIndex = 0;
      queryIndex < fullFamily.queries.length;
      queryIndex += 1
    ) {
      const fullQuery = fullFamily.queries[queryIndex];
      const editorQuery = editorFamily.queries[queryIndex];
      if (fullQuery.kind !== editorQuery.kind) {
        mismatches.push(`families[${familyIndex}].queries[${queryIndex}].kind`);
      }
      if (fullQuery.outcome !== editorQuery.outcome) {
        mismatches.push(
          `families[${familyIndex}].queries[${queryIndex}].outcome`,
        );
      }
      if (fullQuery.sha256 !== editorQuery.sha256) {
        mismatches.push(
          `families[${familyIndex}].queries[${queryIndex}].sha256`,
        );
      }
    }
  }
  if (full.aggregateSha256 !== editor.aggregateSha256) {
    mismatches.push("aggregateSha256");
  }

  return deepFreeze({
    schemaVersion: EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION,
    familyCount: EDITOR_ARTIFACT_FAMILY_COUNT,
    queryCount: EDITOR_ARTIFACT_QUERY_KINDS.length,
    cellCount:
      EDITOR_ARTIFACT_FAMILY_COUNT * EDITOR_ARTIFACT_QUERY_KINDS.length,
    queryKinds: EDITOR_ARTIFACT_QUERY_KINDS,
    variants: Object.freeze({ full, editor }),
    exact: mismatches.length === 0,
    mismatches: Object.freeze(mismatches),
  });
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

export function createEditorArtifactReceipt(input) {
  const ownedInput = cloneOwnedValue(input, "editor artifact receipt input");
  assertExactRecord(
    ownedInput,
    [
      "builds",
      "environment",
      "equivalence",
      "generatedAt",
      "parameters",
      "revision",
      "runs",
    ],
    "editor artifact receipt input",
  );
  const generatedAt = expectIsoDateTime(ownedInput.generatedAt, "generatedAt");
  const revision = validateRevision(ownedInput.revision);
  const environment = validateEnvironment(ownedInput.environment);
  const parameters = validateParameters(ownedInput.parameters);
  const builds = validateBuilds(ownedInput.builds);
  validateAbBaRuns(ownedInput.runs);
  const observedBlocks = new Set(ownedInput.runs.map((run) => run.block)).size;
  if (parameters.blocks !== observedBlocks) {
    throw new TypeError(
      `parameters.blocks ${parameters.blocks} does not match ${observedBlocks} measured blocks.`,
    );
  }
  const equivalence = compareEditorArtifactEquivalenceOwned(
    ownedInput.equivalence,
  );
  const summaries = summarizeEditorArtifactRuns(ownedInput.runs);
  const decision = decideEditorArtifact(summaries, equivalence);
  const authority = createReceiptAuthority(revision, parameters);
  return deepFreeze({
    schemaVersion: EDITOR_ARTIFACT_RECEIPT_SCHEMA_VERSION,
    generatedAt,
    revision,
    environment,
    parameters,
    builds,
    authority,
    equivalence,
    runs: ownedInput.runs,
    summaries,
    decision,
  });
}

function createReceiptAuthority(revision, parameters) {
  const reasons = [];
  if (revision.dirty) {
    reasons.push("The worktree was dirty when the measurement started.");
  }
  if (parameters.buildMode !== "fresh-dedicated-builds") {
    reasons.push("The receipt reused existing dedicated build directories.");
  }
  return Object.freeze({
    authoritative: reasons.length === 0,
    reasons: Object.freeze(reasons),
  });
}

export function validateAbBaRuns(runs) {
  if (!Array.isArray(runs) || runs.length === 0 || runs.length % 2 !== 0) {
    throw new TypeError(
      "Editor artifact evidence must contain paired full/editor AB/BA blocks.",
    );
  }
  const blocks = new Map();
  for (const run of runs) {
    validateRun(run);
    const blockRuns = blocks.get(run.block) ?? [];
    blockRuns.push(run);
    blocks.set(run.block, blockRuns);
  }
  const blockIds = [...blocks.keys()].sort((left, right) => left - right);
  if (blockIds.length < 2 || blockIds.length % 2 !== 0) {
    throw new TypeError(
      "Editor artifact evidence requires an even number of at least two AB/BA blocks.",
    );
  }
  for (const [index, block] of blockIds.entries()) {
    if (block !== index + 1) {
      throw new TypeError(
        "AB/BA block numbers must be contiguous and one-based.",
      );
    }
    const blockRuns = blocks.get(block);
    if (blockRuns.length !== 2) {
      throw new TypeError(
        `AB/BA block ${block} must contain exactly two variants.`,
      );
    }
    const actualPositions = blockRuns
      .map((run) => run.position)
      .sort((left, right) => left - right);
    if (actualPositions[0] !== 1 || actualPositions[1] !== 2) {
      throw new TypeError(`AB/BA block ${block} must use positions 1 and 2.`);
    }
    const expectedOrder =
      block % 2 === 1 ? ["full", "editor"] : ["editor", "full"];
    const actualOrder = [...blockRuns]
      .sort((left, right) => left.position - right.position)
      .map((run) => run.variant);
    if (JSON.stringify(actualOrder) !== JSON.stringify(expectedOrder)) {
      throw new TypeError(
        `AB/BA block ${block} must use ${expectedOrder.join(" then ")} order.`,
      );
    }
  }
}

function validateEquivalenceMatrix(value, label) {
  const matrix = assertExactRecord(
    value,
    [
      "aggregateSha256",
      "cellCount",
      "families",
      "familyCount",
      "queryCount",
      "queryKinds",
      "schemaVersion",
    ],
    `${label} equivalence matrix`,
  );
  if (matrix.schemaVersion !== EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION) {
    throw new TypeError(`${label} equivalence schemaVersion is invalid.`);
  }
  if (matrix.familyCount !== EDITOR_ARTIFACT_FAMILY_COUNT) {
    throw new TypeError(
      `${label} equivalence matrix must contain ${EDITOR_ARTIFACT_FAMILY_COUNT} families.`,
    );
  }
  if (matrix.queryCount !== EDITOR_ARTIFACT_QUERY_KINDS.length) {
    throw new TypeError(
      `${label} equivalence matrix must contain ${EDITOR_ARTIFACT_QUERY_KINDS.length} query kinds.`,
    );
  }
  if (matrix.cellCount !== matrix.familyCount * matrix.queryCount) {
    throw new TypeError(`${label} equivalence cellCount is inconsistent.`);
  }
  expectExactStringArray(
    matrix.queryKinds,
    EDITOR_ARTIFACT_QUERY_KINDS,
    `${label} equivalence queryKinds`,
  );
  if (
    !Array.isArray(matrix.families) ||
    matrix.families.length !== matrix.familyCount
  ) {
    throw new TypeError(`${label} equivalence families are incomplete.`);
  }

  let previousDiagramType = null;
  for (const [familyIndex, familyValue] of matrix.families.entries()) {
    const family = assertExactRecord(
      familyValue,
      ["baselineId", "diagramType", "fixture", "queries", "sourceSha256"],
      `${label} equivalence family ${familyIndex}`,
    );
    const diagramType = expectString(
      family.diagramType,
      `${label} family ${familyIndex} diagramType`,
    );
    if (previousDiagramType !== null && diagramType <= previousDiagramType) {
      throw new TypeError(
        `${label} equivalence families must be unique and sorted by diagramType.`,
      );
    }
    previousDiagramType = diagramType;
    expectString(
      family.baselineId,
      `${label} family ${diagramType} baselineId`,
    );
    expectString(family.fixture, `${label} family ${diagramType} fixture`);
    expectSha256(
      family.sourceSha256,
      `${label} family ${diagramType} sourceSha256`,
    );
    if (
      !Array.isArray(family.queries) ||
      family.queries.length !== matrix.queryCount
    ) {
      throw new TypeError(
        `${label} family ${diagramType} must contain every query digest.`,
      );
    }
    for (const [queryIndex, queryValue] of family.queries.entries()) {
      const query = assertExactRecord(
        queryValue,
        ["kind", "outcome", "sha256"],
        `${label} family ${diagramType} query ${queryIndex}`,
      );
      if (query.kind !== EDITOR_ARTIFACT_QUERY_KINDS[queryIndex]) {
        throw new TypeError(
          `${label} family ${diagramType} query order is invalid at ${queryIndex}.`,
        );
      }
      expectEnum(
        query.outcome,
        ["error", "result"],
        `${label} family ${diagramType} ${query.kind} outcome`,
      );
      expectSha256(
        query.sha256,
        `${label} family ${diagramType} ${query.kind} sha256`,
      );
    }
  }

  const aggregateSha256 = expectSha256(
    matrix.aggregateSha256,
    `${label} aggregateSha256`,
  );
  const body = {
    schemaVersion: matrix.schemaVersion,
    familyCount: matrix.familyCount,
    queryCount: matrix.queryCount,
    cellCount: matrix.cellCount,
    queryKinds: matrix.queryKinds,
    families: matrix.families,
  };
  const expectedAggregate = sha256Text(canonicalStringify(body));
  if (aggregateSha256 !== expectedAggregate) {
    throw new TypeError(
      `${label} equivalence aggregateSha256 does not bind its matrix.`,
    );
  }
  return matrix;
}

function validateEquivalenceComparison(value) {
  const comparison = assertExactRecord(
    value,
    [
      "cellCount",
      "exact",
      "familyCount",
      "mismatches",
      "queryCount",
      "queryKinds",
      "schemaVersion",
      "variants",
    ],
    "editor semantic-equivalence comparison",
  );
  if (
    comparison.schemaVersion !== EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION ||
    comparison.familyCount !== EDITOR_ARTIFACT_FAMILY_COUNT ||
    comparison.queryCount !== EDITOR_ARTIFACT_QUERY_KINDS.length ||
    comparison.cellCount !==
      EDITOR_ARTIFACT_FAMILY_COUNT * EDITOR_ARTIFACT_QUERY_KINDS.length
  ) {
    throw new TypeError("Editor semantic-equivalence comparison is invalid.");
  }
  expectExactStringArray(
    comparison.queryKinds,
    EDITOR_ARTIFACT_QUERY_KINDS,
    "editor semantic-equivalence queryKinds",
  );
  if (typeof comparison.exact !== "boolean") {
    throw new TypeError("Editor semantic-equivalence exact must be boolean.");
  }
  if (
    !Array.isArray(comparison.mismatches) ||
    comparison.mismatches.some((entry) => typeof entry !== "string") ||
    comparison.exact !== (comparison.mismatches.length === 0)
  ) {
    throw new TypeError("Editor semantic-equivalence mismatches are invalid.");
  }
  assertExactRecord(
    comparison.variants,
    EDITOR_ARTIFACT_VARIANTS,
    "editor semantic-equivalence compared variants",
  );
  validateEquivalenceMatrix(comparison.variants.full, "full");
  validateEquivalenceMatrix(comparison.variants.editor, "editor");
  // Do not trust a caller-provided `exact` flag or mismatch list. Recompute
  // the comparison from the owned matrices so eligibility is always derived
  // from every family/query digest.
  return compareEditorArtifactEquivalence(comparison.variants);
}

function validateRevision(value) {
  const revision = assertExactRecord(
    value,
    ["commit", "dirty", "statusSha256"],
    "revision",
  );
  expectString(revision.commit, "revision commit");
  if (typeof revision.dirty !== "boolean") {
    throw new TypeError("revision dirty must be boolean.");
  }
  expectSha256(revision.statusSha256, "revision statusSha256");
  return revision;
}

function validateEnvironment(value) {
  const environment = assertExactRecord(
    value,
    [
      "architecture",
      "browser",
      "cpu",
      "logicalCpuCount",
      "memoryBytes",
      "node",
      "operatingSystem",
      "playwright",
      "transferEncoding",
    ],
    "environment",
  );
  for (const field of [
    "architecture",
    "browser",
    "cpu",
    "node",
    "operatingSystem",
    "playwright",
  ]) {
    expectString(environment[field], `environment ${field}`);
  }
  expectPositiveInteger(
    environment.logicalCpuCount,
    "environment logicalCpuCount",
  );
  expectPositiveInteger(environment.memoryBytes, "environment memoryBytes");
  if (environment.transferEncoding !== "gzip") {
    throw new TypeError("environment transferEncoding must be gzip.");
  }
  return environment;
}

function validateParameters(value) {
  const parameters = assertExactRecord(
    value,
    [
      "blocks",
      "browserMode",
      "buildMode",
      "cachePolicy",
      "coldDefinition",
      "equivalenceDefinition",
      "equivalenceEvidence",
      "equivalenceEvidenceSha256",
      "memoryDefinition",
      "order",
      "primaryLatencies",
      "transferDefinition",
      "warmDefinition",
    ],
    "parameters",
  );
  expectPositiveInteger(parameters.blocks, "parameters blocks");
  if (parameters.blocks < 2 || parameters.blocks % 2 !== 0) {
    throw new TypeError("parameters blocks must be even and at least two.");
  }
  expectEnum(parameters.browserMode, ["headed", "headless"], "browserMode");
  expectEnum(
    parameters.buildMode,
    ["fresh-dedicated-builds", "reuse-existing"],
    "buildMode",
  );
  const cachePolicy = assertExactRecord(
    parameters.cachePolicy,
    ["hashedAssets", "html"],
    "parameters cachePolicy",
  );
  expectString(cachePolicy.hashedAssets, "cachePolicy hashedAssets");
  expectString(cachePolicy.html, "cachePolicy html");
  for (const field of [
    "coldDefinition",
    "equivalenceDefinition",
    "equivalenceEvidence",
    "memoryDefinition",
    "order",
    "transferDefinition",
    "warmDefinition",
  ]) {
    expectString(parameters[field], `parameters ${field}`);
  }
  expectSha256(
    parameters.equivalenceEvidenceSha256,
    "parameters equivalenceEvidenceSha256",
  );
  expectExactStringArray(
    parameters.primaryLatencies,
    PRIMARY_LATENCY_METRICS,
    "parameters primaryLatencies",
  );
  return parameters;
}

function validateBuilds(value) {
  const builds = assertExactRecord(value, EDITOR_ARTIFACT_VARIANTS, "builds");
  for (const variant of EDITOR_ARTIFACT_VARIANTS) {
    validateBuild(builds[variant], variant);
  }
  if (builds.full.mainWasm.file !== builds.full.workerWasm.file) {
    throw new TypeError("Full build must use its main WASM in the Worker.");
  }
  if (builds.editor.mainWasm.file === builds.editor.workerWasm.file) {
    throw new TypeError(
      "Editor build must use a distinct Worker WASM artifact.",
    );
  }
  return builds;
}

function validateBuild(value, label) {
  const build = assertExactRecord(
    value,
    [
      "manifestSha256",
      "mainWasm",
      "outDir",
      "staticBytes",
      "workerBundle",
      "workerWasm",
    ],
    `${label} build`,
  );
  expectSha256(build.manifestSha256, `${label} build manifestSha256`);
  expectString(build.outDir, `${label} build outDir`);
  validateWasmAsset(build.mainWasm, `${label} mainWasm`);
  validateWasmAsset(build.workerWasm, `${label} workerWasm`);
  const staticBytes = assertExactRecord(
    build.staticBytes,
    ["files", "gzipBytes", "rawBytes"],
    `${label} staticBytes`,
  );
  expectPositiveInteger(staticBytes.files, `${label} static files`);
  expectPositiveInteger(staticBytes.gzipBytes, `${label} static gzipBytes`);
  expectPositiveInteger(staticBytes.rawBytes, `${label} static rawBytes`);
  const workerBundle = assertExactRecord(
    build.workerBundle,
    ["bytes", "file", "sha256"],
    `${label} workerBundle`,
  );
  expectPositiveInteger(workerBundle.bytes, `${label} workerBundle bytes`);
  expectString(workerBundle.file, `${label} workerBundle file`);
  expectSha256(workerBundle.sha256, `${label} workerBundle sha256`);
}

function validateWasmAsset(value, label) {
  const asset = assertExactRecord(
    value,
    ["bytes", "file", "sha256", "source"],
    label,
  );
  expectPositiveInteger(asset.bytes, `${label} bytes`);
  expectString(asset.file, `${label} file`);
  expectSha256(asset.sha256, `${label} sha256`);
  expectString(asset.source, `${label} source`);
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

function validateRun(run) {
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

function assertExactRecord(value, keys, label) {
  if (!isRecord(value)) throw new TypeError(`${label} must be an object.`);
  const expected = [...keys].sort();
  const actual = Object.keys(value).sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new TypeError(
      `${label} must contain exactly: ${expected.join(", ")}.`,
    );
  }
  return value;
}

function cloneOwnedValue(value, label) {
  try {
    return structuredClone(value);
  } catch (error) {
    throw new TypeError(
      `${label} must contain structured-cloneable evidence: ${String(error)}.`,
      { cause: error },
    );
  }
}

function deepFreeze(value, seen = new WeakSet()) {
  if (value === null || typeof value !== "object" || seen.has(value)) {
    return value;
  }
  // Receipt data is JSON-shaped. Typed arrays are intentionally not part of
  // the receipt, but avoid attempting to freeze them if a future field adds one.
  if (ArrayBuffer.isView(value)) return value;
  seen.add(value);
  for (const member of Object.values(value)) deepFreeze(member, seen);
  return Object.freeze(value);
}

function expectExactStringArray(value, expected, label) {
  if (
    !Array.isArray(value) ||
    value.length !== expected.length ||
    value.some((entry, index) => entry !== expected[index])
  ) {
    throw new TypeError(`${label} must match the canonical ordered values.`);
  }
}

function expectIsoDateTime(value, label) {
  const text = expectString(value, label);
  const timestamp = Date.parse(text);
  if (
    !Number.isFinite(timestamp) ||
    new Date(timestamp).toISOString() !== text
  ) {
    throw new TypeError(`${label} must be a canonical ISO date-time.`);
  }
  return text;
}

function expectString(value, label) {
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(`${label} must be a non-empty string.`);
  }
  return value;
}

function expectEnum(value, values, label) {
  if (!values.includes(value)) {
    throw new TypeError(`${label} must be one of ${values.join(", ")}.`);
  }
  return value;
}

function expectSha256(value, label) {
  if (typeof value !== "string" || !SHA256_PATTERN.test(value)) {
    throw new TypeError(`${label} must be a lowercase SHA-256 digest.`);
  }
  return value;
}

function expectPositiveInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new TypeError(`${label} must be a positive integer.`);
  }
}

function expectNonNegativeFinite(value, label) {
  if (!Number.isFinite(value) || value < 0) {
    throw new TypeError(`${label} must be a non-negative finite number.`);
  }
}

function sha256Text(value) {
  return createHash("sha256").update(value).digest("hex");
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
