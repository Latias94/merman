/**
 * Fails local and CI static builds when Vite output is missing the wasm-bindgen
 * assets needed by the browser renderer.
 *
 * Override: SKIP_VERIFY_DIST_WASM=1
 */
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  BENCHMARK_ADAPTER_FORBIDDEN_SOURCES,
  BENCHMARK_SOURCES,
  MERMAN_WASM_SHIM_IMPORT,
  inspectBenchmarkSourceBoundaries,
} from "./benchmark-build-graph.mjs";
import { verifyHtmlCsp } from "./csp-policy.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(__dirname, "..");
const DIST = path.join(ROOT, "dist");
const INDEX_HTML = path.join(DIST, "index.html");
const COMPARE_HTML = path.join(DIST, "compare-realm.html");
const BENCHMARK_HTML = path.join(DIST, "benchmark.html");
const MANIFEST_FILE = path.join(DIST, ".vite", "manifest.json");
const ASSETS = path.join(DIST, "assets");

const MERMAN_WASM_SHIM_SOURCE = "../platforms/web/pkg/merman_wasm.js";
const MERMAN_WASM_BINARY_SOURCE =
  "../platforms/web/pkg/merman_wasm_bg.wasm";

if (process.env.SKIP_VERIFY_DIST_WASM === "1") {
  process.exit(0);
}

function isNonEmptyFile(file) {
  try {
    return existsSync(file) && statSync(file).isFile() && statSync(file).size > 0;
  } catch {
    return false;
  }
}

function relativeToDist(file) {
  return path.relative(DIST, file).replaceAll(path.sep, "/");
}

