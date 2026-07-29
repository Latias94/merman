import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { resolveNodeTarget } from "../src/native-loader.mjs";
import { validateRuntimeCatalog } from "../src/engine.mjs";
import {
  acquireExclusiveFileLock,
  ensureOwnedDirectory,
  replaceDirectory,
} from "./replace-directory.mjs";
import { svgTransportEvidence } from "./benchmark/svg-signature.mjs";
import { digestJson, stableJson } from "./stable-json.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");
const descriptor = readJson(path.join(nodeRoot, "candidate-builds.json"));
const packageSurfacePath = path.join(nodeRoot, "package-surfaces.json");
const packageSurface = readJson(packageSurfacePath);
assertDescriptor();
const capabilityDescriptorPath = path.join(
  repositoryRoot,
  descriptor.capability_recipe.descriptor,
);
const capabilitySurface = readJson(capabilityDescriptorPath);
const artifactsRoot = path.join(nodeRoot, "artifacts");
const requireFromBuild = createRequire(import.meta.url);
export const CANDIDATE_BUILD_ENVIRONMENT_REQUIRED_NAMES = Object.freeze([
  "AR",
  "CARGO_BUILD_RUSTC",
  "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
  "CARGO_BUILD_RUSTC_WRAPPER",
  "CARGO_BUILD_TARGET",
  "CARGO",
  "CARGO_ENCODED_RUSTFLAGS",
  "CARGO_HOME",
  "CARGO_INCREMENTAL",
  "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
  "CARGO_PROFILE_RELEASE_DEBUG",
  "CARGO_PROFILE_RELEASE_INCREMENTAL",
  "CARGO_PROFILE_RELEASE_LTO",
  "CARGO_PROFILE_RELEASE_OPT_LEVEL",
  "CARGO_PROFILE_RELEASE_PANIC",
  "CARGO_PROFILE_RELEASE_STRIP",
  "CC",
  "CFLAGS",
  "CPPFLAGS",
  "CXX",
  "CXXFLAGS",
  "DEVELOPER_DIR",
  "DYLD_INSERT_LIBRARIES",
  "HOME",
  "LANG",
  "LC_ALL",
  "LC_CTYPE",
  "LD_PRELOAD",
  "LDFLAGS",
  "MACOSX_DEPLOYMENT_TARGET",
  "NODE_OPTIONS",
  "NODE_PATH",
  "PATH",
  "RUSTC",
  "RUSTC_BOOTSTRAP",
  "RUSTC_WORKSPACE_WRAPPER",
  "RUSTC_WRAPPER",
  "RUSTFLAGS",
  "RUSTUP_HOME",
  "RUSTUP_TOOLCHAIN",
  "SDKROOT",
  "SOURCE_DATE_EPOCH",
  "SystemRoot",
  "SYSTEMROOT",
  "COMSPEC",
  "PATHEXT",
  "TEMP",
  "TMP",
  "TMPDIR",
  "WASM_BINDGEN_TEST_TIMEOUT",
  "WASM_PACK_PROFILE",
]);
const SNAPSHOT_PATHS = Object.freeze([
  ".cargo",
  "Cargo.lock",
  "Cargo.toml",
  "capabilities",
  "crates",
  "platforms/node",
  "rust-toolchain.toml",
]);
const gitSourceValidationCache = new Set();

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
  ensureOwnedDirectory(nodeRoot, artifactsRoot);
  const resolvedTarget = candidate === "napi" ? target ?? resolveNodeTarget() : null;
  const recipe = resolveCandidateRecipe(candidate, resolvedTarget);
  const output = candidate === "napi"
    ? path.join(artifactsRoot, "napi", resolvedTarget)
    : path.join(artifactsRoot, "node-wasm");
  assertBuildOutputPath(output);
  ensureOwnedDirectory(nodeRoot, path.dirname(output));
  const buildLock = acquireExclusiveFileLock(`${output}.build-lock`, {
    purpose: `${candidate} candidate build`,
  });
  let sourceSnapshot;
  let stage;

  try {
    sourceSnapshot = materializeCommittedSourceSnapshot();
    assertCurrentBuildControllerMatchesSnapshot(sourceSnapshot.sourceRoot);
    const buildEnvironment = resolveCandidateBuildEnvironment({
      sourceRoot: sourceSnapshot.sourceRoot,
    });
    const metadata = cargoMetadata(
      recipe,
      sourceSnapshot.sourceRoot,
      buildEnvironment.environment,
    );
    validateCandidatePackageVersions(metadata);
    const dependencyClosure = candidateDependencyClosure(
      metadata,
      recipe,
      sourceSnapshot.sourceRoot,
    );
    const before = collectLocalInputEntries(metadata, sourceSnapshot.sourceRoot);
    const beforeProvenance = resolveSourceProvenance(before, dependencyClosure, {
      commit: sourceSnapshot.commit,
      commitTree: sourceSnapshot.commitTree,
    });
    stage = mkdtempSync(path.join(artifactsRoot, `.stage-${candidate}-`));
    if (candidate === "napi") {
      buildNapi(
        stage,
        recipe,
        buildEnvironment.environment,
        sourceSnapshot.sourceRoot,
      );
    } else {
      buildNodeWasm(
        stage,
        recipe,
        buildEnvironment.environment,
        sourceSnapshot.sourceRoot,
      );
    }
    normalizeArtifacts(stage, candidate);
    const runtime = probeCandidateRuntime(stage, recipe);

    const afterMetadata = cargoMetadata(
      recipe,
      sourceSnapshot.sourceRoot,
      buildEnvironment.environment,
    );
    const after = collectLocalInputEntries(afterMetadata, sourceSnapshot.sourceRoot);
    if (stableJson(before) !== stableJson(after)) {
      throw new Error("Node candidate source inputs changed during the build; rerun it.");
    }
    if (
      dependencyClosure.digest !==
      candidateDependencyClosure(
        afterMetadata,
        recipe,
        sourceSnapshot.sourceRoot,
      ).digest
    ) {
      throw new Error("Node candidate dependency closure changed during the build; rerun it.");
    }
    const afterProvenance = resolveSourceProvenance(after, dependencyClosure, {
      commit: sourceSnapshot.commit,
      commitTree: sourceSnapshot.commitTree,
    });
    if (stableJson(beforeProvenance) !== stableJson(afterProvenance)) {
      throw new Error("Node candidate Git source provenance changed during the build; rerun it.");
    }
    if (
      stableJson(resolveCandidateBuildEnvironment({
        sourceRoot: sourceSnapshot.sourceRoot,
      }).contract) !==
      stableJson(buildEnvironment.contract)
    ) {
      throw new Error("Node candidate build environment changed during the build; rerun it.");
    }
    writeBuildReceipt(
      stage,
      recipe,
      beforeProvenance,
      dependencyClosure,
      runtime,
      buildEnvironment,
      sourceSnapshot.sourceRoot,
    );
    buildLock.assertOwned();
    replaceDirectory(stage, output, { ownershipRoot: nodeRoot });
    console.log(`[merman-node] built ${candidate}${resolvedTarget ? ` for ${resolvedTarget}` : ""}`);
  } finally {
    try {
      if (stage && existsSync(stage)) rmSync(stage, { recursive: true, force: true });
    } finally {
      try {
        sourceSnapshot?.dispose();
      } finally {
        buildLock.release();
      }
    }
  }
}

