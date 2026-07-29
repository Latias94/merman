import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  artifactKeys,
  classifyDecision,
  classifyRssGate,
  confirmationCells,
  explorationCells,
  pairedBootstrapBounds,
  prepareRequestOverlayInputs,
  projectRss,
  projectSemanticEvidence,
  qualifyNoise,
  validateRequestOverlayManifest,
  validateRequestOverlayReport,
  validateRequestOverlayWorkerResult,
} from "../scripts/benchmark/request-overlay-contract.mjs";
import {
  projectHistoricalArtifact,
  readHistoricalRequestOverlayReceipt,
} from "../scripts/benchmark/request-overlay-receipt.mjs";
import {
  parseRequestOverlayArgs,
  requestOverlayHarnessFiles,
  requestOverlayExitCode,
  runRequestOverlayComparison,
  snapshotRequestOverlayArtifacts,
  validateOutputPath,
  verifyAdjacentRevisions,
  verifyCrossRevisionBuildContracts,
  verifySameRevisionContracts,
} from "../scripts/benchmark/request-overlay-run.mjs";
import {
  collectGarbageTwice,
  measureColdSample,
  measureReusedSamples,
  runRequestOverlayWorker,
} from "../scripts/benchmark/request-overlay-worker.mjs";
import { computeBuildReceiptInputDigest } from "../scripts/build-receipt.mjs";
import {
  materializeCommittedSourceSnapshot,
  resolveCandidateBuildEnvironment,
  walkFiles,
} from "../scripts/build-candidate.mjs";
import { digestJson } from "../scripts/stable-json.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");
const manifest = validateRequestOverlayManifest(
  JSON.parse(readFileSync(path.join(nodeRoot, "benchmark", "request-overlay.json"), "utf8")),
);
const sampling = {
  aa_pairs: 8,
  maximum_confirmation_pairs: 32,
  confirmation_pairs: 8,
  cold_samples: 1,
  warmup_iterations: 1,
  reused_samples: 1,
};
const fakeRuntimeCatalog = {
  schema_version: 1,
  transport_api_version: 1,
  package_version: "0.8.0-alpha.4",
  capabilities: { operation_ids: ["semantic-json"] },
};
const fakeRuntimeCatalogDigest = digestJson(fakeRuntimeCatalog);

test("manifest pre-encodes empty, version-only, real, batch, and base-size inputs", () => {
  const prepared = prepareRequestOverlayInputs(manifest);
  const empty = JSON.parse(prepared.request_json_by_overlay.empty);
  const version = JSON.parse(prepared.request_json_by_overlay["version-only"]);
  const real = JSON.parse(prepared.request_json_by_overlay["real-resource-override"]);

  assert.equal("options_json" in empty, false);
  assert.equal(version.options_json, '{"version":1}');
  assert.equal(
    real.options_json,
    '{"version":1,"resources":{"limits":{"max_source_bytes":4096}}}',
  );
  assert.deepEqual(manifest.scaling.batch_sizes, [1, 2, 4, 10, 32, 100]);
  assert.deepEqual(manifest.scaling.base_size_units, [1, 2, 4, 10, 32, 100]);
  const sizes = manifest.scaling.base_size_units.map(
    (units) => Buffer.byteLength(prepared.base_json_by_units[String(units)]),
  );
  assert.deepEqual([...sizes].sort((left, right) => left - right), sizes);
  assert.ok(sizes.at(-1) > sizes[0] * 50);
  const resourceRequest = JSON.parse(prepared.resource_limit_request_json);
  const actualBytes = Buffer.byteLength(resourceRequest.source);
  assert.equal(actualBytes, 4101);
  assert.match(
    manifest.resource_limit_probe.expected.error.message,
    new RegExp(`actual=${actualBytes} max=4096$`),
  );
});

test("manifest cannot turn owner attribution into transport admission", () => {
  const forged = structuredClone(manifest);
  forged.transport_admission = "admitted";
  assert.throws(
    () => validateRequestOverlayManifest(forged),
    /must not claim Node transport admission/,
  );
});

test("balanced process plans retain eight A/A pairs for every artifact", () => {
  const aa = explorationCells({ aaPairs: 8 });
  const ab = confirmationCells({ confirmationPairs: 8 });
  assert.equal(aa.length, 64);
  assert.equal(ab.length, 32);
  assert.deepEqual(
    [...new Set(aa.map((cell) => cell.calibration_revision))],
    ["base", "head"],
  );
  for (const artifactKey of artifactKeys()) {
    assert.equal(
      aa.filter((cell) => cell.artifact_key === artifactKey).length,
      16,
    );
  }
  assert.deepEqual(
    aa.filter((cell) =>
      cell.calibration_revision === "base" && cell.transport === "napi" && cell.pair_index < 2,
    ).map((cell) => cell.role),
    ["a", "b", "b", "a"],
  );
  assert.deepEqual(
    ab.filter((cell) => cell.transport === "node-wasm" && cell.pair_index < 2).map((cell) => cell.role),
    ["base", "head", "head", "base"],
  );
  assert.throws(() => explorationCells({ aaPairs: 6 }), /8\.\.32/);
  assert.throws(() => confirmationCells({ confirmationPairs: 9 }), /8\.\.32/);
});

test("paired bootstrap is deterministic and applies simultaneous confidence", () => {
  const options = {
    interval: "one-sided",
    seedLabel: "contract-test",
    statistics: manifest.statistics,
    familySize: manifest.statistics.confirmation_family_size,
  };
  const base = [100, 101, 99, 100, 102, 98, 101, 99];
  const head = [80, 82, 81, 79, 83, 78, 80, 81];
  const first = pairedBootstrapBounds(base, head, options);
  const second = pairedBootstrapBounds(base, head, options);

  assert.deepEqual(second, first);
  assert.equal(first.confidence_contract.bootstrap_resamples, 10_000);
  assert.equal(first.confidence_contract.component_confidence_level, 0.9875);
  assert.equal(first.confidence_contract.monte_carlo_failure_probability, 0.001);
  assert.equal(first.absolute_ns.monte_carlo.method, "exact-binomial-order-statistic-v1");
  assert.ok(first.log_ratio.upper < 0);
  assert.ok(first.absolute_ns.upper < 0);
});

test("Monte Carlo tail confidence cannot bootstrap across the absolute gate", () => {
  const base = Array(30).fill(1_000_000);
  const head = Array(29).fill(940_000).concat([1_022_500]);
  const bounds = pairedBootstrapBounds(base, head, {
    interval: "one-sided",
    seedLabel: "confirmation:napi",
    statistics: manifest.statistics,
    familySize: manifest.statistics.confirmation_family_size,
  });

  assert.equal(bounds.absolute_ns.upper, -49_000);
  assert.ok(bounds.absolute_ns.upper > -50_000);
  assert.equal(bounds.absolute_ns.monte_carlo.upper_rank, 9_916);
});

test("A/A qualification requires zero inclusion in addition to equivalence", () => {
  const cells = fakeExplorationCells(({ role }) =>
    role === "a" ? 1_000_000 : 1_040_000
  );
  const noise = qualifyNoise(cells, manifest, sampling.maximum_confirmation_pairs);

  assert.equal(noise.stable, false);
  for (const entry of Object.values(noise.by_artifact)) {
    assert.ok(entry.identity_bounds.log_ratio.lower > 0);
    assert.ok(entry.identity_bounds.absolute_ns.lower > 0);
    assert.equal(entry.stable, false);
  }
});

