import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { readBuildReceipt } from "../scripts/benchmark/build-receipt.mjs";
import { stageWasmPackage } from "../scripts/benchmark/footprint.mjs";
import {
  computeInputDigest,
  validateComparisonReport,
} from "../scripts/benchmark/report-contract.mjs";
import { svgTransportEvidence } from "../scripts/benchmark/svg-signature.mjs";
import { digestJson } from "../scripts/stable-json.mjs";

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
    rustc: "rustc 1.95.0",
    cargo: "cargo 1.95.0",
    napi: "3.11.0",
    napi_derive: "3.6.0",
    napi_build: "2.3.2",
    napi_cli: "3.7.4",
  },
  commit: "0123456789abcdef0123456789abcdef01234567",
};
const COMPARISON_INPUT_DIGEST = `sha256:${"a".repeat(64)}`;
const CAPABILITY_RECIPE = {
  default_features: false,
  artifact_profile: {
    descriptor: "capabilities/artifact-profiles-v1.json",
    id: "rust-static-svg",
    features: ["layout-cytoscape", "layout-elk", "math", "svg"],
  },
};
const CAPABILITY_RECIPE_DIGEST = digestJson(CAPABILITY_RECIPE);

function candidate(id) {
  return {
    id,
    input_digest: COMPARISON_INPUT_DIGEST,
    build_receipt: {
      receipt_digest: `sha256:${"c".repeat(64)}`,
      commit: provenance.commit,
      source_digest: `sha256:${"d".repeat(64)}`,
      binding_contract_digest: `sha256:${"e".repeat(64)}`,
      capability_recipe_digest: CAPABILITY_RECIPE_DIGEST,
      input_digest: `sha256:${"e".repeat(64)}`,
      artifact_digest: `sha256:${"f".repeat(64)}`,
    },
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
          path: "a.mmd",
          ok: true,
          sha256: `sha256:${"1".repeat(64)}`,
          svg_structure_sha256: `sha256:${"2".repeat(64)}`,
          svg_geometry_sha256: `sha256:${"3".repeat(64)}`,
          bytes: 10,
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
          semantic: { ok: false, kind: "parse-error" },
        },
      ],
    },
    cold_process: {
      isolated_processes: true,
      samples_ms: [10, 11],
      samples: [
        { elapsed_ms: 10, operation_ms: 8, baseline_rss_bytes: 400, peak_rss_bytes: 900 },
        { elapsed_ms: 11, operation_ms: 9, baseline_rss_bytes: 410, peak_rss_bytes: 910 },
      ],
    },
    warm_latency: {
      samples_ms: [1, 2, 1.5],
      samples: [
        { iteration: 0, path: "a.mmd", elapsed_ms: 1, outcome: { ok: true } },
        { iteration: 0, path: "b.mmd", elapsed_ms: 2, outcome: { ok: false } },
        { iteration: 1, path: "a.mmd", elapsed_ms: 1.5, outcome: { ok: true } },
      ],
    },
    rss: {
      method: "process.resourceUsage.maxRSS",
      peak_bytes: 1000,
      baseline_bytes: 500,
    },
    footprint: {
      packed_bytes: 100,
      unpacked_bytes: 200,
      installed_bytes: 300,
      package_count: 1,
      runtime_api_passed: true,
      install_method: "single-package",
      target_install_passed: true,
      packages: [
        {
          filename: "candidate.tgz",
          size: 100,
          unpacked_size: 200,
          files: [{ path: "package/package.json", bytes: 10 }],
        },
      ],
      installed_files: [
        { path: "candidate/package.json", bytes: 300 },
      ],
    },
    queue_lifecycle: {
      saturation_passed: true,
      dispose_passed: true,
      non_preemptive_abort_passed: true,
    },
    concurrency: {
      workers: 4,
      requests_per_batch: 4,
      batch_samples_ms: [4, 5],
    },
    error_behavior: {
      unknown_operation: { kind: "unknown-operation", capability_id: null },
      missing_capability: { kind: "missing-capability", capability_id: "png" },
      text_measurement_callback_rejected: true,
    },
    target_results: [],
  };
}