function assertBuildOutputPath(output) {
  const relative = path.relative(artifactsRoot, output);
  if (
    relative === "" ||
    relative.startsWith(`..${path.sep}`) ||
    path.isAbsolute(relative)
  ) {
    throw new Error("Candidate build output must stay inside the Node artifacts root.");
  }
}

export function materializeCommittedSourceSnapshot({
  repository = repositoryRoot,
  commit = gitCapture(repository, ["rev-parse", "--verify", "HEAD^{commit}"]),
  paths = SNAPSHOT_PATHS,
} = {}) {
  if (!/^[0-9a-f]{40}$/.test(commit)) {
    throw new Error("Candidate source snapshot requires a canonical Git commit.");
  }
  const commitTree = gitCapture(repository, [
    "rev-parse",
    "--verify",
    `${commit}^{tree}`,
  ]);
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-node-source-"));
  const sourceRoot = path.join(root, "source");
  const indexPath = path.join(root, "git-index");
  mkdirSync(sourceRoot);
  const environment = gitEnvironment({ GIT_INDEX_FILE: indexPath });
  try {
    runGit(repository, ["read-tree", commit], { environment });
    const listed = runGit(repository, [
      "ls-tree",
      "-r",
      "-z",
      "--name-only",
      commit,
      "--",
      ...paths,
    ], { encoding: null });
    runGit(repository, [
      "checkout-index",
      "--stdin",
      "-z",
      `--prefix=${sourceRoot}${path.sep}`,
    ], { environment, input: listed });
    const expectedPaths = listed.toString("utf8").split("\0").filter(Boolean);
    const actualPaths = walkFiles(sourceRoot).map((file) =>
      path.relative(sourceRoot, file).split(path.sep).join("/")
    ).sort(comparePaths);
    if (stableJson(actualPaths) !== stableJson(expectedPaths)) {
      throw new Error("Candidate source snapshot differs from its committed Git tree.");
    }
    let disposed = false;
    return {
      commit,
      commitTree,
      sourceRoot,
      dispose() {
        if (disposed) return;
        disposed = true;
        rmSync(root, { recursive: true, force: true });
      },
    };
  } catch (error) {
    rmSync(root, { recursive: true, force: true });
    throw error;
  }
}

function assertCurrentBuildControllerMatchesSnapshot(sourceRoot) {
  for (const relativePath of [
    "capabilities/feature-surface-v1.json",
    "platforms/node/candidate-builds.json",
    "platforms/node/package-surfaces.json",
    "platforms/node/package.json",
    "platforms/node/scripts/benchmark/svg-signature.mjs",
    "platforms/node/scripts/build-candidate.mjs",
    "platforms/node/scripts/replace-directory.mjs",
    "platforms/node/scripts/stable-json.mjs",
    "platforms/node/src/bounded-executor.mjs",
    "platforms/node/src/engine.mjs",
    "platforms/node/src/errors.mjs",
    "platforms/node/src/native-loader.mjs",
  ]) {
    const current = path.join(repositoryRoot, relativePath);
    const committed = path.join(sourceRoot, relativePath);
    if (digestFile(current) !== digestFile(committed)) {
      throw new Error(
        `Candidate build controller differs from HEAD: ${relativePath}. Commit it before building.`,
      );
    }
  }
}

function buildNapi(stage, recipe, environment, sourceRoot) {
  const cli = path.join(nodeRoot, "node_modules", "@napi-rs", "cli", "dist", "cli.js");
  assertFile(cli, "pinned @napi-rs/cli; run npm ci first");
  const invocation = candidateBuildInvocation(recipe, stage, { sourceRoot });
  run(invocation.command, invocation.args, environment, sourceRoot);
}

function buildNodeWasm(stage, recipe, environment, sourceRoot) {
  const invocation = candidateBuildInvocation(recipe, stage, { sourceRoot });
  run(invocation.command, invocation.args, environment, sourceRoot);
}

