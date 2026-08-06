import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import {
  CHECKED_EDITOR_ARTIFACT_RECEIPT_PATH,
  DEFAULT_EDITOR_ARTIFACT_RECEIPT_PATH,
  EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION,
  EDITOR_ARTIFACT_FAMILY_COUNT,
  EDITOR_ARTIFACT_QUERY_KINDS,
  EDITOR_ARTIFACT_RECEIPT_SCHEMA_VERSION,
  compareEditorArtifactEquivalence,
  createEditorArtifactReceipt,
  decideEditorArtifact,
  summarizeEditorArtifactRuns,
  validateAbBaRuns,
  validateEditorArtifactReceipt,
} from "./contract.mjs";
import {
  canonicalStringify,
  canonicalize,
  compareCanonicalStrings,
} from "./equivalence-shared.mjs";
import {
  digestEntries,
  editorArtifactBuildRuntimeClosure,
  measurementContractDigest,
  runtimePackageArtifactPaths,
  runtimePackageProvenanceContract,
} from "./selection-inputs.mjs";
import { verifyEditorArtifactAuthority } from "./verify-editor-artifact-receipt.mjs";

test("selects editor only when semantics and every R16 metric pass", () => {
  const decision = decideEditorArtifact(
    {
      full: summary({ bytes: 1_000, latency: 100, memory: 2_000 }),
      editor: summary({ bytes: 900, latency: 104, memory: 2_000 }),
    },
    equivalentComparison(),
  );

  assert.equal(decision.selected, "editor");
  assert.equal(decision.editorEligible, true);
  assert.equal(decision.criteria.semanticEquivalence.passes, true);
  assert.equal(
    decision.criteria.semanticEquivalence.cellCount,
    EDITOR_ARTIFACT_FAMILY_COUNT * EDITOR_ARTIFACT_QUERY_KINDS.length,
  );
  assert.equal(decision.criteria.coldBytes.passes, true);
  assert.equal(decision.criteria.peakMemory.passes, true);
  assert.equal(
    decision.criteria.primaryLatencies.every((metric) => metric.passes),
    true,
  );
});

test("makes editor ineligible when any family query digest differs", () => {
  const variants = equivalenceVariants();
  variants.editor.families[17].queries[8].sha256 = digest("different-result");
  variants.editor = rehashMatrix(variants.editor);
  const equivalence = compareEditorArtifactEquivalence(variants);
  const decision = decideEditorArtifact(
    {
      full: summary({ bytes: 1_000, latency: 100, memory: 2_000 }),
      editor: summary({ bytes: 900, latency: 90, memory: 1_500 }),
    },
    equivalence,
  );

  assert.equal(equivalence.exact, false);
  assert.ok(equivalence.mismatches.includes("families[17].queries[8].sha256"));
  assert.ok(equivalence.mismatches.includes("aggregateSha256"));
  assert.equal(decision.selected, "full");
  assert.equal(decision.criteria.semanticEquivalence.passes, false);
});

test("owns and deeply freezes semantic-equivalence comparisons", () => {
  const variants = equivalenceVariants();
  const originalDigest = variants.full.families[0].queries[0].sha256;
  const comparison = compareEditorArtifactEquivalence(variants);

  variants.full.families[0].queries[0].sha256 = digest("caller-mutation");

  assert.equal(
    comparison.variants.full.families[0].queries[0].sha256,
    originalDigest,
  );
  assert.equal(Object.isFrozen(variants), false);
  assert.equal(Object.isFrozen(comparison), true);
  assert.equal(
    Object.isFrozen(comparison.variants.full.families[0].queries[0]),
    true,
  );
  assert.throws(() => {
    comparison.variants.full.families[0].queries[0].sha256 = digest("frozen");
  }, TypeError);
});

test("derives semantic eligibility from matrices instead of trusting a forged comparison flag", () => {
  const variants = equivalenceVariants();
  variants.editor.families[0].queries[0].sha256 = digest("forged-matrix");
  variants.editor = rehashMatrix(variants.editor);
  const forged = {
    ...equivalentComparison(),
    exact: true,
    mismatches: [],
    variants,
  };
  const decision = decideEditorArtifact(
    {
      full: summary({ bytes: 1_000, latency: 100, memory: 2_000 }),
      editor: summary({ bytes: 900, latency: 90, memory: 1_500 }),
    },
    forged,
  );

  assert.equal(decision.selected, "full");
  assert.equal(decision.criteria.semanticEquivalence.passes, false);
});

