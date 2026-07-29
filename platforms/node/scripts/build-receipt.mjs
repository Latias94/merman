import { createHash } from "node:crypto";
import { existsSync, lstatSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  CANDIDATE_BUILD_ENVIRONMENT_REQUIRED_NAMES,
  computeBuildReceiptInputDigest,
  probeCandidateRuntime,
  resolveCandidateBuildEvidence,
  resolveCandidateBuildEnvironment,
  resolveCandidateRecipe,
  resolveCandidateRuntimeContract,
  validateCandidateDependencyPackages,
  validateGitSourceInputs,
} from "./build-candidate.mjs";
import { digestJson, stableJson } from "./stable-json.mjs";
import { validateRuntimeCatalog } from "../src/engine.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");
const cargoLockPath = path.join(repositoryRoot, "crates", "merman-node", "Cargo.lock");
const SHA256 = /^sha256:[0-9a-f]{64}$/;
const packageVersion = JSON.parse(
  readFileSync(path.join(nodeRoot, "package-surfaces.json"), "utf8"),
).version;

export function readBuildReceipt(
  artifact,
  {
    probeCurrentRuntime = probeCandidateRuntime,
    resolveCurrentEvidence = resolveCandidateBuildEvidence,
  } = {},
) {
  const receiptPath = path.join(path.dirname(artifact), "build-receipt.json");
  assertFile(receiptPath, "candidate build receipt");
  const value = JSON.parse(readFileSync(receiptPath, "utf8"));
  if (value.schema_version !== 4) {
    throw new Error("candidate build receipt schema_version must be 4.");
  }
  for (const key of [
    "source_digest",
    "cargo_lock_digest",
    "binding_contract_digest",
    "build_environment_digest",
    "input_digest",
  ]) {
    if (!SHA256.test(value[key] ?? "")) {
      throw new Error(`candidate build receipt has an invalid ${key}.`);
    }
  }
  validateBuildSourceProvenance(value);
  const { buildRecipe, capabilityRecipe } = validateCapabilityRecipe(value.config);
  validateBuildEnvironment(value);
  const currentBuildEnvironment = resolveCandidateBuildEnvironment().contract;
  if (digestJson(currentBuildEnvironment) !== value.build_environment_digest) {
    throw new Error(
      "candidate build receipt build environment is stale for the current process.",
    );
  }
  if (value.cargo_lock_digest !== digestFile(cargoLockPath)) {
    throw new Error("candidate build receipt Cargo lock digest does not match the current lockfile.");
  }
  const dependencyClosureDigest = validateDependencyClosure(
    value.dependency_closure,
    value.config?.candidate,
  );
  validateBuildTools(value.tools);
  const expectedInputDigest = computeBuildReceiptInputDigest(
    value,
    dependencyClosureDigest,
  );
  if (value.input_digest !== expectedInputDigest) {
    throw new Error("candidate build receipt input digest does not match its recorded build inputs.");
  }
  const currentEvidence = resolveCurrentEvidence(buildRecipe);
  for (const key of [
    "source_digest",
    "binding_contract_digest",
    "dependency_closure_digest",
  ]) {
    if (!/^sha256:[0-9a-f]{64}$/.test(currentEvidence?.[key] ?? "")) {
      throw new Error(`current candidate build evidence has an invalid ${key}.`);
    }
    const receiptValue = key === "dependency_closure_digest"
      ? dependencyClosureDigest
      : value[key];
    if (receiptValue !== currentEvidence[key]) {
      throw new Error(`candidate build receipt ${key} is stale for the current source tree.`);
    }
  }
  const receiptRoot = path.dirname(receiptPath);
  assertDirectory(receiptRoot, "candidate build receipt root");
  const artifactPath = path.relative(receiptRoot, artifact).split(path.sep).join("/");
  if (!Array.isArray(value.artifacts) || value.artifacts.length === 0) {
    throw new Error("candidate build receipt contains no artifacts.");
  }
  const seen = new Set();
  for (const recorded of value.artifacts) {
    if (
      !recorded ||
      typeof recorded.path !== "string" ||
      recorded.path.length === 0 ||
      recorded.path !== path.posix.normalize(recorded.path) ||
      recorded.path.startsWith("../") ||
      path.posix.isAbsolute(recorded.path) ||
      seen.has(recorded.path)
    ) {
      throw new Error("candidate build receipt contains an invalid or duplicate artifact path.");
    }
    seen.add(recorded.path);
    const recordedPath = path.resolve(receiptRoot, recorded.path);
    const relative = path.relative(receiptRoot, recordedPath);
    if (relative.startsWith("..") || path.isAbsolute(relative)) {
      throw new Error(`candidate build receipt artifact escapes its root: ${recorded.path}.`);
    }
    assertFile(recordedPath, `candidate artifact ${recorded.path}`);
    if (
      recorded.sha256 !== digestFile(recordedPath) ||
      recorded.bytes !== lstatSync(recordedPath).size
    ) {
      throw new Error(`candidate build receipt does not match ${recorded.path}.`);
    }
  }
  const actualArtifactPaths = collectArtifactPaths(receiptRoot).filter(
    (candidatePath) => candidatePath !== "build-receipt.json",
  );
  const recordedArtifactPaths = [...seen].sort();
  if (JSON.stringify(actualArtifactPaths) !== JSON.stringify(recordedArtifactPaths)) {
    throw new Error("candidate build receipt artifact file set does not match its directory.");
  }
  const recordedArtifact = value.artifacts?.find((item) => item.path === artifactPath);
  const artifactDigest = digestFile(artifact);
  if (recordedArtifact?.sha256 !== artifactDigest) {
    throw new Error(`candidate build receipt does not match ${artifactPath}.`);
  }
  const runtimeCatalogDigest = validateRuntimeEvidence(value.runtime, capabilityRecipe);
  const runtimeArtifacts = runtimeArtifactEntries(value.config.candidate, value.artifacts);
  const currentRuntime = probeCurrentRuntime(receiptRoot, buildRecipe);
  if (stableJson(currentRuntime) !== stableJson(value.runtime)) {
    throw new Error("candidate build receipt runtime evidence does not match the current artifact.");
  }
  return {
    receipt_digest: digestJson(value),
    candidate: value.config.candidate,
    target: value.config.target,
    rust_target: value.config.rust_target,
    wasm_pack_target: value.config.wasm_pack_target,
    commit: value.commit,
    commit_tree: value.commit_tree,
    source_digest: value.source_digest,
    cargo_lock_digest: value.cargo_lock_digest,
    binding_contract_digest: value.binding_contract_digest,
    build_environment_digest: value.build_environment_digest,
    dependency_closure_digest: dependencyClosureDigest,
    capability_recipe_digest: digestJson(capabilityRecipe),
    runtime_catalog_digest: runtimeCatalogDigest,
    input_digest: value.input_digest,
    artifact_digest: artifactDigest,
    runtime_artifacts: runtimeArtifacts,
  };
}