function resolveDistPath(assetPath) {
  const withoutOrigin = assetPath.replace(/^https?:\/\/[^/]+/i, "");
  const withoutQuery = withoutOrigin.split(/[?#]/, 1)[0];
  const withoutBase = withoutQuery.replace(/^\/merman\//, "").replace(/^\//, "");
  return path.join(DIST, withoutBase);
}

function fail(lines) {
  console.error(["[merman-playground] dist WASM verification failed.", ...lines].join("\n"));
  process.exit(1);
}

function verifyOwnedEditorWorkers() {
  const workerAssets = readdirSync(ASSETS)
    .filter((file) => /\.worker-[\w-]+\.js$/.test(file))
    .sort();
  const requiredWorkers = [
    /^editor\.worker-[\w-]+\.js$/,
    /^json\.worker-[\w-]+\.js$/,
    /^merman-language\.worker-[\w-]+\.js$/,
  ];
  const missing = requiredWorkers.filter(
    (pattern) => !workerAssets.some((file) => pattern.test(file)),
  );
  const unexpected = workerAssets.filter(
    (file) => !requiredWorkers.some((pattern) => pattern.test(file)),
  );
  if (missing.length > 0 || unexpected.length > 0) {
    fail([
      `  Required local editor workers missing: ${missing.map(String).join(", ") || "none"}`,
      `  Unowned Monaco workers emitted: ${unexpected.join(", ") || "none"}`,
    ]);
  }

  for (const file of workerAssets) {
    const source = readFileSync(path.join(ASSETS, file), "utf8");
    if (/cdn\.jsdelivr\.net/i.test(source)) {
      fail([`  Editor worker contains the forbidden Monaco CDN URL: ${file}`]);
    }
  }
}

if (!isNonEmptyFile(INDEX_HTML)) {
  fail([
    `  Missing index.html: ${INDEX_HTML}`,
    "  Run `npm run build --prefix playground` before publishing the static artifact.",
  ]);
}
if (!isNonEmptyFile(COMPARE_HTML)) {
  fail([`  Missing Compare realm entry: ${COMPARE_HTML}`]);
}
if (!isNonEmptyFile(BENCHMARK_HTML)) {
  fail([`  Missing Benchmark realm entry: ${BENCHMARK_HTML}`]);
}
if (!isNonEmptyFile(MANIFEST_FILE)) {
  fail([`  Missing Vite build manifest: ${MANIFEST_FILE}`]);
}

const manifest = loadBuildManifest();
verifyOwnedEditorWorkers();
const indexEntry = requireManifestEntry(manifest, "index.html");
const compareEntry = requireManifestEntry(manifest, "compare-realm.html");
const benchmarkEntry = requireManifestEntry(manifest, "benchmark.html");
const benchmarkSourceBoundaries = loadBenchmarkSourceBoundaries();
const wasmModule = requireManifestModule(manifest, MERMAN_WASM_BINARY_SOURCE);
const shimModule = requireManifestModule(manifest, MERMAN_WASM_SHIM_SOURCE);
const wasm = path.join(DIST, wasmModule.chunk.file);
const shim = path.join(DIST, shimModule.chunk.file);

const indexHtml = readFileSync(INDEX_HTML, "utf8");
const compareHtml = readFileSync(COMPARE_HTML, "utf8");
const benchmarkHtml = readFileSync(BENCHMARK_HTML, "utf8");
for (const [fileName, html] of [
  ["index.html", indexHtml],
  ["compare-realm.html", compareHtml],
  ["benchmark.html", benchmarkHtml],
]) {
  const violations = verifyHtmlCsp(fileName, html);
  if (violations.length > 0) {
    fail([
      `  ${fileName} violates the production CSP contract:`,
      ...violations.map((violation) => `    - ${violation}`),
    ]);
  }
}
const indexAssets = htmlExecutableAssets(indexHtml);
const compareAssets = htmlExecutableAssets(compareHtml);
const benchmarkAssets = htmlExecutableAssets(benchmarkHtml);
const entryScripts = indexAssets.filter((asset) => asset.kind === "script").map(
  (asset) => asset.url,
);

if (entryScripts.length === 0) {
  fail(["  index.html does not reference any JavaScript entry script."]);
}

for (const script of entryScripts) {
  const file = resolveDistPath(script);
  if (!isNonEmptyFile(file)) {
    fail([`  index.html references a missing script: ${script}`]);
  }
}

for (const { label, assets } of [
  { label: "index.html", assets: indexAssets },
  { label: "compare-realm.html", assets: compareAssets },
  { label: "benchmark.html", assets: benchmarkAssets },
]) {
  for (const asset of assets) {
    if (/^https?:\/\//i.test(asset.url)) {
      fail([`  ${label} references an external executable asset: ${asset.url}`]);
    }
    if (!isNonEmptyFile(resolveDistPath(asset.url))) {
      fail([`  ${label} references a missing ${asset.kind}: ${asset.url}`]);
    }
  }
}

const compareScripts = compareAssets
  .filter((asset) => asset.kind === "script")
  .map((asset) => relativeToDist(resolveDistPath(asset.url)));
if (compareScripts.length !== 1) {
  fail(["  compare-realm.html must reference exactly one bootstrap script."]);
}
if (compareScripts[0] !== compareEntry.chunk.file) {
  fail(["  compare-realm.html does not reference its manifest entry chunk."]);
}

const benchmarkScripts = benchmarkAssets
  .filter((asset) => asset.kind === "script")
  .map((asset) => relativeToDist(resolveDistPath(asset.url)));
if (benchmarkScripts.length !== 1) {
  fail(["  benchmark.html must reference exactly one bootstrap script."]);
}
if (benchmarkScripts[0] !== benchmarkEntry.chunk.file) {
  fail(["  benchmark.html does not reference its manifest entry chunk."]);
}

const indexScripts = indexAssets
  .filter((asset) => asset.kind === "script")
  .map((asset) => relativeToDist(resolveDistPath(asset.url)));
if (!indexScripts.includes(indexEntry.chunk.file)) {
  fail(["  index.html does not reference its manifest entry chunk."]);
}

const indexStatic = collectManifestClosure(manifest, [indexEntry.key], false);
const indexReachable = collectManifestClosure(manifest, [indexEntry.key], true);
const compareStatic = collectManifestClosure(manifest, [compareEntry.key], false);
const compareDynamicRoots = [...compareStatic].flatMap(
  (key) => manifest[key].dynamicImports ?? [],
);
const compareDynamic = collectManifestClosure(manifest, compareDynamicRoots, true);
const benchmarkStatic = collectManifestClosure(
  manifest,
  [benchmarkEntry.key],
  false,
);
verifyHtmlStaticClosure("index.html", indexAssets, indexStatic, manifest);
verifyHtmlStaticClosure(
  "compare-realm.html",
  compareAssets,
  compareStatic,
  manifest,
);
verifyHtmlStaticClosure(
  "benchmark.html",
  benchmarkAssets,
  benchmarkStatic,
  manifest,
);
const benchmarkDynamicRoots = [...benchmarkStatic].flatMap(
  (key) => manifest[key].dynamicImports ?? [],
);
const uniqueBenchmarkDynamicRoots = new Set(benchmarkDynamicRoots);
if (
  uniqueBenchmarkDynamicRoots.size !== 2 ||
  [...uniqueBenchmarkDynamicRoots].some(
    (key) => !isBenchmarkAdapter(key, manifest[key]),
  )
) {
  fail([
    "  Benchmark bootstrap must expose exactly two dynamic adapter roots.",
  ]);
}
const benchmarkDynamic = collectManifestClosure(
  manifest,
  uniqueBenchmarkDynamicRoots,
  true,
);
const eagerReferenceChunks = new Set(
  [...indexReachable, ...compareStatic, ...benchmarkStatic].filter((key) =>
    isReferenceEngineModule(key, manifest[key])
  )
);
if (eagerReferenceChunks.size > 0) {
  fail([
    `  Reference engine is statically reachable: ${formatManifestKeys(eagerReferenceChunks)}`,
  ]);
}

const eagerMermanChunks = [...indexStatic].filter((key) =>
  isMermanEngineModule(key, manifest[key])
);
if (eagerMermanChunks.length > 0) {
  fail([`  Merman WASM is statically reachable: ${eagerMermanChunks.join(", ")}`]);
}

const benchmarkEagerEngineChunks = [...benchmarkStatic].filter(
  (key) =>
    isBenchmarkAdapter(key, manifest[key]) ||
    isMermanEngineModule(key, manifest[key]) ||
    isMermaidPackageModule(key, manifest[key]),
);
if (benchmarkEagerEngineChunks.length > 0) {
  fail([
    `  Benchmark engine code is statically reachable: ${benchmarkEagerEngineChunks.join(", ")}`,
  ]);
}

const mermaidAdapters = Object.entries(manifest)
  .filter(([key, chunk]) => isMermaidAdapter(key, chunk))
  .map(([key]) => key);
if (mermaidAdapters.length !== 1 || !compareDynamic.has(mermaidAdapters[0])) {
  fail(["  Compare must dynamically reach exactly one Mermaid realm adapter."]);
}

const compareReachable = new Set([...compareStatic, ...compareDynamic]);
const compareMermanChunks = [...compareReachable].filter((key) =>
  isMermanEngineModule(key, manifest[key])
);
if (compareMermanChunks.length > 0) {
  fail([`  Compare can reach Merman WASM: ${compareMermanChunks.join(", ")}`]);
}

const referenceEngineChunks = new Set(
  [...compareDynamic].filter((key) => isMermaidPackageModule(key, manifest[key]))
);
if (referenceEngineChunks.size === 0) {
  fail(["  Compare has no dynamically reachable local Mermaid engine chunks."]);
}

const benchmarkAdapters = Object.entries(manifest)
  .filter(([key, chunk]) => isBenchmarkAdapter(key, chunk))
  .map(([key]) => key);
if (
  benchmarkAdapters.length !== 2 ||
  benchmarkAdapters.some((key) => !benchmarkDynamic.has(key))
) {
  fail(["  Benchmark must dynamically reach exactly two engine adapters."]);
}

const benchmarkMermanAdapter = benchmarkAdapters.find((key) =>
  hasManifestSource(key, manifest[key], BENCHMARK_SOURCES.mermanAdapter),
);
const benchmarkMermaidAdapter = benchmarkAdapters.find((key) =>
  hasManifestSource(key, manifest[key], BENCHMARK_SOURCES.mermaidAdapter),
);
if (!benchmarkMermanAdapter || !benchmarkMermaidAdapter) {
  fail(["  Benchmark engine adapter identities are incomplete."]);
}

const mermanAdapterDynamicRoots = [
  ...new Set(manifest[benchmarkMermanAdapter].dynamicImports ?? []),
];
const mermanShimRoots = mermanAdapterDynamicRoots.filter((key) =>
  hasManifestSource(key, manifest[key], MERMAN_WASM_SHIM_SOURCE),
);
const mermanFacadeRoots = mermanAdapterDynamicRoots.filter(
  (key) => !mermanShimRoots.includes(key),
);
if (
  benchmarkSourceBoundaries.directMermanDynamicImports.length !== 2 ||
  mermanAdapterDynamicRoots.length !== 2 ||
  mermanShimRoots.length !== 1 ||
  mermanFacadeRoots.length !== 1
) {
  fail([
    `  Merman adapter source imports ${MERMAN_WASM_SHIM_IMPORT} and the Web root facade, but its manifest must expose exactly one dynamic root for each.`,
  ]);
}
const [mermanWebFacadeRoot] = mermanFacadeRoots;
if (benchmarkStatic.has(mermanWebFacadeRoot)) {
  fail([
    `  Benchmark bootstrap can statically reach the @mermanjs/web root facade: ${mermanWebFacadeRoot}`,
  ]);
}

const benchmarkMermanClosure = collectLazyOperationClosure(
  manifest,
  [benchmarkMermanAdapter],
);
const benchmarkMermaidClosure = collectLazyOperationClosure(
  manifest,
  [benchmarkMermaidAdapter],
);
verifyAdapterManifestBoundary(
  "Merman",
  benchmarkMermanClosure,
  manifest,
  indexEntry.key,
  compareEntry.key,
);
verifyAdapterManifestBoundary(
  "Mermaid",
  benchmarkMermaidClosure,
  manifest,
  indexEntry.key,
  compareEntry.key,
);
if (
  benchmarkMermanClosure.has(benchmarkMermaidAdapter) ||
  [...benchmarkMermanClosure].some((key) =>
    isMermaidPackageModule(key, manifest[key]),
  )
) {
  fail(["  Merman benchmark adapter can reach the Mermaid engine."]);
}
if (
  benchmarkMermaidClosure.has(benchmarkMermanAdapter) ||
  [...benchmarkMermaidClosure].some((key) =>
    isMermanEngineModule(key, manifest[key]),
  )
) {
  fail(["  Mermaid benchmark adapter can reach Merman WASM."]);
}
if (
  ![...benchmarkMermanClosure].some((key) =>
    isMermanEngineModule(key, manifest[key]),
  ) ||
  ![...benchmarkMermaidClosure].some((key) =>
    isMermaidPackageModule(key, manifest[key]),
  )
) {
  fail(["  Benchmark adapters do not reach their selected local engines."]);
}

const wasmName = path.basename(wasm);
const shimReferencesWasm = readFileSync(shim, "utf8").includes(wasmName);

if (!shimReferencesWasm) {
  fail([`  The wasm-bindgen shim does not reference the WASM binary: ${wasmName}`]);
}

console.log(
  [
    "[merman-playground] dist WASM present.",
    `  WASM: ${relativeToDist(wasm)}`,
    `  JS shim: ${relativeToDist(shim)}`,
    `  Compare entry: ${relativeToDist(COMPARE_HTML)}`,
    `  Benchmark entry: ${relativeToDist(BENCHMARK_HTML)}`,
    `  Dynamic reference chunks: ${referenceEngineChunks.size}`,
  ].join("\n"),
);

function loadBuildManifest() {
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(MANIFEST_FILE, "utf8"));
  } catch (error) {
    fail([`  Invalid Vite build manifest: ${errorMessage(error)}`]);
  }
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    fail(["  Vite build manifest must be an object."]);
  }
  for (const [key, chunk] of Object.entries(manifest)) {
    if (!chunk || typeof chunk !== "object" || typeof chunk.file !== "string") {
      fail([`  Invalid Vite manifest chunk: ${key}`]);
    }
    for (const field of ["imports", "dynamicImports"]) {
      if (
        chunk[field] !== undefined &&
        (!Array.isArray(chunk[field]) ||
          chunk[field].some((value) => typeof value !== "string"))
      ) {
        fail([`  Invalid ${field} list for Vite manifest chunk: ${key}`]);
      }
    }
    if (!isNonEmptyFile(path.join(DIST, chunk.file))) {
      fail([`  Vite manifest references a missing output: ${chunk.file}`]);
    }
  }
  return manifest;
}

