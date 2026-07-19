import { existsSync, readdirSync, rmSync } from "node:fs";
import path from "node:path";

import { WASM_ARTIFACT_FILE_NAMES } from "./input-manifest.mjs";
import { publicWebSurfaceDescriptors } from "./web-surface-descriptor.mjs";

export function publicSurfaceDirectoryNames() {
  return publicWebSurfaceDescriptors.map((surface) =>
    path.posix.basename(surface.pkg_dir_rel),
  );
}

export function assertPackageOutputOwnership(packageOutputRoot) {
  if (!existsSync(packageOutputRoot)) return;
  const surfaceDirectories = publicSurfaceDirectoryNames();
  const ownedDirectories = new Set(["snippets", ...surfaceDirectories]);
  const ownedFiles = new Set(WASM_ARTIFACT_FILE_NAMES);
  const unknown = readdirSync(packageOutputRoot, { withFileTypes: true })
    .filter(
      (entry) =>
        (entry.isDirectory() && !ownedDirectories.has(entry.name)) ||
        (entry.isFile() && !ownedFiles.has(entry.name)) ||
        (!entry.isDirectory() && !entry.isFile()),
    )
    .map((entry) => entry.name)
    .sort(compareNames);
  if (unknown.length > 0) {
    throw new Error(
      [
        "Unowned entries exist in the published WASM package output:",
        ...unknown.map((name) => `  - pkg/${name}`),
        "Remove them or add an explicit public surface descriptor before packaging.",
      ].join("\n"),
    );
  }
  for (const directory of surfaceDirectories) {
    const surfaceRoot = path.join(packageOutputRoot, directory);
    if (existsSync(surfaceRoot)) assertSurfaceOutputOwnership(surfaceRoot, directory);
  }
}

export function pruneUnownedGeneratedDirectories(packageOutputRoot) {
  if (!existsSync(packageOutputRoot)) return [];
  const ownedDirectories = new Set(["snippets", ...publicSurfaceDirectoryNames()]);
  const removed = [];
  for (const entry of readdirSync(packageOutputRoot, { withFileTypes: true })) {
    if (
      entry.isDirectory() &&
      !entry.name.startsWith(".") &&
      !ownedDirectories.has(entry.name)
    ) {
      rmSync(path.join(packageOutputRoot, entry.name), {
        recursive: true,
        force: true,
      });
      removed.push(entry.name);
    }
  }
  return removed.sort(compareNames);
}

function assertSurfaceOutputOwnership(surfaceRoot, directory) {
  const ownedFiles = new Set(WASM_ARTIFACT_FILE_NAMES);
  const unknown = readdirSync(surfaceRoot, { withFileTypes: true })
    .filter(
      (entry) =>
        (entry.isDirectory() && entry.name !== "snippets") ||
        (entry.isFile() && !ownedFiles.has(entry.name)) ||
        (!entry.isDirectory() && !entry.isFile()),
    )
    .map((entry) => entry.name)
    .sort(compareNames);
  if (unknown.length > 0) {
    throw new Error(
      [
        `Unowned entries exist in the published ${directory} WASM surface:`,
        ...unknown.map((name) => `  - pkg/${directory}/${name}`),
      ].join("\n"),
    );
  }
}

function compareNames(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}