export { computeBuildReceiptInputDigest } from "./build-candidate.mjs";

export function validateBuildSourceProvenance(
  value,
  { label = "candidate build receipt" } = {},
) {
  const sourceInputs = validateGitSourceInputs(value, { label });
  if (digestJson(sourceInputs) !== value.source_digest) {
    throw new Error(`${label} source digest does not match its source inputs.`);
  }
  const bindingInputs = sourceInputs.filter(
    (entry) =>
      !entry.path.startsWith("crates/merman-node/") &&
      !entry.path.startsWith("platforms/node/"),
  );
  if (
    bindingInputs.length === 0 ||
    digestJson(bindingInputs) !== value.binding_contract_digest
  ) {
    throw new Error(`${label} binding contract digest does not match its source inputs.`);
  }
  const cargoLock = sourceInputs.find(
    (entry) => entry.path === "crates/merman-node/Cargo.lock",
  );
  if (!cargoLock || cargoLock.sha256 !== value.cargo_lock_digest) {
    throw new Error(`${label} Cargo lock digest does not match its source inputs.`);
  }

  return sourceInputs;
}

export function validateBuildEnvironment(
  value,
  { label = "candidate build receipt" } = {},
) {
  const environment = value.build_environment;
  if (
    !environment ||
    typeof environment !== "object" ||
    Array.isArray(environment) ||
    environment.schema_version !== 2 ||
    environment.enforced?.CARGO_BUILD_JOBS !== "1" ||
    environment.enforced?.CARGO_TARGET_DIR !== "target" ||
    !Array.isArray(environment.inherited) ||
    !Array.isArray(environment.external_inputs)
  ) {
    throw new Error(`${label} build environment contract is invalid.`);
  }
  const inheritedNames = [];
  for (const entry of environment.inherited) {
    if (
      !entry ||
      typeof entry.name !== "string" ||
      entry.name.length === 0 ||
      !new Set(["absent", "present"]).has(entry.state) ||
      (entry.state === "absent" && Object.keys(entry).length !== 2) ||
      (entry.state === "present" && !SHA256.test(entry.value_sha256 ?? ""))
    ) {
      throw new Error(`${label} inherited build environment is invalid.`);
    }
    inheritedNames.push(entry.name);
  }
  if (
    JSON.stringify(inheritedNames) !==
      JSON.stringify([...new Set(inheritedNames)].sort())
  ) {
    throw new Error(`${label} inherited build environment must be sorted and unique.`);
  }
  for (const required of CANDIDATE_BUILD_ENVIRONMENT_REQUIRED_NAMES) {
    if (!inheritedNames.includes(required)) {
      throw new Error(`${label} inherited build environment omits ${required}.`);
    }
  }
  const expectedExternalIds = [
    "build-source/.cargo/config",
    "build-source/.cargo/config.toml",
    "cargo-home/config",
    "cargo-home/config.toml",
    "tool/cargo",
    "tool/git",
    "tool/node",
    "tool/rustc",
    "tool/wasm-pack",
    "tool/napi-cli",
    "tool/napi-cli-node-modules",
  ];
  const expectedEnvironmentToolIds = environment.inherited
    .filter(
      (entry) =>
        entry.state === "present" &&
        (new Set([
          "CARGO_BUILD_RUSTC",
          "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
          "CARGO_BUILD_RUSTC_WRAPPER",
          "RUSTC_WRAPPER",
          "RUSTC_WORKSPACE_WRAPPER",
        ]).has(entry.name) ||
          /^(?:AR|CC|CXX|LD|NM|OBJCOPY|RANLIB|STRIP)(?:_.+)?$/.test(entry.name) ||
          /^CARGO_TARGET_.+_(?:LINKER|RUNNER)$/.test(entry.name)),
    )
    .map((entry) => `environment-tool/${entry.name}`)
    .sort();
  if (
    JSON.stringify(environment.external_inputs.map((entry) => entry?.id)) !==
      JSON.stringify([...expectedExternalIds, ...expectedEnvironmentToolIds])
  ) {
    throw new Error(`${label} external build input coverage is incomplete.`);
  }
  for (const entry of environment.external_inputs) {
    if (entry.state === "absent") {
      if (Object.keys(entry).length !== 2) {
        throw new Error(`${label} absent Cargo configuration evidence is invalid.`);
      }
    } else if (
      entry.state !== "present" ||
      !Number.isSafeInteger(entry.bytes) ||
      entry.bytes < 0 ||
      !SHA256.test(entry.sha256 ?? "") ||
      (entry.path_sha256 !== undefined && !SHA256.test(entry.path_sha256))
    ) {
      throw new Error(`${label} present external build input evidence is invalid.`);
    }
    if (
      entry.state === "present" &&
      (entry.id.startsWith("tool/") || entry.id.startsWith("environment-tool/")) &&
      !SHA256.test(entry.path_sha256 ?? "")
    ) {
      throw new Error(`${label} path-bound external build input evidence is invalid.`);
    }
    if (
      entry.id === "tool/napi-cli-node-modules" &&
      entry.state === "present" &&
      (!Number.isSafeInteger(entry.file_count) || entry.file_count < 1)
    ) {
      throw new Error(`${label} external build input tree evidence is invalid.`);
    }
  }
  for (const required of [
    "tool/cargo",
    "tool/git",
    "tool/node",
    "tool/rustc",
    "tool/napi-cli",
    "tool/napi-cli-node-modules",
  ]) {
    if (environment.external_inputs.find((entry) => entry.id === required)?.state !== "present") {
      throw new Error(`${label} lacks required external build input ${required}.`);
    }
  }
  if (
    !SHA256.test(value.build_environment_digest ?? "") ||
    digestJson(environment) !== value.build_environment_digest
  ) {
    throw new Error(`${label} build environment digest is invalid.`);
  }
  return environment;
}

