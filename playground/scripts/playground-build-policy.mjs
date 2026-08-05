import {
  OPAQUE_REALM_ARTIFACT_PLAN,
  pageForKey,
} from "./opaque-realm-artifact-plan.mjs";
import {
  collectSourceClosure,
  createTypeScriptSourceGraph,
} from "./typescript-source-graph.mjs";
import {
  collectManifestClosure,
  manifestChunk,
  ownersOfAsset,
  parseViteManifest,
  requireUniqueManifestSource,
} from "./vite-manifest-graph.mjs";

export const OPTIONAL_FEATURE_SOURCES = Object.freeze({
  benchmark: "src/components/BenchWorkbench.tsx",
  config: "src/components/ConfigEditorFeature.tsx",
  examples: "src/components/ExampleGallery.tsx",
});

export const PLAYGROUND_BUILD_SOURCES = Object.freeze({
  benchmarkBootstrap: "src/benchmark/realm/bootstrap.ts",
  benchmarkMermanArtifact: "src/benchmark/realm/merman-engine-artifact.ts",
  benchmarkMermaidArtifact: "src/benchmark/realm/opaque-mermaid-artifact.ts",
  benchmarkMermanAdapter: "src/benchmark/realm/engines/merman.ts",
  benchmarkMermaidAdapter: "src/benchmark/realm/engines/mermaid.ts",
  compareArtifact: "src/runtime/realm/opaque-compare-artifact.ts",
  wasmBinary:
    "../platforms/web/packages/full/artifacts/wasm/merman_wasm_bg.wasm",
  wasmShim: "../platforms/web/packages/full/artifacts/wasm/merman_wasm.js",
});

const BENCHMARK_ADAPTER_FORBIDDEN_SOURCES = Object.freeze([
  "src/main.tsx",
  "src/lib/bench-runner.ts",
  "src/runtime/RenderCoordinatorBridge.tsx",
  "src/runtime/mermaid-realm-controller.ts",
  "src/runtime/mermaid-realm.ts",
  "src/runtime/realm/compare-bootstrap.ts",
  "src/runtime/realm/parent-channel.ts",
  "src/runtime/render-coordinator-browser.ts",
  "src/runtime/render-coordinator.ts",
  "src/runtime/use-render-coordinator.ts",
  "src/benchmark/realm/controller.ts",
  "src/benchmark/controller.ts",
  "src/benchmark/sample-plan.ts",
  "src/benchmark/statistics.ts",
  "src/benchmark/report.ts",
  PLAYGROUND_BUILD_SOURCES.benchmarkMermanArtifact,
]);

export function inspectBenchmarkSourceBoundaries(rootDir) {
  const entries = [
    PLAYGROUND_BUILD_SOURCES.benchmarkBootstrap,
    PLAYGROUND_BUILD_SOURCES.benchmarkMermanAdapter,
    PLAYGROUND_BUILD_SOURCES.benchmarkMermaidAdapter,
  ];
  const graph = createTypeScriptSourceGraph({ rootDir, entries });
  const staticFiles = collectSourceClosure(
    graph,
    [PLAYGROUND_BUILD_SOURCES.benchmarkBootstrap],
  );
  const adapterOwnership = Object.freeze({
    merman: collectSourceClosure(
      graph,
      [PLAYGROUND_BUILD_SOURCES.benchmarkMermanAdapter],
      { includeDynamic: true, includeTypeOnly: true },
    ),
    mermaid: collectSourceClosure(
      graph,
      [PLAYGROUND_BUILD_SOURCES.benchmarkMermaidAdapter],
      { includeDynamic: true, includeTypeOnly: true },
    ),
  });
  const violations = [];
  for (const [engine, files] of Object.entries(adapterOwnership)) {
    const otherAdapter =
      engine === "merman"
        ? PLAYGROUND_BUILD_SOURCES.benchmarkMermaidAdapter
        : PLAYGROUND_BUILD_SOURCES.benchmarkMermanAdapter;
    if (files.has(otherAdapter)) {
      violations.push(`${capitalize(engine)} adapter reaches ${otherAdapter}.`);
    }
    for (const source of BENCHMARK_ADAPTER_FORBIDDEN_SOURCES) {
      if (files.has(source)) {
        violations.push(
          `${capitalize(engine)} adapter reaches forbidden source ${source}.`,
        );
      }
    }
  }
  return { graph, staticFiles, adapterOwnership, violations };
}

