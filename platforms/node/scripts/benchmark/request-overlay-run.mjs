import { spawnSync } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import {
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  classifyDecision,
  confirmationCells,
  explorationCells,
  projectRss,
  projectSemanticEvidence,
  qualifyNoise,
  validateRequestOverlayManifest,
  validateRequestOverlayReport,
} from "./request-overlay-contract.mjs";
import {
  projectHistoricalArtifact,
  readHistoricalRequestOverlayReceipt,
} from "./request-overlay-receipt.mjs";

const scriptRoot = path.dirname(fileURLToPath(import.meta.url));
const nodeRoot = path.resolve(scriptRoot, "..", "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");
const workerPath = path.join(scriptRoot, "request-overlay-worker.mjs");
const manifestPath = path.join(nodeRoot, "benchmark", "request-overlay.json");

if (isMainModule()) {
  try {
    const options = parseRequestOverlayArgs(process.argv.slice(2));
    const report = runRequestOverlayComparison(options);
    mkdirSync(path.dirname(options.output), { recursive: true });
    writeFileSync(options.output, `${JSON.stringify(report, null, 2)}\n`);
    console.log(`[merman-node] request-overlay owner report written to ${options.output}`);
    process.exitCode = requestOverlayExitCode(report.decision.status);
  } catch (error) {
    console.error(error instanceof Error ? error.stack ?? error.message : String(error));
    process.exitCode = requestOverlayExitCode("contract-failure");
  }
}

export function runRequestOverlayComparison(
  options,
  { invokeWorker = invokeWorkerProcess } = {},
) {
  const initialHarnessDigest = computeRequestOverlayHarnessDigest();
  validateOutputPath(options.output, options.artifacts);
  const manifest = validateRequestOverlayManifest(readJson(manifestPath));
  const historicalArtifacts = readArtifacts(options.artifacts);
  const projectedArtifacts = historicalArtifacts.map(projectHistoricalArtifact);
  const revisions = verifyAdjacentRevisions(projectedArtifacts);
  verifySameRevisionContracts(projectedArtifacts);

  const samplingBase = {
    aa_pairs: options.aaPairs,
    maximum_confirmation_pairs: manifest.sampling_defaults.maximum_confirmation_pairs,
    confirmation_pairs: 8,
    cold_samples: options.coldSamples,
    warmup_iterations: options.warmupIterations,
    reused_samples: options.reusedSamples,
  };
  const invocationId = randomBytes(16).toString("hex");
  const artifactsByKey = new Map(historicalArtifacts.map((artifact) => [artifact.key, artifact]));
  const aa = executeCells(
    explorationCells({ aaPairs: samplingBase.aa_pairs }),
    { artifactsByKey, manifest, sampling: samplingBase, invocationId, invokeWorker },
  );
  const noise = qualifyNoise(
    aa,
    manifest,
    samplingBase.maximum_confirmation_pairs,
  );
  const confirmationPairs = Math.min(
    samplingBase.maximum_confirmation_pairs,
    Math.max(8, noise.required_confirmation_pairs),
  );
  const sampling = { ...samplingBase, confirmation_pairs: confirmationPairs };
  const confirmation = executeCells(
    confirmationCells({ confirmationPairs }),
    { artifactsByKey, manifest, sampling, invocationId, invokeWorker },
  );
  const cells = [...aa, ...confirmation];
  assertArtifactsUnchanged(projectedArtifacts, options.artifacts);
  const decision = classifyDecision(cells, noise, manifest);
  const report = {
    schema_version: 1,
    report_kind: "merman-node-request-overlay-owner-v1",
    owner: "merman-bindings-core",
    scope: {
      lane_id: manifest.lane_id,
      operation_id: manifest.operation.operation_id,
      transports: ["napi", "node-wasm"],
      timing_clock: "process.hrtime.bigint",
      timing_operation: "raw-engine.executeSync",
      transport_admission: "not-evaluated",
    },
    provenance: provenance(initialHarnessDigest, invocationId, options),
    revisions,
    input: {
      manifest_digest: digestJsonFile(manifestPath),
      manifest,
    },
    sampling,
    artifacts: projectedArtifacts,
    semantic_evidence: projectSemanticEvidence(cells),
    cells,
    rss: projectRss(cells),
    decision,
  };
  assertHarnessUnchanged(initialHarnessDigest);
  return validateRequestOverlayReport(report, { trustedManifest: manifest });
}

