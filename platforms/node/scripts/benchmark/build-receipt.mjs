import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";

import { digestJson } from "../stable-json.mjs";

export function readBuildReceipt(artifact) {
  const receiptPath = path.join(path.dirname(artifact), "build-receipt.json");
  assertFile(receiptPath, "candidate build receipt");
  const value = JSON.parse(readFileSync(receiptPath, "utf8"));
  if (value.schema_version !== 1) {
    throw new Error("candidate build receipt schema_version must be 1.");
  }
  for (const key of ["source_digest", "binding_contract_digest", "input_digest"]) {
    if (!/^sha256:[0-9a-f]{64}$/.test(value[key] ?? "")) {
      throw new Error(`candidate build receipt has an invalid ${key}.`);
    }
  }
  const receiptRoot = path.dirname(receiptPath);
  const artifactPath = path.relative(receiptRoot, artifact).split(path.sep).join("/");
  if (!Array.isArray(value.artifacts) || value.artifacts.length === 0) {
    throw new Error("candidate build receipt contains no artifacts.");
  }
  const seen = new Set();
  for (const recorded of value.artifacts) {
    if (!recorded || typeof recorded.path !== "string" || seen.has(recorded.path)) {
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
      recorded.bytes !== statSync(recordedPath).size
    ) {
      throw new Error(`candidate build receipt does not match ${recorded.path}.`);
    }
  }
  const recordedArtifact = value.artifacts?.find((item) => item.path === artifactPath);
  const artifactDigest = digestFile(artifact);
  if (recordedArtifact?.sha256 !== artifactDigest) {
    throw new Error(`candidate build receipt does not match ${artifactPath}.`);
  }
  const capabilityRecipe = validateCapabilityRecipe(value.config);
  return {
    receipt_digest: digestJson(value),
    commit: value.commit,
    source_digest: value.source_digest,
    binding_contract_digest: value.binding_contract_digest,
    capability_recipe_digest: digestJson(capabilityRecipe),
    input_digest: value.input_digest,
    artifact_digest: artifactDigest,
  };
}

function validateCapabilityRecipe(config) {
  if (!config || typeof config !== "object" || Array.isArray(config)) {
    throw new Error("candidate build receipt config must be an object.");
  }
  if (config.default_features !== false) {
    throw new Error("candidate build receipt must disable Cargo default features.");
  }
  const profile = config.artifact_profile;
  if (
    !profile ||
    typeof profile !== "object" ||
    Array.isArray(profile) ||
    typeof profile.descriptor !== "string" ||
    profile.descriptor.length === 0 ||
    typeof profile.id !== "string" ||
    profile.id.length === 0
  ) {
    throw new Error("candidate build receipt artifact profile is invalid.");
  }
  const capabilityFeatures = sortedUniqueStrings(
    profile.features,
    "artifact profile features",
  );
  const cargoFeatures = sortedUniqueStrings(config.features, "Cargo features");
  const transportFeature =
    config.candidate === "napi"
      ? "transport-napi"
      : config.candidate === "node-wasm"
        ? "transport-wasm"
        : null;
  if (transportFeature === null) {
    throw new Error(`candidate build receipt has unknown candidate ${config.candidate}.`);
  }
  const expectedCargoFeatures = [...capabilityFeatures, transportFeature].sort();
  if (JSON.stringify(cargoFeatures) !== JSON.stringify(expectedCargoFeatures)) {
    throw new Error(
      "candidate build receipt Cargo features must equal the artifact profile leaves plus its transport.",
    );
  }
  return {
    default_features: false,
    artifact_profile: {
      descriptor: profile.descriptor,
      id: profile.id,
      features: capabilityFeatures,
    },
  };
}

function sortedUniqueStrings(value, label) {
  if (
    !Array.isArray(value) ||
    value.length === 0 ||
    value.some((item) => typeof item !== "string" || item.length === 0)
  ) {
    throw new Error(`candidate build receipt ${label} must be non-empty strings.`);
  }
  const normalized = [...new Set(value)].sort();
  if (JSON.stringify(value) !== JSON.stringify(normalized)) {
    throw new Error(`candidate build receipt ${label} must be sorted and unique.`);
  }
  return normalized;
}

function digestFile(file) {
  return `sha256:${createHash("sha256").update(readFileSync(file)).digest("hex")}`;
}

function assertFile(file, label) {
  if (!existsSync(file) || !statSync(file).isFile()) throw new Error(`Missing ${label}: ${file}.`);
}
