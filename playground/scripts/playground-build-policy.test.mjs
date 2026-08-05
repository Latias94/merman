import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  inspectBenchmarkSourceBoundaries,
  inspectOptionalFeatureManifest,
  inspectPlaygroundEmittedGraph,
  OPTIONAL_FEATURE_SOURCES,
  PLAYGROUND_BUILD_SOURCES,
} from "./playground-build-policy.mjs";

const playgroundRoot = path.resolve(import.meta.dirname, "..");

test("current benchmark source ownership resolves through TypeScript", () => {
  const result = inspectBenchmarkSourceBoundaries(playgroundRoot);
  assert.deepEqual(result.violations, []);
  assert.ok(
    result.adapterOwnership.merman.has(
      PLAYGROUND_BUILD_SOURCES.benchmarkMermanAdapter,
    ),
  );
  assert.ok(
    result.adapterOwnership.mermaid.has(
      PLAYGROUND_BUILD_SOURCES.benchmarkMermaidAdapter,
    ),
  );
});

test("type-only ownership cannot bypass the benchmark boundary", async (t) => {
  const root = await createSourcePolicyFixture(t);
  const result = inspectBenchmarkSourceBoundaries(root);
  assert.match(result.violations.join("\n"), /forbidden source src\/main\.tsx/u);
});

test("optional features remain dynamic while realm artifacts retain separate owners", () => {
  const manifest = validManifest();
  assert.deepEqual(inspectOptionalFeatureManifest(manifest).violations, []);
  assert.deepEqual(inspectPlaygroundEmittedGraph(manifest).violations, []);
});

test("emitted policy rejects initial optional code and cross-realm artifact edges", () => {
  const eager = validManifest();
  eager["index.html"].imports.push(OPTIONAL_FEATURE_SOURCES.benchmark);
  assert.match(
    inspectPlaygroundEmittedGraph(eager).violations.join("\n"),
    /benchmark is present in the initial static closure/u,
  );

  const crossed = validManifest();
  crossed.compare.imports.push("benchmark-mermaid");
  assert.match(
    inspectPlaygroundEmittedGraph(crossed).violations.join("\n"),
    /Compare artifact closure reaches forbidden node benchmark-mermaid/u,
  );
});

test("emitted policy rejects Benchmark closures that reach the Compare artifact", () => {
  const featureCrossed = validManifest();
  featureCrossed[OPTIONAL_FEATURE_SOURCES.benchmark].dynamicImports.push(
    "compare",
  );
  assert.match(
    inspectPlaygroundEmittedGraph(featureCrossed).violations.join("\n"),
    /Benchmark feature reachable closure reaches forbidden node compare/u,
  );

  const corpusCrossed = validManifest();
  corpusCrossed["benchmark-corpus.html"].dynamicImports.push("compare");
  assert.match(
    inspectPlaygroundEmittedGraph(corpusCrossed).violations.join("\n"),
    /Benchmark corpus reachable closure reaches forbidden node compare/u,
  );
});

test("emitted policy fails closed on missing entries and ambiguous WASM ownership", () => {
  const missing = validManifest();
  delete missing["benchmark.html"];
  assert.match(
    inspectPlaygroundEmittedGraph(missing).violations.join("\n"),
    /benchmark\.html; found 0/u,
  );

  const duplicateOwner = validManifest();
  duplicateOwner["benchmark-merman"].assets = [
    "assets/engine.wasm",
  ];
  assert.match(
    inspectPlaygroundEmittedGraph(duplicateOwner).violations.join("\n"),
    /found 2/u,
  );

  const missingOwner = validManifest();
  missingOwner["web-full"].assets = [];
  assert.match(
    inspectPlaygroundEmittedGraph(missingOwner).violations.join("\n"),
    /found 0/u,
  );
});