export function parseRequestOverlayArgs(args) {
  const outputDefault = path.join(
    nodeRoot,
    "reports",
    `request-overlay-owner-${new Date().toISOString().replaceAll(":", "-")}.json`,
  );
  const artifacts = {
    "base:napi": requiredPath(args, "--base-napi"),
    "base:node-wasm": requiredPath(args, "--base-wasm"),
    "head:napi": requiredPath(args, "--head-napi"),
    "head:node-wasm": requiredPath(args, "--head-wasm"),
  };
  const options = {
    artifacts,
    output: path.resolve(valueAfter(args, "--output") ?? outputDefault),
    aaPairs: evenIntegerAfter(
      args,
      "--aa-pairs",
      readJson(manifestPath).sampling_defaults.aa_pairs,
      { min: 8, max: 32 },
    ),
    coldSamples: positiveIntegerAfter(
      args,
      "--cold-samples",
      readJson(manifestPath).sampling_defaults.cold_samples,
    ),
    warmupIterations: positiveIntegerAfter(
      args,
      "--warmup-iterations",
      readJson(manifestPath).sampling_defaults.warmup_iterations,
    ),
    reusedSamples: positiveIntegerAfter(
      args,
      "--reused-samples",
      readJson(manifestPath).sampling_defaults.reused_samples,
    ),
  };
  rejectUnknownArgs(args, new Set([
    "--base-napi",
    "--base-wasm",
    "--head-napi",
    "--head-wasm",
    "--output",
    "--aa-pairs",
    "--cold-samples",
    "--warmup-iterations",
    "--reused-samples",
  ]));
  return options;
}

export function requestOverlayExitCode(status) {
  if (status === "confirmed-improvement") return 0;
  if (status === "rejected") return 0;
  if (status === "regressed") return 1;
  if (status === "contract-failure") return 2;
  if (status === "inconclusive") return 3;
  throw new Error(`Unknown request-overlay decision status: ${status}.`);
}

export function verifyAdjacentRevisions(artifacts, { git = runGit } = {}) {
  const byRevision = new Map([
    ["base", artifacts.filter((artifact) => artifact.revision === "base")],
    ["head", artifacts.filter((artifact) => artifact.revision === "head")],
  ]);
  for (const [revision, side] of byRevision) {
    if (side.length !== 2 || new Set(side.map((artifact) => artifact.commit)).size !== 1) {
      throw new Error(`${revision} N-API and Node-WASM receipts must identify one commit.`);
    }
  }
  const base = byRevision.get("base")[0].commit;
  const head = byRevision.get("head")[0].commit;
  const firstParent = git(["rev-parse", `${head}^1`]);
  if (firstParent !== base) {
    throw new Error(`request-overlay revisions are not adjacent: ${head}^1 is ${firstParent}, not ${base}.`);
  }
  return { base, head, relationship: "head^1==base", verified: true };
}

export function verifySameRevisionContracts(artifacts) {
  for (const revision of ["base", "head"]) {
    const side = artifacts.filter((artifact) => artifact.revision === revision);
    if (side.length !== 2) {
      throw new Error(`${revision} contract comparison requires N-API and Node-WASM artifacts.`);
    }
    for (const key of [
      "source_digest",
      "cargo_lock_digest",
      "binding_contract_digest",
      "capability_recipe_digest",
      "runtime_catalog_digest",
    ]) {
      if (new Set(side.map((artifact) => artifact[key])).size !== 1) {
        throw new Error(`${revision} N-API and Node-WASM receipts disagree on ${key}.`);
      }
    }
  }
}