function loadBenchmarkSourceBoundaries() {
  let result;
  try {
    result = inspectBenchmarkSourceBoundaries(ROOT);
  } catch (error) {
    fail([`  Cannot inspect the benchmark source graph: ${errorMessage(error)}`]);
  }
  if (result.violations.length > 0) {
    fail([
      "  Benchmark source graph violates its engine boundary:",
      ...result.violations.map((violation) => `    - ${violation}`),
    ]);
  }
  return result;
}

function requireManifestEntry(manifest, source) {
  const matches = Object.entries(manifest).filter(
    ([key, chunk]) => (key === source || chunk.src === source) && chunk.isEntry === true
  );
  if (matches.length !== 1) {
    fail([`  Expected one Vite manifest entry for ${source}, found ${matches.length}.`]);
  }
  const [key, chunk] = matches[0];
  return { key, chunk };
}

function requireManifestModule(manifest, source) {
  const matches = Object.entries(manifest).filter(([key, chunk]) =>
    hasManifestSource(key, chunk, source),
  );
  if (matches.length !== 1) {
    fail([
      `  Expected one Vite manifest module for ${source}, found ${matches.length}.`,
    ]);
  }
  const [key, chunk] = matches[0];
  return { key, chunk };
}

function collectManifestClosure(manifest, roots, includeDynamicImports) {
  const visited = new Set();
  const pending = [...roots];
  while (pending.length > 0) {
    const key = pending.pop();
    if (visited.has(key)) continue;
    const chunk = manifest[key];
    if (!chunk) {
      fail([`  Vite manifest references an unknown chunk: ${key}`]);
    }
    visited.add(key);
    pending.push(...(chunk.imports ?? []));
    if (includeDynamicImports) {
      pending.push(...(chunk.dynamicImports ?? []));
    }
  }
  return visited;
}

