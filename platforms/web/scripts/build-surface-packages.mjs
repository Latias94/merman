import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const packageRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const srcDir = path.join(packageRoot, "src");
const surfacesDir = path.join(srcDir, "surfaces");

const surfaces = [
  { entry: "core", preset: "browser-core", pkgDirRel: "pkg/core" },
  { entry: "render", preset: "browser-render", pkgDirRel: "pkg/render" },
  { entry: "ascii", preset: "browser-ascii", pkgDirRel: "pkg/ascii" },
  { entry: "full", preset: "browser-full", pkgDirRel: "pkg/full" },
];

rmSync(surfacesDir, { recursive: true, force: true });
mkdirSync(surfacesDir, { recursive: true });

for (const surface of surfaces) {
  run(process.execPath, [
    "scripts/build-wasm.mjs",
    "--preset",
    surface.preset,
    "--out-dir-rel",
    surface.pkgDirRel,
  ]);
  writeSurfaceEntry(surface.entry, surface.pkgDirRel);
}

function writeSurfaceEntry(entryName, pkgDirRel) {
  const normalizedPkgDirRel = normalizeImportPath(pkgDirRel);
  const source = [
    'import {',
    '  initMerman as initRootMerman,',
    '  type MermanInitOptions,',
    '  type MermanInitInput,',
    '  type MermanWasmModule,',
    '} from "../index.js";',
    'export * from "../index.js";',
    "",
    "function surfaceLoader(): Promise<MermanWasmModule> {",
    `  // @ts-ignore -- generated wasm-bindgen artifact exists after build:surfaces runs.`,
    `  return import("../../${normalizedPkgDirRel}/merman_wasm.js");`,
    "}",
    "",
    "export function initMerman(init?: MermanInitInput) {",
    "  if (typeof init === \"function\") {",
    "    return initRootMerman(init);",
    "  }",
    "  const options: MermanInitOptions = init ?? {};",
    "  return initRootMerman({",
    "    loader: surfaceLoader,",
    "    ...options,",
    "  });",
    "}",
    "",
  ].join("\n");
  writeFileSync(path.join(surfacesDir, `${entryName}.ts`), source);
}

function normalizeImportPath(relativePath) {
  return relativePath.split(path.sep).join("/");
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: packageRoot,
    stdio: "inherit",
  });
  if (result.error) {
    console.error(`Failed to run ${command}: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}