export function computeRequestOverlayHarnessDigest() {
  const files = requestOverlayHarnessFiles();
  const digest = createHash("sha256");
  for (const file of files) {
    digest.update(path.relative(repositoryRoot, file).split(path.sep).join("/"));
    digest.update("\0");
    digest.update(readFileSync(file));
    digest.update("\0");
  }
  return `sha256:${digest.digest("hex")}`;
}

function readArtifacts(paths) {
  return [
    readHistoricalRequestOverlayReceipt(paths["base:napi"], {
      revision: "base",
      transport: "napi",
    }),
    readHistoricalRequestOverlayReceipt(paths["base:node-wasm"], {
      revision: "base",
      transport: "node-wasm",
    }),
    readHistoricalRequestOverlayReceipt(paths["head:napi"], {
      revision: "head",
      transport: "napi",
    }),
    readHistoricalRequestOverlayReceipt(paths["head:node-wasm"], {
      revision: "head",
      transport: "node-wasm",
    }),
  ];
}

function assertArtifactsUnchanged(initialArtifacts, paths) {
  const currentArtifacts = readArtifacts(paths).map(projectHistoricalArtifact);
  if (stableJson(currentArtifacts) !== stableJson(initialArtifacts)) {
    throw new Error("request-overlay artifacts or receipts changed during measurement; rerun it.");
  }
}

function executeCells(cells, context) {
  return cells.map((identity) => {
    const artifact = context.artifactsByKey.get(identity.artifact_key);
    if (!artifact) throw new Error(`Unknown request-overlay artifact ${identity.artifact_key}.`);
    const invocation = {
      schema_version: 1,
      artifact_key: identity.artifact_key,
      revision: artifact.revision,
      transport: artifact.transport,
      artifact_path: artifact.artifact_path,
      artifact_identity: workerArtifactIdentity(artifact),
      invocation_nonce: randomBytes(16).toString("hex"),
      parent_invocation_id: context.invocationId,
      manifest: context.manifest,
      sampling: context.sampling,
    };
    return { ...identity, result: context.invokeWorker(invocation) };
  });
}

function invokeWorkerProcess(invocation) {
  const result = spawnSync(
    process.execPath,
    ["--expose-gc", workerPath],
    {
      cwd: repositoryRoot,
      encoding: "utf8",
      input: `${JSON.stringify(invocation)}\n`,
      maxBuffer: 256 * 1024 * 1024,
    },
  );
  if (result.error || result.status !== 0) {
    throw new Error(
      `request-overlay worker ${invocation.artifact_key} failed: ${
        result.error?.message ?? (result.stderr.trim() || `exit ${result.status}`)
      }`,
    );
  }
  let value;
  try {
    value = JSON.parse(result.stdout);
  } catch (cause) {
    throw new Error(
      `request-overlay worker ${invocation.artifact_key} returned invalid JSON: ${
        cause instanceof Error ? cause.message : String(cause)
      }`,
    );
  }
  return value;
}

function workerArtifactIdentity(artifact) {
  const {
    key: _key,
    artifact_path: _artifactPath,
    ...identity
  } = artifact;
  return identity;
}

function provenance(harnessDigest, invocationId, options) {
  return {
    measured_at_utc: new Date().toISOString(),
    invocation_id: invocationId,
    harness_digest: harnessDigest,
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    hostname: os.hostname(),
    platform: process.platform,
    release: os.release(),
    arch: process.arch,
    cpu: os.cpus()[0]?.model ?? "unknown",
    logical_cpus: os.cpus().length,
    total_memory_bytes: os.totalmem(),
    node: process.version,
    command: requestOverlayCommand(options),
  };
}

