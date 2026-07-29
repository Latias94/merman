import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
} from "node:fs";
import path from "node:path";

import { validateCandidateDependencyPackages } from "../build-candidate.mjs";
import {
  computeBuildReceiptInputDigest,
  validateBuildEnvironment,
  validateBuildSourceProvenance,
} from "../build-receipt.mjs";
import { digestJson, stableJson } from "../stable-json.mjs";

const SHA256 = /^sha256:[0-9a-f]{64}$/;

export function readHistoricalRequestOverlayReceipt(
  artifactPath,
  { revision, transport },
) {
  if (!new Set(["base", "head"]).has(revision)) {
    throw new Error(`Unknown request-overlay revision: ${revision}.`);
  }
  if (!new Set(["napi", "node-wasm"]).has(transport)) {
    throw new Error(`Unknown request-overlay transport: ${transport}.`);
  }
  const artifact = path.resolve(artifactPath);
  assertRegularFile(artifact, `${revision} ${transport} artifact`);
  const receiptRoot = path.dirname(artifact);
  const receiptPath = path.join(receiptRoot, "build-receipt.json");
  assertRegularFile(receiptPath, `${revision} ${transport} build receipt`);
  const receipt = parseJsonFile(receiptPath, `${revision} ${transport} build receipt`);
  validateReceipt(receipt, transport);

  const recordedFiles = validateArtifactEntries(receiptRoot, receipt.artifacts);
  const actualFiles = collectFiles(receiptRoot)
    .map((file) => path.relative(receiptRoot, file).split(path.sep).join("/"))
    .filter((relative) => relative !== "build-receipt.json")
    .sort();
  if (stableJson(actualFiles) !== stableJson([...recordedFiles.keys()].sort())) {
    throw new Error(`${revision} ${transport} receipt does not bind its complete artifact file set.`);
  }

  const relativeArtifact = path.relative(receiptRoot, artifact).split(path.sep).join("/");
  const expectedArtifact = transport === "napi" ? "merman.node" : "merman_node.js";
  if (relativeArtifact !== expectedArtifact) {
    throw new Error(
      `${revision} ${transport} artifact must be the canonical ${expectedArtifact} receipt entry.`,
    );
  }
  const recordedArtifact = recordedFiles.get(relativeArtifact);
  if (!recordedArtifact) {
    throw new Error(`${revision} ${transport} receipt does not record ${relativeArtifact}.`);
  }

  return {
    key: `${revision}:${transport}`,
    revision,
    transport,
    commit: receipt.commit,
    commit_tree: receipt.commit_tree,
    target: receipt.config.target,
    rust_target: receipt.config.rust_target,
    wasm_pack_target: receipt.config.wasm_pack_target,
    cargo_features: receipt.config.features,
    capability_ids: receipt.config.capability_recipe.capabilities,
    build_tools: receipt.tools,
    build_environment_digest: receipt.build_environment_digest,
    artifact_path: artifact,
    artifact_path_in_receipt: relativeArtifact,
    artifact_bytes: recordedArtifact.bytes,
    artifact_sha256: recordedArtifact.sha256,
    receipt_digest: digestJson(receipt),
    source_digest: receipt.source_digest,
    cargo_lock_digest: receipt.cargo_lock_digest,
    binding_contract_digest: receipt.binding_contract_digest,
    dependency_closure_digest: receipt.dependency_closure.digest,
    capability_recipe_digest: digestJson(receipt.config.capability_recipe),
    input_digest: receipt.input_digest,
    runtime_catalog_digest: receipt.runtime.catalog_digest,
  };
}

export function projectHistoricalArtifact(value) {
  const { artifact_path: _artifactPath, ...projection } = value;
  return projection;
}

function validateReceipt(value, transport) {
  assertObject(value, "historical build receipt");
  if (value.schema_version !== 4 || value.config?.candidate !== transport) {
    throw new Error(`Historical ${transport} build receipt has an invalid schema or candidate.`);
  }
  for (const key of [
    "source_digest",
    "cargo_lock_digest",
    "binding_contract_digest",
    "build_environment_digest",
    "input_digest",
  ]) {
    assertDigest(value[key], `historical ${transport} receipt ${key}`);
  }
  validateBuildSourceProvenance(value, {
    label: `Historical ${transport} build receipt`,
  });
  validateBuildEnvironment(value, {
    label: `Historical ${transport} build receipt`,
  });

  assertObject(value.config, `historical ${transport} receipt config`);
  assertObject(value.config.capability_recipe, `historical ${transport} capability recipe`);
  if (
    value.config.default_features !== false ||
    value.config.capability_recipe.target !== "native" ||
    !Array.isArray(value.config.capability_recipe.capabilities) ||
    value.config.capability_recipe.capabilities.length === 0 ||
    !Array.isArray(value.config.features) ||
    !value.config.features.includes(`transport-${transport === "napi" ? "napi" : "wasm"}`)
  ) {
    throw new Error(`Historical ${transport} build receipt capability recipe is invalid.`);
  }
  assertSortedUniqueStrings(
    value.config.capability_recipe.capabilities,
    `historical ${transport} capability IDs`,
  );
  assertSortedUniqueStrings(value.config.features, `historical ${transport} Cargo features`);

  assertObject(value.dependency_closure, `historical ${transport} dependency closure`);
  assertDigest(
    value.dependency_closure.digest,
    `historical ${transport} dependency closure digest`,
  );
  if (
    !Array.isArray(value.dependency_closure.packages) ||
    digestJson(value.dependency_closure.packages) !== value.dependency_closure.digest
  ) {
    throw new Error(`Historical ${transport} dependency closure is not self-consistent.`);
  }
  validateCandidateDependencyPackages(value.dependency_closure.packages, transport);
  validateTools(value.tools, transport);
  const expectedInputDigest = computeBuildReceiptInputDigest(value);
  if (value.input_digest !== expectedInputDigest) {
    throw new Error(`Historical ${transport} receipt input digest is not self-consistent.`);
  }
  validateRuntime(value.runtime, transport);
}

