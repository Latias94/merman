import { readFileSync } from "node:fs";
import path from "node:path";

import { repositoryRoot } from "./paths.mjs";

const capabilityDescriptor = JSON.parse(
  readFileSync(
    path.join(repositoryRoot, "capabilities", "feature-surface-v1.json"),
    "utf8",
  ),
);
const webBindingOperations = bindingOperationsForWeb(capabilityDescriptor);

export function assertRuntimeOwnerEvidence(capabilities, evidence) {
  assertExactStringArray(
    capabilities?.capability_ids,
    evidence?.runtime_capability_ids,
    "runtime capability IDs",
  );
  assertExactStringArray(
    capabilities?.output_ids,
    evidence?.runtime_output_ids,
    "runtime output IDs",
  );
  assertExactStringArray(
    capabilities?.operation_ids,
    expectedWebOperationIds(evidence?.runtime_capability_ids),
    "runtime operation IDs",
  );
}

function assertExactStringArray(actual, expected, label) {
  if (
    !Array.isArray(actual) ||
    !actual.every((item) => typeof item === "string") ||
    !Array.isArray(expected) ||
    !expected.every((item) => typeof item === "string") ||
    JSON.stringify(actual) !== JSON.stringify(expected)
  ) {
    throw new Error(`WASM ${label} do not match their artifact owner evidence.`);
  }
}

export function expectedWebOperationIds(capabilityIds) {
  if (
    !Array.isArray(capabilityIds) ||
    !capabilityIds.every((item) => typeof item === "string")
  ) {
    return null;
  }
  const available = new Set(capabilityIds);
  return webBindingOperations
    .filter(
      (operation) =>
        operation.capability === null || available.has(operation.capability),
    )
    .map((operation) => operation.id)
    .sort(compareNames);
}

function bindingOperationsForWeb(descriptor) {
  if (
    !descriptor ||
    typeof descriptor !== "object" ||
    !Array.isArray(descriptor.binding_operations)
  ) {
    throw new Error("WASM runtime evidence requires canonical binding operations.");
  }
  return descriptor.binding_operations
    .filter(
      (operation) =>
        operation &&
        typeof operation === "object" &&
        Array.isArray(operation.targets) &&
        operation.targets.includes("web"),
    )
    .map((operation) => {
      if (
        typeof operation.id !== "string" ||
        !(
          operation.capability === null ||
          typeof operation.capability === "string"
        )
      ) {
        throw new Error("WASM runtime evidence found an invalid binding operation.");
      }
      return {
        capability: operation.capability,
        id: operation.id,
      };
    });
}

function compareNames(left, right) {
  if (left < right) return -1;
  if (left > right) return 1;
  return 0;
}
