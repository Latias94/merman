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

const WASM_FILE = /^merman_wasm_bg(?:-[A-Za-z0-9_-]+)?\.wasm$/;
const SHIM_FILE = /^merman_wasm(?:-[A-Za-z0-9_-]+)?\.js$/;

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

const files = collectFiles(DIST).filter(isNonEmptyFile);
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
const entryScripts = [...indexHtml.matchAll(/<script\b[^>]*\bsrc="([^"]+)"/gi)].map(
  (match) => match[1],
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
  ].join("\n"),
);
