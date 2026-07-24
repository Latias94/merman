import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { resolveNodeTarget } from "../src/native-loader.mjs";
import { replaceDirectory } from "./replace-directory.mjs";
import { digestJson, stableJson } from "./stable-json.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");
const descriptor = readJson(path.join(nodeRoot, "candidate-builds.json"));
assertDescriptor();
const artifactProfileDescriptorPath = path.join(
  repositoryRoot,
  descriptor.artifact_profile.descriptor,
);
const artifactProfiles = readJson(artifactProfileDescriptorPath);
const artifactsRoot = path.join(nodeRoot, "artifacts");

if (isMainModule()) {
  try {
    const selection = parseArgs(process.argv.slice(2));
    buildCandidate(selection);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

export function buildCandidate({ candidate, target = null }) {
  assertDescriptor();
  mkdirSync(artifactsRoot, { recursive: true });
  const resolvedTarget = candidate === "napi" ? target ?? resolveNodeTarget() : null;
  const recipe = resolveCandidateRecipe(candidate, resolvedTarget);
  const metadata = cargoMetadata(recipe);
  const before = collectLocalInputEntries(metadata);
  const stage = mkdtempSync(path.join(artifactsRoot, `.stage-${candidate}-`));
  const output = candidate === "napi"
    ? path.join(artifactsRoot, "napi", resolvedTarget)
    : path.join(artifactsRoot, "node-wasm");

  try {
    if (candidate === "napi") buildNapi(stage, recipe);
    else buildNodeWasm(stage, recipe);
    normalizeArtifacts(stage, candidate);

    const after = collectLocalInputEntries(cargoMetadata(recipe));
    if (stableJson(before) !== stableJson(after)) {
      throw new Error("Node candidate source inputs changed during the build; rerun it.");
    }
    writeBuildReceipt(stage, recipe, before);
    replaceDirectory(stage, output);
    console.log(`[merman-node] built ${candidate}${resolvedTarget ? ` for ${resolvedTarget}` : ""}`);
  } catch (error) {
    rmSync(stage, { recursive: true, force: true });
    throw error;
  }
}

function buildNapi(stage, recipe) {
  const cli = path.join(nodeRoot, "node_modules", "@napi-rs", "cli", "dist", "cli.js");
  assertFile(cli, "pinned @napi-rs/cli; run npm ci first");
  const invocation = candidateBuildInvocation(recipe, stage);
  run(invocation.command, invocation.args);
}

function buildNodeWasm(stage, recipe) {
  const invocation = candidateBuildInvocation(recipe, stage);
  run(invocation.command, invocation.args);
}

export function candidateBuildInvocation(recipe, stage) {
  const featureArgument = recipe.cargoFeatures.join(",");
  if (recipe.candidate === "napi") {
    return {
      command: process.execPath,
      args: [
        path.join(nodeRoot, "node_modules", "@napi-rs", "cli", "dist", "cli.js"),
        "build",
        "--cwd",
        repositoryRoot,
        "--manifest-path",
        descriptor.cargo.manifest,
        "--package-json-path",
        path.join(nodeRoot, "package.json"),
        "--target",
        recipe.rustTarget,
        "--output-dir",
        stage,
        "--platform",
        "--no-js",
        "--dts",
        "native-internal.d.ts",
        "--release",
        "--strip",
        "--no-default-features",
        "--features",
        featureArgument,
        "--",
        "--locked",
      ],
    };
  }
  if (recipe.candidate === "node-wasm") {
    return {
      command: "wasm-pack",
      args: [
        "build",
        path.dirname(path.join(repositoryRoot, descriptor.cargo.manifest)),
        "--target",
        recipe.wasmPackTarget,
        "--release",
        "--no-pack",
        "--out-dir",
        stage,
        "--",
        "--locked",
        "--no-default-features",
        "--features",
        featureArgument,
      ],
    };
  }
  throw new Error(`Unknown candidate: ${recipe.candidate}.`);
}

function normalizeArtifacts(stage, candidate) {
  if (candidate === "napi") {
    const nodeFiles = walkFiles(stage).filter((file) => file.endsWith(".node"));
    if (nodeFiles.length !== 1) {
      throw new Error(`napi build produced ${nodeFiles.length} .node files; expected exactly one.`);
    }
    const canonical = path.join(stage, "merman.node");
    if (path.resolve(nodeFiles[0]) !== path.resolve(canonical)) renameSync(nodeFiles[0], canonical);
    return;
  }
  assertFile(path.join(stage, "merman_node.js"), "Node-targeted wasm-bindgen loader");
  assertFile(path.join(stage, "merman_node_bg.wasm"), "Node-targeted WASM binary");
  const wasmFiles = walkFiles(stage).filter((file) => file.endsWith(".wasm"));
  if (wasmFiles.length !== 1) {
    throw new Error(`Node WASM build produced ${wasmFiles.length} WASM files; expected exactly one.`);
  }
  writeFileSync(
    path.join(stage, "package.json"),
    `${JSON.stringify({ private: true, type: "commonjs" }, null, 2)}\n`,
  );
}

function writeBuildReceipt(stage, recipe, inputEntries) {
  const sourceDigest = digestJson(inputEntries);
  const bindingContractEntries = collectBindingContractEntries(inputEntries);
  const bindingContractDigest = digestJson(bindingContractEntries);
  const tools = {
    cargo: runCapture("cargo", ["--version"]),
    node: process.version,
    rustc: runCapture("rustc", ["--version", "--verbose"]),
    transport_builder:
      recipe.candidate === "napi"
        ? runCapture(process.execPath, [
            path.join(nodeRoot, "node_modules", "@napi-rs", "cli", "dist", "cli.js"),
            "--version",
          ])
        : runCapture("wasm-pack", ["--version"]),
  };
  const config = {
    candidate: recipe.candidate,
    rust_target: recipe.rustTarget,
    wasm_pack_target: recipe.wasmPackTarget,
    default_features: false,
    artifact_profile: {
      descriptor: descriptor.artifact_profile.descriptor,
      id: descriptor.artifact_profile.id,
      features: recipe.capabilityFeatures,
    },
    features: recipe.cargoFeatures,
  };
  const receipt = {
    schema_version: 1,
    config,
    commit: runCapture("git", ["rev-parse", "HEAD"]),
    source_digest: sourceDigest,
    binding_contract_digest: bindingContractDigest,
    input_digest: digestJson({ config, source_digest: sourceDigest, tools }),
    tools,
    artifacts: artifactEntries(stage),
  };
  writeFileSync(path.join(stage, "build-receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`);
}

function collectBindingContractEntries(inputEntries) {
  // A candidate receipt keeps its complete artifact closure separately. This digest
  // intentionally excludes transport wrappers and binds the shared Rust behavior.
  const transportPrefixes = ["crates/merman-node/", "platforms/node/"];
  const entries = inputEntries.filter(
    (entry) => !transportPrefixes.some((prefix) => entry.path.startsWith(prefix)),
  );
  if (entries.length === 0) {
    throw new Error("Node candidate receipt has no shared bindings-contract inputs.");
  }
  return entries;
}

function cargoMetadata(recipe) {
  const args = [
    "metadata",
    "--locked",
    "--format-version",
    "1",
    "--manifest-path",
    path.join(repositoryRoot, descriptor.cargo.manifest),
    "--filter-platform",
    recipe.rustTarget,
  ];
  if (!descriptor.cargo.default_features) args.push("--no-default-features");
  if (recipe.cargoFeatures.length > 0) {
    args.push("--features", recipe.cargoFeatures.join(","));
  }
  return JSON.parse(runCapture("cargo", args));
}

function collectLocalInputEntries(metadata) {
  const roots = new Set();
  for (const item of metadata.packages) {
    if (item.source !== null) continue;
    const manifest = path.resolve(item.manifest_path);
    if (!isWithin(repositoryRoot, manifest)) continue;
    roots.add(path.dirname(manifest));
  }
  const files = new Set([
    path.join(repositoryRoot, "Cargo.toml"),
    path.join(repositoryRoot, "Cargo.lock"),
    artifactProfileDescriptorPath,
    path.join(nodeRoot, "candidate-builds.json"),
    path.join(nodeRoot, "package-lock.json"),
    path.join(nodeRoot, "scripts", "build-candidate.mjs"),
  ]);
  for (const root of roots) {
    for (const file of walkFiles(root, { skipBuildOutputs: true })) files.add(file);
  }
  return [...files]
    .filter((file) => existsSync(file) && statSync(file).isFile())
    .map((file) => ({
      path: path.relative(repositoryRoot, file).split(path.sep).join("/"),
      bytes: statSync(file).size,
      sha256: digestFile(file),
    }))
    .sort((left, right) => left.path.localeCompare(right.path));
}

function artifactEntries(stage) {
  return walkFiles(stage)
    .filter((file) => path.basename(file) !== "build-receipt.json")
    .map((file) => ({
      path: path.relative(stage, file).split(path.sep).join("/"),
      bytes: statSync(file).size,
      sha256: digestFile(file),
    }))
    .sort((left, right) => left.path.localeCompare(right.path));
}

export function resolveCandidateRecipe(candidate, target) {
  assertDescriptor();
  const capabilityFeatures = artifactProfileFeatures();
  if (candidate === "node-wasm") {
    const wasm = descriptor.candidates["node-wasm"];
    return completeCandidateRecipe(
      {
        candidate,
        rustTarget: wasm.rust_target,
        transportFeature: wasm.transport_feature,
        wasmPackTarget: wasm.wasm_pack_target,
      },
      capabilityFeatures,
    );
  }
  if (candidate !== "napi") throw new Error(`Unknown candidate: ${candidate}.`);
  const nativeTarget = descriptor.candidates.napi.targets.find((item) => item.id === target);
  if (!nativeTarget) throw new Error(`Unknown napi target: ${target}.`);
  return completeCandidateRecipe(
    {
      candidate,
      rustTarget: nativeTarget.rust_target,
      transportFeature: descriptor.candidates.napi.transport_feature,
      wasmPackTarget: null,
    },
    capabilityFeatures,
  );
}

function completeCandidateRecipe(recipe, capabilityFeatures) {
  if (
    typeof recipe.transportFeature !== "string" ||
    !recipe.transportFeature.startsWith("transport-")
  ) {
    throw new Error(`Invalid transport feature for ${recipe.candidate}.`);
  }
  if (capabilityFeatures.includes(recipe.transportFeature)) {
    throw new Error(
      `Transport feature collides with the artifact profile: ${recipe.transportFeature}.`,
    );
  }
  return {
    ...recipe,
    capabilityFeatures,
    cargoFeatures: [...capabilityFeatures, recipe.transportFeature].sort(),
  };
}

function artifactProfileFeatures() {
  if (artifactProfiles.schema_version !== 1 || !Array.isArray(artifactProfiles.profiles)) {
    throw new Error("Artifact profile descriptor is invalid.");
  }
  const matches = artifactProfiles.profiles.filter(
    (profile) => profile?.id === descriptor.artifact_profile.id,
  );
  if (matches.length !== 1) {
    throw new Error(
      `Artifact profile ${descriptor.artifact_profile.id} must occur exactly once; found ${matches.length}.`,
    );
  }
  const cargo = matches[0].cargo;
  if (
    cargo?.default_features !== false ||
    !Array.isArray(cargo.features) ||
    cargo.features.length === 0 ||
    cargo.features.some((feature) => typeof feature !== "string" || feature.length === 0)
  ) {
    throw new Error(
      `Artifact profile ${descriptor.artifact_profile.id} has an invalid Cargo recipe.`,
    );
  }
  const features = [...cargo.features];
  if (stableJson(features) !== stableJson([...new Set(features)].sort())) {
    throw new Error(
      `Artifact profile ${descriptor.artifact_profile.id} features must be sorted and unique.`,
    );
  }
  return features;
}

function parseArgs(args) {
  if (args.includes("--help") || args.includes("-h")) {
    console.log("usage: node scripts/build-candidate.mjs --candidate <node-wasm|napi> [--target <target-id>]");
    process.exit(0);
  }
  const candidate = valueAfter(args, "--candidate");
  const target = valueAfter(args, "--target");
  const known = new Set(["--candidate", "--target"]);
  for (let index = 0; index < args.length; index += 2) {
    if (!known.has(args[index]) || args[index + 1] === undefined) {
      throw new Error(`Unknown or incomplete argument: ${args[index] ?? "<missing>"}.`);
    }
  }
  if (!candidate) throw new Error("--candidate is required.");
  if (candidate === "node-wasm" && target !== null) {
    throw new Error("--target applies only to the napi candidate.");
  }
  return { candidate, target };
}

function assertDescriptor() {
  if (
    descriptor.schema_version !== 2 ||
    descriptor.status !== "private-evaluation" ||
    descriptor.cargo.default_features !== false ||
    descriptor.cargo.manifest !== "crates/merman-node/Cargo.toml" ||
    descriptor.artifact_profile?.descriptor !== "capabilities/artifact-profiles-v1.json" ||
    descriptor.artifact_profile?.id !== "rust-static-svg"
  ) {
    throw new Error("Node candidate build descriptor is invalid.");
  }
}

function walkFiles(root, { skipBuildOutputs = false } = {}) {
  if (!existsSync(root)) return [];
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    if (
      skipBuildOutputs &&
      entry.isDirectory() &&
      new Set([".git", "node_modules", "target", "artifacts", "dist-packages", "reports"]).has(entry.name)
    ) {
      continue;
    }
    const absolute = path.join(root, entry.name);
    if (entry.isDirectory()) files.push(...walkFiles(absolute, { skipBuildOutputs }));
    else if (entry.isFile()) files.push(absolute);
  }
  return files;
}

function digestFile(file) {
  const digest = createHash("sha256");
  digest.update(readFileSync(file));
  return `sha256:${digest.digest("hex")}`;
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: nodeRoot,
    env: { ...process.env, CARGO_TARGET_DIR: path.join(repositoryRoot, "target") },
    stdio: "inherit",
  });
  if (result.error) throw new Error(`Failed to run ${command}: ${result.error.message}`);
  if (result.status !== 0) throw new Error(`${command} exited with status ${result.status ?? 1}.`);
}

function runCapture(command, args) {
  const result = spawnSync(command, args, { cwd: nodeRoot, encoding: "utf8" });
  if (result.error) throw new Error(`Failed to run ${command}: ${result.error.message}`);
  if (result.status !== 0) {
    throw new Error(`${command} exited with status ${result.status ?? 1}: ${(result.stderr ?? "").trim()}`);
  }
  return result.stdout.trim();
}

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

function assertFile(file, label) {
  if (!existsSync(file) || !statSync(file).isFile()) throw new Error(`Missing ${label}: ${file}.`);
}

function valueAfter(args, flag) {
  const index = args.indexOf(flag);
  return index === -1 ? null : args[index + 1] ?? null;
}

function isWithin(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative !== "" && !relative.startsWith("..") && !path.isAbsolute(relative);
}

function isMainModule() {
  return process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}
