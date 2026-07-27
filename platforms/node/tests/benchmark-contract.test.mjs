import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import { readBuildReceipt } from "../scripts/benchmark/build-receipt.mjs";
import { stageWasmPackage } from "../scripts/benchmark/footprint.mjs";
import {
  computeCorpusDigest,
  computeInputDigest,
  computeWorkloadComparison,
  validateComparisonReport as validateComparisonReportContract,
} from "../scripts/benchmark/report-contract.mjs";
import { loadCorpus } from "../scripts/benchmark/corpus.mjs";
import { collectHarnessInputFiles } from "../scripts/benchmark/harness-inputs.mjs";
import {
  assertHarnessUnchanged,
  buildCandidateWorkerInputs,
  projectSvgOutcome,
  projectFootprint,
} from "../scripts/benchmark/run.mjs";
import { summarize } from "../scripts/benchmark/stats.mjs";
import { svgTransportEvidence } from "../scripts/benchmark/svg-signature.mjs";
import { digestJson } from "../scripts/stable-json.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");

const provenance = {
  measured_at_utc: "2026-07-23T12:00:00.000Z",
  timezone: "UTC",
  harness_digest: `sha256:${"b".repeat(64)}`,
  machine: {
    hostname: "benchmark-host",
    os: "linux",
    release: "6.12",
    arch: "x64",
    cpu: "Example CPU",
    logical_cpus: 8,
    total_memory_bytes: 16_000_000_000,
  },
  tools: {
    node: "v22.0.0",
    npm: "11.0.0",
    rustc: "rustc 1.95.0",
    cargo: "cargo 1.95.0",
    napi: "3.11.0",
    napi_derive: "3.6.0",
    napi_build: "2.3.2",
    napi_cli: "3.7.4",
  },
  commit: "0123456789abcdef0123456789abcdef01234567",
};
const WORKLOADS = {
  cold_svg: {
    operation_id: "svg",
    path: "@benchmark/cold-flowchart-smoke.mmd",
    source: "flowchart TD\nA-->B",
  },
  concurrency_svg: {
    operation_id: "svg",
    path: "@benchmark/concurrency-flowchart-smoke.mmd",
    source: "flowchart TD\nA-->B",
  },
};
const CORPUS_CASES = [
  { path: "a.mmd", source: "flowchart TD\nA-->B" },
  { path: "b.mmd", source: "invalid" },
];
const BINDING_OPTIONS = {
  version: 1,
  runtime_policy: "deterministic",
  resources: { profile: "trusted-native" },
};
const OPERATION_OPTIONS = { version: 1 };
const CORPUS_DIGEST = computeCorpusDigest(CORPUS_CASES);
const COMPARISON_INPUT_DIGEST = computeInputDigest({
  corpusDigest: CORPUS_DIGEST,
  bindingOptions: BINDING_OPTIONS,
  operationOptions: OPERATION_OPTIONS,
  workloads: WORKLOADS,
});
const TRUSTED_TEST_CORPUS = {
  cases: CORPUS_CASES,
  bindingOptions: BINDING_OPTIONS,
  operationOptions: OPERATION_OPTIONS,
  workloads: WORKLOADS,
  corpusDigest: CORPUS_DIGEST,
  digest: COMPARISON_INPUT_DIGEST,
  manifestPath: path.join(repositoryRoot, "fixtures", "**", "*.mmd"),
};
const REPRESENTATIVE_SVG = '<svg viewBox="0 0 1 1"/>';
const REPRESENTATIVE_SVG_SHA =
  `sha256:${createHash("sha256").update(REPRESENTATIVE_SVG).digest("hex")}`;
const REPRESENTATIVE_SVG_EVIDENCE = svgTransportEvidence(REPRESENTATIVE_SVG);
const SAMPLING = {
  cold_processes: 2,
  warmup_iterations: 1,
  measured_iterations: 2,
  concurrency_iterations: 2,
};

function validateComparisonReport(report) {
  return validateComparisonReportContract(report, {
    trustedCorpus: TRUSTED_TEST_CORPUS,
  });
}
const CAPABILITY_RECIPE = {
  default_features: false,
  capability_recipe: {
    descriptor: "capabilities/feature-surface-v1.json",
    target: "native",
    capabilities: ["layout-cytoscape", "layout-elk", "math", "svg"],
  },
};
const CAPABILITY_RECIPE_DIGEST = digestJson(CAPABILITY_RECIPE);
const RUNTIME_CATALOG = {
  schema_version: 1,
  transport_api_version: 1,
  package_version: "0.8.0-alpha.3",
  capabilities: {
    capability_ids: ["layout-cytoscape", "layout-elk", "math", "svg"],
    output_ids: ["svg"],
    operation_ids: ["layout-json", "semantic-json", "svg", "svg-plan-json"],
    system_adapter_ids: [],
    text_measurement: {
      protocol_version: 1,
      provider_ids: ["vendored"],
    },
  },
  registry: { diagram_family_count: 35 },
  resources: {
    general_binding_default_profile: "interactive",
    cli_default_profile: "trusted-native",
    limits: [{
      id: "max_source_bytes",
      phase: "source",
      description: "Maximum source bytes.",
      overridable: true,
      hard_cap: false,
    }],
    profiles: [
      {
        id: "interactive",
        purpose: "Interactive rendering.",
        trust_assumption: "Cooperative input.",
        recommended_binding_default: true,
        limits: { max_source_bytes: 1024 },
      },
      {
        id: "trusted-native",
        purpose: "Trusted rendering.",
        trust_assumption: "Trusted input.",
        recommended_binding_default: false,
        limits: { max_source_bytes: null },
      },
    ],
  },
};
const RUNTIME_CATALOG_DIGEST = digestJson(RUNTIME_CATALOG);
const TARGETS = {
  "darwin-arm64": {
    platform: "darwin",
    arch: "arm64",
    libc: null,
    rustTarget: "aarch64-apple-darwin",
    packageName: "@mermanjs/node-darwin-arm64",
  },
  "darwin-x64": {
    platform: "darwin",
    arch: "x64",
    libc: null,
    rustTarget: "x86_64-apple-darwin",
    packageName: "@mermanjs/node-darwin-x64",
  },
  "linux-x64-gnu": {
    platform: "linux",
    arch: "x64",
    libc: "gnu",
    rustTarget: "x86_64-unknown-linux-gnu",
    packageName: "@mermanjs/node-linux-x64-gnu",
  },
  "linux-x64-musl": {
    platform: "linux",
    arch: "x64",
    libc: "musl",
    rustTarget: "x86_64-unknown-linux-musl",
    packageName: "@mermanjs/node-linux-x64-musl",
  },
  "win32-x64-msvc": {
    platform: "win32",
    arch: "x64",
    libc: null,
    rustTarget: "x86_64-pc-windows-msvc",
    packageName: "@mermanjs/node-win32-x64-msvc",
  },
};
const INITIAL_TARGETS = Object.keys(TARGETS);
const PACKAGE_VERSION = "0.8.0-alpha.3";
const RUNTIME_EVIDENCE = {
  catalog_digest: RUNTIME_CATALOG_DIGEST,
  catalog: RUNTIME_CATALOG,
  probe: {
    missing_capability_id: "png",
    semantic_json_bytes: 120,
    svg_plan_json_bytes: 120,
    svg_bytes: 120,
    svg_structure_sha256: `sha256:${"7".repeat(64)}`,
    svg_geometry_sha256: `sha256:${"8".repeat(64)}`,
    unknown_operation_kind: "unknown-operation",
    request_options_limit_code_name: "MERMAN_RESOURCE_LIMIT_EXCEEDED",
  },
};

function queueLifecycleEvidence() {
  const fulfilled = () => ({ status: "fulfilled" });
  const rejected = (error) => ({ status: "rejected", error });
  return {
    saturation_passed: true,
    dispose_passed: true,
    queued_abort_passed: true,
    non_preemptive_abort_passed: true,
    process_shutdown_passed: true,
    evidence: {
      saturation: {
        active: fulfilled(),
        queued: fulfilled(),
        saturated: rejected({ code: "MERMAN_QUEUE_SATURATED" }),
        dispose: fulfilled(),
      },
      disposal: {
        active: fulfilled(),
        queued: rejected({ code: "MERMAN_ENGINE_DISPOSED" }),
        dispose: fulfilled(),
      },
      abort: {
        executing: fulfilled(),
        queued: rejected({ name: "AbortError" }),
        dispose: fulfilled(),
      },
      shutdown: { render_succeeded: true, dispose_called: false },
    },
  };
}