function runtimeArtifactEntries(candidate, artifacts) {
  const expectedPaths = candidate === "napi"
    ? ["merman.node"]
    : candidate === "node-wasm"
      ? ["merman_node.js", "merman_node_bg.wasm", "package.json"]
      : null;
  if (expectedPaths === null) throw new Error(`unknown candidate runtime artifacts: ${candidate}.`);
  const byPath = new Map(artifacts.map((artifact) => [artifact.path, artifact]));
  const runtimeArtifacts = expectedPaths.map((artifactPath) => byPath.get(artifactPath));
  if (
    runtimeArtifacts.some(
      (artifact) =>
        !artifact ||
        !Number.isSafeInteger(artifact.bytes) ||
        artifact.bytes < 1 ||
        !/^sha256:[0-9a-f]{64}$/.test(artifact.sha256 ?? ""),
    )
  ) {
    throw new Error(`${candidate} build receipt lacks its complete runtime artifact set.`);
  }
  return runtimeArtifacts.map(({ path: artifactPath, bytes, sha256 }) => ({
    path: artifactPath,
    bytes,
    sha256,
  }));
}

function validateDependencyClosure(closure, candidate) {
  if (
    !closure ||
    typeof closure !== "object" ||
    Array.isArray(closure) ||
    !/^sha256:[0-9a-f]{64}$/.test(closure.digest ?? "") ||
    !Array.isArray(closure.packages) ||
    digestJson(closure.packages) !== closure.digest
  ) {
    throw new Error("candidate build receipt dependency closure is invalid.");
  }
  validateCandidateDependencyPackages(closure.packages, candidate);
  return closure.digest;
}

