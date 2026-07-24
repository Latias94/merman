import { spawnSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  assembleNativePackages,
  projectLegalMaterial,
} from "../assemble-packages.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
export function measureFootprint({ candidate, artifact, target }) {
  const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), `merman-${candidate}-footprint-`));
  try {
    const packageRoots = candidate === "napi"
      ? stageNativePackages(temporaryRoot, target, artifact)
      : [stageWasmPackage(temporaryRoot, artifact)];
    return packAndInstall(temporaryRoot, packageRoots, candidate);
  } finally {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }
}

function stageNativePackages(temporaryRoot, target, artifact) {
  const packagesRoot = path.join(temporaryRoot, "packages");
  assembleNativePackages(target, packagesRoot, artifact);
  return [path.join(packagesRoot, "node"), path.join(packagesRoot, target)];
}

export function stageWasmPackage(temporaryRoot, artifact) {
  const packageRoot = path.join(temporaryRoot, "wasm-package");
  mkdirSync(path.join(packageRoot, "artifact"), { recursive: true });
  cpSync(path.dirname(artifact), path.join(packageRoot, "artifact"), { recursive: true });
  cpSync(path.join(nodeRoot, "src"), path.join(packageRoot, "src"), { recursive: true });
  // wasm-pack writes a wildcard .gitignore for its output directory. npm applies nested ignore
  // files while packing, so retaining it would silently remove the candidate loader and WASM.
  rmSync(path.join(packageRoot, "artifact", ".gitignore"), { force: true });
  writeFileSync(
    path.join(packageRoot, "package.json"),
    `${JSON.stringify({
      name: "@mermanjs/node-wasm-candidate",
      version: "0.8.0-alpha.3",
      private: true,
      type: "module",
      main: "./index.mjs",
      exports: { ".": "./index.mjs" },
      files: [
        "artifact",
        "index.mjs",
        "src",
        "LICENSE-APACHE",
        "LICENSE-MIT",
        "THIRD_PARTY_LICENSES",
        "THIRD_PARTY_NOTICES.md",
      ],
      license: "MIT OR Apache-2.0",
    }, null, 2)}\n`,
  );
  writeFileSync(
    path.join(packageRoot, "index.mjs"),
    [
      'import { createNodeEngine as createEngineWithTransport } from "./src/engine.mjs";',
      'import { loadNodeWasmTransport } from "./src/candidates/wasm.mjs";',
      'const modulePath = new URL("./artifact/merman_node.js", import.meta.url).href;',
      "export function createNodeEngine(options) {",
      "  return createEngineWithTransport(options, {",
      "    loadTransport: (optionsJson) => loadNodeWasmTransport(optionsJson, { modulePath }),",
      "  });",
      "}",
      "",
    ].join("\n"),
  );
  projectLegalMaterial(packageRoot);
  return packageRoot;
}

function packAndInstall(temporaryRoot, packageRoots, candidate) {
  const tarRoot = path.join(temporaryRoot, "tarballs");
  mkdirSync(tarRoot, { recursive: true });
  const packed = packageRoots.map((packageRoot) => npmPack(packageRoot, tarRoot));
  const installRoot = path.join(temporaryRoot, "install");
  mkdirSync(installRoot, { recursive: true });
  const installManifest = {
    name: "merman-node-footprint-probe",
    private: true,
    version: "0.0.0",
    dependencies: {},
  };
  if (candidate === "napi") {
    const [rootPackage, targetPackage] = packed;
    installManifest.dependencies[rootPackage.name] = `file:${path.join(tarRoot, rootPackage.filename)}`;
    installManifest.overrides = {
      [targetPackage.name]: `file:${path.join(tarRoot, targetPackage.filename)}`,
    };
  } else {
    const [wasmPackage] = packed;
    installManifest.dependencies[wasmPackage.name] = `file:${path.join(tarRoot, wasmPackage.filename)}`;
  }
  writeFileSync(
    path.join(installRoot, "package.json"),
    `${JSON.stringify(installManifest, null, 2)}\n`,
  );
  run("npm", [
    "install",
    "--ignore-scripts",
    "--no-audit",
    "--no-fund",
  ], installRoot);
  const installedRoot = path.join(installRoot, "node_modules");
  probeInstalledRuntime(installRoot, candidate);
  const nativePackagePair = candidate === "napi";
  return {
    packed_bytes: packed.reduce((sum, item) => sum + item.size, 0),
    unpacked_bytes: packed.reduce((sum, item) => sum + item.unpacked_size, 0),
    installed_bytes: treeSize(installedRoot),
    runtime_api_passed: true,
    install_method: nativePackagePair ? "root-optional-dependency" : "single-package",
    target_install_passed: true,
    package_count: packed.length,
    packages: packed,
    installed_files: treeEntries(installedRoot),
  };
}