function timedSuccess(path) {
  return {
    path,
    ok: true,
    operation_id: "svg",
    media_type: "image/svg+xml",
    sha256: REPRESENTATIVE_SVG_SHA,
    svg_structure_sha256: REPRESENTATIVE_SVG_EVIDENCE.structure_sha256,
    svg_geometry_sha256: REPRESENTATIVE_SVG_EVIDENCE.geometry_sha256,
    bytes: Buffer.byteLength(REPRESENTATIVE_SVG),
  };
}

function timedSuccessForSvg(path, rawSvg) {
  const evidence = svgTransportEvidence(rawSvg);
  return {
    path,
    ok: true,
    operation_id: "svg",
    media_type: "image/svg+xml",
    sha256: `sha256:${createHash("sha256").update(rawSvg).digest("hex")}`,
    svg_structure_sha256: evidence.structure_sha256,
    svg_geometry_sha256: evidence.geometry_sha256,
    bytes: Buffer.byteLength(rawSvg),
  };
}

function timedFailure(path) {
  return {
    path,
    ok: false,
    kind: "parse-error",
    code_name: null,
    capability_id: null,
  };
}

function workloadRepresentative(workload) {
  return {
    source_sha256:
      `sha256:${createHash("sha256").update(workload.source).digest("hex")}`,
    raw_svg: REPRESENTATIVE_SVG,
  };
}

test("benchmark provenance binds every runtime and package assembly input", () => {
  const inputs = collectHarnessInputFiles();
  for (const expected of [
    "package-lock.json",
    "package-surfaces.json",
    "benchmark/corpus.json",
    "packages/node/package.json",
    "scripts/assemble-packages.mjs",
    "scripts/benchmark/footprint.mjs",
    "scripts/benchmark/run.mjs",
    "src/engine.mjs",
    "src/candidates/native.mjs",
  ]) {
    assert.equal(inputs.some((file) => file.endsWith(expected)), true, expected);
  }
});

test("benchmark measurement rejects harness drift", () => {
  const initial = `sha256:${"1".repeat(64)}`;
  assert.doesNotThrow(() => assertHarnessUnchanged(initial, initial));
  assert.throws(
    () => assertHarnessUnchanged(initial, `sha256:${"2".repeat(64)}`),
    /changed during measurement/i,
  );
});

test("benchmark corpus rejects the superseded schema 1 manifest", (context) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-node-corpus-schema-"));
  context.after(() => rmSync(root, { recursive: true, force: true }));
  const manifest = path.join(root, "corpus.json");
  writeFileSync(manifest, JSON.stringify({ schema_version: 1, roots: [] }));
  assert.throws(() => loadCorpus(manifest), /schema_version must be 2/i);
});

test("process shutdown probe uses a stable valid smoke diagram", (context) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-node-shutdown-probe-"));
  context.after(() => rmSync(root, { recursive: true, force: true }));
  const productModule = path.join(root, "product.mjs");
  writeFileSync(
    productModule,
    [
      "export async function createNodeEngine() {",
      "  return {",
      "    async executeOperation({ source }) {",
      "      if (source.startsWith('architecture-beta')) {",
      "        const error = new Error('expected architecture-beta header');",
      "        error.kind = 'parse-error';",
      "        throw error;",
      "      }",
      `      return { operation_id: 'svg', media_type: 'image/svg+xml', data: '${REPRESENTATIVE_SVG}' };`,
      "    },",
      "    async dispose() {},",
      "  };",
      "}",
      "",
    ].join("\n"),
  );
  const input = path.join(root, "input.json");
  writeFileSync(
    input,
    JSON.stringify({
      mode: "shutdown",
      candidate: "node-wasm",
      productModule: pathToFileURL(productModule).href,
      bindingOptions: {
        version: 1,
        runtime_policy: "deterministic",
        resources: { profile: "trusted-native" },
      },
      operationOptions: { version: 1 },
    }),
  );
  const result = spawnSync(
    process.execPath,
    [path.join(nodeRoot, "scripts", "benchmark", "worker.mjs"), input],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), {
    process_shutdown_passed: true,
    evidence: { render_succeeded: true, dispose_called: false },
  });
});

test("cold latency uses the declared successful workload, not the leading corpus error", (context) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-node-workload-probe-"));
  context.after(() => rmSync(root, { recursive: true, force: true }));
  const productModule = path.join(root, "product.mjs");
  writeFileSync(
    productModule,
    [
      "export async function createNodeEngine() {",
      "  return {",
      "    async executeOperation({ source }) {",
      "      if (source.startsWith('invalid')) {",
      "        const error = new Error('intentional corpus error');",
      "        error.kind = 'parse-error';",
      "        throw error;",
      "      }",
      "      if (source.startsWith('not svg')) {",
      "        return { operation_id: 'svg', media_type: 'image/svg+xml', data: 'plain text' };",
      "      }",
      `      return { operation_id: 'svg', media_type: 'image/svg+xml', data: '${REPRESENTATIVE_SVG}' };`,
      "    },",
      "    async dispose() {},",
      "  };",
      "}",
      "",
    ].join("\n"),
  );
  const input = path.join(root, "cold.json");
  writeFileSync(
    input,
    JSON.stringify({
      mode: "cold",
      candidate: "node-wasm",
      productModule: pathToFileURL(productModule).href,
      bindingOptions: {
        version: 1,
        runtime_policy: "deterministic",
        resources: { profile: "trusted-native" },
      },
      operationOptions: { version: 1 },
      workload: WORKLOADS.cold_svg,
    }),
  );
  const result = spawnSync(
    process.execPath,
    [path.join(nodeRoot, "scripts", "benchmark", "worker.mjs"), input],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
  const rawResult = JSON.parse(result.stdout).result;
  assert.equal(rawResult.path, WORKLOADS.cold_svg.path);
  assert.equal(rawResult.data, REPRESENTATIVE_SVG);
  assert.deepEqual(projectSvgOutcome(rawResult), timedSuccess(WORKLOADS.cold_svg.path));

  const nonSvgInput = path.join(root, "non-svg.json");
  writeFileSync(
    nonSvgInput,
    JSON.stringify({
      ...JSON.parse(readFileSync(input, "utf8")),
      workload: { ...WORKLOADS.cold_svg, source: "not svg" },
    }),
  );
  const nonSvgResult = spawnSync(
    process.execPath,
    [path.join(nodeRoot, "scripts", "benchmark", "worker.mjs"), nonSvgInput],
    { encoding: "utf8" },
  );
  assert.equal(nonSvgResult.status, 0, nonSvgResult.stderr);
  assert.throws(
    () => projectSvgOutcome(JSON.parse(nonSvgResult.stdout).result),
    /Cannot inspect rendered SVG/i,
  );
});

test("cold and concurrency workers receive no corpus payload", () => {
  const inputs = buildCandidateWorkerInputs({
    candidate: "node-wasm",
    productModule: "file:///candidate.mjs",
    corpus: TRUSTED_TEST_CORPUS,
    options: {
      iterations: 3,
      warmupIterations: 1,
      concurrencyIterations: 5,
      concurrency: 4,
      maxQueue: 64,
    },
  });
  assert.equal(inputs.warm.cases, CORPUS_CASES);
  for (const lane of [inputs.cold, inputs.concurrency, inputs.shutdown]) {
    assert.equal("cases" in lane, false);
    assert.equal("workloads" in lane, false);
  }
});

