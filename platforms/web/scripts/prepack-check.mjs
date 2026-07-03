import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants as zlibConstants, gzipSync } from "node:zlib";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const workspaceRoot = path.join(root, "..", "..");
const wasmSizeBudgets = path.join(workspaceRoot, "docs", "release", "WASM_SIZE_BUDGETS.json");
const generatedPackageJson = path.join(root, "pkg", "package.json");
const presetManifest = path.join(root, "pkg", "merman_wasm_preset.json");
const wasmBinary = path.join(root, "pkg", "merman_wasm_bg.wasm");
const surfaceEntries = ["core", "render", "ascii", "full"];
const required = [
  path.join(root, "dist", "index.js"),
  path.join(root, "dist", "index.d.ts"),
  generatedPackageJson,
  presetManifest,
  path.join(root, "pkg", "merman_wasm.js"),
  wasmBinary,
  ...surfaceEntries.flatMap((entry) => [
    path.join(root, "dist", "surfaces", `${entry}.js`),
    path.join(root, "dist", "surfaces", `${entry}.d.ts`),
    path.join(root, "pkg", entry, "package.json"),
    path.join(root, "pkg", entry, "merman_wasm.js"),
    path.join(root, "pkg", entry, "merman_wasm.d.ts"),
    path.join(root, "pkg", entry, "merman_wasm_bg.wasm"),
    path.join(root, "pkg", entry, "merman_wasm_preset.json"),
  ]),
];

const missing = required.filter((file) => {
  try {
    return !existsSync(file) || !statSync(file).isFile() || statSync(file).size === 0;
  } catch {
    return true;
  }
});

if (missing.length > 0) {
  console.error(
    [
      "prepack: missing generated web package files.",
      "Run `npm run build --prefix platforms/web` before pack/publish.",
      ...missing.map((file) => `  - ${path.relative(root, file)}`),
    ].join("\n"),
  );
  process.exit(1);
}

try {
  const packageJson = JSON.parse(readFileSync(generatedPackageJson, "utf8"));
  if (packageJson.type !== "module") {
    console.error("prepack: generated pkg/package.json must declare `type: module`.");
    console.error("Run `npm run build --prefix platforms/web` before pack/publish.");
    process.exit(1);
  }
} catch (error) {
  console.error(`prepack: failed to read generated pkg/package.json: ${error.message}`);
  process.exit(1);
}

try {
  const manifest = JSON.parse(readFileSync(presetManifest, "utf8"));
  const allowNonDefaultPreset = process.env.MERMAN_WEB_ALLOW_NON_DEFAULT_PRESET === "1";
  if (manifest.preset !== "browser-full" && !allowNonDefaultPreset) {
    console.error(
      [
        `prepack: generated WASM preset is '${manifest.preset}', expected 'browser-full'.`,
        "The published @mermanjs/web package currently defaults to the full browser artifact.",
        "Rebuild with `npm run build:wasm:full --prefix platforms/web` before pack/publish,",
        "or set MERMAN_WEB_ALLOW_NON_DEFAULT_PRESET=1 for an intentional local slim package.",
      ].join("\n"),
    );
    process.exit(1);
  }
  if (manifest.preset === "browser-full") {
    checkDefaultBrowserFullWasmBudget(loadDefaultBrowserFullWasmBudget());
  }
} catch (error) {
  console.error(`prepack: failed to read pkg/merman_wasm_preset.json: ${error.message}`);
  process.exit(1);
}

for (const entry of surfaceEntries) {
  checkSurfaceManifest(entry);
}

function loadDefaultBrowserFullWasmBudget() {
  let budgets;
  try {
    budgets = JSON.parse(readFileSync(wasmSizeBudgets, "utf8"));
  } catch (error) {
    console.error(`prepack: failed to read WASM size budgets: ${error.message}`);
    process.exit(1);
  }

  const budget = budgets.web_package?.["browser-full"];
  if (!budget) {
    console.error("prepack: missing web_package.browser-full WASM size budget.");
    process.exit(1);
  }

  return {
    raw: budget.max_raw_bytes,
    gzip: budget.max_gzip_bytes,
    brotli: budget.max_brotli_bytes,
  };
}

function checkSurfaceManifest(entry) {
  const manifestPath = path.join(root, "pkg", entry, "merman_wasm_preset.json");
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch (error) {
    console.error(`prepack: failed to read pkg/${entry}/merman_wasm_preset.json: ${error.message}`);
    process.exit(1);
  }

  const expectedPreset = `browser-${entry}`;
  if (manifest.preset !== expectedPreset) {
    console.error(
      [
        `prepack: generated WASM preset for ./${entry} is '${manifest.preset}', expected '${expectedPreset}'.`,
        "Run `npm run build --prefix platforms/web` before pack/publish.",
      ].join("\n"),
    );
    process.exit(1);
  }
}

function checkDefaultBrowserFullWasmBudget(defaultBrowserFullWasmBudget) {
  const bytes = readFileSync(wasmBinary);
  const sizes = {
    raw: bytes.length,
    gzip: gzipSync(bytes, { level: 9 }).length,
    brotli: brotliCompressSync(bytes, {
      params: {
        [zlibConstants.BROTLI_PARAM_QUALITY]: 11,
      },
    }).length,
  };

  const failures = Object.entries(defaultBrowserFullWasmBudget)
    .filter(([metric, max]) => typeof max !== "number" || sizes[metric] > max)
    .map(
      ([metric, max]) =>
        `  - ${metric}: actual=${sizes[metric]} max=${max}`,
    );

  if (failures.length > 0) {
    console.error(
      [
        "prepack: browser-full WASM size budget exceeded.",
        "The published @mermanjs/web artifact should be built with the workspace wasm-size profile.",
        ...failures,
      ].join("\n"),
    );
    process.exit(1);
  }
}