function validateRuntime(value, transport) {
  assertObject(value, `historical ${transport} runtime evidence`);
  assertObject(value.catalog, `historical ${transport} runtime catalog`);
  assertDigest(value.catalog_digest, `historical ${transport} runtime catalog digest`);
  if (
    digestJson(value.catalog) !== value.catalog_digest ||
    value.catalog.schema_version !== 1 ||
    value.catalog.transport_api_version !== 1 ||
    typeof value.catalog.package_version !== "string" ||
    value.catalog.package_version.length === 0
  ) {
    throw new Error(`Historical ${transport} runtime catalog is not self-consistent.`);
  }
  const capabilities = value.catalog.capabilities;
  assertObject(capabilities, `historical ${transport} runtime capabilities`);
  for (const [key, allowEmpty] of [
    ["capability_ids", false],
    ["output_ids", false],
    ["operation_ids", false],
    ["system_adapter_ids", true],
  ]) {
    assertSortedUniqueStrings(
      capabilities[key],
      `historical ${transport} runtime ${key}`,
      { allowEmpty },
    );
  }
  if (!capabilities.operation_ids.includes("semantic-json")) {
    throw new Error(`Historical ${transport} runtime lacks semantic-json.`);
  }
  assertObject(value.probe, `historical ${transport} runtime probe`);
  if (
    value.probe.request_options_limit_code_name !== "MERMAN_RESOURCE_LIMIT_EXCEEDED" ||
    value.probe.unknown_operation_kind !== "unknown-operation"
  ) {
    throw new Error(`Historical ${transport} runtime probe lacks fixed option/error evidence.`);
  }
}

function validateArtifactEntries(receiptRoot, entries) {
  if (!Array.isArray(entries) || entries.length === 0) {
    throw new Error("Historical build receipt contains no artifacts.");
  }
  const byPath = new Map();
  for (const entry of entries) {
    if (
      !entry ||
      typeof entry.path !== "string" ||
      entry.path.length === 0 ||
      entry.path !== path.posix.normalize(entry.path) ||
      entry.path.startsWith("../") ||
      path.posix.isAbsolute(entry.path) ||
      byPath.has(entry.path) ||
      !Number.isSafeInteger(entry.bytes) ||
      entry.bytes < 1
    ) {
      throw new Error("Historical build receipt contains an invalid artifact entry.");
    }
    assertDigest(entry.sha256, `historical artifact ${entry.path}`);
    const absolute = path.resolve(receiptRoot, entry.path);
    const relative = path.relative(receiptRoot, absolute);
    if (relative.startsWith("..") || path.isAbsolute(relative)) {
      throw new Error(`Historical artifact escapes its receipt root: ${entry.path}.`);
    }
    assertRegularFile(absolute, `historical artifact ${entry.path}`);
    if (lstatSync(absolute).size !== entry.bytes || digestFile(absolute) !== entry.sha256) {
      throw new Error(`Historical artifact receipt is stale for ${entry.path}.`);
    }
    byPath.set(entry.path, entry);
  }
  return byPath;
}

function validateTools(value, transport) {
  assertObject(value, `historical ${transport} build tools`);
  for (const key of ["cargo", "node", "rustc", "transport_builder"]) {
    if (typeof value[key] !== "string" || value[key].length === 0) {
      throw new Error(`Historical ${transport} build receipt lacks tool ${key}.`);
    }
  }
}

function collectFiles(root) {
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const absolute = path.join(root, entry.name);
    if (entry.isDirectory()) files.push(...collectFiles(absolute));
    else if (entry.isFile()) files.push(absolute);
    else throw new Error(`Historical artifact root contains a non-regular entry: ${absolute}.`);
  }
  return files;
}

function parseJsonFile(file, label) {
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch (cause) {
    throw new Error(`${label} is not valid JSON: ${cause instanceof Error ? cause.message : String(cause)}`);
  }
}

function digestFile(file) {
  return `sha256:${createHash("sha256").update(readFileSync(file)).digest("hex")}`;
}

function assertRegularFile(file, label) {
  if (!existsSync(file)) throw new Error(`Missing ${label}: ${file}.`);
  const stat = lstatSync(file);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    throw new Error(`${label} must be a regular non-symlink file: ${file}.`);
  }
}

function assertSortedUniqueStrings(value, label, { allowEmpty = false } = {}) {
  if (
    !Array.isArray(value) ||
    (!allowEmpty && value.length === 0) ||
    value.some((item) => typeof item !== "string" || item.length === 0) ||
    stableJson(value) !== stableJson([...new Set(value)].sort())
  ) {
    throw new Error(`${label} must be sorted unique strings.`);
  }
}

function assertDigest(value, label) {
  if (!SHA256.test(value ?? "")) throw new Error(`${label} must be a SHA-256 digest.`);
}

function assertObject(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object.`);
  }
}