export function candidateBuildInvocation(
  recipe,
  stage,
  { sourceRoot = repositoryRoot } = {},
) {
  const sourceNodeRoot = path.join(sourceRoot, "platforms", "node");
  const featureArgument = recipe.cargoFeatures.join(",");
  if (recipe.candidate === "napi") {
    return {
      command: process.execPath,
      args: [
        path.join(nodeRoot, "node_modules", "@napi-rs", "cli", "dist", "cli.js"),
        "build",
        "--cwd",
        sourceRoot,
        "--manifest-path",
        descriptor.cargo.manifest,
        "--package-json-path",
        path.join(sourceNodeRoot, "package.json"),
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
        "-j1",
      ],
    };
  }
  if (recipe.candidate === "node-wasm") {
    return {
      command: "wasm-pack",
      args: [
        "build",
        path.dirname(path.join(sourceRoot, descriptor.cargo.manifest)),
        "--target",
        recipe.wasmPackTarget,
        "--release",
        "--no-pack",
        "--out-dir",
        stage,
        "--",
        "--locked",
        "-j1",
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

function writeBuildReceipt(
  stage,
  recipe,
  sourceProvenance,
  dependencyClosure,
  runtime,
  buildEnvironment,
  sourceRoot,
) {
  const { commit, commit_tree: commitTree, source_inputs: inputEntries } = sourceProvenance;
  const sourceDigest = digestJson(inputEntries);
  const bindingContractEntries = collectBindingContractEntries(inputEntries);
  const bindingContractDigest = digestJson(bindingContractEntries);
  const cargoLockDigest = inputEntries.find(
    (entry) => entry.path === "crates/merman-node/Cargo.lock",
  )?.sha256;
  if (!cargoLockDigest) {
    throw new Error("Node candidate source closure omits crates/merman-node/Cargo.lock.");
  }
  const environment = buildEnvironment.environment;
  const environmentContract = buildEnvironment.contract;
  const tools = {
    cargo: runCapture(candidateCargoCommand(), ["--version"], {
      cwd: sourceRoot,
      environment,
    }),
    node: process.version,
    rustc: runCapture(candidateRustcCommand(), ["--version", "--verbose"], {
      cwd: sourceRoot,
      environment,
    }),
    transport_builder:
      recipe.candidate === "napi"
        ? runCapture(process.execPath, [
            path.join(nodeRoot, "node_modules", "@napi-rs", "cli", "dist", "cli.js"),
            "--version",
          ], { cwd: sourceRoot, environment })
        : runCapture("wasm-pack", ["--version"], { cwd: sourceRoot, environment }),
  };
  const config = candidateBuildConfig(recipe);
  const inputEvidence = {
    binding_contract_digest: bindingContractDigest,
    build_environment: environmentContract,
    build_environment_digest: digestJson(environmentContract),
    cargo_lock_digest: cargoLockDigest,
    commit,
    commit_tree: commitTree,
    config,
    dependency_closure: dependencyClosure,
    source_digest: sourceDigest,
    source_inputs: inputEntries,
    tools,
  };
  const receipt = {
    schema_version: 4,
    config,
    commit,
    commit_tree: commitTree,
    source_inputs: inputEntries,
    source_digest: sourceDigest,
    cargo_lock_digest: cargoLockDigest,
    binding_contract_digest: bindingContractDigest,
    build_environment: environmentContract,
    build_environment_digest: digestJson(environmentContract),
    dependency_closure: dependencyClosure,
    input_digest: computeBuildReceiptInputDigest(inputEvidence),
    runtime,
    tools,
    artifacts: artifactEntries(stage),
  };
  writeFileSync(path.join(stage, "build-receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`);
}

export function probeCandidateRuntime(stage, recipe) {
  const artifact = path.join(
    stage,
    recipe.candidate === "napi" ? "merman.node" : "merman_node.js",
  );
  const binding = requireFromBuild(artifact);
  const Engine = recipe.candidate === "napi"
    ? binding?.NativeEngine ?? binding?.default?.NativeEngine
    : binding?.WasmEngine ?? binding?.default?.WasmEngine;
  if (typeof Engine !== "function") {
    throw new Error(`${recipe.candidate} candidate does not export its engine constructor.`);
  }
  const engine = new Engine(JSON.stringify({
    version: 1,
    runtime_policy: "deterministic",
    resources: { profile: "interactive" },
  }));
  try {
    const catalog = validateRuntimeCatalog(engine.runtimeCatalogJson());
    const capabilities = catalog?.capabilities;
    const capabilityIds = sortedUniqueStrings(
      capabilities?.capability_ids,
      "runtime capability IDs",
    );
    const outputIds = sortedUniqueStrings(
      capabilities?.output_ids,
      "runtime output IDs",
    );
    const operationIds = sortedUniqueStrings(
      capabilities?.operation_ids,
      "runtime operation IDs",
    );
    const systemAdapterIds = sortedUniqueStrings(
      capabilities?.system_adapter_ids,
      "runtime system adapter IDs",
      { allowEmpty: true },
    );
    const expectedRuntime = resolveCandidateRuntimeContract();
    if (
      catalog.schema_version !== 1 ||
      catalog.transport_api_version !== 1 ||
      catalog.package_version !== packageSurface.version ||
      stableJson(capabilityIds) !== stableJson(expectedRuntime.capabilityIds) ||
      stableJson(outputIds) !== stableJson(expectedRuntime.outputIds) ||
      stableJson(operationIds) !== stableJson(expectedRuntime.operationIds) ||
      stableJson(systemAdapterIds) !== stableJson(expectedRuntime.systemAdapterIds) ||
      stableJson(capabilities?.text_measurement?.provider_ids) !==
        stableJson(expectedRuntime.textMeasurementProviderIds) ||
      !Number.isSafeInteger(catalog.registry?.diagram_family_count) ||
      catalog.registry.diagram_family_count < 1
    ) {
      throw new Error(`${recipe.candidate} runtime catalog disagrees with its capability recipe.`);
    }

    const source = "flowchart TD\nA --> B";
    const semantic = parseWireResponse(engine.executeSync(JSON.stringify({
      operation_id: "semantic-json",
      source,
      uri: null,
      options_json: JSON.stringify({ version: 1 }),
    })));
    if (
      semantic.ok !== true ||
      semantic.result?.operation_id !== "semantic-json" ||
      semantic.result?.media_type !== "application/json" ||
      typeof semantic.result?.data !== "string"
    ) {
      throw new Error(`${recipe.candidate} runtime semantic JSON probe failed.`);
    }
    JSON.parse(semantic.result.data);
    const svgPlan = parseWireResponse(engine.executeSync(JSON.stringify({
      operation_id: "svg-plan-json",
      source,
      uri: null,
      options_json: JSON.stringify({ version: 1 }),
    })));
    if (
      svgPlan.ok !== true ||
      svgPlan.result?.operation_id !== "svg-plan-json" ||
      svgPlan.result?.media_type !== "application/json" ||
      typeof svgPlan.result?.data !== "string"
    ) {
      throw new Error(`${recipe.candidate} runtime SVG capability-plan probe failed.`);
    }
    const parsedSvgPlan = JSON.parse(svgPlan.result.data);
    if (
      parsedSvgPlan?.schema_version !== 1 ||
      parsedSvgPlan?.planned_operation_id !== "svg" ||
      !Array.isArray(parsedSvgPlan?.required_capability_ids) ||
      !Array.isArray(parsedSvgPlan?.missing_capability_ids) ||
      typeof parsedSvgPlan?.ready !== "boolean"
    ) {
      throw new Error(`${recipe.candidate} runtime SVG capability-plan payload is invalid.`);
    }
    const success = parseWireResponse(engine.executeSync(JSON.stringify({
      operation_id: "svg",
      source,
      uri: null,
      options_json: JSON.stringify({ resources: { limits: { max_source_bytes: 4096 } } }),
    })));
    if (
      success.ok !== true ||
      success.result?.operation_id !== "svg" ||
      success.result?.media_type !== "image/svg+xml" ||
      typeof success.result?.data !== "string"
    ) {
      throw new Error(`${recipe.candidate} runtime SVG probe failed.`);
    }
    const svgEvidence = svgTransportEvidence(success.result.data);
    const limited = parseWireResponse(engine.executeSync(JSON.stringify({
      operation_id: "svg",
      source,
      uri: null,
      options_json: JSON.stringify({ resources: { limits: { max_source_bytes: 4 } } }),
    })));
    if (
      limited.ok !== false ||
      limited.error?.code_name !== "MERMAN_RESOURCE_LIMIT_EXCEEDED"
    ) {
      throw new Error(`${recipe.candidate} runtime ignored request-local options JSON.`);
    }
    const unknown = parseWireResponse(engine.executeSync(JSON.stringify({
      operation_id: "bitmap",
      source,
      uri: null,
    })));
    if (unknown.ok !== false || unknown.error?.kind !== "unknown-operation") {
      throw new Error(`${recipe.candidate} runtime lost its typed unknown-operation error.`);
    }
    const missing = parseWireResponse(engine.executeSync(JSON.stringify({
      operation_id: "png",
      source,
      uri: null,
    })));
    if (
      missing.ok !== false ||
      missing.error?.kind !== "missing-capability" ||
      missing.error?.capability_id !== "png"
    ) {
      throw new Error(`${recipe.candidate} runtime lost its typed missing-capability error.`);
    }
    return {
      catalog_digest: digestJson(catalog),
      catalog,
      probe: {
        missing_capability_id: missing.error.capability_id,
        semantic_json_bytes: Buffer.byteLength(semantic.result.data),
        svg_plan_json_bytes: Buffer.byteLength(svgPlan.result.data),
        svg_bytes: Buffer.byteLength(success.result.data),
        svg_structure_sha256: svgEvidence.structure_sha256,
        svg_geometry_sha256: svgEvidence.geometry_sha256,
        unknown_operation_kind: unknown.error.kind,
        request_options_limit_code_name: limited.error.code_name,
      },
    };
  } finally {
    engine.dispose?.();
  }
}

function parseWireResponse(value) {
  try {
    const parsed = JSON.parse(value);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error();
    return parsed;
  } catch {
    throw new Error("Node candidate returned an invalid wire response.");
  }
}

function sortedUniqueStrings(value, label, { allowEmpty = false } = {}) {
  if (
    !Array.isArray(value) ||
    (!allowEmpty && value.length === 0) ||
    value.some((item) => typeof item !== "string" || item.length === 0) ||
    stableJson(value) !== stableJson([...new Set(value)].sort())
  ) {
    throw new Error(`${label} must be sorted, unique strings.`);
  }
  return value;
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

function cargoMetadata(
  recipe,
  sourceRoot = repositoryRoot,
  environment = candidateProcessEnvironment(),
) {
  cargoConfigurationInputs(sourceRoot, environment);
  const args = [
    "metadata",
    "--locked",
    "--format-version",
    "1",
    "--manifest-path",
    path.join(sourceRoot, descriptor.cargo.manifest),
    "--filter-platform",
    recipe.rustTarget,
  ];
  if (!descriptor.cargo.default_features) args.push("--no-default-features");
  if (recipe.cargoFeatures.length > 0) {
    args.push("--features", recipe.cargoFeatures.join(","));
  }
  return JSON.parse(runCapture(candidateCargoCommand(), args, { cwd: sourceRoot, environment }));
}

function collectLocalInputEntries(metadata, sourceRoot = repositoryRoot) {
  const roots = new Set();
  for (const item of metadata.packages) {
    if (item.source !== null) continue;
    const manifest = path.resolve(item.manifest_path);
    if (!isWithin(sourceRoot, manifest)) continue;
    roots.add(path.dirname(manifest));
  }
  const files = new Set([
    path.join(sourceRoot, "Cargo.toml"),
    path.join(sourceRoot, "rust-toolchain.toml"),
    path.join(sourceRoot, "crates", "merman-node", "Cargo.lock"),
    path.join(sourceRoot, descriptor.capability_recipe.descriptor),
    path.join(sourceRoot, "platforms", "node", "candidate-builds.json"),
    path.join(sourceRoot, "platforms", "node", "package-surfaces.json"),
    path.join(sourceRoot, "platforms", "node", "package.json"),
    path.join(sourceRoot, "platforms", "node", "package-lock.json"),
    path.join(sourceRoot, "platforms", "node", "scripts", "benchmark", "svg-signature.mjs"),
    path.join(sourceRoot, "platforms", "node", "scripts", "build-candidate.mjs"),
    path.join(sourceRoot, "platforms", "node", "scripts", "replace-directory.mjs"),
    path.join(sourceRoot, "platforms", "node", "scripts", "stable-json.mjs"),
    path.join(sourceRoot, "platforms", "node", "src", "bounded-executor.mjs"),
    path.join(sourceRoot, "platforms", "node", "src", "engine.mjs"),
    path.join(sourceRoot, "platforms", "node", "src", "errors.mjs"),
    path.join(sourceRoot, "platforms", "node", "src", "native-loader.mjs"),
  ]);
  for (const file of walkFiles(path.join(sourceRoot, "crates"), {
    skipBuildOutputs: true,
  })) {
    files.add(file);
  }
  for (const file of walkFiles(path.join(sourceRoot, "capabilities"), {
    skipBuildOutputs: true,
  })) {
    files.add(file);
  }
  for (const root of roots) {
    for (const file of walkFiles(root, { skipBuildOutputs: true })) files.add(file);
  }
  return [...files]
    .filter((file) => existsSync(file) && statSync(file).isFile())
    .map((file) => ({
      path: path.relative(sourceRoot, file).split(path.sep).join("/"),
      bytes: statSync(file).size,
      sha256: digestFile(file),
    }))
    .sort((left, right) => comparePaths(left.path, right.path));
}

function resolveSourceProvenance(
  sourceInputs,
  dependencyClosure,
  {
    commit = gitCapture(repositoryRoot, ["rev-parse", "--verify", "HEAD^{commit}"]),
    commitTree = gitCapture(repositoryRoot, ["rev-parse", "--verify", `${commit}^{tree}`]),
  } = {},
) {
  const provenance = { commit, commit_tree: commitTree, source_inputs: sourceInputs };
  validateGitSourceInputs(
    { ...provenance, dependency_closure: dependencyClosure },
    { label: "Node candidate source provenance" },
  );
  return provenance;
}

export function validateGitSourceInputs(
  value,
  { label = "Node candidate Git source provenance" } = {},
) {
  if (
    !/^[0-9a-f]{40}$/.test(value.commit ?? "") ||
    !/^[0-9a-f]{40}$/.test(value.commit_tree ?? "")
  ) {
    throw new Error(`${label} must use canonical SHA-1 commit and tree object IDs.`);
  }
  const sourceInputs = validateSourceInputEntries(value.source_inputs, label);
  const validationKey = `${value.commit}:${value.commit_tree}`;
  if (gitSourceValidationCache.has(validationKey)) return sourceInputs;
  const resolvedCommit = gitCapture(repositoryRoot, [
    "rev-parse",
    "--verify",
    `${value.commit}^{commit}`,
  ]);
  if (resolvedCommit !== value.commit) {
    throw new Error(`${label} commit does not resolve to its declared Git object.`);
  }
  const resolvedTree = gitCapture(repositoryRoot, [
    "rev-parse",
    "--verify",
    `${value.commit}^{tree}`,
  ]);
  if (resolvedTree !== value.commit_tree) {
    throw new Error(`${label} commit tree does not match Git.`);
  }
  gitSourceValidationCache.add(validationKey);
  return sourceInputs;
}

function validateSourceInputEntries(entries, label) {
  if (!Array.isArray(entries) || entries.length === 0) {
    throw new Error(`${label} source input list must be non-empty.`);
  }
  const paths = new Set();
  for (const entry of entries) {
    if (
      !entry ||
      typeof entry.path !== "string" ||
      entry.path.length === 0 ||
      entry.path !== path.posix.normalize(entry.path) ||
      entry.path.startsWith("../") ||
      path.posix.isAbsolute(entry.path) ||
      entry.path.includes("\\") ||
      /[\0\r\n]/.test(entry.path) ||
      paths.has(entry.path) ||
      !Number.isSafeInteger(entry.bytes) ||
      entry.bytes < 0 ||
      !/^sha256:[0-9a-f]{64}$/.test(entry.sha256 ?? "")
    ) {
      throw new Error(`${label} source input list contains an invalid or duplicate entry.`);
    }
    paths.add(entry.path);
  }
  const sortedPaths = [...paths].sort(comparePaths);
  if (stableJson(entries.map((entry) => entry.path)) !== stableJson(sortedPaths)) {
    throw new Error(`${label} source input list must be sorted by repository path.`);
  }
  return entries;
}

export function computeBuildReceiptInputDigest(
  value,
  dependencyClosureDigest = value?.dependency_closure?.digest,
) {
  return digestJson({
    binding_contract_digest: value.binding_contract_digest,
    build_environment: value.build_environment,
    build_environment_digest: value.build_environment_digest,
    cargo_lock_digest: value.cargo_lock_digest,
    commit: value.commit,
    commit_tree: value.commit_tree,
    config: value.config,
    dependency_closure_digest: dependencyClosureDigest,
    source_digest: value.source_digest,
    source_inputs: value.source_inputs,
    tools: value.tools,
  });
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
  const capabilityFeatures = candidateCapabilityFeatures();
  if (candidate === "node-wasm") {
    const wasm = descriptor.candidates["node-wasm"];
    return completeCandidateRecipe(
      {
        candidate,
        targetId: null,
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
      targetId: target,
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
      `Transport feature collides with the candidate capability recipe: ${recipe.transportFeature}.`,
    );
  }
  return {
    ...recipe,
    capabilityFeatures,
    cargoFeatures: [...capabilityFeatures, recipe.transportFeature].sort(),
  };
}

function candidateCapabilityFeatures() {
  const recipe = descriptor.capability_recipe;
  if (
    capabilitySurface.schema_version !== 1 ||
    !Array.isArray(capabilitySurface.targets) ||
    !capabilitySurface.targets.some((target) => target?.id === recipe.target) ||
    !Array.isArray(capabilitySurface.capabilities)
  ) {
    throw new Error("Capability surface descriptor is invalid.");
  }
  const capabilities = recipe.capabilities;
  if (
    !Array.isArray(capabilities) ||
    capabilities.length === 0 ||
    capabilities.some((capability) => typeof capability !== "string" || capability.length === 0) ||
    stableJson(capabilities) !== stableJson([...new Set(capabilities)].sort())
  ) {
    throw new Error("Candidate capability recipe capabilities must be sorted, unique, non-empty strings.");
  }
  const known = new Map(
    capabilitySurface.capabilities.map((capability) => [capability?.id, capability]),
  );
  for (const capabilityId of capabilities) {
    const capability = known.get(capabilityId);
    if (!capability || !Array.isArray(capability.targets) || !capability.targets.includes(recipe.target)) {
      throw new Error(
        `Candidate capability recipe includes ${capabilityId}, which is unavailable on ${recipe.target}.`,
      );
    }
    if (
      !Array.isArray(capability.implications) ||
      capability.implications.some((implication) => !capabilities.includes(implication))
    ) {
      throw new Error(
        `Candidate capability recipe omits an implication of ${capabilityId}.`,
      );
    }
  }
  return capabilities;
}

export function resolveCandidateRuntimeContract() {
  const capabilityIds = candidateCapabilityFeatures();
  const target = descriptor.capability_recipe.target;
  if (
    !Array.isArray(capabilitySurface.outputs) ||
    !Array.isArray(capabilitySurface.binding_operations)
  ) {
    throw new Error("Capability surface descriptor lacks outputs or binding operations.");
  }
  const capabilityById = new Map(
    capabilitySurface.capabilities.map((capability) => [capability?.id, capability]),
  );
  const outputIds = capabilitySurface.outputs
    .filter(
      (output) =>
        output?.targets?.includes(target) &&
        capabilityIds.includes(output.capability),
    )
    .map((output) => output.id)
    .sort();
  const operationIds = capabilitySurface.binding_operations
    .filter(
      (operation) =>
        operation?.targets?.includes(target) &&
        (operation.capability === null || capabilityIds.includes(operation.capability)),
    )
    .map((operation) => operation.id)
    .sort();
  const systemAdapterIds = capabilityIds
    .filter((capabilityId) => capabilityById.get(capabilityId)?.kind === "adapter")
    .sort();
  for (const [label, ids] of [
    ["output", outputIds],
    ["operation", operationIds],
  ]) {
    if (
      ids.length === 0 ||
      ids.some((id) => typeof id !== "string" || id.length === 0) ||
      stableJson(ids) !== stableJson([...new Set(ids)].sort())
    ) {
      throw new Error(`Candidate runtime ${label} IDs are invalid.`);
    }
  }
  return {
    capabilityRecipe: {
      descriptor: descriptor.capability_recipe.descriptor,
      target,
      capabilities: capabilityIds,
    },
    capabilityIds,
    outputIds,
    operationIds,
    systemAdapterIds,
    textMeasurementProviderIds: ["vendored"],
  };
}

export function candidateDependencyClosure(
  metadata,
  recipe,
  sourceRoot = repositoryRoot,
) {
  if (
    !metadata ||
    !Array.isArray(metadata.packages) ||
    !Array.isArray(metadata.resolve?.nodes)
  ) {
    throw new Error("Cargo metadata lacks a resolved dependency graph.");
  }
  const packageById = new Map(metadata.packages.map((item) => [item.id, item]));
  const packages = metadata.resolve.nodes
    .map((node) => {
      const item = packageById.get(node.id);
      if (!item) throw new Error(`Cargo metadata cannot resolve package ${node.id}.`);
      const source = item.source ?? localPackageSource(item.manifest_path, sourceRoot);
      return {
        name: item.name,
        version: item.version,
        source,
      };
    })
    .sort((left, right) => comparePackageIdentities(packageIdentity(left), packageIdentity(right)));
  validateCandidateDependencyPackages(packages, recipe.candidate);
  return {
    digest: digestJson(packages),
    packages,
  };
}

export function candidateBuildConfig(recipe) {
  return {
    candidate: recipe.candidate,
    target: recipe.targetId,
    rust_target: recipe.rustTarget,
    wasm_pack_target: recipe.wasmPackTarget,
    default_features: false,
    capability_recipe: {
      descriptor: descriptor.capability_recipe.descriptor,
      target: descriptor.capability_recipe.target,
      capabilities: recipe.capabilityFeatures,
    },
    features: recipe.cargoFeatures,
  };
}

export function resolveCandidateBuildEvidence(recipe) {
  const metadata = cargoMetadata(recipe);
  validateCandidatePackageVersions(metadata);
  const dependencyClosure = candidateDependencyClosure(metadata, recipe);
  const inputEntries = collectLocalInputEntries(metadata);
  const sourceProvenance = resolveSourceProvenance(inputEntries, dependencyClosure);
  return {
    ...sourceProvenance,
    source_digest: digestJson(inputEntries),
    binding_contract_digest: digestJson(collectBindingContractEntries(inputEntries)),
    dependency_closure_digest: dependencyClosure.digest,
  };
}

export function validateCandidateCargoMetadata(recipe) {
  const metadata = cargoMetadata(recipe);
  validateCandidatePackageVersions(metadata);
  candidateDependencyClosure(metadata, recipe);
  return packageSurface.version;
}

export function validateCandidatePackageVersions(metadata) {
  if (!metadata || !Array.isArray(metadata.packages)) {
    throw new Error("Cargo metadata lacks candidate package versions.");
  }
  for (const name of ["merman-node-candidate", "merman-bindings-core"]) {
    const matches = metadata.packages.filter((item) => item.name === name);
    if (
      matches.length !== 1 ||
      matches[0].source !== null ||
      matches[0].version !== packageSurface.version
    ) {
      throw new Error(
        `${name} must be the local ${packageSurface.version} package for this Node candidate.`,
      );
    }
  }
  return packageSurface.version;
}

export function validateCandidateDependencyPackages(packages, candidate) {
  if (
    !Array.isArray(packages) ||
    packages.length === 0 ||
    packages.some(
      (item) =>
        !item ||
        typeof item.name !== "string" ||
        item.name.length === 0 ||
        typeof item.version !== "string" ||
        item.version.length === 0 ||
        typeof item.source !== "string" ||
        item.source.length === 0,
    )
  ) {
    throw new Error("Node candidate dependency closure is invalid.");
  }
  const identities = packages.map(packageIdentity);
  const normalizedIdentities = [...identities].sort(comparePackageIdentities);
  if (
    new Set(identities).size !== identities.length ||
    stableJson(identities) !== stableJson(normalizedIdentities)
  ) {
    throw new Error("Node candidate dependency closure packages must be sorted and unique.");
  }
  const names = new Set(packages.map((item) => item.name));
  for (const name of ["merman-node-candidate", "merman-bindings-core"]) {
    const matches = packages.filter((item) => item.name === name);
    if (matches.length !== 1) {
      throw new Error(`${name} is missing or ambiguous in the Node candidate dependency closure.`);
    }
    if (
      matches[0].version !== packageSurface.version ||
      !matches[0].source.startsWith("path:")
    ) {
      throw new Error(
        `${name} must be the local ${packageSurface.version} package in the Node candidate dependency closure.`,
      );
    }
  }
  if (candidate === "node-wasm") {
    for (const forbidden of ["napi", "napi-build", "napi-derive"]) {
      if (names.has(forbidden)) {
        throw new Error(`${forbidden} leaked into the Node WASM dependency closure.`);
      }
    }
    for (const required of ["serde-wasm-bindgen", "wasm-bindgen"]) {
      if (!names.has(required)) {
        throw new Error(`${required} is missing from the Node WASM dependency closure.`);
      }
    }
    return;
  }
  if (candidate === "napi") {
    for (const required of ["napi", "napi-build", "napi-derive"]) {
      if (!names.has(required)) {
        throw new Error(`${required} is missing from the napi dependency closure.`);
      }
    }
    return;
  }
  throw new Error(`Unknown candidate dependency closure: ${candidate}.`);
}

function packageIdentity(item) {
  return `${item.name}\0${item.version}\0${item.source}`;
}

function comparePackageIdentities(left, right) {
  if (left < right) return -1;
  if (left > right) return 1;
  return 0;
}

function comparePaths(left, right) {
  if (left < right) return -1;
  if (left > right) return 1;
  return 0;
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
    descriptor.schema_version !== 3 ||
    descriptor.status !== "private-evaluation" ||
    descriptor.cargo.default_features !== false ||
    descriptor.cargo.manifest !== "crates/merman-node/Cargo.toml" ||
    descriptor.capability_recipe?.descriptor !== "capabilities/feature-surface-v1.json" ||
    descriptor.capability_recipe?.target !== "native" ||
    packageSurface.schema_version !== 1 ||
    packageSurface.admission_status !== "candidate" ||
    typeof packageSurface.version !== "string" ||
    packageSurface.version.length === 0
  ) {
    throw new Error("Node candidate build descriptor is invalid.");
  }
}

export function walkFiles(root, { skipBuildOutputs = false } = {}) {
  if (!existsSync(root)) return [];
  const rootStat = lstatSync(root);
  if (!rootStat.isDirectory() || rootStat.isSymbolicLink()) {
    throw new Error(`Candidate build input root must be a regular directory: ${root}.`);
  }
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
    else {
      throw new Error(`Candidate build input tree contains a non-regular entry: ${absolute}.`);
    }
  }
  return files;
}

function digestFile(file) {
  return digestBytes(readFileSync(file));
}

function digestBytes(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}

function localPackageSource(manifestPath, sourceRoot = repositoryRoot) {
  const directory = path.dirname(path.resolve(manifestPath));
  if (!isWithin(sourceRoot, directory)) {
    throw new Error(`Local Cargo package escapes the repository: ${manifestPath}.`);
  }
  return `path:${path.relative(sourceRoot, directory).split(path.sep).join("/")}`;
}

function run(command, args, environment, cwd = nodeRoot) {
  const result = spawnSync(command, args, {
    cwd,
    env: environment,
    stdio: "inherit",
  });
  if (result.error) throw new Error(`Failed to run ${command}: ${result.error.message}`);
  if (result.status !== 0) throw new Error(`${command} exited with status ${result.status ?? 1}.`);
}

export function resolveCandidateBuildEnvironment({
  sourceRoot = repositoryRoot,
} = {}) {
  const enforced = {
    CARGO_BUILD_JOBS: "1",
    CARGO_TARGET_DIR: path.join(repositoryRoot, "target"),
  };
  const inheritedNames = candidateBuildInheritedNames(enforced);
  const environment = candidateProcessEnvironment(enforced, inheritedNames);
  const inherited = [...inheritedNames].sort().map((name) => {
    const value = environment[name];
    return value === undefined
      ? { name, state: "absent" }
      : { name, state: "present", value_sha256: digestBytes(value) };
  });
  const externalInputs = [
    ...cargoConfigurationInputs(sourceRoot, environment),
    executableInput("tool/cargo", candidateCargoCommand(), sourceRoot, environment),
    executableInput("tool/git", "git", sourceRoot, environment),
    executableInput("tool/node", process.execPath, sourceRoot, environment),
    executableInput("tool/rustc", candidateRustcCommand(), sourceRoot, environment),
    executableInput("tool/wasm-pack", "wasm-pack", sourceRoot, environment),
    pathBoundFileInput(
      "tool/napi-cli",
      path.join(nodeRoot, "node_modules", "@napi-rs", "cli", "dist", "cli.js"),
    ),
    treeInput("tool/napi-cli-node-modules", path.join(nodeRoot, "node_modules")),
    ...environmentToolInputs(sourceRoot, environment),
  ];
  const contract = {
    schema_version: 2,
    enforced: {
      CARGO_BUILD_JOBS: enforced.CARGO_BUILD_JOBS,
      CARGO_TARGET_DIR: "target",
    },
    inherited,
    external_inputs: externalInputs,
  };
  return {
    environment,
    contract,
  };
}

function candidateBuildInheritedNames(enforced = {}) {
  const inheritedNames = new Set(CANDIDATE_BUILD_ENVIRONMENT_REQUIRED_NAMES);
  for (const name of Object.keys(process.env)) {
    if (!Object.hasOwn(enforced, name) && isBuildInfluencingEnvironmentName(name)) {
      inheritedNames.add(name);
    }
  }
  return inheritedNames;
}

function candidateProcessEnvironment(
  enforced = {
    CARGO_BUILD_JOBS: "1",
    CARGO_TARGET_DIR: path.join(repositoryRoot, "target"),
  },
  inheritedNames = candidateBuildInheritedNames(enforced),
) {
  rejectInjectedBuildEnvironment();
  const environment = {};
  for (const name of inheritedNames) {
    if (process.env[name] !== undefined) environment[name] = process.env[name];
  }
  return { ...environment, ...enforced };
}

function rejectInjectedBuildEnvironment() {
  for (const name of [
    "NODE_OPTIONS",
    "NODE_PATH",
    "LD_PRELOAD",
    "DYLD_INSERT_LIBRARIES",
  ]) {
    if (process.env[name] !== undefined && process.env[name] !== "") {
      throw new Error(`Candidate builds reject ${name} process injection.`);
    }
  }
}

function isBuildInfluencingEnvironmentName(name) {
  return CANDIDATE_BUILD_ENVIRONMENT_REQUIRED_NAMES.includes(name) ||
    /^CARGO_/.test(name) ||
    /^RUSTC_/.test(name) ||
    /^RUSTUP_/.test(name) ||
    /^(?:AR|CC|CXX|LD|NM|OBJCOPY|RANLIB|STRIP|CFLAGS|CXXFLAGS|CPPFLAGS|LDFLAGS)(?:_.+)?$/.test(name) ||
    /^(?:BINDGEN|CCACHE|LIBCLANG|NAPI_RS|OPENSSL|PKG_CONFIG|SCCACHE|WASM_BINDGEN|WASM_PACK)_/.test(name);
}

function cargoConfigurationInputs(sourceRoot, environment) {
  const cargoHome = environment.CARGO_HOME ||
    (environment.HOME ? path.join(environment.HOME, ".cargo") : null);
  rejectUnboundCargoConfiguration(sourceRoot, cargoHome);
  const candidates = [
    ["build-source/.cargo/config", path.join(sourceRoot, ".cargo", "config")],
    ["build-source/.cargo/config.toml", path.join(sourceRoot, ".cargo", "config.toml")],
    ["cargo-home/config", cargoHome ? path.join(cargoHome, "config") : null],
    ["cargo-home/config.toml", cargoHome ? path.join(cargoHome, "config.toml") : null],
  ];
  return candidates.map(([id, file]) => fileInput(id, file));
}

function rejectUnboundCargoConfiguration(sourceRoot, cargoHome) {
  const allowedCargoHome = cargoHome === null ? null : path.resolve(cargoHome);
  let ancestor = path.dirname(path.resolve(sourceRoot));
  while (true) {
    const cargoDirectory = path.join(ancestor, ".cargo");
    if (path.resolve(cargoDirectory) !== allowedCargoHome) {
      for (const name of ["config", "config.toml"]) {
        if (existsSync(path.join(cargoDirectory, name))) {
          throw new Error(
            `Candidate build cwd inherits unbound Cargo configuration: ${path.join(cargoDirectory, name)}.`,
          );
        }
      }
    }
    const parent = path.dirname(ancestor);
    if (parent === ancestor) break;
    ancestor = parent;
  }
}

function fileInput(id, file) {
  if (file === null || !existsSync(file)) return { id, state: "absent" };
  const stat = lstatSync(file);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    throw new Error(`Candidate build input must be a regular non-symlink file: ${id}.`);
  }
  return {
    id,
    state: "present",
    bytes: stat.size,
    sha256: digestFile(file),
  };
}

function pathBoundFileInput(id, file) {
  const input = fileInput(id, file);
  if (input.state === "absent") return input;
  return { ...input, path_sha256: digestBytes(realpathSync(file)) };
}

function executableInput(id, command, cwd, environment) {
  const resolved = resolveExecutable(command, cwd, environment);
  if (resolved === null) return { id, state: "absent" };
  return pathBoundFileInput(id, resolved);
}

function treeInput(id, root) {
  if (!existsSync(root)) return { id, state: "absent" };
  const rootStat = lstatSync(root);
  if (!rootStat.isDirectory() || rootStat.isSymbolicLink()) {
    throw new Error(`Candidate build input must be a non-symlink directory: ${id}.`);
  }
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    if (entry.name === ".bin") continue;
    const absolute = path.join(root, entry.name);
    if (entry.isDirectory()) files.push(...walkFiles(absolute));
    else if (entry.isFile()) files.push(absolute);
    else {
      throw new Error(`Candidate build input tree contains a non-regular entry: ${id}.`);
    }
  }
  const entries = files.map((file) => ({
    path: path.relative(root, file).split(path.sep).join("/"),
    bytes: statSync(file).size,
    sha256: digestFile(file),
  })).sort((left, right) => comparePaths(left.path, right.path));
  return {
    id,
    state: "present",
    file_count: entries.length,
    bytes: entries.reduce((total, entry) => total + entry.bytes, 0),
    sha256: digestJson(entries),
    path_sha256: digestBytes(realpathSync(root)),
  };
}

function environmentToolInputs(sourceRoot, environment) {
  return Object.keys(environment)
    .filter((name) => isToolSelectingEnvironmentName(name))
    .sort()
    .map((name) => {
      const command = environment[name];
      if (command === undefined || command === "") {
        return { id: `environment-tool/${name}`, state: "absent" };
      }
      const resolved = resolveExecutable(command, sourceRoot, environment);
      if (resolved === null) {
        throw new Error(`Candidate build environment tool ${name} cannot be resolved.`);
      }
      return executableInput(`environment-tool/${name}`, command, sourceRoot, environment);
    });
}

function isToolSelectingEnvironmentName(name) {
  return new Set([
    "CARGO_BUILD_RUSTC",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
  ]).has(name) ||
    /^(?:AR|CC|CXX|LD|NM|OBJCOPY|RANLIB|STRIP)(?:_.+)?$/.test(name) ||
    /^CARGO_TARGET_.+_(?:LINKER|RUNNER)$/.test(name);
}

function resolveExecutable(command, cwd, environment = process.env) {
  if (typeof command !== "string" || command.length === 0 || /[\0\r\n]/.test(command)) {
    throw new Error("Candidate build tool command is invalid.");
  }
  if (path.isAbsolute(command) || command.includes(path.sep)) {
    const resolved = path.resolve(cwd, command);
    return existsSync(resolved) ? realpathSync(resolved) : null;
  }
  const locator = process.platform === "win32" ? "where" : "which";
  const result = spawnSync(locator, [command], { cwd, encoding: "utf8", env: environment });
  if (result.error || result.status !== 0) return null;
  const first = result.stdout.split(/\r?\n/, 1)[0];
  return first ? realpathSync(first) : null;
}

function candidateCargoCommand() {
  return process.env.CARGO ?? "cargo";
}

function candidateRustcCommand() {
  return process.env.CARGO_BUILD_RUSTC ?? process.env.RUSTC ?? "rustc";
}

function gitCapture(repository, args) {
  return runGit(repository, args, { encoding: "utf8" }).trim();
}

function runGit(
  repository,
  args,
  { encoding = "utf8", environment = gitEnvironment(), input } = {},
) {
  const result = spawnSync("git", args, {
    cwd: repository,
    encoding,
    env: environment,
    input,
    maxBuffer: 512 * 1024 * 1024,
  });
  if (result.error || result.status !== 0) {
    const stderr = Buffer.isBuffer(result.stderr)
      ? result.stderr.toString("utf8")
      : result.stderr ?? "";
    throw new Error(
      `git ${args.join(" ")} failed: ${result.error?.message ?? stderr.trim()}`,
    );
  }
  return result.stdout;
}

function gitEnvironment(overrides = {}) {
  const environment = candidateProcessEnvironment();
  for (const name of Object.keys(environment)) {
    if (name.startsWith("GIT_")) delete environment[name];
  }
  return { ...environment, ...overrides };
}

function runCapture(
  command,
  args,
  { cwd = nodeRoot, environment = candidateProcessEnvironment() } = {},
) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8", env: environment });
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