function candidate(id) {
  const target = id === "napi" ? "linux-x64-gnu" : null;
  const buildReceipt = buildReceiptFor(id, target);
  const value = {
    id,
    input_digest: COMPARISON_INPUT_DIGEST,
    build_receipt: buildReceipt,
    corpus: {
      cases: 2,
      matched: 2,
      mismatched: 0,
      geometry_svg_mismatches: 0,
      raw_svg_byte_mismatches: 0,
      mismatch_paths: [],
      geometry_mismatch_paths: [],
      successful: 1,
      failed: 1,
      outcomes: [
        {
          ...timedSuccess("a.mmd"),
          semantic: {
            ok: true,
            operation_id: "semantic-json",
            media_type: "application/json",
            sha256: `sha256:${"4".repeat(64)}`,
          },
        },
        {
          path: "b.mmd",
          ok: false,
          kind: "parse-error",
          code_name: null,
          capability_id: null,
          semantic: { ok: false, kind: "parse-error" },
        },
      ],
    },
    cold_process: {
      isolated_processes: true,
      workload_id: "cold_svg",
      timing_scope: "parent-dispatch-through-worker-raw-svg-result",
      operation_timing_scope:
        "worker-engine-init-through-first-svg-operation-result",
      evidence_excluded: true,
      representative: workloadRepresentative(WORKLOADS.cold_svg),
      samples_ms: [10, 11],
      samples: [
        {
          elapsed_ms: 10,
          operation_ms: 8,
          baseline_rss_bytes: 400,
          peak_rss_bytes: 900,
          outcome: timedSuccess(WORKLOADS.cold_svg.path),
        },
        {
          elapsed_ms: 11,
          operation_ms: 9,
          baseline_rss_bytes: 410,
          peak_rss_bytes: 910,
          outcome: timedSuccess(WORKLOADS.cold_svg.path),
        },
      ],
    },
    warm_latency: {
      samples_ms: [1, 2, 1.5, 2.5],
      samples: [
        {
          iteration: 0,
          path: "a.mmd",
          elapsed_ms: 1,
          outcome: timedSuccess("a.mmd"),
        },
        {
          iteration: 0,
          path: "b.mmd",
          elapsed_ms: 2,
          outcome: timedFailure("b.mmd"),
        },
        {
          iteration: 1,
          path: "a.mmd",
          elapsed_ms: 1.5,
          outcome: timedSuccess("a.mmd"),
        },
        {
          iteration: 1,
          path: "b.mmd",
          elapsed_ms: 2.5,
          outcome: timedFailure("b.mmd"),
        },
      ],
      successful_svg: {
        samples_ms: [1, 1.5],
      },
    },
    rss: {
      method: "process.resourceUsage.maxRSS",
      peak_bytes: 1000,
      baseline_bytes: 500,
    },
    footprint: footprintFor(id, buildReceipt, target ?? "linux-x64-gnu"),
    queue_lifecycle: queueLifecycleEvidence(),
    concurrency: {
      workload_id: "concurrency_svg",
      timing_scope: "warmed-engine-raw-svg-operation-batch",
      evidence_excluded: true,
      representative: workloadRepresentative(WORKLOADS.concurrency_svg),
      workers: 4,
      requests_per_batch: 4,
      batch_samples_ms: [4, 5],
      samples: [
        {
          iteration: 0,
          elapsed_ms: 4,
          outcomes: [
            timedSuccess(WORKLOADS.concurrency_svg.path),
            timedSuccess(WORKLOADS.concurrency_svg.path),
            timedSuccess(WORKLOADS.concurrency_svg.path),
            timedSuccess(WORKLOADS.concurrency_svg.path),
          ],
        },
        {
          iteration: 1,
          elapsed_ms: 5,
          outcomes: [
            timedSuccess(WORKLOADS.concurrency_svg.path),
            timedSuccess(WORKLOADS.concurrency_svg.path),
            timedSuccess(WORKLOADS.concurrency_svg.path),
            timedSuccess(WORKLOADS.concurrency_svg.path),
          ],
        },
      ],
    },
    error_behavior: {
      unknown_operation: { kind: "unknown-operation", capability_id: null },
      missing_capability: { kind: "missing-capability", capability_id: "png" },
      text_measurement_callback_rejected: true,
    },
    target_results: [],
  };
  value.corpus.results_digest = digestJson(value.corpus.outcomes);
  value.cold_process.summary = summarize(value.cold_process.samples_ms);
  value.warm_latency.summary = summarize(value.warm_latency.samples_ms);
  value.warm_latency.successful_svg.summary = summarize(
    value.warm_latency.successful_svg.samples_ms,
  );
  value.concurrency.summary = summarize(value.concurrency.batch_samples_ms);
  return value;
}

function buildReceiptFor(id, target) {
  const receiptTarget = id === "napi" ? target : null;
  const runtimeArtifacts = id === "napi"
    ? [{ path: "merman.node", bytes: 100, sha256: `sha256:${"f".repeat(64)}` }]
    : [
        { path: "merman_node.js", bytes: 100, sha256: `sha256:${"f".repeat(64)}` },
        { path: "merman_node_bg.wasm", bytes: 200, sha256: `sha256:${"9".repeat(64)}` },
        { path: "package.json", bytes: 20, sha256: `sha256:${"a".repeat(64)}` },
      ];
  return {
    receipt_digest: digestJson({ candidate: id, target: receiptTarget, kind: "receipt" }),
    candidate: id,
    target: receiptTarget,
    rust_target: id === "napi" ? TARGETS[target].rustTarget : "wasm32-unknown-unknown",
    wasm_pack_target: id === "node-wasm" ? "nodejs" : null,
    commit: provenance.commit,
    source_digest: `sha256:${"d".repeat(64)}`,
    cargo_lock_digest: `sha256:${"5".repeat(64)}`,
    binding_contract_digest: `sha256:${"e".repeat(64)}`,
    dependency_closure_digest: `sha256:${"6".repeat(64)}`,
    capability_recipe_digest: CAPABILITY_RECIPE_DIGEST,
    runtime_catalog_digest: RUNTIME_CATALOG_DIGEST,
    input_digest: digestJson({ candidate: id, target: receiptTarget, kind: "input" }),
    artifact_digest: `sha256:${"f".repeat(64)}`,
    runtime_artifacts: runtimeArtifacts,
  };
}

function footprintFor(id, buildReceipt, target) {
  const runtimeProbe = {
    runtime_catalog_digest: RUNTIME_CATALOG_DIGEST,
    semantic_operation: {
      operation_id: "semantic-json",
      media_type: "application/json",
      result_digest: `sha256:${"4".repeat(64)}`,
      bytes: 80,
    },
    svg_plan_operation: {
      operation_id: "svg-plan-json",
      media_type: "application/json",
      result_digest: `sha256:${"0".repeat(64)}`,
      planned_operation_id: "svg",
      ready: true,
      bytes: 96,
    },
    svg_operation: {
      operation_id: "svg",
      media_type: "image/svg+xml",
      output_digest: `sha256:${"1".repeat(64)}`,
      structure_sha256: `sha256:${"2".repeat(64)}`,
      geometry_sha256: `sha256:${"3".repeat(64)}`,
      bytes: 120,
    },
    request_options_error: {
      code_name: "MERMAN_RESOURCE_LIMIT_EXCEEDED",
      kind: "resource-limit",
      capability_id: null,
    },
  };
  if (id === "napi") {
    const contract = TARGETS[target];
    const rootManifest = {
      name: "@mermanjs/node",
      version: PACKAGE_VERSION,
      optionalDependencies: { [contract.packageName]: PACKAGE_VERSION },
    };
    const targetManifest = {
      name: contract.packageName,
      version: PACKAGE_VERSION,
      main: "./merman.node",
      os: [contract.platform],
      cpu: [contract.arch],
      ...(contract.libc === null
        ? {}
        : { libc: [contract.libc === "gnu" ? "glibc" : contract.libc] }),
    };
    const productEntrypoint = "@mermanjs/node/dist/index.mjs";
    const artifactPath = `${contract.packageName}/merman.node`;
    return {
      packed_bytes: 200,
      unpacked_bytes: 400,
      installed_bytes: 120,
      package_count: 2,
      runtime_api_passed: true,
      runtime_catalog_passed: true,
      generic_operation_passed: true,
      svg_plan_operation_passed: true,
      svg_operation_passed: true,
      request_options_passed: true,
      browser_fallback_absent: true,
      optional_platform_package_passed: true,
      install_method: "root-optional-dependency",
      target_install_passed: true,
      packages: [
        packedPackage("@mermanjs/node", "node.tgz"),
        packedPackage(contract.packageName, "target.tgz"),
      ],
      installed_files: [
        { path: productEntrypoint, bytes: 20 },
        { path: artifactPath, bytes: 100 },
      ],
      installation_evidence: {
        root_package: { name: "@mermanjs/node", version: PACKAGE_VERSION, manifest: rootManifest },
        target_package: { name: contract.packageName, version: PACKAGE_VERSION, manifest: targetManifest },
        product_entrypoint: productEntrypoint,
        loaded_artifacts: [{ path: artifactPath, bytes: 100, sha256: buildReceipt.artifact_digest }],
        install_manifest: {
          name: "merman-node-footprint-probe",
          private: true,
          version: "0.0.0",
          dependencies: { "@mermanjs/node": "file:../tarballs/node.tgz" },
          overrides: { [contract.packageName]: "file:../tarballs/target.tgz" },
        },
        package_lock: {
          lockfileVersion: 3,
          packages: {
            "": { dependencies: { "@mermanjs/node": "file:../tarballs/node.tgz" } },
            "node_modules/@mermanjs/node": {
              version: PACKAGE_VERSION,
              resolved: "file:../tarballs/node.tgz",
              optionalDependencies: { [contract.packageName]: PACKAGE_VERSION },
            },
            [`node_modules/${contract.packageName}`]: {
              version: PACKAGE_VERSION,
              resolved: "file:../tarballs/target.tgz",
              optional: true,
            },
          },
        },
      },
      runtime_probe: runtimeProbe,
    };
  }

  const productEntrypoint = "@mermanjs/node-wasm-candidate/index.mjs";
  const loaderPath = "@mermanjs/node-wasm-candidate/artifact/merman_node.js";
  const wasmPath = "@mermanjs/node-wasm-candidate/artifact/merman_node_bg.wasm";
  const artifactManifestPath = "@mermanjs/node-wasm-candidate/artifact/package.json";
  return {
    packed_bytes: 100,
    unpacked_bytes: 200,
    installed_bytes: 340,
    package_count: 1,
    runtime_api_passed: true,
    runtime_catalog_passed: true,
    generic_operation_passed: true,
    svg_plan_operation_passed: true,
    svg_operation_passed: true,
    request_options_passed: true,
    browser_fallback_absent: true,
    optional_platform_package_passed: null,
    install_method: "single-package",
    target_install_passed: true,
    packages: [packedPackage(WASM_PACKAGE_NAME_FOR_TEST, "wasm.tgz")],
    installed_files: [
      { path: productEntrypoint, bytes: 20 },
      { path: loaderPath, bytes: 100 },
      { path: wasmPath, bytes: 200 },
      { path: artifactManifestPath, bytes: 20 },
    ],
    installation_evidence: {
      root_package: {
        name: WASM_PACKAGE_NAME_FOR_TEST,
        version: PACKAGE_VERSION,
        manifest: { name: WASM_PACKAGE_NAME_FOR_TEST, version: PACKAGE_VERSION },
      },
      target_package: null,
      product_entrypoint: productEntrypoint,
      loaded_artifacts: [
        { path: loaderPath, bytes: 100, sha256: buildReceipt.artifact_digest },
        { path: wasmPath, bytes: 200, sha256: `sha256:${"9".repeat(64)}` },
        { path: artifactManifestPath, bytes: 20, sha256: `sha256:${"a".repeat(64)}` },
      ],
      install_manifest: {
        name: "merman-node-footprint-probe",
        private: true,
        version: "0.0.0",
        dependencies: {
          [WASM_PACKAGE_NAME_FOR_TEST]: "file:../tarballs/wasm.tgz",
        },
      },
      package_lock: {
        lockfileVersion: 3,
        packages: {
          "": {
            dependencies: {
              [WASM_PACKAGE_NAME_FOR_TEST]: "file:../tarballs/wasm.tgz",
            },
          },
          [`node_modules/${WASM_PACKAGE_NAME_FOR_TEST}`]: {
            version: PACKAGE_VERSION,
            resolved: "file:../tarballs/wasm.tgz",
          },
        },
      },
    },
    runtime_probe: runtimeProbe,
  };
}