test("rejects incomplete, reordered, and unbound equivalence matrices", () => {
  const missingFamily = equivalenceVariants();
  missingFamily.editor.families.pop();
  assert.throws(
    () => compareEditorArtifactEquivalence(missingFamily),
    /families are incomplete/u,
  );

  const reorderedQuery = equivalenceVariants();
  reorderedQuery.editor.families[0].queries.reverse();
  reorderedQuery.editor = rehashMatrix(reorderedQuery.editor);
  assert.throws(
    () => compareEditorArtifactEquivalence(reorderedQuery),
    /query order is invalid/u,
  );

  const staleAggregate = equivalenceVariants();
  staleAggregate.editor.families[0].queries[0].sha256 = digest("tampered");
  assert.throws(
    () => compareEditorArtifactEquivalence(staleAggregate),
    /does not bind its matrix/u,
  );
});

test("canonicalization sorts nested keys, converts Uint32Array, and preserves __proto__ safely", () => {
  const input = JSON.parse(
    '{"z":{"b":2,"a":1},"tokens":{"0":9},"__proto__":{"polluted":true}}',
  );
  input.tokens = new Uint32Array([9, 4]);
  const canonical = canonicalize(input);

  assert.equal(Object.prototype.polluted, undefined);
  assert.deepEqual(["treemap", "treeView"].sort(compareCanonicalStrings), [
    "treeView",
    "treemap",
  ]);
  assert.deepEqual(Object.keys(canonical), ["__proto__", "tokens", "z"]);
  assert.equal(
    canonicalStringify(input),
    '{"__proto__":{"polluted":true},"tokens":[9,4],"z":{"a":1,"b":2}}',
  );
});

test("retains full when editor does not lower cold total transfer", () => {
  const decision = decideEditorArtifact(
    {
      full: summary({ bytes: 1_000, latency: 100, memory: 2_000 }),
      editor: summary({ bytes: 1_000, latency: 90, memory: 1_500 }),
    },
    equivalentComparison(),
  );

  assert.equal(decision.selected, "full");
  assert.equal(decision.criteria.coldBytes.passes, false);
});

test("retains full for any peak-memory increase", () => {
  const decision = decideEditorArtifact(
    {
      full: summary({ bytes: 1_000, latency: 100, memory: 2_000 }),
      editor: summary({ bytes: 900, latency: 90, memory: 2_001 }),
    },
    equivalentComparison(),
  );

  assert.equal(decision.selected, "full");
  assert.equal(decision.criteria.peakMemory.passes, false);
});

test("latency fails only when regression exceeds both 5 percent and 20 ms", () => {
  const full = summary({ bytes: 1_000, latency: 100, memory: 2_000 });
  const equivalence = equivalentComparison();

  assert.equal(
    decideEditorArtifact(
      {
        full,
        editor: summary({ bytes: 900, latency: 121, memory: 1_900 }),
      },
      equivalence,
    ).selected,
    "full",
  );
  assert.equal(
    decideEditorArtifact(
      {
        full: summary({ bytes: 1_000, latency: 1_000, memory: 2_000 }),
        editor: summary({ bytes: 900, latency: 1_040, memory: 1_900 }),
      },
      equivalence,
    ).selected,
    "editor",
  );
  assert.equal(
    decideEditorArtifact(
      {
        full,
        editor: summary({ bytes: 900, latency: 119, memory: 1_900 }),
      },
      equivalence,
    ).selected,
    "editor",
  );
  assert.equal(
    decideEditorArtifact(
      {
        full,
        editor: summary({ bytes: 900, latency: 120, memory: 1_900 }),
      },
      equivalence,
    ).selected,
    "editor",
  );
});

test("summarizes medians and the maximum observed peak memory", () => {
  const runs = abBaRuns([
    { full: mode(1_000, 100, 2_000), editor: mode(900, 90, 1_500) },
    { full: mode(1_200, 120, 2_200), editor: mode(1_000, 100, 1_700) },
  ]);
  const summaries = summarizeEditorArtifactRuns(runs);

  assert.equal(summaries.full.modes.cold.totalTransferBytes, 1_100);
  assert.equal(summaries.editor.modes.cold.workerReadyMs, 95);
  assert.equal(summaries.full.peakMemoryBytes, 2_200);
  assert.equal(summaries.editor.peakMemoryBytes, 1_700);
});

