/**
 * Verifies the production Vite graph, CSP, WASM ownership, and published realm
 * artifacts. Prepared artifact integrity remains owned by the artifact verifier.
 *
 * Override: SKIP_VERIFY_DIST_WASM=1
 */
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

import {
  OPAQUE_REALM_ARTIFACT_PLAN,
  engineForId,
  pageForKey,
  publicEngineDirectory,
  publicEngineFiles,
} from "./opaque-realm-artifact-plan.mjs";
import { verifyPreparedOpaqueRealmArtifacts } from "./opaque-realm-artifact-verifier.mjs";
import {
  inspectBenchmarkSourceBoundaries,
  inspectPlaygroundEmittedGraph,
} from "./playground-build-policy.mjs";
import {
  collectManifestClosure,
  emittedFiles,
  emittedResources,
  htmlStaticAssets,
  manifestChunk,
  missingManifestOutputs,
  missingStaticStylesheets,
  parseViteManifest,
  ownersOfAsset,
} from "./vite-manifest-graph.mjs";
import {
  createExpectedCspPolicies,
  verifyHtmlCsp,
} from "./csp-policy.mjs";
import { loadOpaqueRealmCspHashes } from "./opaque-realm-csp.mjs";

if (process.env.SKIP_VERIFY_DIST_WASM === "1") process.exit(0);

const playgroundRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const distRoot = path.join(playgroundRoot, "dist");
const manifestFile = path.join(distRoot, ".vite", "manifest.json");
const assetsRoot = path.join(distRoot, "assets");
const plan = OPAQUE_REALM_ARTIFACT_PLAN;
const opaqueRealmCspHashes = loadOpaqueRealmCspHashes(playgroundRoot);
const expectedCspPolicies = createExpectedCspPolicies(
  opaqueRealmCspHashes,
  plan,
);

try {
  await verifyPreparedOpaqueRealmArtifacts(playgroundRoot, plan);
  verifyRequiredFiles();
  const graph = loadBuildManifest();
  verifyOwnedEditorWorkers();
  verifyPublishedOpaqueEngineAssets();
  const emitted = inspectPlaygroundEmittedGraph(graph);
  if (emitted.violations.length > 0) {
    fail([
      "  Production ownership graph is invalid:",
      ...emitted.violations.map((violation) => `    - ${violation}`),
    ]);
  }
  verifyBenchmarkSourceBoundaries();
  verifyHtmlEntries(graph, emitted.pageEntries);
  const wasm = verifyWasmOutput(graph, emitted.wasmBinary, emitted.wasmShim);
  reportBuild(graph, emitted, wasm);
} catch (error) {
  fail([`  ${errorMessage(error)}`]);
}

function verifyRequiredFiles() {
  if (!isNonEmptyFile(manifestFile)) {
    fail([`  Missing Vite build manifest: ${manifestFile}`]);
  }
  for (const page of plan.pages) {
    const file = path.join(distRoot, page.source);
    if (!isNonEmptyFile(file)) {
      fail([
        `  Missing production page: ${file}`,
        "  Run `npm run build --prefix playground` before publishing the static artifact.",
      ]);
    }
  }
}

function loadBuildManifest() {
  let value;
  try {
    value = JSON.parse(readFileSync(manifestFile, "utf8"));
  } catch (error) {
    fail([`  Invalid Vite build manifest: ${errorMessage(error)}`]);
  }
  const graph = parseViteManifest(value);
  const missing = missingManifestOutputs(graph, (file) =>
    isNonEmptyFile(path.join(distRoot, file)),
  );
  if (missing.length > 0) {
    fail([
      "  Vite manifest references missing or empty outputs:",
      ...missing.map(
        (output) =>
          `    - ${output.key} ${output.kind}: ${output.file}`,
      ),
    ]);
  }
  return graph;
}

