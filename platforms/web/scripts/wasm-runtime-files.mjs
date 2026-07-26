import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
  statSync,
} from "node:fs";
import path from "node:path";

export const WASM_RUNTIME_TOP_LEVEL_FILES = Object.freeze([
  "merman_wasm.js",
  "merman_wasm.d.ts",
  "merman_wasm_bg.wasm",
  "merman_wasm_bg.wasm.d.ts",
]);

/**
 * Returns the exact copied wasm-bindgen runtime file set with content evidence.
 *
 * Source build directories may contain build manifests beside the runtime files, while a package
 * artifact directory must contain only this runtime plus an optional snippets tree. Callers choose
 * strictness accordingly and compare these records across the build/package/release boundaries.
 */
export function wasmRuntimeFileRecords(
  runtimeRoot,
  { strictTopLevel = false, relativePrefix = "artifacts/wasm" } = {},
) {
  if (!existsSync(runtimeRoot) || !lstatSync(runtimeRoot).isDirectory()) {
    throw new Error(`Missing WASM runtime directory: ${runtimeRoot}.`);
  }

  if (strictTopLevel) {
    const allowed = new Set([...WASM_RUNTIME_TOP_LEVEL_FILES, "snippets"]);
    for (const entry of readdirSync(runtimeRoot, { withFileTypes: true })) {
      if (!allowed.has(entry.name)) {
        throw new Error(`Unexpected WASM runtime artifact ${path.join(runtimeRoot, entry.name)}.`);
      }
    }
  }

  const records = WASM_RUNTIME_TOP_LEVEL_FILES.map((name) =>
    fileRecord(path.join(runtimeRoot, name), `${relativePrefix}/${name}`),
  );
  const snippetsRoot = path.join(runtimeRoot, "snippets");
  if (existsSync(snippetsRoot)) {
    records.push(...walkSnippetFiles(snippetsRoot, `${relativePrefix}/snippets`));
  }
  return records.sort(compareRecords);
}

export function packageDistFileRecords(
  distRoot,
  packageId,
  { allowSiblingPackageEntries = false, allowSharedSourceMaps = false } = {},
) {
  if (!existsSync(distRoot) || !lstatSync(distRoot).isDirectory()) {
    throw new Error(`Missing compiled package directory: ${distRoot}.`);
  }
  const names = [
    `${packageId}.d.ts`,
    `${packageId}.d.ts.map`,
    `${packageId}.js`,
    `${packageId}.js.map`,
  ];
  const entryRoot = path.join(distRoot, "package-entries");
  if (!existsSync(entryRoot) || !lstatSync(entryRoot).isDirectory()) {
    throw new Error(`Missing compiled package entry directory: ${entryRoot}.`);
  }
  const entryNames = readdirSync(entryRoot, { withFileTypes: true });
  const actualNames = entryNames.map((entry) => entry.name).sort();
  if (
    !allowSiblingPackageEntries &&
    JSON.stringify(actualNames) !== JSON.stringify(names)
  ) {
    throw new Error(`Compiled package entry directory contains unexpected files: ${entryRoot}.`);
  }
  for (const name of names) {
    if (!actualNames.includes(name)) {
      throw new Error(`Missing compiled package entry: ${path.join(entryRoot, name)}.`);
    }
  }

  const records = names.map((name) =>
    fileRecord(path.join(entryRoot, name), `dist/package-entries/${name}`),
  );
  for (const entry of readdirSync(distRoot, { withFileTypes: true })) {
    if (entry.name === "package-entries") continue;
    const absolute = path.join(distRoot, entry.name);
    if (entry.isDirectory()) {
      records.push(
        ...walkDistFiles(absolute, `dist/${entry.name}`, {
          allowSharedSourceMaps,
        }),
      );
    } else if (entry.isFile()) {
      if (entry.name.endsWith(".map")) {
        if (!allowSharedSourceMaps) {
          throw new Error(
            `Compiled package directory must not contain shared source maps: ${absolute}.`,
          );
        }
        continue;
      }
      records.push(fileRecord(absolute, `dist/${entry.name}`));
    } else {
      throw new Error(`Compiled package directory must contain regular files only: ${absolute}.`);
    }
  }
  return records.sort(compareRecords);
}

function walkSnippetFiles(root, relativeRoot) {
  if (!lstatSync(root).isDirectory()) {
    throw new Error(`WASM snippets path must be a directory: ${root}.`);
  }
  const records = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const absolute = path.join(root, entry.name);
    const relative = `${relativeRoot}/${entry.name}`;
    if (entry.isDirectory()) {
      records.push(...walkSnippetFiles(absolute, relative));
    } else if (entry.isFile()) {
      records.push(fileRecord(absolute, relative));
    } else {
      throw new Error(`WASM snippets must contain regular files only: ${absolute}.`);
    }
  }
  return records;
}

function walkDistFiles(root, relativeRoot, { allowSharedSourceMaps }) {
  const records = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const absolute = path.join(root, entry.name);
    const relative = `${relativeRoot}/${entry.name}`;
    if (entry.isDirectory()) {
      records.push(...walkDistFiles(absolute, relative, { allowSharedSourceMaps }));
    } else if (entry.isFile()) {
      if (entry.name.endsWith(".map")) {
        if (!allowSharedSourceMaps) {
          throw new Error(
            `Compiled package directory must not contain shared source maps: ${absolute}.`,
          );
        }
        continue;
      }
      records.push(fileRecord(absolute, relative));
    } else {
      throw new Error(`Compiled package directory must contain regular files only: ${absolute}.`);
    }
  }
  return records;
}

function fileRecord(file, relativePath) {
  if (!existsSync(file) || !lstatSync(file).isFile()) {
    throw new Error(`Missing WASM runtime artifact: ${file}.`);
  }
  const bytes = statSync(file).size;
  if (bytes === 0) {
    throw new Error(`WASM runtime artifact is empty: ${file}.`);
  }
  const digest = createHash("sha256");
  digest.update(readFileSync(file));
  return {
    path: relativePath,
    bytes,
    sha256: `sha256:${digest.digest("hex")}`,
  };
}

function compareRecords(left, right) {
  if (left.path < right.path) return -1;
  if (left.path > right.path) return 1;
  return 0;
}
