import { isDeepStrictEqual } from "node:util";

import {
  EDITOR_ARTIFACT_RECEIPT_SCHEMA_VERSION,
  EDITOR_ARTIFACT_SELECTION_INPUT_SCHEMA_VERSION,
  EDITOR_ARTIFACT_VARIANTS,
  PRIMARY_LATENCY_METRICS,
  assertExactRecord,
  cloneOwnedValue,
  deepFreeze,
  expectEnum,
  expectExactStringArray,
  expectIsoDateTime,
  expectPositiveInteger,
  expectSha256,
  expectString,
} from "./contract-shared.mjs";
import { compareEditorArtifactEquivalenceOwned } from "./contract-equivalence.mjs";
import {
  decideEditorArtifact,
  summarizeEditorArtifactRuns,
  validateRun,
} from "./contract-decision.mjs";

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
      "selectionInputs",
    ],
    "editor artifact receipt input",
  );
  const generatedAt = expectIsoDateTime(ownedInput.generatedAt, "generatedAt");
  const revision = validateRevision(ownedInput.revision);
  const environment = validateEnvironment(ownedInput.environment);
  const parameters = validateParameters(ownedInput.parameters);
  const builds = validateBuilds(ownedInput.builds);
  const selectionInputs = validateSelectionInputs(ownedInput.selectionInputs);
  if (
    selectionInputs.equivalenceEvidenceSha256 !==
    parameters.equivalenceEvidenceSha256
  ) {
    throw new TypeError(
      "selectionInputs equivalenceEvidenceSha256 must match parameters evidence.",
    );
  }
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
    selectionInputs,
    authority,
    equivalence,
    runs: ownedInput.runs,
    summaries,
    decision,
  });
}

export function validateEditorArtifactReceipt(value) {
  const receipt = assertExactRecord(
    value,
    [
      "authority",
      "builds",
      "decision",
      "environment",
      "equivalence",
      "generatedAt",
      "parameters",
      "revision",
      "runs",
      "schemaVersion",
      "selectionInputs",
      "summaries",
    ],
    "editor artifact receipt",
  );
  if (receipt.schemaVersion !== EDITOR_ARTIFACT_RECEIPT_SCHEMA_VERSION) {
    throw new TypeError(
      `editor artifact receipt schemaVersion must be ${EDITOR_ARTIFACT_RECEIPT_SCHEMA_VERSION}.`,
    );
  }
  const projected = createEditorArtifactReceipt({
    builds: receipt.builds,
    environment: receipt.environment,
    equivalence: receipt.equivalence.variants,
    generatedAt: receipt.generatedAt,
    parameters: receipt.parameters,
    revision: receipt.revision,
    runs: receipt.runs,
    selectionInputs: receipt.selectionInputs,
  });
  if (!isDeepStrictEqual(receipt, projected)) {
    throw new TypeError(
      "editor artifact receipt derived evidence does not match its measured inputs.",
    );
  }
  return projected;
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

function validateSelectionInputs(value) {
  const selectionInputs = assertExactRecord(
    value,
    [
      "editorPackageProvenanceSha256",
      "equivalenceEvidenceSha256",
      "fullPackageProvenanceSha256",
      "measurementContractSha256",
      "schemaVersion",
      "startupClosureSha256",
      "workerClosureSha256",
    ],
    "selectionInputs",
  );
  if (
    selectionInputs.schemaVersion !==
    EDITOR_ARTIFACT_SELECTION_INPUT_SCHEMA_VERSION
  ) {
    throw new TypeError(
      `selectionInputs schemaVersion must be ${EDITOR_ARTIFACT_SELECTION_INPUT_SCHEMA_VERSION}.`,
    );
  }
  for (const field of [
    "editorPackageProvenanceSha256",
    "equivalenceEvidenceSha256",
    "fullPackageProvenanceSha256",
    "measurementContractSha256",
    "startupClosureSha256",
    "workerClosureSha256",
  ]) {
    expectSha256(selectionInputs[field], `selectionInputs ${field}`);
  }
  return selectionInputs;
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