function requestOverlayCommand(options) {
  return [
    process.execPath,
    path.relative(repositoryRoot, fileURLToPath(import.meta.url)).split(path.sep).join("/"),
    "--base-napi", displayPath(options.artifacts["base:napi"]),
    "--base-wasm", displayPath(options.artifacts["base:node-wasm"]),
    "--head-napi", displayPath(options.artifacts["head:napi"]),
    "--head-wasm", displayPath(options.artifacts["head:node-wasm"]),
    "--output", displayPath(options.output),
    "--aa-pairs", String(options.aaPairs),
    "--cold-samples", String(options.coldSamples),
    "--warmup-iterations", String(options.warmupIterations),
    "--reused-samples", String(options.reusedSamples),
  ];
}

function displayPath(value) {
  const absolute = path.resolve(value);
  const relative = path.relative(repositoryRoot, absolute);
  return relative.startsWith("..") || path.isAbsolute(relative)
    ? absolute
    : relative.split(path.sep).join("/");
}

function assertHarnessUnchanged(initialDigest) {
  if (computeRequestOverlayHarnessDigest() !== initialDigest) {
    throw new Error("request-overlay harness inputs changed during measurement; rerun it.");
  }
}

function runGit(args) {
  const result = spawnSync("git", args, {
    cwd: repositoryRoot,
    encoding: "utf8",
  });
  if (result.error || result.status !== 0) {
    throw new Error(`git ${args.join(" ")} failed: ${result.error?.message ?? result.stderr}`);
  }
  return result.stdout.trim();
}

function requiredPath(args, flag) {
  const value = valueAfter(args, flag);
  if (!value) throw new Error(`${flag} is required; the harness never fabricates missing artifacts.`);
  return path.resolve(value);
}

function positiveIntegerAfter(args, flag, fallback) {
  const raw = valueAfter(args, flag);
  const value = raw === null ? fallback : Number(raw);
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new Error(`${flag} must be a positive integer.`);
  }
  return value;
}

function evenIntegerAfter(args, flag, fallback, { min, max }) {
  const value = positiveIntegerAfter(args, flag, fallback);
  if (value < min || value > max || value % 2 !== 0) {
    throw new Error(`${flag} must be an even integer in ${min}..${max}.`);
  }
  return value;
}

function valueAfter(args, flag) {
  const index = args.indexOf(flag);
  return index === -1 ? null : args[index + 1] ?? null;
}

function rejectUnknownArgs(args, knownFlags) {
  const seen = new Set();
  for (let index = 0; index < args.length; index += 2) {
    if (!knownFlags.has(args[index])) throw new Error(`Unknown request-overlay flag: ${args[index]}.`);
    if (seen.has(args[index])) throw new Error(`Duplicate request-overlay flag: ${args[index]}.`);
    seen.add(args[index]);
    if (args[index + 1] === undefined || args[index + 1].startsWith("--")) {
      throw new Error(`${args[index]} requires a value.`);
    }
  }
}

function validateOutputPath(output, artifacts) {
  const resolvedOutput = path.resolve(output);
  if (path.extname(resolvedOutput) !== ".json") {
    throw new Error("request-overlay output must be a .json report path.");
  }
  if (requestOverlayHarnessFiles().includes(resolvedOutput)) {
    throw new Error("request-overlay output must not overwrite a harness input.");
  }
  for (const artifact of Object.values(artifacts)) {
    const root = path.dirname(path.resolve(artifact));
    const relative = path.relative(root, resolvedOutput);
    if (relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative))) {
      throw new Error("request-overlay output must not modify a receipt-bound artifact directory.");
    }
  }
}

function requestOverlayHarnessFiles() {
  return [
    manifestPath,
    path.join(nodeRoot, "package.json"),
    path.join(nodeRoot, "scripts", "stable-json.mjs"),
    ...readdirSync(scriptRoot)
      .filter((name) => /^request-overlay-[a-z-]+\.mjs$/.test(name))
      .map((name) => path.join(scriptRoot, name)),
  ].map((file) => path.resolve(file)).sort();
}

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

function digestJsonFile(file) {
  const value = readJson(file);
  const stable = stableJson(value);
  return `sha256:${createHash("sha256").update(stable).digest("hex")}`;
}

function stableJson(value) {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

function isMainModule() {
  return process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}