function validManifest() {
  return {
    "index.html": {
      file: "assets/index.js",
      src: "index.html",
      isEntry: true,
      imports: ["shared"],
      dynamicImports: [
        ...Object.values(OPTIONAL_FEATURE_SOURCES),
        "compare",
      ],
    },
    "benchmark-corpus.html": {
      file: "assets/corpus.js",
      src: "benchmark-corpus.html",
      isEntry: true,
      imports: ["shared"],
      dynamicImports: ["benchmark-mermaid", "benchmark-merman"],
    },
    "benchmark.html": {
      file: "assets/realm.js",
      src: "benchmark.html",
      isEntry: true,
      imports: ["realm-shared"],
    },
    shared: { file: "assets/shared.js" },
    "realm-shared": { file: "assets/realm-shared.js" },
    [OPTIONAL_FEATURE_SOURCES.benchmark]: {
      file: "assets/bench.js",
      src: OPTIONAL_FEATURE_SOURCES.benchmark,
      isDynamicEntry: true,
      imports: ["shared"],
      dynamicImports: ["benchmark-mermaid", "benchmark-merman"],
    },
    [OPTIONAL_FEATURE_SOURCES.config]: {
      file: "assets/config.js",
      src: OPTIONAL_FEATURE_SOURCES.config,
      isDynamicEntry: true,
      imports: ["shared"],
    },
    [OPTIONAL_FEATURE_SOURCES.examples]: {
      file: "assets/examples.js",
      src: OPTIONAL_FEATURE_SOURCES.examples,
      isDynamicEntry: true,
      imports: ["shared"],
    },
    compare: {
      file: "assets/compare.js",
      src: PLAYGROUND_BUILD_SOURCES.compareArtifact,
      isDynamicEntry: true,
      imports: ["artifact-shared"],
    },
    "benchmark-mermaid": {
      file: "assets/benchmark-mermaid.js",
      src: PLAYGROUND_BUILD_SOURCES.benchmarkMermaidArtifact,
      isDynamicEntry: true,
      imports: ["artifact-shared"],
    },
    "benchmark-merman": {
      file: "assets/benchmark-merman.js",
      src: PLAYGROUND_BUILD_SOURCES.benchmarkMermanArtifact,
      isDynamicEntry: true,
      imports: ["artifact-shared", "web-full"],
    },
    "artifact-shared": { file: "assets/artifact-shared.js" },
    "web-full": {
      file: "assets/web-full.js",
      dynamicImports: ["wasm-shim"],
      assets: ["assets/engine.wasm"],
    },
    "wasm-shim": {
      file: "assets/wasm.js",
      src: PLAYGROUND_BUILD_SOURCES.wasmShim,
      isDynamicEntry: true,
    },
    "wasm-binary": {
      file: "assets/engine.wasm",
      src: PLAYGROUND_BUILD_SOURCES.wasmBinary,
    },
  };
}

async function createSourcePolicyFixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), "merman-source-policy-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  await writeFile(
    path.join(root, "tsconfig.json"),
    JSON.stringify({
      compilerOptions: {
        allowImportingTsExtensions: true,
        jsx: "react-jsx",
        module: "ESNext",
        moduleResolution: "Bundler",
        noEmit: true,
        target: "ES2022",
      },
    }),
  );
  const files = {
    [PLAYGROUND_BUILD_SOURCES.benchmarkBootstrap]: "export {};\n",
    [PLAYGROUND_BUILD_SOURCES.benchmarkMermanAdapter]:
      'import type { AppBoundary } from "../../../main.tsx";\nexport type Owned = AppBoundary;\n',
    [PLAYGROUND_BUILD_SOURCES.benchmarkMermaidAdapter]: "export {};\n",
    "src/main.tsx": "export interface AppBoundary { value: string }\n",
  };
  for (const [file, source] of Object.entries(files)) {
    const destination = path.join(root, file);
    await mkdir(path.dirname(destination), { recursive: true });
    await writeFile(destination, source);
  }
  return root;
}
