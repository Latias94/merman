import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { readBuildReceipt } from "./build-receipt.mjs";
import { replaceDirectory } from "./replace-directory.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");
const descriptor = JSON.parse(readFileSync(path.join(nodeRoot, "package-surfaces.json"), "utf8"));

if (isMainModule()) {
  try {
    const target = valueAfter(process.argv.slice(2), "--target");
    if (!target) throw new Error("usage: node scripts/assemble-packages.mjs --target <target-id>");
    assembleNativePackages(target);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

export function assembleNativePackages(
  target,
  outputRoot = path.join(nodeRoot, "dist-packages"),
  binaryOverride = null,
  { readReceipt = readBuildReceipt } = {},
) {
  const targetDescriptor = descriptor.targets.find((item) => item.target === target);
  if (!targetDescriptor) throw new Error(`Unknown Node target: ${target}.`);
  assertAssemblyPackageManifest(descriptor.root);
  assertAssemblyPackageManifest(targetDescriptor);
  const binary = binaryOverride ?? path.join(nodeRoot, "artifacts", "napi", target, "merman.node");
  assertFile(binary, "native candidate binary");
  assertNativeBuildProvenance(targetDescriptor, readReceipt(binary));

  const stage = `${outputRoot}.stage-${process.pid}`;
  rmSync(stage, { recursive: true, force: true });
  mkdirSync(stage, { recursive: true });
  try {
    assembleLoaderPackage(path.join(stage, "node"));
    assemblePlatformPackage(targetDescriptor, binary, path.join(stage, target));
    replaceDirectory(stage, outputRoot);
  } catch (error) {
    rmSync(stage, { recursive: true, force: true });
    throw error;
  }
}

function assembleLoaderPackage(output) {
  const source = path.join(nodeRoot, descriptor.root.directory);
  mkdirSync(path.join(output, "dist", "candidates"), { recursive: true });
  cpSync(path.join(source, "package.json"), path.join(output, "package.json"));
  cpSync(path.join(source, "README.md"), path.join(output, "README.md"));
  for (const name of ["index.mjs", "engine.mjs", "errors.mjs", "bounded-executor.mjs", "native-loader.mjs"]) {
    cpSync(path.join(nodeRoot, "src", name), path.join(output, "dist", name));
  }
  cpSync(
    path.join(nodeRoot, "src", "candidates", "native.mjs"),
    path.join(output, "dist", "candidates", "native.mjs"),
  );
  cpSync(
    path.join(nodeRoot, "src", "candidates", "wrap-engine.mjs"),
    path.join(output, "dist", "candidates", "wrap-engine.mjs"),
  );
  cpSync(path.join(nodeRoot, "src", "index.d.ts"), path.join(output, "dist", "index.d.ts"));
  projectLegalMaterial(output);
}

function assemblePlatformPackage(targetDescriptor, binary, output) {
  const source = path.join(nodeRoot, targetDescriptor.directory);
  mkdirSync(output, { recursive: true });
  cpSync(path.join(source, "package.json"), path.join(output, "package.json"));
  cpSync(binary, path.join(output, targetDescriptor.node_artifact));
  writeFileSync(
    path.join(output, "README.md"),
    `# ${targetDescriptor.name}\n\nPrivate U14 native candidate for ${targetDescriptor.target}.\n`,
  );
  projectLegalMaterial(output);
}

function assertNativeBuildProvenance(targetDescriptor, receipt) {
  if (receipt?.candidate !== "napi") {
    throw new Error(
      `Native candidate build receipt candidate ${JSON.stringify(receipt?.candidate)} must be napi.`,
    );
  }
  if (receipt.target !== targetDescriptor.target) {
    throw new Error(
      `Native candidate build receipt target ${JSON.stringify(receipt?.target)} does not match ${targetDescriptor.target}.`,
    );
  }
}

function assertAssemblyPackageManifest(packageDescriptor) {
  const manifestPath = path.join(nodeRoot, packageDescriptor.directory, "package.json");
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch (cause) {
    throw new Error(`Invalid Node candidate package manifest: ${manifestPath}.`, { cause });
  }
  if (
    manifest?.name !== packageDescriptor.name ||
    manifest.version !== descriptor.version ||
    manifest.private !== true
  ) {
    throw new Error(
      `${packageDescriptor.name} manifest must be the private ${descriptor.version} candidate package.`,
    );
  }
}

export function projectLegalMaterial(output) {
  cpSync(path.join(repositoryRoot, "LICENSE-APACHE"), path.join(output, "LICENSE-APACHE"));
  cpSync(path.join(repositoryRoot, "LICENSE-MIT"), path.join(output, "LICENSE-MIT"));
  cpSync(
    path.join(repositoryRoot, "THIRD_PARTY_NOTICES.md"),
    path.join(output, "THIRD_PARTY_NOTICES.md"),
  );
  cpSync(
    path.join(repositoryRoot, "THIRD_PARTY_LICENSES"),
    path.join(output, "THIRD_PARTY_LICENSES"),
    { recursive: true },
  );
}

function assertFile(file, label) {
  if (!existsSync(file) || !statSync(file).isFile()) throw new Error(`Missing ${label}: ${file}.`);
}

function valueAfter(args, flag) {
  const index = args.indexOf(flag);
  return index === -1 ? null : args[index + 1] ?? null;
}

function isMainModule() {
  return process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}
