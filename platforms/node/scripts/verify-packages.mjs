import { readFile } from "node:fs/promises";
import { statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  inspectPackageManifests,
  verifyPackedFileOwnership,
} from "./package-contract.mjs";
import { spawnNpmSync } from "../../../scripts/npm-command.mjs";

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
    const packedRoot = valueAfter(process.argv.slice(2), "--packed-root");
    if (packedRoot) verifyPackedRoot(path.resolve(packedRoot), descriptor);
    console.log("[merman-node] candidate package contracts verified");
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

function verifyPackedRoot(root, descriptor) {
  verifyPackage(path.join(root, "node"), descriptor.root.name, "loader");
  for (const target of descriptor.targets) {
    if (!existsForTarget(root, target.target)) continue;
    verifyPackage(path.join(root, target.target), target.name, "platform");
  }
}

function verifyPackage(packageRoot, packageName, role) {
  const result = spawnNpmSync(["pack", "--json", "--dry-run"], {
    cwd: packageRoot,
    encoding: "utf8",
  });
  if (result.error || result.status !== 0) {
    throw new Error(`npm pack failed for ${packageName}: ${result.error?.message ?? result.stderr}`);
  }
  const output = JSON.parse(result.stdout);
  verifyPackedFileOwnership({ packageName, role, files: output[0]?.files ?? [] });
}

export function npmExecutable(platform = process.platform) {
  return platform === "win32" ? "npm.cmd" : "npm";
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
  return process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}
