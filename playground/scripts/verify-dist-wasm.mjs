/**
 * Fails local and CI static builds when Vite output is missing the wasm-bindgen
 * assets needed by the browser renderer.
 *
 * Override: SKIP_VERIFY_DIST_WASM=1
 */
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(__dirname, "..");
const DIST = path.join(ROOT, "dist");
const INDEX_HTML = path.join(DIST, "index.html");
const COMPARE_HTML = path.join(DIST, "compare-realm.html");
const MANIFEST_FILE = path.join(DIST, ".vite", "manifest.json");

const WASM_FILE = /^merman_wasm_bg(?:-[A-Za-z0-9_-]+)?\.wasm$/;
const SHIM_FILE = /^merman_wasm(?:-[A-Za-z0-9_-]+)?\.js$/;
const REFERENCE_ENGINE_ASSET = /(?:^|\/)(?:mermaid-(?!requirements-)|mermaid\.core-|mermaid-zenuml|mermaid-layout-elk|zenuml-definition-|render-)[A-Za-z0-9_.-]*\.js$/;

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

function collectFiles(dir) {
  if (!existsSync(dir)) {
    return [];
  }

  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(dir, entry.name);
    return entry.isDirectory() ? collectFiles(file) : [file];
  });
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

if (!isNonEmptyFile(INDEX_HTML)) {
  fail([
    `  Missing index.html: ${INDEX_HTML}`,
    "  Run `npm run build --prefix playground` before publishing the static artifact.",
  ]);
}
if (!isNonEmptyFile(COMPARE_HTML)) {
  fail([`  Missing Compare realm entry: ${COMPARE_HTML}`]);
}
if (!isNonEmptyFile(MANIFEST_FILE)) {
  fail([`  Missing Vite build manifest: ${MANIFEST_FILE}`]);
}

const files = collectFiles(DIST).filter(isNonEmptyFile);
const manifest = loadBuildManifest();
const indexEntry = requireManifestEntry(manifest, "index.html");
const compareEntry = requireManifestEntry(manifest, "compare-realm.html");
const wasm = files.find((file) => WASM_FILE.test(path.basename(file)));
const shim = files.find((file) => SHIM_FILE.test(path.basename(file)));

if (!wasm || !shim) {
  fail([
    "  Expected WASM: dist/assets/merman_wasm_bg[-hash].wasm",
    "  Expected JS shim: dist/assets/merman_wasm[-hash].js",
    "  Build `platforms/web` first, then build the playground so Vite can bundle the generated wasm-bindgen output.",
  ]);
}

const indexHtml = readFileSync(INDEX_HTML, "utf8");
const compareHtml = readFileSync(COMPARE_HTML, "utf8");
const indexAssets = htmlExecutableAssets(indexHtml);
const compareAssets = htmlExecutableAssets(compareHtml);
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
]) {
  for (const asset of assets) {
    if (/^https?:\/\//i.test(asset.url)) {
      fail([`  ${label} references an external executable asset: ${asset.url}`]);
    }
    if (REFERENCE_ENGINE_ASSET.test(asset.url)) {
      fail([`  ${label} eagerly loads a reference-engine asset: ${asset.url}`]);
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
const eagerReferenceChunks = new Set(
  [...indexReachable, ...compareStatic].filter((key) =>
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

const jsFiles = files.filter((file) => file.endsWith(".js"));
const shimName = path.basename(shim);
const wasmName = path.basename(wasm);
const appReferencesShim = jsFiles.some((file) => readFileSync(file, "utf8").includes(shimName));
const shimReferencesWasm = readFileSync(shim, "utf8").includes(wasmName);

if (!appReferencesShim) {
  fail([`  No bundled JavaScript file references the wasm-bindgen shim: ${shimName}`]);
}

if (!shimReferencesWasm) {
  fail([`  The wasm-bindgen shim does not reference the WASM binary: ${wasmName}`]);
}

console.log(
  [
    "[merman-playground] dist WASM present.",
    `  WASM: ${relativeToDist(wasm)}`,
    `  JS shim: ${relativeToDist(shim)}`,
    `  Compare entry: ${relativeToDist(COMPARE_HTML)}`,
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

function moduleIdentity(key, chunk) {
  return [key, chunk.src ?? "", chunk.file].join("\n").replaceAll("\\", "/");
}

function isMermaidAdapter(key, chunk) {
  return moduleIdentity(key, chunk).includes("src/runtime/realm/engines/mermaid.ts");
}

function isMermaidPackageModule(key, chunk) {
  return /(?:^|\n|\/)node_modules\/(?:mermaid|@mermaid-js)\//.test(
    moduleIdentity(key, chunk)
  );
}

function isReferenceEngineModule(key, chunk) {
  return isMermaidAdapter(key, chunk) || isMermaidPackageModule(key, chunk);
}

function isMermanEngineModule(key, chunk) {
  return moduleIdentity(key, chunk).includes("platforms/web/pkg/merman_wasm");
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
