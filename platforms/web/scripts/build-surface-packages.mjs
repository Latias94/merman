import {
  existsSync,
  mkdirSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { surfaces } from "./surface-manifest.mjs";

const packageRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const srcDir = path.join(packageRoot, "src");
const surfacesDir = path.join(srcDir, "surfaces");
const tempSurfacesDir = path.join(srcDir, `.surfaces-${process.pid}-${Date.now()}`);
const backupSurfacesDir = path.join(srcDir, `.surfaces-backup-${process.pid}-${Date.now()}`);
const backupSurfacesDirNamePrefix = ".surfaces-backup-";

if (isMainModule()) {
  let exitCode = 0;
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
    replaceSurfacesDir({
      surfacesDir,
      tempSurfacesDir,
      backupSurfacesDir,
    });
  } catch (error) {
    exitCode =
      error && typeof error === "object" && "exitCode" in error
        ? Number(error.exitCode) || 1
        : 1;
    console.error(error instanceof Error ? error.message : String(error));
  } finally {
    rmSync(tempSurfacesDir, { recursive: true, force: true });
  }

  if (exitCode !== 0) {
    process.exit(exitCode);
  }
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

export function replaceSurfacesDir({
  surfacesDir,
  tempSurfacesDir,
  backupSurfacesDir,
  fsOps = { existsSync, readdirSync, renameSync, rmSync, statSync },
}) {
  restoreInterruptedSurfacesReplacement({ surfacesDir, backupSurfacesDir, fsOps });
  try {
    if (fsOps.existsSync(surfacesDir)) {
      fsOps.renameSync(surfacesDir, backupSurfacesDir);
    }
    fsOps.renameSync(tempSurfacesDir, surfacesDir);
    fsOps.rmSync(backupSurfacesDir, { recursive: true, force: true });
  } catch (error) {
    if (!fsOps.existsSync(surfacesDir) && fsOps.existsSync(backupSurfacesDir)) {
      fsOps.renameSync(backupSurfacesDir, surfacesDir);
    }
    throw error;
  }
}

function restoreInterruptedSurfacesReplacement({ surfacesDir, backupSurfacesDir, fsOps }) {
  const backupDirs = findBackupSurfacesDirs({ surfacesDir, backupSurfacesDir, fsOps });
  if (backupDirs.length === 0) {
    return;
  }
  if (fsOps.existsSync(surfacesDir)) {
    removeBackupDirs(backupDirs, fsOps);
    return;
  }
  const [backupToRestore, ...staleBackupDirs] = backupDirs;
  fsOps.renameSync(backupToRestore, surfacesDir);
  removeBackupDirs(staleBackupDirs, fsOps);
}

function findBackupSurfacesDirs({ surfacesDir, backupSurfacesDir, fsOps }) {
  const srcDir = path.dirname(surfacesDir);
  const backupDirs = new Set();
  if (fsOps.existsSync(backupSurfacesDir) && isDirectory(backupSurfacesDir, fsOps)) {
    backupDirs.add(path.resolve(backupSurfacesDir));
  }
  if (typeof fsOps.readdirSync === "function") {
    for (const entry of fsOps.readdirSync(srcDir, { withFileTypes: true })) {
      if (entry.isDirectory() && entry.name.startsWith(backupSurfacesDirNamePrefix)) {
        backupDirs.add(path.resolve(srcDir, entry.name));
      }
    }
  }
  backupDirs.delete(path.resolve(surfacesDir));
  return [...backupDirs].sort((left, right) => {
    const mtimeDelta = backupMtimeMs(right, fsOps) - backupMtimeMs(left, fsOps);
    if (mtimeDelta !== 0) {
      return mtimeDelta;
    }
    return right.localeCompare(left);
  });
}

function backupMtimeMs(backupDir, fsOps) {
  if (typeof fsOps.statSync !== "function") {
    return 0;
  }
  try {
    return fsOps.statSync(backupDir).mtimeMs;
  } catch {
    return 0;
  }
}

function isDirectory(backupDir, fsOps) {
  if (typeof fsOps.statSync !== "function") {
    return true;
  }
  try {
    return fsOps.statSync(backupDir).isDirectory();
  } catch {
    return false;
  }
}

function removeBackupDirs(backupDirs, fsOps) {
  for (const backupDir of backupDirs) {
    if (fsOps.existsSync(backupDir)) {
      fsOps.rmSync(backupDir, { recursive: true, force: true });
    }
  }
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: packageRoot,
    stdio: "inherit",
  });
  if (result.error) {
    throw new Error(`Failed to run ${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const error = new Error(`${command} exited with status ${result.status ?? 1}`);
    error.exitCode = result.status ?? 1;
    throw error;
  }
}

function isMainModule() {
  return (
    process.argv[1] !== undefined &&
    path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
  );
}