test("requires contiguous alternating AB/BA pairs", () => {
  const valid = validRuns();
  assert.doesNotThrow(() => validateAbBaRuns(valid));
  assert.throws(
    () => validateAbBaRuns(valid.slice(0, 2)),
    /even number of at least two/u,
  );

  const invalid = structuredClone(valid);
  invalid[2].position = 2;
  invalid[3].position = 1;
  assert.throws(() => validateAbBaRuns(invalid), /editor then full/u);

  const duplicatePosition = structuredClone(valid);
  duplicatePosition[1].position = 1;
  assert.throws(
    () => validateAbBaRuns(duplicatePosition),
    /positions 1 and 2/u,
  );
});

test("creates an owned authoritative receipt", () => {
  const input = validReceiptInput();
  const receipt = createEditorArtifactReceipt(input);

  assert.equal(receipt.schemaVersion, EDITOR_ARTIFACT_RECEIPT_SCHEMA_VERSION);
  assert.equal(receipt.equivalence.schemaVersion, 1);
  assert.equal(receipt.equivalence.exact, true);
  assert.equal(receipt.decision.selected, "editor");
  assert.equal(receipt.authority.authoritative, true);
  input.builds.full.staticBytes.rawBytes = 1;
  input.runs[0].cold.network.bodyBytes = 1;
  assert.equal(receipt.builds.full.staticBytes.rawBytes, 50_000);
  assert.equal(receipt.runs[0].cold.network.bodyBytes, 1_000);
  assert.equal(Object.isFrozen(receipt), true);
  assert.equal(Object.isFrozen(receipt.builds.full.staticBytes), true);
  assert.throws(() => {
    receipt.builds.full.staticBytes.rawBytes = 2;
  }, TypeError);
  assert.equal(
    DEFAULT_EDITOR_ARTIFACT_RECEIPT_PATH,
    "target/playground/editor-artifact-measurement/receipt-v2.json",
  );
  assert.equal(
    CHECKED_EDITOR_ARTIFACT_RECEIPT_PATH,
    "docs/workstreams/web-wasm-playground/editor-artifact-receipt-v2.json",
  );
});

test("validates stored derived receipt evidence", () => {
  const receipt = createEditorArtifactReceipt(validReceiptInput());
  assert.deepEqual(validateEditorArtifactReceipt(receipt), receipt);

  const tampered = structuredClone(receipt);
  tampered.decision.selected = "full";
  assert.throws(
    () => validateEditorArtifactReceipt(tampered),
    /derived evidence does not match/u,
  );
});

test("binds artifact authority to deterministic selection inputs and package choice", () => {
  const receipt = createEditorArtifactReceipt(validReceiptInput());
  const verified = verifyEditorArtifactAuthority({
    packageDependencies: {
      "@mermanjs/web": "file:../platforms/web/packages/full",
      "@mermanjs/web-editor": "file:../platforms/web/packages/editor",
      react: "19.2.8",
    },
    packageLock: editorPackageLock(),
    receipt,
    selectionInputs: receipt.selectionInputs,
    workerGraph: workerGraph("@mermanjs/web-editor"),
  });
  assert.equal(verified.selected, "editor");

  assert.throws(
    () =>
      verifyEditorArtifactAuthority({
        packageDependencies: {
          "@mermanjs/web": "file:../platforms/web/packages/full",
          "@mermanjs/web-editor": "file:../platforms/web/packages/editor",
        },
        packageLock: editorPackageLock(),
        receipt,
        selectionInputs: {
          ...receipt.selectionInputs,
          buildRuntimeClosureSha256: digest("stale"),
        },
        workerGraph: workerGraph("@mermanjs/web-editor"),
      }),
    /buildRuntimeClosureSha256 changed/u,
  );
  assert.throws(
    () =>
      verifyEditorArtifactAuthority({
        packageDependencies: {
          "@mermanjs/web": "file:../platforms/web/packages/full",
        },
        packageLock: editorPackageLock(),
        receipt,
        selectionInputs: receipt.selectionInputs,
        workerGraph: workerGraph("@mermanjs/web-editor"),
      }),
    /dependencies do not match/u,
  );
});

