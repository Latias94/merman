/**
 * Fails local and CI static builds when Vite output is missing the wasm-bindgen
 * assets needed by the browser renderer.
 *
 * Override: SKIP_VERIFY_DIST_WASM=1
 */
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  BENCHMARK_SOURCES,
  inspectBenchmarkSourceBoundaries,
} from "./benchmark-build-graph.mjs";
import {
  createExpectedCspPolicies,
  verifyHtmlCsp,
} from "./csp-policy.mjs";
import { loadOpaqueRealmCspHashes } from "./opaque-realm-csp.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(__dirname, "..");
const DIST = path.join(ROOT, "dist");
const INDEX_HTML = path.join(DIST, "index.html");
const BENCHMARK_HTML = path.join(DIST, "benchmark.html");
const MANIFEST_FILE = path.join(DIST, ".vite", "manifest.json");
const ASSETS = path.join(DIST, "assets");
const BENCHMARK_MERMAN_ENGINE_SOURCE = path.join(
  ROOT,
  ".runtime",
  "benchmark-merman-engine.js",
);
const BENCHMARK_MERMAN_ENGINE_MANIFEST = path.join(
  ROOT,
  ".runtime",
  "benchmark-merman-engine.json",
);
const OPAQUE_ENGINE_ASSETS = [
  "mermaid-engine",
  "benchmark-merman-engine",
];
const opaqueRealmCspHashes = loadOpaqueRealmCspHashes(ROOT);
const expectedCspPolicies = createExpectedCspPolicies(opaqueRealmCspHashes);

const MERMAN_WASM_SHIM_SOURCE =
  "../platforms/web/packages/full/artifacts/wasm/merman_wasm.js";
const MERMAN_WASM_BINARY_SOURCE =
  "../platforms/web/packages/full/artifacts/wasm/merman_wasm_bg.wasm";

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
if (!isNonEmptyFile(BENCHMARK_HTML)) {
  fail([`  Missing Benchmark realm entry: ${BENCHMARK_HTML}`]);
}
if (!isNonEmptyFile(MANIFEST_FILE)) {
  fail([`  Missing Vite build manifest: ${MANIFEST_FILE}`]);
}

const manifest = loadBuildManifest();
verifyOwnedEditorWorkers();
verifyOpaqueEngineAssets();
const indexEntry = requireManifestEntry(manifest, "index.html");
const benchmarkEntry = requireManifestEntry(manifest, "benchmark.html");
loadBenchmarkSourceBoundaries();
const benchmarkMermanEngine = loadBenchmarkMermanEngineIdentity();
const wasmModule = requireManifestModule(manifest, MERMAN_WASM_BINARY_SOURCE);
const shimModule = requireManifestModule(manifest, MERMAN_WASM_SHIM_SOURCE);
const wasm = path.join(DIST, wasmModule.chunk.file);
const shim = path.join(DIST, shimModule.chunk.file);