function verifyOwnedEditorWorkers() {
  const workerAssets = readdirSync(assetsRoot)
    .filter((file) => /\.worker-[\w-]+\.js$/u.test(file))
    .sort();
  const requiredWorkers = [
    /^editor\.worker-[\w-]+\.js$/u,
    /^json\.worker-[\w-]+\.js$/u,
    /^mermaid-syntax\.worker-[\w-]+\.js$/u,
    /^merman-language\.worker-[\w-]+\.js$/u,
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
    if (/cdn\.jsdelivr\.net/iu.test(readFileSync(path.join(assetsRoot, file), "utf8"))) {
      fail([`  Editor worker contains the forbidden Monaco CDN URL: ${file}`]);
    }
  }
}

function verifyPublishedOpaqueEngineAssets() {
  const publicRoot = path.join(distRoot, publicEngineDirectory(plan));
  const expected = [...publicEngineFiles(plan)].sort();
  const actual = readdirSync(publicRoot).sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail([
      `  Opaque realm public asset set is invalid: expected ${expected.join(", ")}; found ${actual.join(", ")}`,
    ]);
  }
  for (const engine of plan.engines.filter((candidate) => candidate.publish)) {
    const source = readFileSync(
      path.join(playgroundRoot, plan.roots.generated, `${engine.outputBase}.js`),
      "utf8",
    );
    const manifest = JSON.parse(
      readFileSync(
        path.join(
          playgroundRoot,
          plan.roots.generated,
          `${engine.outputBase}.json`,
        ),
        "utf8",
      ),
    );
    const published = readFileSync(
      path.join(publicRoot, `${engine.outputBase}.js`),
      "utf8",
    );
    const digest = createHash("sha256").update(published).digest("hex");
    if (
      source !== published ||
      manifest.id !== engine.id ||
      manifest.bytes !== Buffer.byteLength(published) ||
      manifest.sha256 !== digest
    ) {
      fail([`  Published opaque realm source drifted from ${engine.id} identity.`]);
    }
  }
}

function verifyBenchmarkSourceBoundaries() {
  const result = inspectBenchmarkSourceBoundaries(playgroundRoot);
  if (result.violations.length > 0) {
    fail([
      "  Benchmark source graph violates its engine boundary:",
      ...result.violations.map((violation) => `    - ${violation}`),
    ]);
  }
}

function verifyHtmlEntries(graph, pageEntries) {
  for (const page of plan.pages) {
    const html = readFileSync(path.join(distRoot, page.source), "utf8");
    const cspViolations = verifyHtmlCsp(
      page.source,
      html,
      expectedCspPolicies,
    );
    if (cspViolations.length > 0) {
      fail([
        `  ${page.source} violates the production CSP contract:`,
        ...cspViolations.map((violation) => `    - ${violation}`),
      ]);
    }
    const assets = htmlStaticAssets(html);
    for (const asset of assets) {
      if (/^https?:\/\//iu.test(asset.url)) {
        fail([`  ${page.source} references external ${asset.kind}: ${asset.url}`]);
      }
      if (!isNonEmptyFile(resolveDistPath(asset.url))) {
        fail([`  ${page.source} references missing ${asset.kind}: ${asset.url}`]);
      }
    }
    const entryKey = pageEntries[page.key];
    const staticClosure = collectManifestClosure(graph, [entryKey], "static");
    verifyHtmlStaticClosure(page.source, assets, staticClosure, graph);
    const scripts = assets
      .filter((asset) => asset.kind === "script")
      .map((asset) => relativeToDist(resolveDistPath(asset.url)));
    const entryFile = manifestChunk(graph, entryKey).file;
    if (
      (page.cspProfile === "playground-v1" && !scripts.includes(entryFile)) ||
      (page.cspProfile !== "playground-v1" &&
        (scripts.length !== 1 || scripts[0] !== entryFile))
    ) {
      fail([`  ${page.source} does not load exactly its declared Vite entry.`]);
    }
  }
}