test("artifact authority follows indirect runtime package imports", () => {
  const receipt = createEditorArtifactReceipt(validReceiptInput());
  assert.doesNotThrow(() =>
    verifyEditorArtifactAuthority({
      packageDependencies: {
        "@mermanjs/web": "file:../platforms/web/packages/full",
        "@mermanjs/web-editor": "file:../platforms/web/packages/editor",
      },
      packageLock: editorPackageLock(),
      receipt,
      selectionInputs: receipt.selectionInputs,
      workerGraph: workerGraph("@mermanjs/web-editor", { indirect: true }),
    }),
  );
});

test("selection input hashing is order-independent and path-bound", () => {
  const first = { path: "b/input.js", bytes: Buffer.from("same") };
  const second = { path: "a/input.js", bytes: Buffer.from("same") };
  const digestValue = digestEntries([first, second]);

  assert.equal(digestEntries([second, first]), digestValue);
  assert.notEqual(
    digestEntries([
      second,
      { path: first.path, bytes: Buffer.from("changed") },
    ]),
    digestValue,
  );
  assert.notEqual(
    digestEntries([
      second,
      { path: "c/input.js", bytes: first.bytes },
    ]),
    digestValue,
  );
  assert.throws(() => digestEntries([first, first]), /duplicated/u);
});

test("measurement freshness ignores documentation but binds executable inputs", (t) => {
  const repositoryRoot = mkdtempSync(
    path.join(tmpdir(), "merman-editor-artifact-inputs-"),
  );
  t.after(() => rmSync(repositoryRoot, { recursive: true, force: true }));
  writeFixture(
    repositoryRoot,
    "playground/scripts/typescript-source-graph.mjs",
    "export {};\n",
  );
  writeFixture(
    repositoryRoot,
    "playground/vite.editor-artifact-measurement.config.ts",
    "export default {};\n",
  );
  const contractPath =
    "playground/scripts/editor-artifact-measurement/contract.mjs";
  const readmePath = "playground/scripts/editor-artifact-measurement/README.md";
  writeFixture(repositoryRoot, contractPath, "export const schema = 2;\n");
  writeFixture(repositoryRoot, readmePath, "Initial documentation.\n");

  const initial = measurementContractDigest(repositoryRoot);
  writeFixture(repositoryRoot, readmePath, "Rewritten documentation.\n");
  assert.equal(measurementContractDigest(repositoryRoot), initial);

  writeFixture(repositoryRoot, contractPath, "export const schema = 3;\n");
  assert.notEqual(measurementContractDigest(repositoryRoot), initial);
});

test("build freshness covers every planned production runtime entry", () => {
  const graph = {
    files: new Set([
      "src/main.tsx",
      "src/benchmark/corpus-browser.ts",
      "src/benchmark/realm/trusted-merman-entry.ts",
      "src/shared.ts",
    ]),
    edges: [
      {
        external: false,
        from: "src/benchmark/corpus-browser.ts",
        kind: "static",
        to: "src/shared.ts",
      },
      {
        external: false,
        from: "src/benchmark/realm/trusted-merman-entry.ts",
        kind: "static",
        to: "src/shared.ts",
      },
    ],
  };

  assert.deepEqual([...editorArtifactBuildRuntimeClosure(graph)].sort(), [
    "src/benchmark/corpus-browser.ts",
    "src/benchmark/realm/trusted-merman-entry.ts",
    "src/main.tsx",
    "src/shared.ts",
  ]);
});

test("build freshness includes dynamic sources but excludes type-only inputs", () => {
  const graph = {
    files: new Set([
      "src/main.tsx",
      "src/lazy.ts",
      "src/lazy-types.ts",
    ]),
    edges: [
      {
        external: false,
        from: "src/main.tsx",
        kind: "dynamic",
        to: "src/lazy.ts",
      },
      {
        external: false,
        from: "src/lazy.ts",
        kind: "type",
        to: "src/lazy-types.ts",
      },
    ],
  };

  assert.deepEqual(
    [
      ...editorArtifactBuildRuntimeClosure(graph, ["src/main.tsx"]),
    ].sort(),
    ["src/lazy.ts", "src/main.tsx"],
  );
});