function npmPack(packageRoot, tarRoot) {
  const result = spawnSync("npm", ["pack", "--json", "--pack-destination", tarRoot], {
    cwd: packageRoot,
    encoding: "utf8",
  });
  if (result.error || result.status !== 0) {
    throw new Error(`npm pack failed: ${result.error?.message ?? result.stderr}`);
  }
  const output = JSON.parse(result.stdout)[0];
  if (
    typeof output?.name !== "string" ||
    output.name.length === 0 ||
    !output.filename ||
    !Number.isFinite(output.size) ||
    !Number.isFinite(output.unpackedSize)
  ) {
    throw new Error("npm pack returned an incomplete footprint result.");
  }
  if (!Array.isArray(output.files)) throw new Error("npm pack returned no package contents.");
  return {
    name: output.name,
    filename: output.filename,
    size: output.size,
    unpacked_size: output.unpackedSize,
    files: output.files.map((file) => ({ path: file.path, bytes: file.size })),
  };
}

function treeSize(root) {
  let bytes = 0;
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const absolute = path.join(root, entry.name);
    if (entry.isDirectory()) bytes += treeSize(absolute);
    else if (entry.isFile()) bytes += statSync(absolute).size;
  }
  return bytes;
}

function treeEntries(root, current = root) {
  const files = [];
  for (const entry of readdirSync(current, { withFileTypes: true })) {
    const absolute = path.join(current, entry.name);
    if (entry.isDirectory()) files.push(...treeEntries(root, absolute));
    else if (entry.isFile()) {
      files.push({
        path: path.relative(root, absolute).split(path.sep).join("/"),
        bytes: statSync(absolute).size,
      });
    }
  }
  return files.sort((left, right) => left.path.localeCompare(right.path));
}

function probeInstalledRuntime(installRoot, candidate) {
  if (candidate === "napi") {
    const script = [
      'import { createNodeEngine } from "@mermanjs/node";',
      'const engine = await createNodeEngine({ bindingOptions: { version: 1, runtime_policy: "deterministic", resources: { profile: "trusted-native" } } });',
      'const svg = await engine.renderSvg("flowchart TD\\nA-->B");',
      'await engine.dispose();',
      'if (!svg.includes("<svg")) process.exit(1);',
    ].join("\n");
    run(process.execPath, ["--input-type=module", "--eval", script], installRoot);
    return;
  }
  if (candidate === "node-wasm") {
    const script = [
      'import { createNodeEngine } from "@mermanjs/node-wasm-candidate";',
      'const engine = await createNodeEngine({ bindingOptions: { version: 1, runtime_policy: "deterministic", resources: { profile: "trusted-native" } } });',
      'const svg = await engine.renderSvg("flowchart TD\\nA-->B");',
      'await engine.dispose();',
      'if (!svg.includes("<svg")) process.exit(1);',
    ].join("\n");
    run(process.execPath, ["--input-type=module", "--eval", script], installRoot);
    return;
  }
  throw new Error(`Unknown footprint candidate: ${candidate}.`);
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.error || result.status !== 0) {
    throw new Error(`${command} failed: ${result.error?.message ?? result.stderr}`);
  }
}
