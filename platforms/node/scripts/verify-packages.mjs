import { readFile } from "node:fs/promises";
import { readFileSync, realpathSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  inspectPackageManifests,
  verifyPackedFileOwnership,
} from "./package-contract.mjs";
import {
  assertSuccessfulNpmSpawn,
  npmPackRecord,
  spawnNpmSync,
} from "../../../scripts/npm-command.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

if (isMainModule()) {
  await main();
}

async function main() {
  try {
    const descriptor = JSON.parse(
      await readFile(path.join(nodeRoot, "package-surfaces.json"), "utf8"),
    );
    await inspectPackageManifests(nodeRoot, descriptor);
    const args = process.argv.slice(2);
    const packedRoot = valueAfter(args, "--packed-root");
    const localShoal = args.includes("--local-shoal");
    if (localShoal && !packedRoot) throw new Error("--local-shoal requires --packed-root.");
    if (packedRoot) verifyPackedRoot(path.resolve(packedRoot), descriptor, localShoal);
    console.log("[merman-node] candidate package contracts verified");
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

function verifyPackedRoot(root, descriptor, localShoal) {
  const hasLoader = existsForTarget(root, "node");
  const targets = descriptor.targets.filter((target) => existsForTarget(root, target.target));
  const wasmDirectory = descriptor.wasm?.directory?.split("/").at(-1);
  const hasWasm = Boolean(wasmDirectory && existsForTarget(root, wasmDirectory));
  if (localShoal) {
    const nativePair = hasLoader && targets.length === 1 && !hasWasm;
    const wasmOnly = hasWasm && !hasLoader && targets.length === 0;
    if (!nativePair && !wasmOnly) {
      throw new Error("The local Shoal package root must contain a native pair or one Node WASM package.");
    }
  }
  if (!hasLoader && targets.length === 0 && !hasWasm) {
    throw new Error("packed Node package root contains no recognized package.");
  }
  if (hasLoader) verifyPackage(path.join(root, "node"), descriptor.root.name, "loader", localShoal);
  for (const target of targets) {
    verifyPackage(path.join(root, target.target), target.name, "platform", localShoal);
  }
  if (hasWasm) verifyPackage(path.join(root, wasmDirectory), descriptor.wasm.name, "wasm", localShoal);
}

function verifyPackage(packageRoot, packageName, role, localShoal) {
  const manifest = JSON.parse(readFileSync(path.join(packageRoot, "package.json"), "utf8"));
  if (localShoal) {
    if (manifest.private !== true || manifest.mermanLocalShoal !== true || Object.hasOwn(manifest, "publishConfig")) {
      throw new Error(`${packageName} must be private and marked as a local Shoal package.`);
    }
  } else if (manifest.private === true || Object.hasOwn(manifest, "mermanLocalShoal")) {
    throw new Error(`${packageName} is a private package and cannot be used for release verification.`);
  }
  const result = spawnNpmSync(["pack", "--json", "--dry-run"], {
    cwd: packageRoot,
    encoding: "utf8",
  });
  assertSuccessfulNpmSpawn(result, `npm pack for ${packageName}`);
  const output = npmPackRecord(result.stdout, packageName, { allowNpm11: true });
  verifyPackedFileOwnership({ packageName, role, files: output.files ?? [] });
}

function existsForTarget(root, target) {
  try {
    return statSync(path.join(root, target)).isDirectory();
  } catch {
    return false;
  }
}

function valueAfter(args, flag) {
  const index = args.indexOf(flag);
  return index === -1 ? null : args[index + 1] ?? null;
}

function isMainModule() {
  return process.argv[1] && realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url));
}