test("package provenance freshness binds portable runtime modules", () => {
  assert.deepEqual(
    runtimePackageArtifactPaths({
      javascriptModules: ["package-entries/editor.js", "runtime-core.js"],
      wasmArtifactPaths: [
        "artifacts/wasm/merman_wasm.js",
        "artifacts/wasm/merman_wasm.d.ts",
        "artifacts/wasm/merman_wasm_bg.wasm",
        "artifacts/wasm/merman_wasm_bg.wasm.d.ts",
        "dist/not-in-runtime-closure.js",
      ],
    }),
    [
      "artifacts/wasm/merman_wasm.js",
      "dist/package-entries/editor.js",
      "dist/runtime-core.js",
    ],
  );
});

test("package freshness binds WASM sources without binding platform-specific binaries", () => {
  const provenance = {
    schema_version: 2,
    package: { id: "editor", name: "@mermanjs/web-editor" },
    artifact_profile: "web-editor",
    runtime_capability_ids: ["analysis", "editor"],
    outputs: [],
    artifact_files: [
      {
        path: "dist/package-entries/editor.js",
        bytes: 12,
        sha256: `sha256:${digest("entry")}`,
      },
      {
        path: "artifacts/wasm/merman_wasm_bg.wasm",
        bytes: 34,
        sha256: `sha256:${digest("wasm")}`,
      },
    ],
    wasm: {
      path: "wasm/merman_wasm_bg.wasm",
      input_digest: digest("inputs"),
      source_digest: digest("sources"),
      tool_versions: { rustc: "rustc old" },
    },
  };
  const paths = ["dist/package-entries/editor.js"];
  const contract = runtimePackageProvenanceContract(provenance, paths);
  const platformDrift = structuredClone(provenance);
  platformDrift.artifact_files[1].bytes = 56;
  platformDrift.artifact_files[1].sha256 = `sha256:${digest("other-wasm")}`;
  platformDrift.wasm.input_digest = digest("new-inputs");
  platformDrift.wasm.tool_versions.rustc = "rustc same-version other-host";

  assert.deepEqual(
    runtimePackageProvenanceContract(platformDrift, paths),
    contract,
  );
  const sourceDrift = structuredClone(provenance);
  sourceDrift.wasm.source_digest = digest("new-sources");
  assert.notDeepEqual(
    runtimePackageProvenanceContract(sourceDrift, paths),
    contract,
  );
  const runtimeDrift = structuredClone(provenance);
  runtimeDrift.runtime_capability_ids = ["analysis", "editor", "svg"];
  assert.notDeepEqual(
    runtimePackageProvenanceContract(runtimeDrift, paths),
    contract,
  );
});

test("marks dirty or reused-build receipts as provisional", () => {
  const dirty = validReceiptInput();
  dirty.revision.dirty = true;
  const dirtyReceipt = createEditorArtifactReceipt(dirty);
  assert.equal(dirtyReceipt.authority.authoritative, false);
  assert.match(dirtyReceipt.authority.reasons[0], /dirty/u);

  const reused = validReceiptInput();
  reused.parameters.buildMode = "reuse-existing";
  const reusedReceipt = createEditorArtifactReceipt(reused);
  assert.equal(reusedReceipt.authority.authoritative, false);
  assert.match(reusedReceipt.authority.reasons[0], /reused/u);
});

test("rejects invalid dates, build evidence, network totals, and memory samples", () => {
  const invalidDate = validReceiptInput();
  invalidDate.generatedAt = "today";
  assert.throws(
    () => createEditorArtifactReceipt(invalidDate),
    /canonical ISO date-time/u,
  );

  const invalidBuild = validReceiptInput();
  delete invalidBuild.builds.full.workerBundle.sha256;
  assert.throws(
    () => createEditorArtifactReceipt(invalidBuild),
    /workerBundle must contain exactly/u,
  );

  const leakedEditorWasm = validReceiptInput();
  leakedEditorWasm.builds.editor.workerWasm.file =
    leakedEditorWasm.builds.editor.mainWasm.file;
  assert.throws(
    () => createEditorArtifactReceipt(leakedEditorWasm),
    /distinct Worker WASM/u,
  );

  const invalidNetwork = validReceiptInput();
  invalidNetwork.runs[0].cold.network.bodyBytes += 1;
  assert.throws(
    () => createEditorArtifactReceipt(invalidNetwork),
    /network bodyBytes does not match requests/u,
  );

  const invalidMemory = validReceiptInput();
  invalidMemory.runs[0].warm.peakMemory.samples = [];
  assert.throws(
    () => createEditorArtifactReceipt(invalidMemory),
    /samples must be non-empty/u,
  );
});