const WASM_PACKAGE_NAME_FOR_TEST = "@mermanjs/node-wasm-candidate";

function packedPackage(name, filename) {
  return {
    name,
    version: PACKAGE_VERSION,
    filename,
    size: 100,
    unpacked_size: 200,
    files: [{ path: "package/package.json", bytes: 200 }],
  };
}

function targetResultFor(candidateValue, target) {
  const contract = TARGETS[target];
  const buildReceipt = candidateValue.id === "napi"
    ? buildReceiptFor("napi", target)
    : candidateValue.build_receipt;
  const targetProvenance = {
    ...provenance,
    machine: {
      ...provenance.machine,
      hostname: `${target}-host`,
      os: contract.platform,
      arch: contract.arch,
    },
  };
  const payload = {
    schema_version: 1,
    host: {
      platform: contract.platform,
      arch: contract.arch,
      libc: contract.libc,
      resolved_target: target,
      node: provenance.tools.node,
    },
    provenance: targetProvenance,
    build_receipt: buildReceipt,
    footprint: footprintFor(candidateValue.id, buildReceipt, target),
    queue_lifecycle: structuredClone(candidateValue.queue_lifecycle),
    error_behavior: structuredClone(candidateValue.error_behavior),
  };
  return {
    target,
    runtime_passed: true,
    install_passed: true,
    node: provenance.tools.node,
    evidence: { ...payload, digest: digestJson(payload) },
  };
}

function resignTargetEvidence(result) {
  const { digest: _digest, ...payload } = result.evidence;
  result.evidence.digest = digestJson(payload);
}

test("the digest binds corpus bytes, policy, operation options, and benchmark workloads", () => {
  const firstCorpusDigest = computeCorpusDigest([
    { path: "a.mmd", source: "flowchart TD\nA" },
    { path: "b.mmd", source: "sequenceDiagram\nA->>B: hi" },
  ]);
  const secondCorpusDigest = computeCorpusDigest([
    { path: "a.mmd", source: "flowchart TD\nA" },
    { path: "b.mmd", source: "sequenceDiagram\nA->>B: changed" },
  ]);
  const first = computeInputDigest({
    corpusDigest: firstCorpusDigest,
    bindingOptions: BINDING_OPTIONS,
    operationOptions: OPERATION_OPTIONS,
    workloads: WORKLOADS,
  });
  const second = computeInputDigest({
    corpusDigest: secondCorpusDigest,
    bindingOptions: BINDING_OPTIONS,
    operationOptions: OPERATION_OPTIONS,
    workloads: WORKLOADS,
  });
  const changedWorkload = computeInputDigest({
    corpusDigest: firstCorpusDigest,
    bindingOptions: BINDING_OPTIONS,
    operationOptions: OPERATION_OPTIONS,
    workloads: {
      ...WORKLOADS,
      cold_svg: { ...WORKLOADS.cold_svg, source: "flowchart TD\nA-->C" },
    },
  });
  assert.match(first, /^sha256:[0-9a-f]{64}$/);
  assert.notEqual(firstCorpusDigest, secondCorpusDigest);
  assert.notEqual(first, second);
  assert.notEqual(first, changedWorkload);
  assert.equal(
    first,
    computeInputDigest({
      corpusDigest: firstCorpusDigest,
      bindingOptions: BINDING_OPTIONS,
      operationOptions: OPERATION_OPTIONS,
      workloads: WORKLOADS,
    }),
  );
});

test("comparison projection preserves installed SVG planning evidence", () => {
  assert.equal(
    projectFootprint({ svg_plan_operation_passed: true }).svg_plan_operation_passed,
    true,
  );
});

test("SVG transport evidence separates structure from exact geometry", () => {
  const nativePoints = Buffer.from(JSON.stringify([{ x: 23.22905012309, y: 10 }])).toString("base64");
  const wasmPoints = Buffer.from(JSON.stringify([{ x: 23.229050123089, y: 10 }])).toString("base64");
  const native = [
    '<svg viewBox="0 0 1 1" style="max-width: 23.22905012309px; background: white">',
    `<path d="M0 -19.184615184444468 L23.22905012309 0" data-points="${nativePoints}"/>`,
    '<text id="node-1.0">1.000000000000001</text>',
    "</svg>",
  ].join("");
  const wasm = native
    .replace("-19.184615184444468", "-19.184615184444464")
    .replace("L23.22905012309 0", "L23.229050123089 0")
    .replace("max-width: 23.22905012309px", "max-width: 23.229050123089px")
    .replace(nativePoints, wasmPoints);
  const nativeEvidence = svgTransportEvidence(native);
  const wasmEvidence = svgTransportEvidence(wasm);
  assert.equal(nativeEvidence.structure_sha256, wasmEvidence.structure_sha256);
  assert.notEqual(nativeEvidence.geometry_sha256, wasmEvidence.geometry_sha256);
  assert.notEqual(
    nativeEvidence.structure_sha256,
    svgTransportEvidence(native.replace("node-1.0", "node-1.1")).structure_sha256,
  );
  assert.notEqual(
    nativeEvidence.structure_sha256,
    svgTransportEvidence(native.replace("1.000000000000001</text>", "changed</text>"))
      .structure_sha256,
  );
  assert.equal(
    nativeEvidence.structure_sha256,
    svgTransportEvidence(native.replace("-19.184615184444468", "-19.184615")).structure_sha256,
  );
  assert.throws(() => svgTransportEvidence("<svg><path></svg>"), /inspect/i);
  assert.throws(() => svgTransportEvidence("<not-svg/>"), /inspect/i);
});