export function inspectOptionalFeatureManifest(
  manifest,
  entrySource = pageForKey(OPAQUE_REALM_ARTIFACT_PLAN, "playground").source,
) {
  const violations = [];
  let graph;
  try {
    graph = asManifestGraph(manifest);
  } catch (error) {
    violations.push(errorMessage(error));
    return emptyOptionalResult(violations);
  }
  const entryKey = uniqueSource(
    graph,
    entrySource,
    (chunk) => chunk.isEntry === true,
    violations,
  );
  if (entryKey === null) return emptyOptionalResult(violations);

  const initialStaticKeys = collectManifestClosure(graph, [entryKey], "static");
  const initialReachableKeys = collectManifestClosure(
    graph,
    [entryKey],
    "reachable",
  );
  const initialStaticFiles = new Set(
    [...initialStaticKeys].map((key) => manifestChunk(graph, key).file),
  );
  const featureRoots = {};
  for (const [feature, source] of Object.entries(OPTIONAL_FEATURE_SOURCES)) {
    const root = uniqueSource(graph, source, () => true, violations);
    if (root === null) continue;
    featureRoots[feature] = root;
    if (
      initialStaticKeys.has(root) ||
      initialStaticFiles.has(manifestChunk(graph, root).file)
    ) {
      violations.push(`${feature} is present in the initial static closure.`);
    }
    if (!initialReachableKeys.has(root)) {
      violations.push(`${feature} is not dynamically reachable from ${entrySource}.`);
    }
  }
  return {
    graph,
    entryKey,
    featureRoots: Object.freeze(featureRoots),
    initialReachableKeys,
    initialStaticKeys,
    violations,
  };
}