function collectLazyOperationClosure(manifest, roots) {
  const visited = new Set();
  const pending = [...roots];
  while (pending.length > 0) {
    const key = pending.pop();
    if (visited.has(key)) continue;
    const chunk = manifest[key];
    if (!chunk) {
      fail([`  Vite manifest references an unknown chunk: ${key}`]);
    }
    visited.add(key);
    pending.push(...(chunk.imports ?? []));
    // A dynamic adapter may import its already-evaluated HTML entry chunk.
    // Dynamic expressions owned by that entry are not executed by the adapter.
    if (chunk.isEntry !== true) {
      pending.push(...(chunk.dynamicImports ?? []));
    }
  }
  return visited;
}

function verifyHtmlStaticClosure(label, assets, closure, manifest) {
  const staticFiles = new Set(
    [...closure].map((key) => manifest[key].file),
  );
  const unexpectedAssets = assets
    .map((asset) => relativeToDist(resolveDistPath(asset.url)))
    .filter((file) => !staticFiles.has(file));
  if (unexpectedAssets.length > 0) {
    fail([
      `  ${label} loads executable assets outside its static manifest closure: ${unexpectedAssets.join(", ")}`,
    ]);
  }
}

function verifyAdapterManifestBoundary(
  label,
  closure,
  manifest,
  indexEntryKey,
  compareEntryKey,
) {
  const forbiddenEntries = [indexEntryKey, compareEntryKey].filter((key) =>
    closure.has(key),
  );
  const forbiddenSources = [...closure].filter((key) =>
    BENCHMARK_ADAPTER_FORBIDDEN_SOURCES.has(manifestSource(key, manifest[key])),
  );
  if (forbiddenEntries.length > 0 || forbiddenSources.length > 0) {
    fail([
      `  ${label} benchmark adapter reaches parent-owned modules: ${formatManifestKeys(new Set([...forbiddenEntries, ...forbiddenSources]))}`,
    ]);
  }
}