test("a build receipt is bound to the exact measured artifact", (context) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-node-receipt-"));
  context.after(() => rmSync(root, { recursive: true, force: true }));
  const artifact = path.join(root, "merman.node");
  writeFileSync(artifact, "candidate-v1");
  const artifactDigest = `sha256:${createHash("sha256").update("candidate-v1").digest("hex")}`;
  const cargoLockDigest = `sha256:${createHash("sha256")
    .update(readFileSync(path.join(repositoryRoot, "crates", "merman-node", "Cargo.lock")))
    .digest("hex")}`;
  const tools = {
    cargo: "cargo 1.95.0",
    node: "v22.0.0",
    rustc: "rustc 1.95.0",
    transport_builder: "3.7.4",
  };
  const receipt = {
    schema_version: 1,
    config: {
      candidate: "napi",
      target: "darwin-arm64",
      rust_target: "aarch64-apple-darwin",
      wasm_pack_target: null,
      default_features: false,
      capability_recipe: CAPABILITY_RECIPE.capability_recipe,
      features: [
        "layout-cytoscape",
        "layout-elk",
        "math",
        "svg",
        "transport-napi",
      ],
    },
    commit: provenance.commit,
    source_digest: `sha256:${"d".repeat(64)}`,
    cargo_lock_digest: cargoLockDigest,
    binding_contract_digest: `sha256:${"e".repeat(64)}`,
    dependency_closure: {
      digest: digestJson([
        {
          name: "merman-node-candidate",
          version: "0.8.0-alpha.3",
          source: "path:crates/merman-node",
        },
        { name: "napi", version: "3.11.0", source: "registry+test" },
        { name: "napi-build", version: "2.3.2", source: "registry+test" },
        { name: "napi-derive", version: "3.6.0", source: "registry+test" },
      ]),
      packages: [
        {
          name: "merman-node-candidate",
          version: "0.8.0-alpha.3",
          source: "path:crates/merman-node",
        },
        { name: "napi", version: "3.11.0", source: "registry+test" },
        { name: "napi-build", version: "2.3.2", source: "registry+test" },
        { name: "napi-derive", version: "3.6.0", source: "registry+test" },
      ],
    },
    input_digest: null,
    runtime: RUNTIME_EVIDENCE,
    tools,
    artifacts: [
      { path: "merman.node", bytes: 12, sha256: artifactDigest },
      {
        path: "runtime.sidecar",
        bytes: 10,
        sha256: `sha256:${createHash("sha256").update("sidecar-v1").digest("hex")}`,
      },
    ],
  };
  receipt.input_digest = digestJson({
    cargo_lock_digest: receipt.cargo_lock_digest,
    config: receipt.config,
    dependency_closure_digest: receipt.dependency_closure.digest,
    source_digest: receipt.source_digest,
    tools: receipt.tools,
  });
  const currentEvidence = {
    source_digest: receipt.source_digest,
    binding_contract_digest: receipt.binding_contract_digest,
    dependency_closure_digest: receipt.dependency_closure.digest,
  };
  const readReceipt = () => readBuildReceipt(artifact, {
    probeCurrentRuntime: () => receipt.runtime,
    resolveCurrentEvidence: () => currentEvidence,
  });
  writeFileSync(path.join(root, "runtime.sidecar"), "sidecar-v1");
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(receipt));

  assert.deepEqual(readReceipt(), {
    receipt_digest: digestJson(receipt),
    candidate: "napi",
    target: "darwin-arm64",
    rust_target: "aarch64-apple-darwin",
    wasm_pack_target: null,
    commit: receipt.commit,
    source_digest: receipt.source_digest,
    cargo_lock_digest: receipt.cargo_lock_digest,
    binding_contract_digest: receipt.binding_contract_digest,
    dependency_closure_digest: receipt.dependency_closure.digest,
    capability_recipe_digest: CAPABILITY_RECIPE_DIGEST,
    runtime_catalog_digest: RUNTIME_CATALOG_DIGEST,
    input_digest: receipt.input_digest,
    artifact_digest: artifactDigest,
    runtime_artifacts: [{ path: "merman.node", bytes: 12, sha256: artifactDigest }],
  });

  assert.throws(
    () => readBuildReceipt(artifact, {
      probeCurrentRuntime: () => receipt.runtime,
      resolveCurrentEvidence: () => ({
        ...currentEvidence,
        source_digest: `sha256:${"0".repeat(64)}`,
      }),
    }),
    /source_digest.*stale.*current source tree/i,
  );
  assert.throws(
    () => readBuildReceipt(artifact, {
      probeCurrentRuntime: () => ({
        ...receipt.runtime,
        probe: { ...receipt.runtime.probe, svg_bytes: receipt.runtime.probe.svg_bytes + 1 },
      }),
      resolveCurrentEvidence: () => currentEvidence,
    }),
    /runtime evidence.*current artifact/i,
  );

  const forgedInputDigest = structuredClone(receipt);
  forgedInputDigest.input_digest = `sha256:${"0".repeat(64)}`;
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(forgedInputDigest));
  assert.throws(() => readReceipt(), /input digest.*recorded build inputs/i);
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(receipt));

  const wrongTarget = structuredClone(receipt);
  wrongTarget.config.rust_target = "x86_64-apple-darwin";
  wrongTarget.input_digest = digestJson({
    cargo_lock_digest: wrongTarget.cargo_lock_digest,
    config: wrongTarget.config,
    dependency_closure_digest: wrongTarget.dependency_closure.digest,
    source_digest: wrongTarget.source_digest,
    tools: wrongTarget.tools,
  });
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(wrongTarget));
  assert.throws(() => readReceipt(), /target configuration.*canonical/i);
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(receipt));

  writeFileSync(path.join(root, "unrecorded.sidecar"), "unrecorded");
  assert.throws(() => readReceipt(), /artifact file set.*does not match/i);
  rmSync(path.join(root, "unrecorded.sidecar"));

  const profileAsFeature = structuredClone(receipt);
  profileAsFeature.config.features = [
    "layout-cytoscape",
    "layout-elk",
    "math",
    "rust-static-svg",
    "transport-napi",
  ];
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(profileAsFeature));
  assert.throws(() => readReceipt(), /capability recipe capabilities plus its transport/i);
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(receipt));

  const missingRuntimeEvidence = structuredClone(receipt);
  delete missingRuntimeEvidence.runtime;
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(missingRuntimeEvidence));
  assert.throws(() => readReceipt(), /runtime evidence/i);
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(receipt));

  const unparsedSvgEvidence = structuredClone(receipt);
  delete unparsedSvgEvidence.runtime.probe.svg_structure_sha256;
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(unparsedSvgEvidence));
  assert.throws(() => readReceipt(), /runtime probe/i);
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(receipt));

  const phantomRuntimeOperation = structuredClone(receipt);
  phantomRuntimeOperation.runtime.catalog.capabilities.operation_ids.splice(
    1,
    0,
    "phantom-json",
  );
  phantomRuntimeOperation.runtime.catalog_digest = digestJson(
    phantomRuntimeOperation.runtime.catalog,
  );
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(phantomRuntimeOperation));
  assert.throws(() => readReceipt(), /capability recipe/i);
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(receipt));

  const uncallableRuntimeProvider = structuredClone(receipt);
  uncallableRuntimeProvider.runtime.catalog.capabilities.text_measurement.provider_ids = [
    "host-callback",
    "vendored",
  ];
  uncallableRuntimeProvider.runtime.catalog_digest = digestJson(
    uncallableRuntimeProvider.runtime.catalog,
  );
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(uncallableRuntimeProvider));
  assert.throws(() => readReceipt(), /text measurement provider/i);
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(receipt));

  writeFileSync(path.join(root, "runtime.sidecar"), "sidecar-v2");
  assert.throws(() => readReceipt(), /does not match/i);
  writeFileSync(path.join(root, "runtime.sidecar"), "sidecar-v1");

  writeFileSync(artifact, "candidate-v2");
  assert.throws(() => readReceipt(), /does not match/i);
});

test("WASM footprint staging preserves generated artifacts despite wasm-pack's wildcard ignore", (context) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-node-wasm-package-"));
  context.after(() => rmSync(root, { recursive: true, force: true }));
  const artifactRoot = path.join(root, "artifact-source");
  mkdirSync(artifactRoot, { recursive: true });
  const loader = path.join(artifactRoot, "merman_node.js");
  writeFileSync(loader, "module.exports = {};\n");
  writeFileSync(path.join(artifactRoot, "merman_node_bg.wasm"), "wasm bytes");
  writeFileSync(path.join(artifactRoot, "package.json"), '{"private":true,"type":"commonjs"}\n');
  writeFileSync(path.join(artifactRoot, ".gitignore"), "*\n");

  const packageRoot = stageWasmPackage(path.join(root, "stage"), loader);
  const result = spawnSync("npm", ["pack", "--json", "--dry-run"], {
    cwd: packageRoot,
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr);
  const files = JSON.parse(result.stdout)[0].files.map((file) => file.path);
  assert(files.includes("artifact/merman_node.js"));
  assert(files.includes("artifact/merman_node_bg.wasm"));
});