function validateRuntimeEvidence(runtime, capabilityRecipe) {
  if (!runtime || typeof runtime !== "object" || Array.isArray(runtime)) {
    throw new Error("candidate build receipt runtime evidence is required.");
  }
  if (!/^sha256:[0-9a-f]{64}$/.test(runtime.catalog_digest ?? "")) {
    throw new Error("candidate build receipt runtime catalog digest is invalid.");
  }
  let catalog;
  try {
    catalog = validateRuntimeCatalog(runtime.catalog);
  } catch (cause) {
    throw new Error(
      `candidate build receipt runtime catalog is invalid: ${cause instanceof Error ? cause.message : String(cause)}`,
    );
  }
  if (
    !catalog ||
    typeof catalog !== "object" ||
    Array.isArray(catalog) ||
    digestJson(catalog) !== runtime.catalog_digest ||
    catalog.schema_version !== 1 ||
    catalog.transport_api_version !== 1 ||
    catalog.package_version !== packageVersion ||
    !Number.isSafeInteger(catalog.registry?.diagram_family_count) ||
    catalog.registry.diagram_family_count < 1
  ) {
    throw new Error("candidate build receipt runtime evidence is invalid.");
  }
  const capabilities = catalog.capabilities;
  const capabilityIds = sortedUniqueStrings(
    capabilities?.capability_ids,
    "runtime capability IDs",
  );
  const outputIds = sortedUniqueStrings(capabilities?.output_ids, "runtime output IDs");
  const operationIds = sortedUniqueStrings(
    capabilities?.operation_ids,
    "runtime operation IDs",
  );
  const systemAdapterIds = sortedUniqueStrings(
    capabilities?.system_adapter_ids,
    "runtime system adapter IDs",
    { allowEmpty: true },
  );
  const textMeasurementProviderIds = sortedUniqueStrings(
    capabilities?.text_measurement?.provider_ids,
    "runtime text measurement provider IDs",
  );
  const expectedRuntime = resolveCandidateRuntimeContract();
  if (
    JSON.stringify(textMeasurementProviderIds) !==
    JSON.stringify(expectedRuntime.textMeasurementProviderIds)
  ) {
    throw new Error(
      "candidate build receipt runtime text measurement provider IDs are not callable by Node.",
    );
  }
  if (
    JSON.stringify(capabilityIds) !==
      JSON.stringify(expectedRuntime.capabilityIds) ||
    JSON.stringify(outputIds) !== JSON.stringify(expectedRuntime.outputIds) ||
    JSON.stringify(operationIds) !== JSON.stringify(expectedRuntime.operationIds) ||
    JSON.stringify(systemAdapterIds) !== JSON.stringify(expectedRuntime.systemAdapterIds)
  ) {
    throw new Error("candidate build receipt runtime catalog disagrees with its capability recipe.");
  }
  if (
    !runtime.probe ||
    typeof runtime.probe !== "object" ||
    Array.isArray(runtime.probe) ||
    runtime.probe.missing_capability_id !== "png" ||
    !Number.isSafeInteger(runtime.probe.semantic_json_bytes) ||
    runtime.probe.semantic_json_bytes < 1 ||
    !Number.isSafeInteger(runtime.probe.svg_plan_json_bytes) ||
    runtime.probe.svg_plan_json_bytes < 1 ||
    !Number.isSafeInteger(runtime.probe.svg_bytes) ||
    runtime.probe.svg_bytes < 1 ||
    !/^sha256:[0-9a-f]{64}$/.test(runtime.probe.svg_structure_sha256 ?? "") ||
    !/^sha256:[0-9a-f]{64}$/.test(runtime.probe.svg_geometry_sha256 ?? "") ||
    runtime.probe.unknown_operation_kind !== "unknown-operation" ||
    runtime.probe.request_options_limit_code_name !== "MERMAN_RESOURCE_LIMIT_EXCEEDED"
  ) {
    throw new Error("candidate build receipt runtime probe is invalid.");
  }
  return runtime.catalog_digest;
}