test("Bonferroni-adjusted power above the registered cap remains inconclusive", () => {
  const aa = fakeExplorationCells(({ role, pair_index: pairIndex }) => {
    if (role === "a") return 1_000_000;
    return pairIndex % 2 === 0 ? 750_000 : 1_250_000;
  });
  const noise = qualifyNoise(aa, manifest, sampling.maximum_confirmation_pairs);
  assert.equal(noise.power_contract.component_confidence_level, 0.9875);
  assert.ok(Math.abs(noise.power_contract.critical_z - 2.2414027264652865) < 1e-12);
  assert.ok(noise.required_confirmation_pairs > sampling.maximum_confirmation_pairs);
  assert.equal(noise.within_budget, false);

  const confirmation = fakeConfirmationCells(() => 1_000_000);
  assert.equal(classifyDecision([...aa, ...confirmation], noise, manifest).status, "inconclusive");
});

test("RSS slope, absolute growth, and head-regression bounds gate latency wins", () => {
  const qualifiedCells = fakeConfirmationCells(({ role }) =>
    role === "base" ? 1_000_000 : 800_000
  );
  for (const cell of qualifiedCells) {
    setRssScaleGrowth(cell.result, () => 512 * 1024);
  }
  const qualified = classifyRssGate(qualifiedCells, manifest);
  assert.equal(qualified.qualified, true);
  assert.equal(qualified.upper_bound_method, "observed-process-maximum-v1");

  const steepCells = structuredClone(qualifiedCells);
  for (const cell of steepCells) {
    setRssScaleGrowth(cell.result, (_dimension, scale) => scale ** 2 * 1024);
  }
  const steep = classifyRssGate(steepCells, manifest);
  assert.equal(steep.qualified, false);
  assert.ok(
    steep.by_transport.napi.dimensions["batch-reused"]
      .slope_upper_bound.upper > manifest.rss_contract.maximum_slope_upper_bound,
  );

  const regressedCells = structuredClone(qualifiedCells);
  for (const cell of regressedCells) {
    setRssScaleGrowth(
      cell.result,
      () => cell.role === "head" ? 80 * 1024 * 1024 : 512 * 1024,
    );
  }
  const regressed = classifyRssGate(regressedCells, manifest);
  assert.equal(regressed.qualified, false);
  assert.ok(
    regressed.by_transport.napi.dimensions["batch-reused"]
      .absolute_growth_upper_bound_bytes.upper >
        manifest.rss_contract.maximum_absolute_growth_bytes,
  );
  assert.ok(
    regressed.by_transport.napi.dimensions["batch-reused"]
      .head_regression_upper_bound_bytes.upper >
        manifest.rss_contract.maximum_head_regression_bytes,
  );

  const aa = fakeExplorationCells(() => 1_000_000);
  const noise = qualifyNoise(aa, manifest, sampling.maximum_confirmation_pairs);
  const decision = classifyDecision([...aa, ...regressedCells], noise, manifest);
  assert.equal(decision.status, "inconclusive");
  assert.equal(decision.rss.qualified, false);
  assert.match(decision.reasons[0], /RSS curve exceeded/);
});

test("RSS observed envelope cannot bootstrap away a sparse over-budget regression", () => {
  const cells = fakeConfirmationCells(() => 1_000_000);
  for (const cell of cells) {
    const regression =
      cell.role === "head" && cell.pair_index < 4 ? 1_100_000 : 0;
    setRssScaleGrowth(cell.result, () => regression);
  }
  const gate = classifyRssGate(cells, manifest);
  assert.equal(gate.qualified, false);
  assert.equal(
    gate.by_transport.napi.dimensions["batch-reused"]
      .head_regression_upper_bound_bytes.upper,
    1_100_000,
  );
});

test("RSS gate rejects a startup high-water blind spot larger than its budget", () => {
  const cells = fakeConfirmationCells(() => 1_000_000);
  for (const cell of cells) {
    setRssScaleGrowth(cell.result, () => 0);
    cell.result.rss.baseline_history_gap_bytes = 150 * 1024 * 1024;
  }
  const gate = classifyRssGate(cells, manifest);
  assert.equal(gate.qualified, false);
  assert.equal(
    gate.by_transport.napi.process_envelope
      .startup_history_gap_upper_bound_bytes.upper,
    150 * 1024 * 1024,
  );
});

test("RSS gate rejects a history gap created before a measurement lane", () => {
  const cells = fakeConfirmationCells(() => 1_000_000);
  for (const cell of cells) {
    setRssScaleGrowth(cell.result, () => 0);
    for (const lane of cell.result.rss.lanes) {
      if (lane.lane_id.startsWith("measurement:")) {
        lane.baseline_history_gap_bytes = 47 * 1024 * 1024;
      }
    }
  }
  const gate = classifyRssGate(cells, manifest);
  assert.equal(gate.qualified, false);
  assert.equal(
    gate.by_transport.napi.process_envelope
      .lane_history_gap_upper_bound_bytes.upper,
    47 * 1024 * 1024,
  );
});

test("raw timing boundaries separate construction, batch, and total nanoseconds", () => {
  const events = [];
  class Engine {
    constructor(baseJson) {
      events.push(["construct", baseJson]);
    }
    executeSync(requestJson) {
      events.push(["execute", requestJson]);
      return successRaw();
    }
    dispose() {
      events.push(["dispose"]);
    }
  }
  const clock = [0n, 10n, 30n, 40n, 70n, 80n];
  const measured = measureColdSample({
    Engine,
    baseJson: "base",
    requestJson: "request",
    batchSize: 1,
    collectGarbage: () => events.push(["double-gc"]),
    now: () => clock.shift(),
  });

  assert.deepEqual(measured.sample, {
    construct_ns: 20,
    batch_ns: 30,
    total_ns: 80,
  });
  assert.deepEqual(events.map(([event]) => event), [
    "double-gc",
    "construct",
    "execute",
    "dispose",
  ]);
});

test("reused timing warms outside the clock and double-collects outside every sample", () => {
  let executions = 0;
  let collections = 0;
  class Engine {
    executeSync() {
      executions += 1;
      return successRaw();
    }
  }
  const clock = [0n, 1n, 11n, 12n, 20n, 21n, 31n, 32n];
  const measured = measureReusedSamples({
    Engine,
    baseJson: "{}",
    requestJson: "{}",
    batchSize: 2,
    warmupIterations: 2,
    samples: 2,
    collectGarbage: () => {
      collections += 1;
    },
    now: () => clock.shift(),
  });
  assert.equal(executions, 8);
  assert.equal(collections, 2);
  assert.deepEqual(measured.samples, [
    { construct_ns: 0, batch_ns: 10, total_ns: 12 },
    { construct_ns: 0, batch_ns: 10, total_ns: 12 },
  ]);
});

test("production garbage collection hook invokes exposed GC exactly twice", () => {
  const original = globalThis.gc;
  let calls = 0;
  globalThis.gc = () => {
    calls += 1;
  };
  try {
    collectGarbageTwice();
    assert.equal(calls, 2);
  } finally {
    if (original === undefined) delete globalThis.gc;
    else globalThis.gc = original;
  }
});