test("a comparison report rejects missing provenance and mismatched inputs", () => {
  const report = {
    schema_version: 2,
    provenance,
    input: {
      digest: COMPARISON_INPUT_DIGEST,
      corpus_digest: CORPUS_DIGEST,
      corpus: "fixtures/**/*.mmd",
      cases: 2,
      binding_options: {
        version: 1,
        runtime_policy: "deterministic",
        resources: { profile: "trusted-native" },
      },
      operation_options: { version: 1 },
      workloads: WORKLOADS,
    },
    sampling: SAMPLING,
    candidates: [candidate("node-wasm"), candidate("napi")],
    decision: { status: "inconclusive", selected: null, reasons: ["target matrix incomplete"] },
  };
  report.workload_comparison = computeWorkloadComparison(report.candidates);
  assert.deepEqual(validateComparisonReport(report), report);
  assert.throws(
    () => validateComparisonReportContract(report),
    /trusted corpus manifest/i,
  );

  const oldSchema = structuredClone(report);
  oldSchema.schema_version = 1;
  assert.throws(() => validateComparisonReport(oldSchema), /schema_version must be 2/i);

  const forgedCorpusDigest = structuredClone(report);
  forgedCorpusDigest.input.corpus_digest = `sha256:${"9".repeat(64)}`;
  forgedCorpusDigest.input.digest = computeInputDigest({
    corpusDigest: forgedCorpusDigest.input.corpus_digest,
    bindingOptions: forgedCorpusDigest.input.binding_options,
    operationOptions: forgedCorpusDigest.input.operation_options,
    workloads: forgedCorpusDigest.input.workloads,
  });
  for (const candidateResult of forgedCorpusDigest.candidates) {
    candidateResult.input_digest = forgedCorpusDigest.input.digest;
  }
  assert.throws(
    () => validateComparisonReport(forgedCorpusDigest),
    /trusted corpus manifest/i,
  );

  const forgedInputDigest = structuredClone(report);
  forgedInputDigest.input.digest = `sha256:${"8".repeat(64)}`;
  assert.throws(
    () => validateComparisonReport(forgedInputDigest),
    /input digest does not match the recorded corpus/i,
  );

  const missingCommit = structuredClone(report);
  delete missingCommit.provenance.commit;
  assert.throws(() => validateComparisonReport(missingCommit), /commit/i);

  const missingHarnessDigest = structuredClone(report);
  delete missingHarnessDigest.provenance.harness_digest;
  assert.throws(() => validateComparisonReport(missingHarnessDigest), /harness digest/i);

  const mismatched = structuredClone(report);
  mismatched.candidates[1].input_digest = `sha256:${"b".repeat(64)}`;
  assert.throws(() => validateComparisonReport(mismatched), /input digest/i);

  const missingOperationOptions = structuredClone(report);
  delete missingOperationOptions.input.operation_options;
  assert.throws(
    () => validateComparisonReport(missingOperationOptions),
    /operation options/i,
  );

  const missingSampling = structuredClone(report);
  delete missingSampling.sampling;
  assert.throws(() => validateComparisonReport(missingSampling), /sampling/i);

  const fakeCold = structuredClone(report);
  fakeCold.candidates[0].cold_process.isolated_processes = false;
  assert.throws(() => validateComparisonReport(fakeCold), /isolated process/i);

  const forgedColdTimingScope = structuredClone(report);
  forgedColdTimingScope.candidates[0].cold_process.evidence_excluded = false;
  assert.throws(
    () => validateComparisonReport(forgedColdTimingScope),
    /cold timing scope.*exclude SVG evidence projection/i,
  );

  const missingBuildReceipt = structuredClone(report);
  delete missingBuildReceipt.candidates[0].build_receipt.receipt_digest;
  assert.throws(() => validateComparisonReport(missingBuildReceipt), /build receipt/i);

  const incompleteWasmReceipt = structuredClone(report);
  incompleteWasmReceipt.candidates[0].build_receipt.runtime_artifacts.splice(1, 1);
  assert.throws(
    () => validateComparisonReport(incompleteWasmReceipt),
    /runtime artifact set/i,
  );

  const erasedErrorKind = structuredClone(report);
  erasedErrorKind.candidates[0].error_behavior.missing_capability.kind = "generic";
  assert.throws(() => validateComparisonReport(erasedErrorKind), /missing-capability/i);

  const mismatchedContract = structuredClone(report);
  mismatchedContract.candidates[1].build_receipt.binding_contract_digest =
    `sha256:${"9".repeat(64)}`;
  assert.throws(() => validateComparisonReport(mismatchedContract), /bindings-contract digest/i);

  const mismatchedCapabilityRecipe = structuredClone(report);
  mismatchedCapabilityRecipe.candidates[1].build_receipt.capability_recipe_digest =
    `sha256:${"7".repeat(64)}`;
  assert.throws(
    () => validateComparisonReport(mismatchedCapabilityRecipe),
    /capability-recipe digest/i,
  );

  const mismatchedRuntimeCatalog = structuredClone(report);
  mismatchedRuntimeCatalog.candidates[1].build_receipt.runtime_catalog_digest =
    `sha256:${"6".repeat(64)}`;
  assert.throws(
    () => validateComparisonReport(mismatchedRuntimeCatalog),
    /runtime.catalog|runtime-catalog/i,
  );

  const distinctArtifactClosures = structuredClone(report);
  distinctArtifactClosures.candidates[1].build_receipt.source_digest =
    `sha256:${"8".repeat(64)}`;
  assert.throws(
    () => validateComparisonReport(distinctArtifactClosures),
    /source digest/i,
  );

  const missingOutcomes = structuredClone(report);
  delete missingOutcomes.candidates[0].corpus.outcomes;
  assert.throws(() => validateComparisonReport(missingOutcomes), /raw corpus outcomes/i);

  const forgedCaseCount = structuredClone(report);
  forgedCaseCount.input.cases += 1;
  assert.throws(
    () => validateComparisonReport(forgedCaseCount),
    /case count.*trusted corpus manifest/i,
  );

  const forgedOutcomePath = structuredClone(report);
  forgedOutcomePath.candidates[0].corpus.outcomes[1].path = "forged.mmd";
  forgedOutcomePath.candidates[0].corpus.results_digest = digestJson(
    forgedOutcomePath.candidates[0].corpus.outcomes,
  );
  assert.throws(
    () => validateComparisonReport(forgedOutcomePath),
    /outcome paths.*trusted corpus manifest/i,
  );

  const missingColdSamples = structuredClone(report);
  delete missingColdSamples.candidates[0].cold_process.samples;
  assert.throws(() => validateComparisonReport(missingColdSamples), /raw cold-process samples/i);

  const missingWarmSamples = structuredClone(report);
  delete missingWarmSamples.candidates[0].warm_latency.samples;
  assert.throws(() => validateComparisonReport(missingWarmSamples), /raw warm-latency samples/i);

  const missingSuccessfulSvgSummary = structuredClone(report);
  delete missingSuccessfulSvgSummary.candidates[0].warm_latency.successful_svg;
  assert.throws(
    () => validateComparisonReport(missingSuccessfulSvgSummary),
    /successful SVG latency/i,
  );

  const missingPackageContents = structuredClone(report);
  delete missingPackageContents.candidates[0].footprint.packages;
  assert.throws(() => validateComparisonReport(missingPackageContents), /package contents/i);

  const forgedFootprintTotal = structuredClone(report);
  forgedFootprintTotal.candidates[0].footprint.installed_bytes += 1;
  assert.throws(
    () => validateComparisonReport(forgedFootprintTotal),
    /installed footprint total.*file contents/i,
  );

  const replacedInstalledWasm = structuredClone(report);
  replacedInstalledWasm.candidates[0].footprint.installation_evidence.loaded_artifacts
    .find((artifact) => artifact.path.endsWith("merman_node_bg.wasm")).sha256 =
      `sha256:${"0".repeat(64)}`;
  assert.throws(
    () => validateComparisonReport(replacedInstalledWasm),
    /bind the installed WASM artifacts/i,
  );

  const failedProductEntrypoint = structuredClone(report);
  failedProductEntrypoint.candidates[0].footprint.runtime_api_passed = false;
  assert.throws(
    () => validateComparisonReport(failedProductEntrypoint),
    /runtime_api_passed.*must be true/i,
  );

  const browserFallback = structuredClone(report);
  browserFallback.candidates[1].footprint.browser_fallback_absent = false;
  assert.throws(
    () => validateComparisonReport(browserFallback),
    /browser_fallback_absent.*must be true/i,
  );

  const explicitPlatformInstall = structuredClone(report);
  const napiFootprint = explicitPlatformInstall.candidates[1].footprint;
  const targetName = napiFootprint.installation_evidence.target_package.name;
  napiFootprint.installation_evidence.install_manifest.dependencies[targetName] =
    "file:../tarballs/target.tgz";
  napiFootprint.installation_evidence.package_lock.packages[""].dependencies[targetName] =
    "file:../tarballs/target.tgz";
  assert.throws(
    () => validateComparisonReport(explicitPlatformInstall),
    /install manifest or lockfile root edge/i,
  );

  const failedSvgProbe = structuredClone(report);
  failedSvgProbe.candidates[0].footprint.svg_operation_passed = false;
  assert.throws(
    () => validateComparisonReport(failedSvgProbe),
    /svg_operation_passed.*must be true/i,
  );

  const failedSvgPlanProbe = structuredClone(report);
  failedSvgPlanProbe.candidates[0].footprint.svg_plan_operation_passed = false;
  assert.throws(
    () => validateComparisonReport(failedSvgPlanProbe),
    /svg_plan_operation_passed.*must be true/i,
  );

  const missingShutdownEvidence = structuredClone(report);
  delete missingShutdownEvidence.candidates[0].queue_lifecycle.process_shutdown_passed;
  assert.throws(
    () => validateComparisonReport(missingShutdownEvidence),
    /process_shutdown_passed/i,
  );

  const failedQueuedAbortProbe = structuredClone(report);
  failedQueuedAbortProbe.candidates[0].queue_lifecycle.queued_abort_passed = false;
  assert.throws(
    () => validateComparisonReport(failedQueuedAbortProbe),
    /queued_abort_passed.*must be true/i,
  );

  const forgedLifecycleSummary = structuredClone(report);
  forgedLifecycleSummary.candidates[0].queue_lifecycle.evidence.saturation.active = {
    status: "rejected",
    error: { code: "MERMAN_QUEUE_SATURATED" },
  };
  assert.throws(
    () => validateComparisonReport(forgedLifecycleSummary),
    /saturation active.*settlement is invalid/i,
  );

  const forgedLatencySummary = structuredClone(report);
  forgedLatencySummary.candidates[0].warm_latency.summary.mean_ms += 1;
  assert.throws(
    () => validateComparisonReport(forgedLatencySummary),
    /warm latency summary.*raw samples/i,
  );

  const forgedSuccessfulSvgSummary = structuredClone(report);
  forgedSuccessfulSvgSummary.candidates[0].warm_latency.successful_svg.summary.mean_ms += 1;
  assert.throws(
    () => validateComparisonReport(forgedSuccessfulSvgSummary),
    /successful SVG latency summary.*raw samples/i,
  );

  const duplicateWarmSample = structuredClone(report);
  duplicateWarmSample.candidates[0].warm_latency.samples[3] = {
    ...duplicateWarmSample.candidates[0].warm_latency.samples[2],
    elapsed_ms: duplicateWarmSample.candidates[0].warm_latency.samples_ms[3],
  };
  assert.throws(
    () => validateComparisonReport(duplicateWarmSample),
    /raw warm-latency sample/i,
  );

  const forgedTimedOutcome = structuredClone(report);
  forgedTimedOutcome.candidates[0].warm_latency.samples[0].outcome.sha256 =
    `sha256:${"0".repeat(64)}`;
  assert.throws(
    () => validateComparisonReport(forgedTimedOutcome),
    /raw warm-latency sample/i,
  );

  const failedConcurrentOutcome = structuredClone(report);
  failedConcurrentOutcome.candidates[0].concurrency.samples[0].outcomes[0] =
    timedFailure("a.mmd");
  assert.throws(
    () => validateComparisonReport(failedConcurrentOutcome),
    /concurrency sample 0.*failed outcome/i,
  );

  const coldEvidenceDrift = structuredClone(report);
  coldEvidenceDrift.candidates[0].cold_process.samples[1]
    .outcome.svg_geometry_sha256 = `sha256:${"4".repeat(64)}`;
  assert.throws(
    () => validateComparisonReport(coldEvidenceDrift),
    /cold workload SVG evidence drifted/i,
  );

  const concurrencyEvidenceDrift = structuredClone(report);
  concurrencyEvidenceDrift.candidates[0].concurrency.samples[1]
    .outcomes[0].sha256 = `sha256:${"4".repeat(64)}`;
  assert.throws(
    () => validateComparisonReport(concurrencyEvidenceDrift),
    /concurrency workload SVG evidence drifted/i,
  );

  const forgedRepresentativeSource = structuredClone(report);
  forgedRepresentativeSource.candidates[0].cold_process.representative.source_sha256 =
    `sha256:${"4".repeat(64)}`;
  assert.throws(
    () => validateComparisonReport(forgedRepresentativeSource),
    /representative is not bound to its workload source/i,
  );

  const forgedRepresentativeRawSvg = structuredClone(report);
  forgedRepresentativeRawSvg.candidates[0].cold_process.representative.raw_svg =
    '<svg viewBox="0 0 9 9"/>';
  assert.throws(
    () => validateComparisonReport(forgedRepresentativeRawSvg),
    /evidence does not match its representative raw SVG/i,
  );

  const forgedConcurrencyRepresentative = structuredClone(report);
  forgedConcurrencyRepresentative.candidates[0].concurrency.representative.raw_svg =
    "<svg><g/></svg>";
  assert.throws(
    () => validateComparisonReport(forgedConcurrencyRepresentative),
    /evidence does not match its representative raw SVG/i,
  );

  const forgedWorkloadComparison = structuredClone(report);
  forgedWorkloadComparison.workload_comparison.cold_svg.raw_svg_matched = false;
  assert.throws(
    () => validateComparisonReport(forgedWorkloadComparison),
    /workload comparison does not match/i,
  );

  const recordedWorkloadDrift = structuredClone(report);
  const geometryDriftSvg = '<svg viewBox="0 0 2 2"/>';
  const geometryDriftOutcome = timedSuccessForSvg(
    WORKLOADS.cold_svg.path,
    geometryDriftSvg,
  );
  recordedWorkloadDrift.candidates[1].cold_process.representative.raw_svg =
    geometryDriftSvg;
  for (const sample of recordedWorkloadDrift.candidates[1].cold_process.samples) {
    sample.outcome = { ...geometryDriftOutcome };
  }
  recordedWorkloadDrift.workload_comparison =
    computeWorkloadComparison(recordedWorkloadDrift.candidates);
  assert.deepEqual(recordedWorkloadDrift.workload_comparison.cold_svg, {
    structure_matched: true,
    geometry_matched: false,
    raw_svg_matched: false,
    bytes_matched: true,
  });
  assert.deepEqual(
    validateComparisonReport(recordedWorkloadDrift),
    recordedWorkloadDrift,
  );

  const crossCandidateStructureMismatch = structuredClone(report);
  const structureDriftSvg = "<svg><g/></svg>";
  const structureDriftOutcome = timedSuccessForSvg(
    WORKLOADS.concurrency_svg.path,
    structureDriftSvg,
  );
  crossCandidateStructureMismatch.candidates[1]
    .concurrency.representative.raw_svg = structureDriftSvg;
  for (const sample of crossCandidateStructureMismatch.candidates[1].concurrency.samples) {
    for (const outcome of sample.outcomes) {
      Object.assign(outcome, structureDriftOutcome);
    }
  }
  crossCandidateStructureMismatch.workload_comparison =
    computeWorkloadComparison(crossCandidateStructureMismatch.candidates);
  assert.throws(
    () => validateComparisonReport(crossCandidateStructureMismatch),
    /concurrency_svg SVG structure differs/i,
  );

  const forgedOutcomeDigest = structuredClone(report);
  forgedOutcomeDigest.candidates[0].corpus.results_digest = `sha256:${"0".repeat(64)}`;
  assert.throws(
    () => validateComparisonReport(forgedOutcomeDigest),
    /raw corpus outcomes.*recorded summary/i,
  );

  const forgedParity = structuredClone(report);
  forgedParity.candidates[1].corpus.outcomes[0].semantic.sha256 = `sha256:${"0".repeat(64)}`;
  forgedParity.candidates[1].corpus.results_digest = digestJson(
    forgedParity.candidates[1].corpus.outcomes,
  );
  assert.throws(
    () => validateComparisonReport(forgedParity),
    /raw corpus outcomes.*cross-candidate parity/i,
  );

  const genericTransportFailures = structuredClone(report);
  for (const candidateResult of genericTransportFailures.candidates) {
    candidateResult.corpus.outcomes[0] = {
      path: "a.mmd",
      ok: false,
      kind: "generic",
      semantic: { ok: false, kind: "generic" },
    };
    candidateResult.corpus.successful = 0;
    candidateResult.corpus.failed = 2;
    candidateResult.corpus.results_digest = digestJson(candidateResult.corpus.outcomes);
  }
  assert.throws(
    () => validateComparisonReport(genericTransportFailures),
    /typed error evidence/i,
  );

  const typedBindingStatusFailures = structuredClone(report);
  for (const candidateResult of typedBindingStatusFailures.candidates) {
    for (const outcome of candidateResult.corpus.outcomes) {
      if (!outcome.ok) {
        recordParseStatus(outcome);
        recordParseStatus(outcome.semantic);
      }
    }
    for (const sample of candidateResult.cold_process.samples) {
      if (!sample.outcome.ok) recordParseStatus(sample.outcome);
    }
    for (const sample of candidateResult.warm_latency.samples) {
      if (!sample.outcome.ok) recordParseStatus(sample.outcome);
    }
    for (const sample of candidateResult.concurrency.samples) {
      for (const outcome of sample.outcomes) {
        if (!outcome.ok) recordParseStatus(outcome);
      }
    }
    candidateResult.corpus.results_digest = digestJson(candidateResult.corpus.outcomes);
  }
  assert.deepEqual(
    validateComparisonReport(typedBindingStatusFailures),
    typedBindingStatusFailures,
  );

  const internalFailures = structuredClone(typedBindingStatusFailures);
  for (const candidateResult of internalFailures.candidates) {
    const outcome = candidateResult.corpus.outcomes.find((item) => !item.ok);
    outcome.code_name = "MERMAN_INTERNAL_ERROR";
    outcome.semantic.code_name = "MERMAN_INTERNAL_ERROR";
    candidateResult.corpus.results_digest = digestJson(candidateResult.corpus.outcomes);
  }
  assert.throws(
    () => validateComparisonReport(internalFailures),
    /typed error evidence/i,
  );

  const failedLifecycleProbe = structuredClone(report);
  failedLifecycleProbe.candidates[0].queue_lifecycle.dispose_passed = false;
  assert.throws(
    () => validateComparisonReport(failedLifecycleProbe),
    /dispose_passed.*must be true/i,
  );

  const mismatchedCargoLock = structuredClone(report);
  mismatchedCargoLock.candidates[1].build_receipt.cargo_lock_digest =
    `sha256:${"4".repeat(64)}`;
  assert.throws(
    () => validateComparisonReport(mismatchedCargoLock),
    /Cargo lock digest/i,
  );
});