function validateCapabilityRecipe(config) {
  if (!config || typeof config !== "object" || Array.isArray(config)) {
    throw new Error("candidate build receipt config must be an object.");
  }
  if (config.default_features !== false) {
    throw new Error("candidate build receipt must disable Cargo default features.");
  }
  assertExactKeys(config, [
    "candidate",
    "target",
    "rust_target",
    "wasm_pack_target",
    "default_features",
    "capability_recipe",
    "features",
  ], "candidate build receipt config");
  const recipe = config.capability_recipe;
  if (
    !recipe ||
    typeof recipe !== "object" ||
    Array.isArray(recipe) ||
    typeof recipe.descriptor !== "string" ||
    recipe.descriptor.length === 0 ||
    typeof recipe.target !== "string" ||
    recipe.target.length === 0
  ) {
    throw new Error("candidate build receipt capability recipe is invalid.");
  }
  const capabilityFeatures = sortedUniqueStrings(
    recipe.capabilities,
    "capability recipe capabilities",
  );
  const expectedRecipe = resolveCandidateRuntimeContract().capabilityRecipe;
  if (
    recipe.descriptor !== expectedRecipe.descriptor ||
    recipe.target !== expectedRecipe.target ||
    JSON.stringify(capabilityFeatures) !== JSON.stringify(expectedRecipe.capabilities)
  ) {
    throw new Error("candidate build receipt capability recipe is not the canonical Node recipe.");
  }
  const cargoFeatures = sortedUniqueStrings(config.features, "Cargo features");
  let expectedBuildRecipe;
  try {
    expectedBuildRecipe = resolveCandidateRecipe(config.candidate, config.target);
  } catch (error) {
    throw new Error(
      `candidate build receipt has an invalid candidate target: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  if (
    config.target !== expectedBuildRecipe.targetId ||
    config.rust_target !== expectedBuildRecipe.rustTarget ||
    config.wasm_pack_target !== expectedBuildRecipe.wasmPackTarget
  ) {
    throw new Error("candidate build receipt target configuration is not canonical.");
  }
  if (JSON.stringify(cargoFeatures) !== JSON.stringify(expectedBuildRecipe.cargoFeatures)) {
    throw new Error(
      "candidate build receipt Cargo features must equal the capability recipe capabilities plus its transport.",
    );
  }
  return {
    buildRecipe: expectedBuildRecipe,
    capabilityRecipe: {
      default_features: false,
      capability_recipe: {
        descriptor: recipe.descriptor,
        target: recipe.target,
        capabilities: capabilityFeatures,
      },
    },
  };
}

function validateBuildTools(tools) {
  if (!tools || typeof tools !== "object" || Array.isArray(tools)) {
    throw new Error("candidate build receipt tools are required.");
  }
  assertExactKeys(
    tools,
    ["cargo", "node", "rustc", "transport_builder"],
    "candidate build receipt tools",
  );
  for (const [key, value] of Object.entries(tools)) {
    if (typeof value !== "string" || value.length === 0) {
      throw new Error(`candidate build receipt tool ${key} is invalid.`);
    }
  }
}

function assertExactKeys(value, expected, label) {
  const actual = Object.keys(value).sort();
  const normalizedExpected = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(normalizedExpected)) {
    throw new Error(`${label} fields are invalid.`);
  }
}

function sortedUniqueStrings(value, label, { allowEmpty = false } = {}) {
  if (
    !Array.isArray(value) ||
    (!allowEmpty && value.length === 0) ||
    value.some((item) => typeof item !== "string" || item.length === 0)
  ) {
    throw new Error(
      `candidate build receipt ${label} must be ${allowEmpty ? "strings" : "non-empty strings"}.`,
    );
  }
  const normalized = [...new Set(value)].sort();
  if (JSON.stringify(value) !== JSON.stringify(normalized)) {
    throw new Error(`candidate build receipt ${label} must be sorted and unique.`);
  }
  return normalized;
}

function digestFile(file) {
  return digestBytes(readFileSync(file));
}

function digestBytes(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}

function collectArtifactPaths(root, current = root) {
  assertDirectory(current, "candidate artifact directory");
  const files = [];
  for (const entry of readdirSync(current, { withFileTypes: true })) {
    const absolute = path.join(current, entry.name);
    const stat = lstatSync(absolute);
    if (stat.isDirectory() && !stat.isSymbolicLink()) {
      files.push(...collectArtifactPaths(root, absolute));
    } else if (stat.isFile() && !stat.isSymbolicLink()) {
      files.push(path.relative(root, absolute).split(path.sep).join("/"));
    } else {
      throw new Error(`Candidate artifact root contains a non-regular entry: ${absolute}.`);
    }
  }
  return files.sort();
}

function assertFile(file, label) {
  if (!existsSync(file)) throw new Error(`Missing ${label}: ${file}.`);
  const stat = lstatSync(file);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    throw new Error(`${label} must be a regular non-symlink file: ${file}.`);
  }
}

function assertDirectory(directory, label) {
  const stat = lstatSync(directory);
  if (!stat.isDirectory() || stat.isSymbolicLink()) {
    throw new Error(`${label} must be a non-symlink directory: ${directory}.`);
  }
}