test("keeps a zero latency baseline JSON-safe", () => {
  const decision = decideEditorArtifact(
    {
      full: summary({ bytes: 1_000, latency: 0, memory: 2_000 }),
      editor: summary({ bytes: 900, latency: 21, memory: 1_900 }),
    },
    equivalentComparison(),
  );

  assert.equal(decision.selected, "full");
  assert.equal(decision.criteria.primaryLatencies[0].regressionRatio, null);
  assert.doesNotThrow(() => JSON.stringify(decision));
});

function equivalentComparison() {
  return compareEditorArtifactEquivalence(equivalenceVariants());
}

function equivalenceVariants() {
  const full = equivalenceMatrix();
  return { full, editor: structuredClone(full) };
}

function equivalenceMatrix() {
  const families = Array.from(
    { length: EDITOR_ARTIFACT_FAMILY_COUNT },
    (_, index) => {
      const diagramType = `family-${String(index).padStart(2, "0")}`;
      return {
        baselineId: `${diagramType}-baseline`,
        diagramType,
        fixture: `fixtures/${diagramType}/basic.mmd`,
        sourceSha256: digest(`${diagramType}:source`),
        queries: EDITOR_ARTIFACT_QUERY_KINDS.map((kind) => ({
          kind,
          outcome: "result",
          sha256: digest(`${diagramType}:${kind}:result`),
        })),
      };
    },
  );
  return rehashMatrix({
    schemaVersion: EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION,
    familyCount: EDITOR_ARTIFACT_FAMILY_COUNT,
    queryCount: EDITOR_ARTIFACT_QUERY_KINDS.length,
    cellCount:
      EDITOR_ARTIFACT_FAMILY_COUNT * EDITOR_ARTIFACT_QUERY_KINDS.length,
    queryKinds: [...EDITOR_ARTIFACT_QUERY_KINDS],
    families,
  });
}

function rehashMatrix(matrix) {
  const body = {
    schemaVersion: matrix.schemaVersion,
    familyCount: matrix.familyCount,
    queryCount: matrix.queryCount,
    cellCount: matrix.cellCount,
    queryKinds: matrix.queryKinds,
    families: matrix.families,
  };
  return { ...body, aggregateSha256: digest(canonicalStringify(body)) };
}

function digest(value) {
  return createHash("sha256").update(value).digest("hex");
}

function summary({ bytes, latency, memory }) {
  const metrics = {
    firstDiagnosticsMs: latency,
    mainCompileInitializeMs: latency,
    mainFirstResultMs: latency,
    totalTransferBytes: bytes,
    workerCompileInitializeMs: latency,
    workerReadyMs: latency,
  };
  return {
    modes: { cold: { ...metrics }, warm: { ...metrics } },
    peakMemoryBytes: memory,
    peakMemoryScope: "user-agent-specific-memory",
    runCount: 2,
  };
}

function mode(bytes, latency, memory) {
  return {
    firstDiagnosticsMs: latency,
    mainCompileInitializeMs: latency,
    mainFirstResultMs: latency,
    network: {
      bodyBytes: bytes,
      requests: [
        {
          bodyBytes: bytes,
          cacheControl: "no-cache",
          contentEncoding: "gzip",
          finishedWallTimeMs: 1_000 + latency,
          method: "GET",
          pathname: "/index.html",
        },
      ],
    },
    peakMemory: {
      bytes: memory,
      samples: [{ atMs: latency, bytes: memory }],
      scope: "user-agent-specific-memory",
    },
    totalTransferBytes: bytes,
    workerCompileInitializeMs: latency,
    workerReadyMs: latency,
  };
}

function validRuns() {
  return abBaRuns([
    { full: mode(1_000, 100, 2_000), editor: mode(900, 90, 1_500) },
    { full: mode(1_100, 110, 2_100), editor: mode(950, 95, 1_600) },
  ]);
}

function abBaRuns(blocks) {
  return blocks.flatMap((block, index) => {
    const blockNumber = index + 1;
    const order =
      blockNumber % 2 === 1 ? ["full", "editor"] : ["editor", "full"];
    return order.map((variant, position) => ({
      block: blockNumber,
      cold: structuredClone(block[variant]),
      position: position + 1,
      variant,
      warm: structuredClone(block[variant]),
    }));
  });
}