function recordParseStatus(evidence) {
  evidence.kind = "generic";
  evidence.code_name = "MERMAN_PARSE_ERROR";
  evidence.capability_id = null;
}

test("the report cannot announce a winner without complete target evidence", () => {
  const report = {
    schema_version: 2,
    provenance,
    input: {
      digest: COMPARISON_INPUT_DIGEST,
      corpus_digest: CORPUS_DIGEST,
      corpus: "fixtures/**/*.mmd",
      cases: 2,
      binding_options: {
        version: 1,
        runtime_policy: "deterministic",
        resources: { profile: "trusted-native" },
      },
      operation_options: { version: 1 },
      workloads: WORKLOADS,
    },
    sampling: SAMPLING,
    candidates: [candidate("node-wasm"), candidate("napi")],
    decision: { status: "admitted", selected: "napi", reasons: ["complete evidence"] },
  };
  report.workload_comparison = computeWorkloadComparison(report.candidates);
  assert.throws(
    () => validateComparisonReport(report),
    /selected targets.*runtime and installation evidence/i,
  );

  report.candidates[1].target_results = [
    { target: "darwin-arm64", runtime_passed: true, install_passed: true },
  ];
  assert.throws(
    () => validateComparisonReport(report),
    /runtime and installation evidence/i,
  );

  report.candidates[1].target_results = INITIAL_TARGETS.map((target) => ({
    target,
    runtime_passed: true,
    install_passed: true,
  }));
  assert.throws(
    () => validateComparisonReport(report),
    /runtime and installation evidence/i,
  );

  report.candidates[1].target_results = [
    targetResultFor(report.candidates[1], "darwin-arm64"),
  ];
  assert.throws(
    () => validateComparisonReport(report),
    /complete initial target matrix/i,
  );

  report.candidates[1].target_results = INITIAL_TARGETS.map((target) =>
    targetResultFor(report.candidates[1], target));

  const duplicateNativeReceipt = structuredClone(report);
  duplicateNativeReceipt.candidates[1].target_results[1].evidence.build_receipt.receipt_digest =
    duplicateNativeReceipt.candidates[1].target_results[0].evidence.build_receipt.receipt_digest;
  resignTargetEvidence(duplicateNativeReceipt.candidates[1].target_results[1]);
  assert.throws(
    () => validateComparisonReport(duplicateNativeReceipt),
    /distinct build receipt per native target/i,
  );

  const wrongHost = structuredClone(report);
  wrongHost.candidates[1].target_results[0].evidence.host.platform = "win32";
  resignTargetEvidence(wrongHost.candidates[1].target_results[0]);
  assert.throws(
    () => validateComparisonReport(wrongHost),
    /host evidence does not match the target/i,
  );

  const wrongReceiptTarget = structuredClone(report);
  wrongReceiptTarget.candidates[1].target_results[0].evidence.build_receipt.target =
    "darwin-x64";
  resignTargetEvidence(wrongReceiptTarget.candidates[1].target_results[0]);
  assert.throws(
    () => validateComparisonReport(wrongReceiptTarget),
    /build receipt target configuration/i,
  );

  const wrongLoadedArtifact = structuredClone(report);
  wrongLoadedArtifact.candidates[1].target_results[0]
    .evidence.footprint.installation_evidence.loaded_artifacts[0].sha256 =
      `sha256:${"0".repeat(64)}`;
  resignTargetEvidence(wrongLoadedArtifact.candidates[1].target_results[0]);
  assert.throws(
    () => validateComparisonReport(wrongLoadedArtifact),
    /bind the target package and loaded artifact/i,
  );

  const falseInnerInstallFlag = structuredClone(report);
  falseInnerInstallFlag.candidates[1].target_results[0]
    .evidence.footprint.target_install_passed = false;
  resignTargetEvidence(falseInnerInstallFlag.candidates[1].target_results[0]);
  assert.throws(
    () => validateComparisonReport(falseInnerInstallFlag),
    /pass flags.*complete target evidence/i,
  );

  report.candidates[1].target_results[2].install_passed = false;
  assert.throws(
    () => validateComparisonReport(report),
    /pass flags.*complete target evidence/i,
  );

  report.candidates[1].target_results[2].install_passed = true;
  report.candidates[1].footprint.target_install_passed = false;
  assert.throws(
    () => validateComparisonReport(report),
    /product installation probe/i,
  );

  report.candidates[1].footprint.target_install_passed = true;
  report.candidates[1].footprint.install_method = "explicit-package-pair";
  assert.throws(
    () => validateComparisonReport(report),
    /root optional dependency/i,
  );

  report.candidates[1].footprint.install_method = "root-optional-dependency";
  report.candidates[1].queue_lifecycle.dispose_passed = false;
  assert.throws(
    () => validateComparisonReport(report),
    /dispose_passed.*must be true/i,
  );

  report.candidates[1].queue_lifecycle.dispose_passed = true;
  report.candidates[1].corpus.outcomes[0].svg_geometry_sha256 = `sha256:${"8".repeat(64)}`;
  report.candidates[1].corpus.results_digest = digestJson(report.candidates[1].corpus.outcomes);
  for (const candidateResult of report.candidates) {
    candidateResult.corpus.geometry_svg_mismatches = 1;
    candidateResult.corpus.geometry_mismatch_paths = ["a.mmd"];
  }
  assert.deepEqual(validateComparisonReport(report), report);
});

test("a rejected report records no selected transport", () => {
  const report = {
    schema_version: 2,
    provenance,
    input: {
      digest: COMPARISON_INPUT_DIGEST,
      corpus_digest: CORPUS_DIGEST,
      corpus: "fixtures/**/*.mmd",
      cases: 2,
      binding_options: {
        version: 1,
        runtime_policy: "deterministic",
        resources: { profile: "trusted-native" },
      },
      operation_options: { version: 1 },
      workloads: WORKLOADS,
    },
    sampling: SAMPLING,
    candidates: [candidate("node-wasm"), candidate("napi")],
    decision: { status: "rejected", selected: null, reasons: ["semantic corpus mismatch"] },
  };
  report.workload_comparison = computeWorkloadComparison(report.candidates);
  assert.deepEqual(validateComparisonReport(report), report);
});
