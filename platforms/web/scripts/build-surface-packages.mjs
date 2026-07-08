import { existsSync, mkdirSync, renameSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { surfaces } from "./surface-manifest.mjs";

const packageRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const srcDir = path.join(packageRoot, "src");
const surfacesDir = path.join(srcDir, "surfaces");
const tempSurfacesDir = path.join(srcDir, `.surfaces-${process.pid}-${Date.now()}`);
const backupSurfacesDir = path.join(srcDir, `.surfaces-backup-${process.pid}-${Date.now()}`);

rmSync(tempSurfacesDir, { recursive: true, force: true });
mkdirSync(tempSurfacesDir, { recursive: true });

try {
  for (const surface of surfaces) {
    run(process.execPath, [
      "scripts/build-wasm.mjs",
      "--preset",
      surface.preset,
      "--out-dir-rel",
      surface.pkgDirRel,
    ]);
    writeSurfaceEntry(surface, tempSurfacesDir);
  }
  replaceSurfacesDir();
} finally {
  rmSync(tempSurfacesDir, { recursive: true, force: true });
}

function writeSurfaceEntry(surface, targetDir) {
  const normalizedPkgDirRel = normalizeImportPath(surface.pkgDirRel);
  const source = [
    'import { bindSurfaceRuntime } from "../surface-runtime.js";',
    'import type { MermanWasmModule } from "../index.js";',
    'export type * from "../index.js";',
    "export {",
    ...surface.valueExportNames.map((name) => `  ${surfaceValueExportSpec(surface, name)},`),
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
  writeFileSync(path.join(targetDir, `${surface.entry}.ts`), source);
}

function surfaceValueExportSpec(surface, name) {
  if (name === "DEFAULT_BINDING_CAPABILITIES") {
    return `${surface.defaultBindingCapabilitiesExportName} as DEFAULT_BINDING_CAPABILITIES`;
  }
  return name;
}

function normalizeImportPath(relativePath) {
  return relativePath.split(path.sep).join("/");
}

function replaceSurfacesDir() {
  if (existsSync(backupSurfacesDir)) {
    rmSync(backupSurfacesDir, { recursive: true, force: true });
  }
  try {
    if (existsSync(surfacesDir)) {
      renameSync(surfacesDir, backupSurfacesDir);
    }
    renameSync(tempSurfacesDir, surfacesDir);
    rmSync(backupSurfacesDir, { recursive: true, force: true });
  } catch (error) {
    if (!existsSync(surfacesDir) && existsSync(backupSurfacesDir)) {
      renameSync(backupSurfacesDir, surfacesDir);
    }
    throw error;
  }
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
