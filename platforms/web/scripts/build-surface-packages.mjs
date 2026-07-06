import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { surfaces } from "./surface-manifest.mjs";

const packageRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const srcDir = path.join(packageRoot, "src");
const surfacesDir = path.join(srcDir, "surfaces");

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
  writeSurfaceEntry(surface);
}

function writeSurfaceEntry(surface) {
  const normalizedPkgDirRel = normalizeImportPath(surface.pkgDirRel);
  const source = [
    'import { bindSurfaceRuntime } from "../surface-runtime.js";',
    'import type { MermanWasmModule } from "../index.js";',
    'export type * from "../index.js";',
    "export {",
    ...surface.valueExportNames.map((name) => `  ${name},`),
    '} from "../index.js";',
    "",
    "function surfaceLoader(): Promise<MermanWasmModule> {",
    `  // @ts-ignore -- generated wasm-bindgen artifact exists after build:surfaces runs.`,
    `  return import("../../${normalizedPkgDirRel}/merman_wasm.js");`,
    "}",
    "",
    "const runtime = bindSurfaceRuntime(surfaceLoader);",
    "",
    "export const {",
    ...surface.runtimeExportNames.map((name) => `  ${name},`),
    "} = runtime;",
    "",
  ].join("\n");
  writeFileSync(path.join(surfacesDir, `${surface.entry}.ts`), source);
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
