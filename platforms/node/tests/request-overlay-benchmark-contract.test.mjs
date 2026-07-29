import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  artifactKeys,
  classifyDecision,
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
  requestOverlayExitCode,
  runRequestOverlayComparison,
  verifyAdjacentRevisions,
  verifySameRevisionContracts,
} from "../scripts/benchmark/request-overlay-run.mjs";
import {
  collectGarbageTwice,
  measureColdSample,
  measureReusedSamples,
  runRequestOverlayWorker,
} from "../scripts/benchmark/request-overlay-worker.mjs";
import { digestJson } from "../scripts/stable-json.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
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
  assert.ok(first.log_ratio.upper < 0);
  assert.ok(first.absolute_ns.upper < 0);
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
    const value = readHistoricalRequestOverlayReceipt(artifact, {
      revision: "base",
      transport: "napi",
    });
    assert.equal(value.key, "base:napi");
    assert.equal(value.artifact_path_in_receipt, "merman.node");
    assert.equal(projectHistoricalArtifact(value).artifact_path, undefined);

    writeFileSync(artifact, "tampered");
    assert.throws(
      () => readHistoricalRequestOverlayReceipt(artifact, {
        revision: "base",
        transport: "napi",
      }),
      /receipt is stale/,
    );
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
    result.rss.operation_peak_growth_bytes,
    Math.max(0, result.rss.peak_rss_bytes - result.rss.baseline_peak_rss_bytes),
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

test("RSS contract rejects attributing pre-operation historical peak to the operation", () => {
  const artifact = fakeArtifacts()[0];
  const result = fakeWorkerResult(artifact, "1".repeat(32));
  result.rss = {
    method: "process.memoryUsage.rss/process.resourceUsage.maxRSS",
    baseline_current_rss_bytes: 100,
    baseline_peak_rss_bytes: 200,
    final_current_rss_bytes: 150,
    peak_rss_bytes: 300,
    operation_peak_growth_bytes: 200,
  };
  assert.throws(
    () => validateRequestOverlayWorkerResult(result, {
      artifactKey: artifact.key,
      artifactIdentity: workerIdentity(artifact),
      manifest,
      sampling,
    }),
    /RSS evidence is invalid/,
  );
});

test("schema-1 owner report retains every process cell and cannot claim transport admission", () => {
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
    schema_version: 1,
    report_kind: "merman-node-request-overlay-owner-v1",
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
      head: artifacts[2].commit,
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
  let tick = 0n;
  return runRequestOverlayWorker(
    {
      schema_version: 1,
      artifact_key: artifact.key,
      revision: artifact.revision,
      transport: artifact.transport,
      artifact_path: "/unused/fake-artifact",
      artifact_identity: workerIdentity(artifact),
      invocation_nonce: nonce,
      parent_invocation_id: "a".repeat(32),
      manifest,
      sampling,
    },
    {
      loadEngineConstructor: () => FakeEngine,
      collectGarbage: () => {},
      now: () => {
        tick += 100n;
        return tick;
      },
    },
  );
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
    return options?.runtime_policy === "native"
      ? JSON.stringify(manifest.error_probe.expected)
      : successRaw();
  }

  runtimeCatalogJson() {
    return JSON.stringify(fakeRuntimeCatalog);
  }

  dispose() {}
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
    packages: [{ name: "merman", version: "0.8.0-alpha.4", source: "path:crates/merman" }],
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
    schema_version: 1,
    config,
    commit: "1".repeat(40),
    source_digest: sha("1"),
    cargo_lock_digest: sha("2"),
    binding_contract_digest: sha("3"),
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
  receipt.input_digest = digestJson({
    cargo_lock_digest: receipt.cargo_lock_digest,
    config,
    dependency_closure_digest: dependencyClosure.digest,
    source_digest: receipt.source_digest,
    tools,
  });
  writeFileSync(path.join(root, "build-receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`);
  return artifact;
}

function digestBytes(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}

function sha(character) {
  return `sha256:${character.repeat(64)}`;
}