function verifyHtmlStaticClosure(label, assets, closure, graph) {
  const files = emittedResources(graph, closure);
  const normalizedAssets = assets.map((asset) => ({
    file: relativeToDist(resolveDistPath(asset.url)),
    kind: asset.kind,
  }));
  const unexpected = normalizedAssets
    .map((asset) => asset.file)
    .filter((file) => !files.has(file));
  if (unexpected.length > 0) {
    fail([
      `  ${label} loads assets outside its static manifest closure: ${unexpected.join(", ")}`,
    ]);
  }
  const missingStylesheets = missingStaticStylesheets(
    graph,
    closure,
    normalizedAssets
      .filter((asset) => asset.kind === "stylesheet")
      .map((asset) => asset.file),
  );
  if (missingStylesheets.length > 0) {
    fail([
      `  ${label} omits stylesheets from its static manifest closure: ${missingStylesheets.join(", ")}`,
    ]);
  }
}

function verifyWasmOutput(graph, wasmKey, shimKey) {
  const wasm = path.join(distRoot, manifestChunk(graph, wasmKey).file);
  const shim = path.join(distRoot, manifestChunk(graph, shimKey).file);
  if (!isNonEmptyFile(wasm) || !isNonEmptyFile(shim)) {
    fail(["  Production WASM module or wasm-bindgen shim is missing."]);
  }
  const owners = ownersOfAsset(graph, relativeToDist(wasm));
  if (owners.length !== 1) {
    fail([
      `  Production WASM must have one manifest owner; found ${owners.length}.`,
    ]);
  }
  return {
    owner: path.join(distRoot, manifestChunk(graph, owners[0]).file),
    shim,
    wasm,
  };
}

function reportBuild(graph, emitted, wasm) {
  const initialJavaScript = observeManifestClosure(
    emitted.initialStaticKeys,
    graph,
  );
  const optionalRoots = Object.entries(emitted.featureRoots)
    .map(([feature, key]) => `${feature}: ${manifestChunk(graph, key).file}`)
    .join(", ");
  const benchmarkEngine = engineForId(plan, "benchmark-merman");
  const corpusPage = pageForKey(plan, "benchmarkCorpus");
  const benchmarkPage = pageForKey(plan, "benchmarkRealm");
  console.log(
    [
      "[merman-playground] production artifact graph verified.",
      `  WASM: ${relativeToDist(wasm.wasm)}`,
      `  JS shim: ${relativeToDist(wasm.shim)}`,
      `  WASM URL owner: ${relativeToDist(wasm.owner)}`,
      `  Initial JS closure: ${initialJavaScript.files} files, ${formatBytes(initialJavaScript.rawBytes)} raw, ${formatBytes(initialJavaScript.gzipBytes)} gzip`,
      `  Activation-owned roots: ${optionalRoots}`,
      "  Compare execution: opaque verified static realm artifact",
      `  Benchmark corpus entry: ${corpusPage.source}`,
      `  Benchmark entry: ${benchmarkPage.source}`,
      `  Trusted Benchmark engine: ${benchmarkEngine.id}`,
    ].join("\n"),
  );
}

function observeManifestClosure(keys, graph) {
  const files = emittedFiles(graph, keys);
  let rawBytes = 0;
  let gzipBytes = 0;
  for (const file of files) {
    const source = readFileSync(path.join(distRoot, file));
    rawBytes += source.byteLength;
    gzipBytes += gzipSync(source).byteLength;
  }
  return { files: files.size, rawBytes, gzipBytes };
}

function resolveDistPath(assetPath) {
  const withoutOrigin = assetPath.replace(/^https?:\/\/[^/]+/iu, "");
  const withoutQuery = withoutOrigin.split(/[?#]/u, 1)[0];
  const withoutBase = withoutQuery.replace(/^\/merman\//u, "").replace(/^\//u, "");
  return path.join(distRoot, withoutBase);
}

function relativeToDist(file) {
  return path.relative(distRoot, file).replaceAll(path.sep, "/");
}

function isNonEmptyFile(file) {
  try {
    const stat = statSync(file);
    return stat.isFile() && stat.size > 0;
  } catch {
    return false;
  }
}

function formatBytes(bytes) {
  return `${(bytes / 1024).toFixed(2)} KiB`;
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function fail(lines) {
  console.error(
    ["[merman-playground] dist verification failed.", ...lines].join("\n"),
  );
  process.exit(1);
}