test("the digest binds corpus bytes, options, profile, and format options", () => {
  const first = computeInputDigest({
    cases: [
      { path: "a.mmd", source: "flowchart TD\nA" },
      { path: "b.mmd", source: "sequenceDiagram\nA->>B: hi" },
    ],
    bindingOptions: {
      version: 1,
      runtime_policy: "deterministic",
      resources: { profile: "trusted-native" },
    },
    formatOptions: { version: 1 },
  });
  const second = computeInputDigest({
    cases: [
      { path: "a.mmd", source: "flowchart TD\nA" },
      { path: "b.mmd", source: "sequenceDiagram\nA->>B: changed" },
    ],
    bindingOptions: {
      version: 1,
      runtime_policy: "deterministic",
      resources: { profile: "trusted-native" },
    },
    formatOptions: { version: 1 },
  });
  assert.match(first, /^sha256:[0-9a-f]{64}$/);
  assert.notEqual(first, second);
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
});

test("a build receipt is bound to the exact measured artifact", (context) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-node-receipt-"));
  context.after(() => rmSync(root, { recursive: true, force: true }));
  const artifact = path.join(root, "merman.node");
  writeFileSync(artifact, "candidate-v1");
  const artifactDigest = `sha256:${createHash("sha256").update("candidate-v1").digest("hex")}`;
  const receipt = {
    schema_version: 1,
    config: {
      candidate: "napi",
      default_features: false,
      artifact_profile: CAPABILITY_RECIPE.artifact_profile,
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
    binding_contract_digest: `sha256:${"e".repeat(64)}`,
    input_digest: `sha256:${"e".repeat(64)}`,
    artifacts: [
      { path: "merman.node", bytes: 12, sha256: artifactDigest },
      {
        path: "runtime.sidecar",
        bytes: 10,
        sha256: `sha256:${createHash("sha256").update("sidecar-v1").digest("hex")}`,
      },
    ],
  };
  writeFileSync(path.join(root, "runtime.sidecar"), "sidecar-v1");
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(receipt));

  assert.deepEqual(readBuildReceipt(artifact), {
    receipt_digest: digestJson(receipt),
    commit: receipt.commit,
    source_digest: receipt.source_digest,
    binding_contract_digest: receipt.binding_contract_digest,
    capability_recipe_digest: CAPABILITY_RECIPE_DIGEST,
    input_digest: receipt.input_digest,
    artifact_digest: artifactDigest,
  });

  const profileAsFeature = structuredClone(receipt);
  profileAsFeature.config.features = [
    "layout-cytoscape",
    "layout-elk",
    "math",
    "rust-static-svg",
    "transport-napi",
  ];
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(profileAsFeature));
  assert.throws(() => readBuildReceipt(artifact), /artifact profile leaves plus its transport/i);
  writeFileSync(path.join(root, "build-receipt.json"), JSON.stringify(receipt));

  writeFileSync(path.join(root, "runtime.sidecar"), "sidecar-v2");
  assert.throws(() => readBuildReceipt(artifact), /does not match/i);
  writeFileSync(path.join(root, "runtime.sidecar"), "sidecar-v1");

  writeFileSync(artifact, "candidate-v2");
  assert.throws(() => readBuildReceipt(artifact), /does not match/i);
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
    schema_version: 1,
    provenance,
    input: {
      digest: COMPARISON_INPUT_DIGEST,
      corpus: "fixtures/**/*.mmd",
      binding_options: {
        version: 1,
        runtime_policy: "deterministic",
        resources: { profile: "trusted-native" },
      },
      format_options: { version: 1 },
    },
    candidates: [candidate("node-wasm"), candidate("napi")],
    decision: { status: "inconclusive", selected: null, reasons: ["target matrix incomplete"] },
  };
  assert.deepEqual(validateComparisonReport(report), report);

  const missingCommit = structuredClone(report);
  delete missingCommit.provenance.commit;
  assert.throws(() => validateComparisonReport(missingCommit), /commit/i);

  const missingHarnessDigest = structuredClone(report);
  delete missingHarnessDigest.provenance.harness_digest;
  assert.throws(() => validateComparisonReport(missingHarnessDigest), /harness digest/i);

  const mismatched = structuredClone(report);
  mismatched.candidates[1].input_digest = `sha256:${"b".repeat(64)}`;
  assert.throws(() => validateComparisonReport(mismatched), /input digest/i);

  const fakeCold = structuredClone(report);
  fakeCold.candidates[0].cold_process.isolated_processes = false;
  assert.throws(() => validateComparisonReport(fakeCold), /isolated process/i);

  const missingBuildReceipt = structuredClone(report);
  delete missingBuildReceipt.candidates[0].build_receipt.receipt_digest;
  assert.throws(() => validateComparisonReport(missingBuildReceipt), /build receipt/i);

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

  const distinctArtifactClosures = structuredClone(report);
  distinctArtifactClosures.candidates[1].build_receipt.source_digest =
    `sha256:${"8".repeat(64)}`;
  assert.deepEqual(validateComparisonReport(distinctArtifactClosures), distinctArtifactClosures);

  const missingOutcomes = structuredClone(report);
  delete missingOutcomes.candidates[0].corpus.outcomes;
  assert.throws(() => validateComparisonReport(missingOutcomes), /raw corpus outcomes/i);

  const missingColdSamples = structuredClone(report);
  delete missingColdSamples.candidates[0].cold_process.samples;
  assert.throws(() => validateComparisonReport(missingColdSamples), /raw cold-process samples/i);

  const missingWarmSamples = structuredClone(report);
  delete missingWarmSamples.candidates[0].warm_latency.samples;
  assert.throws(() => validateComparisonReport(missingWarmSamples), /raw warm-latency samples/i);

  const missingPackageContents = structuredClone(report);
  delete missingPackageContents.candidates[0].footprint.packages;
  assert.throws(() => validateComparisonReport(missingPackageContents), /package contents/i);
});