function manifestSource(key, chunk) {
  return (chunk.src ?? key).replaceAll("\\", "/");
}

function hasManifestSource(key, chunk, source) {
  return manifestSource(key, chunk) === source;
}

function isMermaidAdapter(key, chunk) {
  return hasManifestSource(
    key,
    chunk,
    "src/runtime/realm/engines/mermaid.ts",
  );
}

function isBenchmarkAdapter(key, chunk) {
  const source = manifestSource(key, chunk);
  return (
    source === BENCHMARK_SOURCES.mermanAdapter ||
    source === BENCHMARK_SOURCES.mermaidAdapter
  );
}

function isMermaidPackageModule(key, chunk) {
  const source = manifestSource(key, chunk);
  return (
    source.startsWith("node_modules/mermaid/") ||
    source.startsWith("node_modules/@mermaid-js/")
  );
}

function isReferenceEngineModule(key, chunk) {
  return (
    isMermaidAdapter(key, chunk) ||
    hasManifestSource(key, chunk, BENCHMARK_SOURCES.mermaidAdapter) ||
    isMermaidPackageModule(key, chunk)
  );
}

function isMermanEngineModule(key, chunk) {
  const source = manifestSource(key, chunk);
  return (
    source === MERMAN_WASM_SHIM_SOURCE ||
    source === MERMAN_WASM_BINARY_SOURCE
  );
}

function formatManifestKeys(keys) {
  return [...keys].sort().join(", ");
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function htmlExecutableAssets(html) {
  const scripts = [...html.matchAll(/<script\b[^>]*\bsrc="([^"]+)"/gi)].map(
    (match) => ({ kind: "script", url: match[1] }),
  );
  const preloads = [
    ...html.matchAll(
      /<link\b(?=[^>]*\brel="modulepreload")(?=[^>]*\bhref="([^"]+)")[^>]*>/gi,
    ),
  ].map((match) => ({ kind: "modulepreload", url: match[1] }));
  return [...scripts, ...preloads];
}
