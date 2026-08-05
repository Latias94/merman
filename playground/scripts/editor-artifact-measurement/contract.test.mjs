import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";

import {
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
} from "./contract.mjs";
import {
  canonicalStringify,
  canonicalize,
  compareCanonicalStrings,
} from "./equivalence-shared.mjs";

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
    "target/playground/editor-artifact-measurement/receipt-v1.json",
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
