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
    const args = process.argv.slice(2);
    const outputRoot = valueAfter(args, "--output-root") ?? path.join(nodeRoot, "dist-packages");
    if (args.includes("--loader-only")) {
      assembleLoaderOnly(outputRoot);
    } else if (args.includes("--wasm")) {
      assembleWasmPackage(outputRoot);
    } else {
      const target = valueAfter(args, "--target");
      if (!target) {
        throw new Error(
          "usage: node scripts/assemble-packages.mjs (--loader-only | --wasm | --target <target-id>) [--output-root <path>]",
        );
      }
      assembleNativePackages(target, outputRoot);
    }
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

export function assembleLoaderOnly(outputRoot = path.join(nodeRoot, "dist-packages")) {
  assertAssemblyPackageManifest(descriptor.root);
  const stage = `${outputRoot}.stage-${process.pid}`;
  rmSync(stage, { recursive: true, force: true });
  mkdirSync(stage, { recursive: true });
  try {
    assembleLoaderPackage(path.join(stage, "node"));
    replaceDirectory(stage, outputRoot);
  } catch (error) {
    rmSync(stage, { recursive: true, force: true });
    throw error;
  }
}

export function assembleWasmPackage(
  outputRoot = path.join(nodeRoot, "dist-packages"),
  wasmOverride = null,
  { readReceipt = readBuildReceipt } = {},
) {
  const wasmDescriptor = descriptor.wasm;
  if (!wasmDescriptor) throw new Error("Node package surface does not declare a WASM package.");
  assertAssemblyPackageManifest(wasmDescriptor);
  const candidateRoot = wasmOverride
    ? path.dirname(wasmOverride)
    : path.join(nodeRoot, "artifacts", "node-wasm");
  const wasm = wasmOverride ?? path.join(candidateRoot, wasmDescriptor.wasm_artifact);
  const loader = path.join(candidateRoot, "merman_node.js");
  assertFile(wasm, "Node-targeted WASM binary");
  assertFile(loader, "Node-targeted WASM loader");
  assertWasmBuildProvenance(wasmDescriptor, readReceipt(wasm));

  const stage = `${outputRoot}.stage-${process.pid}`;
  rmSync(stage, { recursive: true, force: true });
  mkdirSync(stage, { recursive: true });
  try {
    assembleWasmPackageContents(
      wasmDescriptor,
      loader,
      wasm,
      path.join(stage, path.basename(wasmDescriptor.directory)),
    );
    replaceDirectory(stage, outputRoot);
  } catch (error) {
    rmSync(stage, { recursive: true, force: true });
    throw error;
  }
}