test("the report cannot announce a winner without complete target evidence", () => {
  const report = {
    schema_version: 1,
    provenance,
    input: {
      digest: COMPARISON_INPUT_DIGEST,
      corpus: "fixtures/**/*.mmd",
      binding_options: {
        version: 1,
        runtime_policy: "deterministic",
        resources: { profile: "trusted-native" },
      },
      format_options: { version: 1 },
    },
    candidates: [candidate("node-wasm"), candidate("napi")],
    decision: { status: "admitted", selected: "napi", reasons: ["complete evidence"] },
  };
  assert.throws(
    () => validateComparisonReport(report),
    /selected targets.*runtime and installation evidence/i,
  );

  report.candidates[1].target_results = [
    { target: "darwin-arm64", runtime_passed: true, install_passed: true },
  ];
  assert.throws(
    () => validateComparisonReport(report),
    /complete initial target matrix/i,
  );

  report.candidates[1].target_results = [
    "darwin-arm64",
    "darwin-x64",
    "linux-x64-gnu",
    "linux-x64-musl",
    "win32-x64-msvc",
  ].map((target) => ({ target, runtime_passed: true, install_passed: true }));
  report.candidates[1].target_results[2].install_passed = false;
  assert.throws(
    () => validateComparisonReport(report),
    /runtime and installation evidence/i,
  );

  report.candidates[1].target_results[2].install_passed = true;
  report.candidates[1].footprint.install_method = "explicit-package-pair";
  report.candidates[1].footprint.target_install_passed = false;
  assert.throws(
    () => validateComparisonReport(report),
    /product installation probe/i,
  );

  report.candidates[1].footprint.target_install_passed = true;
  assert.throws(
    () => validateComparisonReport(report),
    /root optional dependency/i,
  );

  report.candidates[1].footprint.install_method = "root-optional-dependency";
  report.candidates[1].queue_lifecycle.dispose_passed = false;
  assert.throws(
    () => validateComparisonReport(report),
    /queue and lifecycle/i,
  );

  report.candidates[1].queue_lifecycle.dispose_passed = true;
  report.candidates[1].corpus.geometry_svg_mismatches = 1;
  report.candidates[1].corpus.geometry_mismatch_paths = ["a.mmd"];
  assert.deepEqual(validateComparisonReport(report), report);
});

test("a rejected report records no selected transport", () => {
  const report = {
    schema_version: 1,
    provenance,
    input: {
      digest: COMPARISON_INPUT_DIGEST,
      corpus: "fixtures/**/*.mmd",
      binding_options: {
        version: 1,
        runtime_policy: "deterministic",
        resources: { profile: "trusted-native" },
      },
      format_options: { version: 1 },
    },
    candidates: [candidate("node-wasm"), candidate("napi")],
    decision: { status: "rejected", selected: null, reasons: ["semantic corpus mismatch"] },
  };
  assert.deepEqual(validateComparisonReport(report), report);
});