test("historical receipt validation is self-contained and rejects artifact drift", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-request-overlay-receipt-"));
  try {
    const artifact = writeHistoricalReceipt(root, "napi");
    const receiptPath = path.join(root, "build-receipt.json");
    const receipt = JSON.parse(readFileSync(receiptPath, "utf8"));
    const readReceipt = () => readHistoricalRequestOverlayReceipt(
      artifact,
      { revision: "base", transport: "napi" },
    );
    const value = readReceipt();
    assert.equal(value.key, "base:napi");
    assert.equal(value.artifact_path_in_receipt, "merman.node");
    assert.equal(projectHistoricalArtifact(value).artifact_path, undefined);

    const downgraded = structuredClone(receipt);
    downgraded.schema_version = 1;
    writeFileSync(receiptPath, `${JSON.stringify(downgraded, null, 2)}\n`);
    assert.throws(
      readReceipt,
      /invalid schema/,
    );
    writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);

    writeFileSync(artifact, "tampered");
    assert.throws(readReceipt, /receipt is stale/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("runner requires all four artifacts and proves head first-parent adjacency", () => {
  assert.throws(() => parseRequestOverlayArgs([]), /never fabricates missing artifacts/);
  const args = [
    "--base-napi", "base.node",
    "--base-wasm", "base.js",
    "--head-napi", "head.node",
    "--head-wasm", "head.js",
  ];
  assert.deepEqual(Object.keys(parseRequestOverlayArgs(args).artifacts), artifactKeys());
  assert.throws(
    () => parseRequestOverlayArgs([...args, "--unknown", "value"]),
    /Unknown request-overlay flag/,
  );
  assert.throws(
    () => parseRequestOverlayArgs([...args, "--base-napi", "duplicate.node"]),
    /Duplicate request-overlay flag/,
  );
  assert.throws(
    () => parseRequestOverlayArgs([...args, "--warmup-iterations", "101"]),
    /must not exceed 100/,
  );

  const artifacts = fakeArtifacts();
  const calls = [];
  const revisions = verifyAdjacentRevisions(artifacts, {
    git: (gitArgs) => {
      calls.push(gitArgs);
      return artifacts[0].commit;
    },
  });
  assert.equal(revisions.relationship, "head^1==base");
  assert.deepEqual(calls, [["rev-parse", `${artifacts[2].commit}^1`]]);
  assert.throws(
    () => verifyAdjacentRevisions(artifacts, { git: () => "f".repeat(40) }),
    /are not adjacent/,
  );
});

test("harness digest covers transitive artifact-attestation code", () => {
  const relative = requestOverlayHarnessFiles().map((file) =>
    path.relative(nodeRoot, file).split(path.sep).join("/")
  );
  for (const expected of [
    "scripts/build-candidate.mjs",
    "scripts/build-receipt.mjs",
    "scripts/benchmark/svg-signature.mjs",
    "candidate-builds.json",
    "package-surfaces.json",
    "src/engine.mjs",
    "src/native-loader.mjs",
  ]) {
    assert.ok(relative.includes(expected), expected);
  }
});

test("runner snapshots receipt-bound artifacts before measurement", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-request-overlay-snapshot-"));
  let snapshot;
  try {
    const artifacts = [];
    for (const revision of ["base", "head"]) {
      for (const transport of ["napi", "node-wasm"]) {
        const receiptRoot = path.join(root, `${revision}-${transport}`);
        mkdirSync(receiptRoot);
        const artifact = writeHistoricalReceipt(receiptRoot, transport);
        artifacts.push(readHistoricalRequestOverlayReceipt(
          artifact,
          { revision, transport },
        ));
      }
    }
    snapshot = snapshotRequestOverlayArtifacts(artifacts);
    writeFileSync(artifacts[0].artifact_path, "replaced-after-attestation");
    assert.equal(readFileSync(snapshot.artifacts[0].artifact_path, "utf8"), "artifact");
    assert.equal(
      projectHistoricalArtifact(snapshot.artifacts[0]).artifact_sha256,
      projectHistoricalArtifact(artifacts[0]).artifact_sha256,
    );
  } finally {
    const snapshotRoot = snapshot?.root;
    snapshot?.dispose();
    if (snapshotRoot) assert.equal(existsSync(snapshotRoot), false);
    rmSync(root, { recursive: true, force: true });
  }
});

test("runner exit codes preserve the performance decision priority", () => {
  assert.equal(requestOverlayExitCode("confirmed-improvement"), 0);
  assert.equal(requestOverlayExitCode("rejected"), 0);
  assert.equal(requestOverlayExitCode("regressed"), 1);
  assert.equal(requestOverlayExitCode("contract-failure"), 2);
  assert.equal(requestOverlayExitCode("inconclusive"), 3);
  assert.throws(() => requestOverlayExitCode("admitted"), /Unknown request-overlay decision/);
  const missingArtifacts = spawnSync(
    process.execPath,
    [path.join(nodeRoot, "scripts", "benchmark", "request-overlay-run.mjs")],
    { encoding: "utf8" },
  );
  assert.equal(missingArtifacts.status, 2);
  assert.match(missingArtifacts.stderr, /never fabricates missing artifacts/);
});

test("runner refuses report paths that could overwrite artifacts or harness inputs", () => {
  const artifacts = {
    "base:napi": path.join(nodeRoot, "artifacts", "base-napi", "merman.node"),
    "base:node-wasm": path.join(nodeRoot, "artifacts", "base-wasm", "merman_node.js"),
    "head:napi": path.join(nodeRoot, "artifacts", "head-napi", "merman.node"),
    "head:node-wasm": path.join(nodeRoot, "artifacts", "head-wasm", "merman_node.js"),
  };
  assert.throws(
    () => runRequestOverlayComparison({ artifacts, output: path.join(nodeRoot, "package.json") }),
    /must not overwrite a harness input/,
  );
  assert.throws(
    () => runRequestOverlayComparison({ artifacts, output: artifacts["base:napi"] }),
    /must be a \.json report path/,
  );
});

test("runner atomically replaces stale success evidence with a failure receipt", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-request-overlay-failure-"));
  try {
    const output = path.join(root, "report.json");
    writeFileSync(output, '{"decision":{"status":"confirmed-improvement"}}\n');
    const result = spawnSync(
      process.execPath,
      [
        path.join(nodeRoot, "scripts", "benchmark", "request-overlay-run.mjs"),
        "--base-napi", path.join(root, "base.node"),
        "--base-wasm", path.join(root, "base.js"),
        "--head-napi", path.join(root, "head.node"),
        "--head-wasm", path.join(root, "head.js"),
        "--output", output,
      ],
      { encoding: "utf8" },
    );

    assert.equal(result.status, 2);
    const receipt = JSON.parse(readFileSync(output, "utf8"));
    assert.equal(receipt.report_kind, "merman-node-request-overlay-contract-failure-v1");
    assert.equal(receipt.status, "contract-failure");
    assert.equal(receipt.transport_admission, "not-evaluated");
    assert.equal(receipt.failure.stage, "artifact-attestation");
    assert.equal(receipt.failure.name, "RequestOverlayContractError");
    assert.match(receipt.failure.message, /Missing base napi artifact/);
    assert.deepEqual(receipt.evidence.completed_stages, [
      "harness-digest",
      "output-contract",
      "manifest-contract",
    ]);
    assert.equal(receipt.evidence.output_path_validated, true);
    assert.equal(receipt.evidence.harness_digest_available, true);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("runner rejects output symlinks before publishing evidence", (context) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-request-overlay-symlink-"));
  try {
    const target = path.join(root, "target.json");
    const output = path.join(root, "report.json");
    writeFileSync(target, "protected\n");
    try {
      symlinkSync(target, output, "file");
    } catch (error) {
      if (error?.code === "EPERM") {
        context.skip("the host does not permit test symlinks");
        return;
      }
      throw error;
    }
    assert.throws(
      () => validateOutputPath(output, {
        "base:napi": path.join(root, "artifacts", "base.node"),
        "base:node-wasm": path.join(root, "artifacts", "base.js"),
        "head:napi": path.join(root, "artifacts", "head.node"),
        "head:node-wasm": path.join(root, "artifacts", "head.js"),
      }),
      /regular non-symlink file/,
    );
    assert.equal(readFileSync(target, "utf8"), "protected\n");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("candidate source discovery rejects symbolic links", (context) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-build-source-link-"));
  try {
    const outside = path.join(root, "outside.rs");
    const packageRoot = path.join(root, "package");
    mkdirSync(packageRoot);
    writeFileSync(outside, "uncommitted source\n");
    try {
      symlinkSync(outside, path.join(packageRoot, "lib.rs"), "file");
    } catch (error) {
      if (error?.code === "EPERM") {
        context.skip("the host does not permit test symlinks");
        return;
      }
      throw error;
    }
    assert.throws(
      () => walkFiles(packageRoot, { skipBuildOutputs: true }),
      /non-regular entry/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("candidate builds materialize source from an immutable commit snapshot", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-build-source-snapshot-"));
  const repository = path.join(root, "repository");
  let snapshot;
  try {
    mkdirSync(path.join(repository, "crates", "example", "src"), { recursive: true });
    writeFileSync(path.join(repository, "Cargo.toml"), "[workspace]\nmembers=[]\n");
    const source = path.join(repository, "crates", "example", "src", "lib.rs");
    writeFileSync(source, "pub const VALUE: u8 = 1;\n");
    for (const args of [
      ["init", "--quiet"],
      ["config", "user.email", "snapshot@example.invalid"],
      ["config", "user.name", "Snapshot Test"],
      ["add", "."],
      ["commit", "--quiet", "-m", "snapshot"],
    ]) {
      const result = spawnSync("git", args, { cwd: repository, encoding: "utf8" });
      assert.equal(result.status, 0, result.stderr);
    }
    const commitResult = spawnSync(
      "git",
      ["rev-parse", "--verify", "HEAD^{commit}"],
      { cwd: repository, encoding: "utf8" },
    );
    assert.equal(commitResult.status, 0, commitResult.stderr);
    const originalGitDirectory = process.env.GIT_DIR;
    try {
      process.env.GIT_DIR = path.join(root, "injected-git-directory");
      snapshot = materializeCommittedSourceSnapshot({
        repository,
        commit: commitResult.stdout.trim(),
        paths: ["Cargo.toml", "crates"],
      });
    } finally {
      if (originalGitDirectory === undefined) delete process.env.GIT_DIR;
      else process.env.GIT_DIR = originalGitDirectory;
    }
    writeFileSync(source, "pub const VALUE: u8 = 2;\n");
    assert.equal(
      readFileSync(
        path.join(snapshot.sourceRoot, "crates", "example", "src", "lib.rs"),
        "utf8",
      ),
      "pub const VALUE: u8 = 1;\n",
    );
  } finally {
    const sourceRoot = snapshot?.sourceRoot;
    snapshot?.dispose();
    if (sourceRoot) assert.equal(existsSync(sourceRoot), false);
    rmSync(root, { recursive: true, force: true });
  }
});

test("same-revision transports must share source and runtime contract digests", () => {
  const artifacts = fakeArtifacts();
  verifySameRevisionContracts(artifacts);
  artifacts[1].source_digest = sha("e");
  assert.throws(
    () => verifySameRevisionContracts(artifacts),
    /base .* disagree on source_digest/,
  );
  artifacts[1].source_digest = artifacts[0].source_digest;
  artifacts[1].runtime_catalog_digest = sha("e");
  assert.throws(
    () => verifySameRevisionContracts(artifacts),
    /base .* disagree on runtime_catalog_digest/,
  );
});

test("base and head builds must use one code-generation environment", () => {
  const artifacts = fakeArtifacts();
  verifyCrossRevisionBuildContracts(artifacts);
  artifacts.find(
    (artifact) => artifact.revision === "head" && artifact.transport === "napi",
  ).build_environment_digest = sha("f");
  assert.throws(
    () => verifyCrossRevisionBuildContracts(artifacts),
    /napi base and head builds disagree on build_environment_digest/,
  );
});

test("candidate build evidence binds Cargo and Rust tool selection", () => {
  const baseline = resolveCandidateBuildEnvironment().contract;
  const inheritedNames = baseline.inherited.map((entry) => entry.name);
  assert.ok(inheritedNames.includes("CARGO"));
  assert.ok(inheritedNames.includes("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"));
  assert.ok(inheritedNames.includes("CARGO_INCREMENTAL"));
  assert.ok(inheritedNames.includes("RUSTC_BOOTSTRAP"));
  assert.ok(inheritedNames.includes("RUSTUP_TOOLCHAIN"));
  assert.deepEqual(
    baseline.external_inputs.map((entry) => entry.id),
    [
      "build-source/.cargo/config",
      "build-source/.cargo/config.toml",
      "cargo-home/config",
      "cargo-home/config.toml",
      "tool/cargo",
      "tool/git",
      "tool/node",
      "tool/rustc",
      "tool/wasm-pack",
      "tool/napi-cli",
      "tool/napi-cli-node-modules",
    ],
  );
  const nodeModules = baseline.external_inputs.find(
    (entry) => entry.id === "tool/napi-cli-node-modules",
  );
  assert.ok(nodeModules.file_count > 1);
  const originalCargo = process.env.CARGO;
  const originalCc = process.env.CC;
  try {
    process.env.CARGO = process.execPath;
    const changed = resolveCandidateBuildEnvironment().contract;
    assert.notEqual(digestJson(changed), digestJson(baseline));

    process.env.CC = process.execPath;
    const wrapped = resolveCandidateBuildEnvironment().contract;
    assert.notEqual(digestJson(wrapped), digestJson(changed));
    assert.equal(
      wrapped.external_inputs.find((entry) => entry.id === "environment-tool/CC")
        .path_sha256,
      digestBytes(process.execPath),
    );
  } finally {
    if (originalCargo === undefined) delete process.env.CARGO;
    else process.env.CARGO = originalCargo;
    if (originalCc === undefined) delete process.env.CC;
    else process.env.CC = originalCc;
  }
  const originalNodePath = process.env.NODE_PATH;
  try {
    process.env.NODE_PATH = path.join(os.tmpdir(), "injected-node-modules");
    assert.throws(
      () => resolveCandidateBuildEnvironment(),
      /reject NODE_PATH process injection/,
    );
  } finally {
    if (originalNodePath === undefined) delete process.env.NODE_PATH;
    else process.env.NODE_PATH = originalNodePath;
  }

  const originalIcuData = process.env.ICU4X_DATA_DIR;
  try {
    process.env.ICU4X_DATA_DIR = path.join(os.tmpdir(), "unbound-icu-data");
    const sanitized = resolveCandidateBuildEnvironment();
    assert.equal(Object.hasOwn(sanitized.environment, "ICU4X_DATA_DIR"), false);
    assert.equal(
      sanitized.contract.inherited.some((entry) => entry.name === "ICU4X_DATA_DIR"),
      false,
    );
  } finally {
    if (originalIcuData === undefined) delete process.env.ICU4X_DATA_DIR;
    else process.env.ICU4X_DATA_DIR = originalIcuData;
  }
});

test("worker covers exact semantics, errors, lifecycle matrix, stress points, and RSS baselines", () => {
  const artifact = fakeArtifacts()[0];
  const result = fakeWorkerResult(artifact, "0".repeat(32));
  assert.equal(result.semantic_evidence.passed, true);
  assert.equal(result.measurements.overlays.length, 6);
  assert.deepEqual(
    result.measurements.batch_scaling.map((lane) => lane.batch_size),
    [1, 2, 4, 10, 32, 100],
  );
  assert.equal(
    result.measurements.base_size_scaling.find(
      (lane) => lane.base_size_units === 100 && lane.engine_lifecycle === "reused",
    ).overlay_id,
    "version-only",
  );
  assert.equal(
    result.rss.fresh_process_envelope_growth_bytes,
    Math.max(
      result.rss.sampled_current_peak_growth_bytes,
      result.rss.process_max_peak_growth_bytes,
    ),
  );
  assert.equal(
    result.rss.method,
    "lane-local-retained/fresh-process-envelope-v4",
  );
  assert.equal(result.rss.lanes.length, 37);
  assert.equal(result.rss.lanes[0].lane_id, "lifecycle:artifact-load");
  assert.deepEqual(
    result.rss.lanes.filter((lane) => lane.lane_id.startsWith("measurement:batch:"))
      .map((lane) => lane.lane_id),
    [1, 2, 4, 10, 32, 100].map(
      (batchSize) => `measurement:batch:${batchSize}:reused`,
    ),
  );
  assert.deepEqual(
    result.semantic_evidence.resource_limit_probe.envelope,
    manifest.resource_limit_probe.expected,
  );
  assert.equal(result.semantic_evidence.runtime_catalog_digest, artifact.runtime_catalog_digest);
});

test("worker rejects a loaded runtime catalog that differs from the receipt", () => {
  const artifact = fakeArtifacts()[0];
  artifact.runtime_catalog_digest = sha("e");
  assert.throws(
    () => fakeWorkerResult(artifact, "f".repeat(32)),
    /runtime catalog differs from its historical receipt/,
  );
});

test("reused semantic oracle rejects a first-call-only complete wire response", () => {
  const artifact = fakeArtifacts()[0];
  const result = runRequestOverlayWorker(
    workerInvocation(artifact, "9".repeat(32)),
    {
      loadEngineConstructor: () => FirstCallOnlyCompleteEngine,
      collectGarbage: () => {},
      now: monotonicClock(),
    },
  );
  assert.equal(result.semantic_evidence.passed, false);
  const probe = result.semantic_evidence.reused_success_probes.find(
    (entry) => entry.id === "overlay:version-only",
  );
  assert.equal(probe.matching_observations, 1);
  assert.equal(probe.first_mismatch.iteration, 1);
  assert.equal(probe.first_mismatch.envelope.result.metadata_json, undefined);

  const aa = fakeExplorationCells(() => 1_000_000);
  const noise = qualifyNoise(aa, manifest, sampling.maximum_confirmation_pairs);
  const confirmation = fakeConfirmationCells(({ role }) =>
    role === "base" ? 1_000_000 : 800_000
  );
  for (const cell of confirmation) {
    setRssScaleGrowth(cell.result, () => 0);
    if (cell.role === "head") cell.result.semantic_evidence.passed = false;
  }
  const decision = classifyDecision([...aa, ...confirmation], noise, manifest);
  assert.equal(decision.status, "rejected");
  assert.equal(decision.semantic.head_qualified, false);
  assert.match(decision.reasons[0], /mandatory semantic owner gate/);
});

test("every timed response is checked after the clock stops", () => {
  const artifact = fakeArtifacts()[0];
  const result = runRequestOverlayWorker(
      workerInvocation(artifact, "8".repeat(32)),
      {
        loadEngineConstructor: () => FirstThirtyTwoCallsCompleteEngine,
        collectGarbage: () => {},
        now: monotonicClock(),
      },
  );
  assert.equal(result.semantic_evidence.probe_passed, true);
  assert.equal(result.semantic_evidence.measurement_passed, false);
  assert.equal(result.semantic_evidence.passed, false);
  const lane = result.measurements.batch_scaling.find(
    (entry) => entry.batch_size === 100,
  );
  assert.equal(lane.semantic_contract.passed, false);
  assert.equal(lane.semantic_contract.first_mismatch.observation_index, 32);
});

test("timed responses must retain deterministic wire bytes after the semantic oracle", () => {
  const artifact = fakeArtifacts()[0];
  const result = runRequestOverlayWorker(
    workerInvocation(artifact, "7".repeat(32)),
    {
      loadEngineConstructor: () => FirstThirtyTwoCallsStableWireEngine,
      collectGarbage: () => {},
      now: monotonicClock(),
    },
  );
  assert.equal(result.semantic_evidence.probe_passed, true);
  assert.equal(result.semantic_evidence.measurement_passed, false);
  const lane = result.measurements.batch_scaling.find(
    (entry) => entry.batch_size === 100,
  );
  assert.equal(lane.semantic_contract.matching_observations, 200);
  assert.equal(lane.semantic_contract.first_mismatch, null);
  assert.equal(lane.semantic_contract.wire_deterministic, false);
  assert.equal(lane.semantic_contract.unique_response_sha256.length, 2);
});

test("semantic gate rejects fresh-process timed-response wire-byte drift", () => {
  const aa = fakeExplorationCells(() => 1_000_000);
  const noise = qualifyNoise(aa, manifest, sampling.maximum_confirmation_pairs);
  const confirmation = fakeConfirmationCells(({ role }) =>
    role === "base" ? 1_000_000 : 800_000
  );
  for (const cell of confirmation) setRssScaleGrowth(cell.result, () => 0);
  const drifting = confirmation.find(
    (cell) => cell.artifact_key === "head:napi" && cell.pair_index === 1,
  );
  drifting.result.measurements.overlays[0].semantic_contract.response_sequence_sha256 =
    sha("f");

  const decision = classifyDecision([...aa, ...confirmation], noise, manifest);
  assert.equal(decision.status, "rejected");
  assert.equal(
    decision.semantic.by_artifact["head:napi"].fresh_process_deterministic,
    false,
  );
  assert.equal(decision.semantic.head_qualified, false);
});

test("worker binds timing, RSS, GC, and byte intrinsics before loading the artifact", () => {
  const artifact = fakeArtifacts()[0];
  const saved = [
    [process.hrtime, "bigint", Object.getOwnPropertyDescriptor(process.hrtime, "bigint")],
    [process, "memoryUsage", Object.getOwnPropertyDescriptor(process, "memoryUsage")],
    [process, "resourceUsage", Object.getOwnPropertyDescriptor(process, "resourceUsage")],
    [Buffer, "byteLength", Object.getOwnPropertyDescriptor(Buffer, "byteLength")],
    [globalThis, "gc", Object.getOwnPropertyDescriptor(globalThis, "gc")],
  ];
  let trustedGarbageCollections = 0;
  let forgedGarbageCollections = 0;
  Object.defineProperty(globalThis, "gc", {
    configurable: true,
    writable: true,
    value: () => {
      trustedGarbageCollections += 1;
    },
  });

  try {
    const result = runRequestOverlayWorker(
      workerInvocation(artifact, "d".repeat(32)),
      {
        loadEngineConstructor: () => {
          process.hrtime.bigint = () => 1n;
          process.memoryUsage = () => ({ rss: 1 });
          process.resourceUsage = () => ({ maxRSS: 1 });
          Buffer.byteLength = () => 1;
          globalThis.gc = () => {
            forgedGarbageCollections += 1;
          };
          return FakeEngine;
        },
      },
    );

    assert.ok(trustedGarbageCollections > 0);
    assert.equal(forgedGarbageCollections, 0);
    assert.ok(result.rss.baseline_current_rss_bytes > 1);
    assert.ok(result.rss.baseline_process_max_rss_bytes > 1);
    assert.ok(result.measurements.base_size_scaling[0].base_options_bytes > 1);
    assert.ok(result.measurements.overlays[0].samples[0].batch_ns > 1);
  } finally {
    for (const [owner, key, descriptor] of saved) {
      if (descriptor === undefined) delete owner[key];
      else Object.defineProperty(owner, key, descriptor);
    }
  }
});

test("worker binds its result channel before loading candidate code", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-request-overlay-output-"));
  try {
    const artifactPath = path.join(root, "hostile.cjs");
    writeFileSync(artifactPath, [
      `const runtimeCatalog = ${JSON.stringify(fakeRuntimeCatalog)};`,
      `const success = ${JSON.stringify(manifest.success_contract)};`,
      `const error = ${JSON.stringify(manifest.error_probe.expected)};`,
      `const resource = ${JSON.stringify(manifest.resource_limit_probe.expected)};`,
      "class NativeEngine {",
      "  constructor(baseJson) {",
      "    JSON.parse(baseJson);",
      "    process.stdout.write = () => { throw new Error('candidate stdout'); };",
      "    process.stderr.write = () => { throw new Error('candidate stderr'); };",
      "  }",
      "  executeSync(requestJson) {",
      "    const request = JSON.parse(requestJson);",
      "    const options = request.options_json === undefined ? null : JSON.parse(request.options_json);",
      "    if (options?.runtime_policy === 'native') return JSON.stringify(error);",
      "    if (options?.resources?.limits?.max_source_bytes !== undefined &&",
      "        request.source.length > options.resources.limits.max_source_bytes) {",
      "      return JSON.stringify(resource);",
      "    }",
      "    return JSON.stringify(success);",
      "  }",
      "  runtimeCatalogJson() { return JSON.stringify(runtimeCatalog); }",
      "  dispose() {}",
      "}",
      "module.exports = { NativeEngine };",
      "",
    ].join("\n"));
    const artifact = fakeArtifacts()[0];
    const invocation = {
      ...workerInvocation(artifact, "e".repeat(32)),
      artifact_path: artifactPath,
    };
    const result = spawnSync(
      process.execPath,
      ["--expose-gc", path.join(nodeRoot, "scripts", "benchmark", "request-overlay-worker.mjs")],
      { encoding: "utf8", input: `${JSON.stringify(invocation)}\n` },
    );
    assert.equal(result.status, 0, result.stderr);
    assert.equal(JSON.parse(result.stdout).semantic_evidence.passed, true);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("RSS contract preserves current growth hidden below a historical process maximum", () => {
  const artifact = fakeArtifacts()[0];
  const result = fakeWorkerResult(artifact, "1".repeat(32));
  result.rss.baseline_current_rss_bytes = 100;
  result.rss.baseline_process_max_rss_bytes = 500;
  result.rss.baseline_history_gap_bytes = 400;
  result.rss.final_current_rss_bytes = 100;
  result.rss.final_process_max_rss_bytes = 500;
  for (const [index, lane] of result.rss.lanes.entries()) {
    lane.baseline_current_rss_bytes = index === 1 ? 350 : 100;
    lane.baseline_process_max_rss_bytes = 500;
    lane.baseline_history_gap_bytes =
      lane.baseline_process_max_rss_bytes - lane.baseline_current_rss_bytes;
    lane.peak_sampled_current_rss_bytes = index < 2 ? 400 : 100;
    lane.peak_process_max_rss_bytes = 500;
    lane.sampled_current_growth_bytes = index === 0 ? 300 : index === 1 ? 50 : 0;
    lane.process_max_growth_bytes = 0;
    lane.operation_peak_growth_bytes = lane.sampled_current_growth_bytes;
  }
  result.rss.peak_sampled_current_rss_bytes = 400;
  result.rss.peak_process_max_rss_bytes = 500;
  result.rss.sampled_current_peak_growth_bytes = 300;
  result.rss.process_max_peak_growth_bytes = 0;
  result.rss.fresh_process_envelope_growth_bytes = 300;
  validateRequestOverlayWorkerResult(result, {
    artifactKey: artifact.key,
    artifactIdentity: workerIdentity(artifact),
    manifest,
    sampling,
  });

  result.rss.fresh_process_envelope_growth_bytes += 1;
  assert.throws(
    () => validateRequestOverlayWorkerResult(result, {
      artifactKey: artifact.key,
      artifactIdentity: workerIdentity(artifact),
      manifest,
      sampling,
    }),
    /RSS aggregate evidence is invalid/,
  );
});

test("RSS contract rejects missing scale lanes and forged observations", () => {
  const artifact = fakeArtifacts()[0];
  const valid = fakeWorkerResult(artifact, "2".repeat(32));
  for (const mutate of [
    (rss) => rss.lanes.pop(),
    (rss) => {
      rss.lanes[0].observation_count += 1;
    },
    (rss) => {
      rss.lanes[0].operation_peak_growth_bytes += 1;
    },
  ]) {
    const result = structuredClone(valid);
    mutate(result.rss);
    assert.throws(
      () => validateRequestOverlayWorkerResult(result, {
        artifactKey: artifact.key,
        artifactIdentity: workerIdentity(artifact),
        manifest,
        sampling,
      }),
      /RSS/,
    );
  }
});

test("schema-2 owner report retains every process cell and cannot claim transport admission", () => {
  const artifacts = fakeArtifacts();
  const templates = new Map(
    artifacts.map((artifact, index) => [
      artifact.key,
      fakeWorkerResult(artifact, (index + 1).toString(16).padStart(32, "0")),
    ]),
  );
  const identities = [
    ...explorationCells({ aaPairs: sampling.aa_pairs }),
    ...confirmationCells({ confirmationPairs: sampling.confirmation_pairs }),
  ];
  const cells = identities.map((identity, index) => {
    const result = structuredClone(templates.get(identity.artifact_key));
    result.process.invocation_nonce = (index + 100).toString(16).padStart(32, "0");
    return { ...identity, result };
  });
  const noise = qualifyNoise(cells, manifest, sampling.maximum_confirmation_pairs);
  const decision = classifyDecision(cells, noise, manifest);
  const report = {
    schema_version: 2,
    report_kind: "merman-node-request-overlay-owner-v2",
    owner: "merman-bindings-core",
    scope: {
      lane_id: manifest.lane_id,
      operation_id: "semantic-json",
      transports: ["napi", "node-wasm"],
      timing_clock: "process.hrtime.bigint",
      timing_operation: "raw-engine.executeSync",
      transport_admission: "not-evaluated",
    },
    provenance: {
      measured_at_utc: "2026-07-29T00:00:00.000Z",
      invocation_id: "a".repeat(32),
      harness_digest: sha("a"),
      node: process.version,
      platform: process.platform,
      arch: process.arch,
      timezone: "UTC",
      hostname: "test-host",
      release: "test-release",
      cpu: "test-cpu",
      logical_cpus: 8,
      total_memory_bytes: 16_000_000_000,
      command: [
        process.execPath,
        "platforms/node/scripts/benchmark/request-overlay-run.mjs",
        "--base-napi", "base/merman.node",
        "--base-wasm", "base/merman_node.js",
        "--head-napi", "head/merman.node",
        "--head-wasm", "head/merman_node.js",
        "--output", "target/bench/request-overlay.json",
        "--aa-pairs", "8",
        "--cold-samples", "1",
        "--warmup-iterations", "1",
        "--reused-samples", "1",
      ],
    },
    revisions: {
      base: artifacts[0].commit,
      base_tree: artifacts[0].commit_tree,
      head: artifacts[2].commit,
      head_tree: artifacts[2].commit_tree,
      relationship: "head^1==base",
      verified: true,
    },
    input: { manifest_digest: digestJson(manifest), manifest },
    sampling,
    artifacts,
    semantic_evidence: projectSemanticEvidence(cells),
    cells,
    rss: projectRss(cells),
    decision,
  };
  validateRequestOverlayReport(report, { trustedManifest: manifest });
  assert.equal(report.cells.length, 96);
  assert.equal(report.rss[0].processes[0].lanes.length, 37);
  assert.equal(report.decision.status, "rejected");
  assert.equal(report.decision.transport_admission, "not-evaluated");

  const admitted = structuredClone(report);
  admitted.scope.transport_admission = "admitted";
  assert.throws(
    () => validateRequestOverlayReport(admitted, { trustedManifest: manifest }),
    /scope differs/,
  );

  const forgedDecision = structuredClone(report);
  forgedDecision.decision.status = "confirmed-improvement";
  assert.throws(
    () => validateRequestOverlayReport(forgedDecision, { trustedManifest: manifest }),
    /report decision differs/,
  );

  const reusedProcess = structuredClone(report);
  reusedProcess.cells[1].result.process.invocation_nonce =
    reusedProcess.cells[0].result.process.invocation_nonce;
  reusedProcess.semantic_evidence = projectSemanticEvidence(reusedProcess.cells);
  reusedProcess.rss = projectRss(reusedProcess.cells);
  assert.throws(
    () => validateRequestOverlayReport(reusedProcess, { trustedManifest: manifest }),
    /fresh worker process/,
  );
});

function fakeWorkerResult(artifact, nonce) {
  return runRequestOverlayWorker(
    workerInvocation(artifact, nonce),
    {
      loadEngineConstructor: () => FakeEngine,
      collectGarbage: () => {},
      now: monotonicClock(),
    },
  );
}

function monotonicClock() {
  let tick = 0n;
  return () => {
    tick += 100n;
    return tick;
  };
}

function workerInvocation(artifact, nonce) {
  return {
    schema_version: 2,
    artifact_key: artifact.key,
    revision: artifact.revision,
    transport: artifact.transport,
    artifact_path: "/unused/fake-artifact",
    artifact_identity: workerIdentity(artifact),
    invocation_nonce: nonce,
    parent_invocation_id: "a".repeat(32),
    manifest,
    sampling,
  };
}

function fakeExplorationCells(primaryEstimate) {
  return fakeCells(
    explorationCells({ aaPairs: sampling.aa_pairs }),
    primaryEstimate,
  );
}

function fakeConfirmationCells(primaryEstimate) {
  return fakeCells(
    confirmationCells({ confirmationPairs: sampling.confirmation_pairs }),
    primaryEstimate,
  );
}

function fakeCells(identities, primaryEstimate) {
  const artifacts = new Map(fakeArtifacts().map((artifact) => [artifact.key, artifact]));
  const templates = new Map(
    [...artifacts].map(([key, artifact], index) => [
      key,
      fakeWorkerResult(artifact, (index + 1).toString(16).padStart(32, "0")),
    ]),
  );
  return identities.map((identity, index) => {
    const result = structuredClone(templates.get(identity.artifact_key));
    result.process.invocation_nonce = (index + 100).toString(16).padStart(32, "0");
    setPrimaryEstimate(result, primaryEstimate(identity));
    return { ...identity, result };
  });
}

function setPrimaryEstimate(result, value) {
  const lane = result.measurements.overlays.find(
    (entry) =>
      entry.overlay_id === "version-only" && entry.engine_lifecycle === "reused",
  );
  lane.summary.per_operation_batch_ns.p50_ns = value;
}

function setRssScaleGrowth(result, valueFor) {
  result.rss.baseline_history_gap_bytes = 0;
  result.rss.fresh_process_envelope_growth_bytes = 512 * 1024;
  for (const lane of result.rss.lanes) lane.baseline_history_gap_bytes = 0;
  for (const dimension of manifest.rss_contract.dimensions) {
    for (const scale of manifest.scaling.batch_sizes) {
      const laneId = dimension.measurement === "batch"
        ? `measurement:batch:${scale}:${dimension.lifecycle}`
        : `measurement:base-size:${scale}:${dimension.lifecycle}`;
      const lane = result.rss.lanes.find((entry) => entry.lane_id === laneId);
      lane.sampled_current_growth_bytes = valueFor(dimension, scale);
    }
  }
}

class FakeEngine {
  constructor(baseJson) {
    JSON.parse(baseJson);
  }

  executeSync(requestJson) {
    const request = JSON.parse(requestJson);
    const options = request.options_json === undefined
      ? null
      : JSON.parse(request.options_json);
    if (options?.runtime_policy === "native") {
      return JSON.stringify(manifest.error_probe.expected);
    }
    if (
      options?.resources?.limits?.max_source_bytes !== undefined &&
      request.source.length > options.resources.limits.max_source_bytes
    ) {
      return JSON.stringify(manifest.resource_limit_probe.expected);
    }
    return successRaw();
  }

  runtimeCatalogJson() {
    return JSON.stringify(fakeRuntimeCatalog);
  }

  dispose() {}
}

class FirstCallOnlyCompleteEngine extends FakeEngine {
  constructor(baseJson) {
    super(baseJson);
    this.calls = 0;
  }

  executeSync(requestJson) {
    this.calls += 1;
    const raw = super.executeSync(requestJson);
    const envelope = JSON.parse(raw);
    if (this.calls > 1 && envelope.ok === true) {
      delete envelope.result.metadata_json;
      return JSON.stringify(envelope);
    }
    return raw;
  }
}

class FirstThirtyTwoCallsCompleteEngine extends FakeEngine {
  constructor(baseJson) {
    super(baseJson);
    this.calls = 0;
  }

  executeSync(requestJson) {
    this.calls += 1;
    if (this.calls <= 32) return super.executeSync(requestJson);
    return JSON.stringify(manifest.error_probe.expected);
  }
}

class FirstThirtyTwoCallsStableWireEngine extends FakeEngine {
  constructor(baseJson) {
    super(baseJson);
    this.calls = 0;
  }

  executeSync(requestJson) {
    this.calls += 1;
    const raw = super.executeSync(requestJson);
    if (this.calls <= 32) return raw;
    const envelope = JSON.parse(raw);
    if (envelope.ok !== true) return raw;
    return JSON.stringify({ result: envelope.result, ok: envelope.ok, version: envelope.version });
  }
}

function successRaw() {
  return JSON.stringify(manifest.success_contract);
}

function fakeArtifacts() {
  const baseCommit = "1".repeat(40);
  const headCommit = "2".repeat(40);
  return artifactKeys().map((key, index) => {
    const [revision, transport] = key.split(":");
    const side = revision === "base" ? "a" : "b";
    return {
      key,
      revision,
      transport,
      commit: revision === "base" ? baseCommit : headCommit,
      commit_tree: revision === "base" ? "3".repeat(40) : "4".repeat(40),
      target: transport === "napi" ? "darwin-arm64" : null,
      rust_target: transport === "napi" ? "aarch64-apple-darwin" : "wasm32-unknown-unknown",
      wasm_pack_target: transport === "napi" ? null : "nodejs",
      cargo_features: ["svg", `transport-${transport === "napi" ? "napi" : "wasm"}`],
      capability_ids: ["svg"],
      build_tools: {
        cargo: "cargo 1.95.0",
        node: process.version,
        rustc: "rustc 1.95.0",
        transport_builder: "test-builder 1",
      },
      artifact_path_in_receipt: transport === "napi" ? "merman.node" : "merman_node.js",
      artifact_bytes: 100 + index,
      artifact_sha256: sha((index + 1).toString(16)),
      receipt_digest: sha((index + 5).toString(16)),
      source_digest: sha(side),
      cargo_lock_digest: sha(side),
      binding_contract_digest: sha(side),
      build_environment_digest: sha("e"),
      dependency_closure_digest: sha(side),
      capability_recipe_digest: sha(side),
      input_digest: sha(side),
      runtime_catalog_digest: fakeRuntimeCatalogDigest,
    };
  });
}

function workerIdentity(artifact) {
  const { key: _key, ...identity } = artifact;
  return identity;
}

function writeHistoricalReceipt(root, transport) {
  const artifactName = transport === "napi" ? "merman.node" : "merman_node.js";
  const artifact = path.join(root, artifactName);
  writeFileSync(artifact, "artifact");
  const config = {
    candidate: transport,
    target: transport === "napi" ? "darwin-arm64" : null,
    rust_target: transport === "napi" ? "aarch64-apple-darwin" : "wasm32-unknown-unknown",
    wasm_pack_target: transport === "napi" ? null : "nodejs",
    default_features: false,
    capability_recipe: {
      descriptor: "capabilities/feature-surface-v1.json",
      target: "native",
      capabilities: ["svg"],
    },
    features: ["svg", `transport-${transport === "napi" ? "napi" : "wasm"}`],
  };
  const dependencyClosure = {
    packages: [
      {
        name: "merman-bindings-core",
        version: "0.8.0-alpha.4",
        source: "path:crates/merman-bindings-core",
      },
      {
        name: "merman-core",
        version: "0.8.0-alpha.4",
        source: "path:crates/merman-core",
      },
      {
        name: "merman-node-candidate",
        version: "0.8.0-alpha.4",
        source: "path:crates/merman-node",
      },
      ...(transport === "napi"
        ? [
            { name: "napi", version: "3.11.0", source: "registry+test" },
            { name: "napi-build", version: "2.3.2", source: "registry+test" },
            { name: "napi-derive", version: "3.6.0", source: "registry+test" },
          ]
        : [
            {
              name: "serde-wasm-bindgen",
              version: "0.6.5",
              source: "registry+test",
            },
            { name: "wasm-bindgen", version: "0.2.100", source: "registry+test" },
          ]),
    ],
  };
  dependencyClosure.digest = digestJson(dependencyClosure.packages);
  const tools = {
    cargo: "cargo 1.95.0",
    node: process.version,
    rustc: "rustc 1.95.0",
    transport_builder: "test-builder 1",
  };
  const runtimeCatalog = {
    schema_version: 1,
    transport_api_version: 1,
    package_version: "0.8.0-alpha.4",
    capabilities: {
      capability_ids: ["svg"],
      output_ids: ["svg"],
      operation_ids: ["semantic-json", "svg"],
      system_adapter_ids: [],
    },
  };
  const receipt = {
    schema_version: 4,
    config,
    commit: gitCapture(["rev-parse", "--verify", "HEAD^{commit}"]),
    commit_tree: gitCapture(["rev-parse", "--verify", "HEAD^{tree}"]),
    source_inputs: committedSourceClosure(),
    source_digest: null,
    cargo_lock_digest: null,
    binding_contract_digest: null,
    build_environment: resolveCandidateBuildEnvironment().contract,
    build_environment_digest: null,
    dependency_closure: dependencyClosure,
    runtime: {
      catalog_digest: digestJson(runtimeCatalog),
      catalog: runtimeCatalog,
      probe: {
        request_options_limit_code_name: "MERMAN_RESOURCE_LIMIT_EXCEEDED",
        unknown_operation_kind: "unknown-operation",
      },
    },
    tools,
    artifacts: [{
      path: artifactName,
      bytes: Buffer.byteLength("artifact"),
      sha256: digestBytes("artifact"),
    }],
  };
  receipt.source_digest = digestJson(receipt.source_inputs);
  receipt.cargo_lock_digest = receipt.source_inputs.find(
    (entry) => entry.path === "crates/merman-node/Cargo.lock",
  ).sha256;
  receipt.binding_contract_digest = digestJson(
    receipt.source_inputs.filter(
      (entry) =>
        !entry.path.startsWith("crates/merman-node/") &&
        !entry.path.startsWith("platforms/node/"),
    ),
  );
  receipt.build_environment_digest = digestJson(receipt.build_environment);
  receipt.input_digest = computeBuildReceiptInputDigest(receipt);
  writeFileSync(path.join(root, "build-receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`);
  return artifact;
}

function committedSourceInput(relativePath) {
  const bytes = gitBytes(["show", `HEAD:${relativePath}`]);
  return {
    path: relativePath,
    bytes: bytes.length,
    sha256: digestBytes(bytes),
  };
}

function committedSourceClosure() {
  return ["Cargo.toml", "crates/merman-node/Cargo.lock"].map(committedSourceInput);
}

function gitCapture(args) {
  return gitBytes(args).toString("utf8").trim();
}

function gitBytes(args) {
  const result = spawnSync("git", args, {
    cwd: repositoryRoot,
    encoding: null,
  });
  if (result.error || result.status !== 0) {
    throw new Error(`git ${args.join(" ")} failed: ${result.error?.message ?? result.stderr}`);
  }
  return result.stdout;
}

function digestBytes(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}

function sha(character) {
  return `sha256:${character.repeat(64)}`;
}