export function inspectPlaygroundEmittedGraph(manifest) {
  const optional = inspectOptionalFeatureManifest(manifest);
  const violations = [...optional.violations];
  if (!optional.graph || optional.entryKey === null) {
    return { ...optional, pageEntries: Object.freeze({}), violations };
  }
  const graph = optional.graph;
  const pageEntries = {};
  for (const page of OPAQUE_REALM_ARTIFACT_PLAN.pages) {
    const key = uniqueSource(
      graph,
      page.source,
      (chunk) => chunk.isEntry === true,
      violations,
    );
    if (key !== null) pageEntries[page.key] = key;
  }
  const artifactRoots = Object.freeze({
    compare: uniqueSource(
      graph,
      PLAYGROUND_BUILD_SOURCES.compareArtifact,
      () => true,
      violations,
    ),
    benchmarkMermaid: uniqueSource(
      graph,
      PLAYGROUND_BUILD_SOURCES.benchmarkMermaidArtifact,
      () => true,
      violations,
    ),
    benchmarkMerman: uniqueSource(
      graph,
      PLAYGROUND_BUILD_SOURCES.benchmarkMermanArtifact,
      () => true,
      violations,
    ),
  });
  const wasmBinary = uniqueSource(
    graph,
    PLAYGROUND_BUILD_SOURCES.wasmBinary,
    () => true,
    violations,
  );
  const wasmShim = uniqueSource(
    graph,
    PLAYGROUND_BUILD_SOURCES.wasmShim,
    () => true,
    violations,
  );
  if (
    Object.values(pageEntries).length !== OPAQUE_REALM_ARTIFACT_PLAN.pages.length ||
    Object.values(artifactRoots).some((value) => value === null) ||
    wasmBinary === null ||
    wasmShim === null ||
    !optional.featureRoots.benchmark
  ) {
    return {
      ...optional,
      artifactRoots,
      pageEntries: Object.freeze(pageEntries),
      violations,
    };
  }

  const corpusEntry = pageEntries.benchmarkCorpus;
  const benchmarkEntry = pageEntries.benchmarkRealm;
  const corpusStatic = collectManifestClosure(graph, [corpusEntry], "static");
  const corpusReachable = collectManifestClosure(
    graph,
    [corpusEntry],
    "reachable",
  );
  const benchmarkStatic = collectManifestClosure(
    graph,
    [benchmarkEntry],
    "static",
  );
  const benchmarkReachable = collectManifestClosure(
    graph,
    [benchmarkEntry],
    "reachable",
  );
  const benchmarkFeatureReachable = collectManifestClosure(
    graph,
    [optional.featureRoots.benchmark],
    "reachable",
  );
  const compareStatic = collectManifestClosure(
    graph,
    [artifactRoots.compare],
    "static",
  );
  const benchmarkMermaidStatic = collectManifestClosure(
    graph,
    [artifactRoots.benchmarkMermaid],
    "static",
  );
  const benchmarkMermanStatic = collectManifestClosure(
    graph,
    [artifactRoots.benchmarkMerman],
    "static",
  );

  for (const root of Object.values(artifactRoots)) {
    forbid(optional.initialStaticKeys, root, "initial static closure", violations);
  }
  requireMember(
    optional.initialReachableKeys,
    artifactRoots.compare,
    "Compare artifact is not dynamically reachable from the Playground entry.",
    violations,
  );
  forbid(
    benchmarkFeatureReachable,
    artifactRoots.compare,
    "Benchmark feature reachable closure",
    violations,
  );
  forbid(
    corpusReachable,
    artifactRoots.compare,
    "Benchmark corpus reachable closure",
    violations,
  );
  for (const root of [artifactRoots.benchmarkMermaid, artifactRoots.benchmarkMerman]) {
    requireMember(
      benchmarkFeatureReachable,
      root,
      `Benchmark feature cannot reach ${root}.`,
      violations,
    );
    requireMember(
      corpusReachable,
      root,
      `Benchmark corpus cannot reach ${root}.`,
      violations,
    );
    forbid(corpusStatic, root, "Benchmark corpus static closure", violations);
    forbid(benchmarkStatic, root, "trusted Benchmark realm", violations);
  }
  forbid(
    compareStatic,
    artifactRoots.benchmarkMermaid,
    "Compare artifact closure",
    violations,
  );
  forbid(
    benchmarkMermaidStatic,
    artifactRoots.compare,
    "Benchmark Mermaid artifact closure",
    violations,
  );
  if (!sameSet(benchmarkStatic, benchmarkReachable)) {
    violations.push("Trusted Benchmark realm must not own dynamic module roots.");
  }
  for (const closure of [
    optional.initialStaticKeys,
    corpusStatic,
    benchmarkStatic,
  ]) {
    forbid(closure, wasmShim, "eager page closure", violations);
    forbid(closure, wasmBinary, "eager page closure", violations);
  }
  forbid(
    benchmarkMermanStatic,
    wasmShim,
    "Merman artifact static closure",
    violations,
  );
  const wasmFile = manifestChunk(graph, wasmBinary).file;
  const wasmOwners = ownersOfAsset(graph, wasmFile);
  if (wasmOwners.length !== 1) {
    violations.push(
      `Production WASM must have exactly one manifest owner for ${wasmFile}; found ${wasmOwners.length}.`,
    );
  } else if (!benchmarkMermanStatic.has(wasmOwners[0])) {
    violations.push(
      `Production WASM owner ${wasmOwners[0]} is outside the Merman artifact closure.`,
    );
  }

  return {
    ...optional,
    artifactRoots,
    benchmarkFeatureReachable,
    benchmarkMermanStatic,
    benchmarkStatic,
    corpusReachable,
    corpusStatic,
    pageEntries: Object.freeze(pageEntries),
    violations,
    wasmBinary,
    wasmShim,
  };
}

function asManifestGraph(value) {
  return value?.chunks ? value : parseViteManifest(value);
}

function uniqueSource(graph, source, predicate, violations) {
  try {
    return requireUniqueManifestSource(graph, source, predicate);
  } catch (error) {
    violations.push(errorMessage(error));
    return null;
  }
}

function emptyOptionalResult(violations) {
  return {
    graph: null,
    entryKey: null,
    featureRoots: Object.freeze({}),
    initialReachableKeys: new Set(),
    initialStaticKeys: new Set(),
    violations,
  };
}

function forbid(closure, key, label, violations) {
  if (closure.has(key)) violations.push(`${label} reaches forbidden node ${key}.`);
}

function requireMember(closure, key, message, violations) {
  if (!closure.has(key)) violations.push(message);
}

function sameSet(left, right) {
  return left.size === right.size && [...left].every((value) => right.has(value));
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function capitalize(value) {
  return value[0].toUpperCase() + value.slice(1);
}