const indexHtml = readFileSync(INDEX_HTML, "utf8");
const benchmarkHtml = readFileSync(BENCHMARK_HTML, "utf8");
for (const [fileName, html] of [
  ["index.html", indexHtml],
  ["benchmark.html", benchmarkHtml],
]) {
  const violations = verifyHtmlCsp(fileName, html, expectedCspPolicies);
  if (violations.length > 0) {
    fail([
      `  ${fileName} violates the production CSP contract:`,
      ...violations.map((violation) => `    - ${violation}`),
    ]);
  }
}
const indexAssets = htmlExecutableAssets(indexHtml);
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
const benchmarkStatic = collectManifestClosure(
  manifest,
  [benchmarkEntry.key],
  false,
);
verifyHtmlStaticClosure("index.html", indexAssets, indexStatic, manifest);
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
if (uniqueBenchmarkDynamicRoots.size !== 0) {
  fail([
    `  Benchmark realm must receive a verified engine artifact instead of importing dynamic roots: ${formatManifestKeys(uniqueBenchmarkDynamicRoots)}`,
  ]);
}
const eagerReferenceChunks = new Set(
  [...indexReachable, ...benchmarkStatic].filter((key) =>
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

const indexDynamicRoots = new Set(
  [...indexStatic].flatMap((key) => manifest[key].dynamicImports ?? []),
);
const mermanArtifactRoots = [...indexDynamicRoots].filter((key) =>
  hasManifestSource(key, manifest[key], BENCHMARK_SOURCES.mermanArtifact),
);
if (mermanArtifactRoots.length !== 1) {
  fail([
    `  Parent application must expose exactly one Merman benchmark engine-artifact root, found ${mermanArtifactRoots.length}.`,
  ]);
}
const [mermanArtifactRoot] = mermanArtifactRoots;
const mermanArtifactClosure = collectManifestClosure(
  manifest,
  [mermanArtifactRoot],
  false,
);
if (benchmarkStatic.has(mermanArtifactRoot)) {
  fail(["  Benchmark realm statically reaches the parent-owned Merman engine artifact."]);
}
const forbiddenMermanArtifactChunks = [...mermanArtifactClosure].filter(
  (key) =>
    isBenchmarkAdapter(key, manifest[key]) ||
    isMermaidPackageModule(key, manifest[key]) ||
    hasManifestSource(key, manifest[key], MERMAN_WASM_SHIM_SOURCE),
);
if (forbiddenMermanArtifactChunks.length > 0) {
  fail([
    `  Merman engine-artifact module reaches executable engine modules outside its verified source payload: ${forbiddenMermanArtifactChunks.join(", ")}`,
  ]);
}
const wasmAssetOwners = [...mermanArtifactClosure].filter((key) =>
  (manifest[key].assets ?? []).includes(wasmModule.chunk.file),
);
if (wasmAssetOwners.length !== 1) {
  fail([
    `  Merman engine-artifact module must own exactly one WASM resource URL, found ${wasmAssetOwners.length}.`,
  ]);
}
const artifactClosureSource = [...mermanArtifactClosure]
  .map((key) => readFileSync(path.join(DIST, manifest[key].file), "utf8"))
  .join("\n");
if (artifactClosureSource.includes("__mermanEngineArtifact")) {
  fail(["  Merman engine-artifact output must not inline the verified engine source."]);
}
if (!artifactClosureSource.includes("opaque-realm/benchmark-merman-engine.js")) {
  fail(["  Merman engine-artifact output does not retain its verified source URL."]);
}
if (!artifactClosureSource.includes(benchmarkMermanEngine.sha256)) {
  fail(["  Merman engine-artifact output does not retain its generated SHA-256 identity."]);
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
    "  Compare execution: opaque verified static realm artifact",
    `  Benchmark entry: ${relativeToDist(BENCHMARK_HTML)}`,
    "  Trusted Benchmark engines: Merman only",
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

function verifyOpaqueEngineAssets() {
  const publicRoot = path.join(DIST, "opaque-realm");
  const expected = OPAQUE_ENGINE_ASSETS.map((name) => `${name}.js`).sort();
  let actual;
  try {
    actual = readdirSync(publicRoot).sort();
  } catch (error) {
    fail([`  Missing opaque realm engine assets: ${errorMessage(error)}`]);
    return;
  }
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail([
      `  Opaque realm engine asset set is invalid: expected ${expected.join(", ")}; found ${actual.join(", ")}`,
    ]);
  }
  for (const name of OPAQUE_ENGINE_ASSETS) {
    const sourcePath = path.join(ROOT, ".runtime", `${name}.js`);
    const manifestPath = path.join(ROOT, ".runtime", `${name}.json`);
    const publicPath = path.join(publicRoot, `${name}.js`);
    if (!isNonEmptyFile(sourcePath) || !isNonEmptyFile(manifestPath)) {
      fail([`  Missing generated opaque realm source: ${name}`]);
    }
    if (!isNonEmptyFile(publicPath)) {
      fail([`  Missing published opaque realm source: ${name}`]);
    }
    let identity;
    try {
      identity = JSON.parse(readFileSync(manifestPath, "utf8"));
    } catch (error) {
      fail([`  Invalid opaque realm manifest ${name}: ${errorMessage(error)}`]);
      continue;
    }
    const source = readFileSync(sourcePath, "utf8");
    const published = readFileSync(publicPath, "utf8");
    const digest = createHash("sha256").update(published).digest("hex");
    if (
      source !== published ||
      identity?.bytes !== Buffer.byteLength(published) ||
      identity?.sha256 !== digest
    ) {
      fail([`  Published opaque realm source drifted from ${name} identity.`]);
    }
    if (!published.includes("__mermanEngineArtifact")) {
      fail([`  Published opaque realm source does not expose its engine contract: ${name}`]);
    }
  }
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

function loadBenchmarkMermanEngineIdentity() {
  if (
    !isNonEmptyFile(BENCHMARK_MERMAN_ENGINE_SOURCE) ||
    !isNonEmptyFile(BENCHMARK_MERMAN_ENGINE_MANIFEST)
  ) {
    fail([
      "  Missing generated Merman benchmark engine artifact. Run `npm run prepare:browser-runtime`.",
    ]);
  }
  const source = readFileSync(BENCHMARK_MERMAN_ENGINE_SOURCE, "utf8");
  let identity;
  try {
    identity = JSON.parse(readFileSync(BENCHMARK_MERMAN_ENGINE_MANIFEST, "utf8"));
  } catch (error) {
    fail([`  Invalid Merman benchmark engine manifest: ${errorMessage(error)}`]);
  }
  const sha256 = createHash("sha256").update(source).digest("hex");
  if (
    identity?.schemaVersion !== 1 ||
    identity.id !== "benchmark-merman" ||
    identity.bytes !== Buffer.byteLength(source) ||
    identity.sha256 !== sha256
  ) {
    fail(["  Generated Merman benchmark engine identity does not match its source bytes."]);
  }
  return identity;
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

function manifestSource(key, chunk) {
  return (chunk.src ?? key).replaceAll("\\", "/");
}

function hasManifestSource(key, chunk, source) {
  return manifestSource(key, chunk) === source;
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
    hasManifestSource(key, chunk, "src/runtime/realm/engines/mermaid.ts") ||
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