function validReceiptInput() {
  const fullWasm = {
    bytes: 12_000,
    file: "assets/full.wasm",
    sha256: digest("full-wasm"),
    source: "platforms/web/packages/full/artifacts/wasm/merman_wasm_bg.wasm",
  };
  return {
    builds: {
      full: build("full", fullWasm, fullWasm),
      editor: build("editor", fullWasm, {
        bytes: 4_000,
        file: "assets/editor.wasm",
        sha256: digest("editor-wasm"),
        source:
          "platforms/web/packages/editor/artifacts/wasm/merman_wasm_bg.wasm",
      }),
    },
    environment: {
      architecture: "arm64",
      browser: "Chromium 140",
      cpu: "test cpu",
      logicalCpuCount: 8,
      memoryBytes: 16_000_000_000,
      node: "v24.0.0",
      operatingSystem: "darwin test",
      playwright: "1.61.1",
      transferEncoding: "gzip",
    },
    equivalence: equivalenceVariants(),
    generatedAt: "2026-08-05T00:00:00.000Z",
    parameters: {
      blocks: 2,
      browserMode: "headless",
      buildMode: "fresh-dedicated-builds",
      cachePolicy: {
        hashedAssets: "public, max-age=31536000, immutable",
        html: "no-cache",
      },
      coldDefinition: "fresh process and context",
      equivalenceDefinition: "35 families by 11 queries",
      equivalenceEvidence: "editor-language/token-equivalence-v1.json",
      equivalenceEvidenceSha256: digest("evidence"),
      memoryDefinition: "user-agent-specific-memory",
      order: "AB/BA",
      primaryLatencies: [
        "workerReadyMs",
        "firstDiagnosticsMs",
        "mainFirstResultMs",
      ],
      transferDefinition: "gzip response bodies",
      warmDefinition: "same context after about:blank",
    },
    revision: {
      commit: "0123456789abcdef0123456789abcdef01234567",
      dirty: false,
      statusSha256: digest(""),
    },
    runs: validRuns(),
    selectionInputs: validSelectionInputs(),
  };
}

function validSelectionInputs() {
  return {
    schemaVersion: 4,
    buildRuntimeClosureSha256: digest("build-runtime-closure"),
    measurementContractSha256: digest("measurement-contract"),
    workerClosureSha256: digest("worker-closure"),
    fullPackageProvenanceSha256: digest("full-package"),
    editorPackageProvenanceSha256: digest("editor-package"),
    equivalenceEvidenceSha256: digest("evidence"),
  };
}

function editorPackageLock() {
  return {
    packages: {
      "": {
        dependencies: {
          "@mermanjs/web": "file:../platforms/web/packages/full",
          "@mermanjs/web-editor": "file:../platforms/web/packages/editor",
        },
      },
      "node_modules/@mermanjs/web": { link: true },
      "node_modules/@mermanjs/web-editor": { link: true },
    },
  };
}

function workerGraph(packageSpecifier, { indirect = false } = {}) {
  const root = "src/editor/merman-language.worker.ts";
  const owner = "src/editor/worker-runtime.ts";
  return {
    files: new Set(indirect ? [root, owner] : [root]),
    edges: [
      ...(indirect
        ? [
            {
              external: false,
              from: root,
              kind: "static",
              specifier: "./worker-runtime",
              to: owner,
            },
          ]
        : []),
      {
        external: true,
        from: indirect ? owner : root,
        kind: "static",
        specifier: packageSpecifier,
        to: null,
      },
    ],
  };
}

function build(label, mainWasm, workerWasm) {
  return {
    manifestSha256: digest(`${label}:manifest`),
    mainWasm: structuredClone(mainWasm),
    outDir: `target/${label}`,
    staticBytes: { files: 10, gzipBytes: 20_000, rawBytes: 50_000 },
    workerBundle: {
      bytes: 1_000,
      file: `assets/${label}-worker.js`,
      sha256: digest(`${label}:worker`),
    },
    workerWasm: structuredClone(workerWasm),
  };
}

function writeFixture(repositoryRoot, relativePath, contents) {
  const file = path.join(repositoryRoot, relativePath);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, contents);
}