function assembleLoaderPackage(output) {
  const source = path.join(nodeRoot, descriptor.root.directory);
  mkdirSync(path.join(output, "dist", "candidates"), { recursive: true });
  mkdirSync(path.join(output, "dist", "generated"), { recursive: true });
  cpSync(path.join(source, "package.json"), path.join(output, "package.json"));
  cpSync(path.join(source, "README.md"), path.join(output, "README.md"));
  cpSync(path.join(nodeRoot, "CHANGELOG.md"), path.join(output, "CHANGELOG.md"));
  for (const name of [
    "index.mjs",
    "engine.mjs",
    "errors.mjs",
    "bounded-executor.mjs",
    "native-loader.mjs",
    "transport-contract.mjs",
  ]) {
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
  cpSync(
    path.join(nodeRoot, "src", "generated", "capability-surface.mjs"),
    path.join(output, "dist", "generated", "capability-surface.mjs"),
  );
  cpSync(
    path.join(nodeRoot, "src", "generated", "binding-contract.mjs"),
    path.join(output, "dist", "generated", "binding-contract.mjs"),
  );
  cpSync(
    path.join(nodeRoot, "src", "generated", "node-wire-contract.json"),
    path.join(output, "dist", "generated", "node-wire-contract.json"),
  );
  cpSync(path.join(nodeRoot, "src", "index.d.ts"), path.join(output, "dist", "index.d.ts"));
  projectLegalMaterial(output);
}

function assembleWasmPackageContents(packageDescriptor, loader, wasm, output) {
  const source = path.join(nodeRoot, packageDescriptor.directory);
  mkdirSync(path.join(output, "dist", "candidates"), { recursive: true });
  mkdirSync(path.join(output, "dist", "generated"), { recursive: true });
  mkdirSync(path.join(output, packageDescriptor.artifact_directory), { recursive: true });
  cpSync(path.join(source, "package.json"), path.join(output, "package.json"));
  cpSync(path.join(source, "README.md"), path.join(output, "README.md"));
  cpSync(path.join(nodeRoot, "CHANGELOG.md"), path.join(output, "CHANGELOG.md"));
  for (const name of [
    "engine.mjs",
    "errors.mjs",
    "bounded-executor.mjs",
    "native-loader.mjs",
    "transport-contract.mjs",
  ]) {
    cpSync(path.join(nodeRoot, "src", name), path.join(output, "dist", name));
  }
  cpSync(
    path.join(nodeRoot, "src", "node-wasm-index.mjs"),
    path.join(output, "dist", "index.mjs"),
  );
  cpSync(
    path.join(nodeRoot, "src", "candidates", "wasm.mjs"),
    path.join(output, "dist", "candidates", "wasm.mjs"),
  );
  cpSync(
    path.join(nodeRoot, "src", "candidates", "wrap-engine.mjs"),
    path.join(output, "dist", "candidates", "wrap-engine.mjs"),
  );
  cpSync(
    path.join(nodeRoot, "src", "generated", "capability-surface.mjs"),
    path.join(output, "dist", "generated", "capability-surface.mjs"),
  );
  cpSync(
    path.join(nodeRoot, "src", "generated", "binding-contract.mjs"),
    path.join(output, "dist", "generated", "binding-contract.mjs"),
  );
  cpSync(
    path.join(nodeRoot, "src", "generated", "node-wire-contract.json"),
    path.join(output, "dist", "generated", "node-wire-contract.json"),
  );
  cpSync(path.join(nodeRoot, "src", "index.d.ts"), path.join(output, "dist", "index.d.ts"));
  cpSync(loader, path.join(output, packageDescriptor.artifact_directory, packageDescriptor.node_artifact));
  cpSync(wasm, path.join(output, packageDescriptor.artifact_directory, packageDescriptor.wasm_artifact));
  projectLegalMaterial(output);
}

function assemblePlatformPackage(targetDescriptor, binary, output) {
  const source = path.join(nodeRoot, targetDescriptor.directory);
  mkdirSync(output, { recursive: true });
  cpSync(path.join(source, "package.json"), path.join(output, "package.json"));
  cpSync(binary, path.join(output, targetDescriptor.node_artifact));
  writeFileSync(
    path.join(output, "README.md"),
    `# ${targetDescriptor.name}\n\nNative runtime package for \`@mermanjs/node\` on ${targetDescriptor.target}. Install \`@mermanjs/node\` instead of depending on this package directly.\n`,
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

function assertWasmBuildProvenance(packageDescriptor, receipt) {
  if (receipt?.candidate !== "node-wasm") {
    throw new Error(
      `Node WASM candidate build receipt candidate ${JSON.stringify(receipt?.candidate)} must be node-wasm.`,
    );
  }
  if (receipt.target !== null) {
    throw new Error(
      `Node WASM candidate build receipt target ${JSON.stringify(receipt.target)} must be null.`,
    );
  }
  if (packageDescriptor.wasm_artifact !== "merman_node_bg.wasm") {
    throw new Error("Node WASM package descriptor must use the canonical wasm-bindgen artifact.");
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
    manifest.private === true ||
    manifest.publishConfig?.access !== "public" ||
    manifest.engines?.node !== descriptor.node_engine
  ) {
    throw new Error(
      `${packageDescriptor.name} manifest must be the public ${descriptor.version} alpha package with Node ${descriptor.node_engine}.`,
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
