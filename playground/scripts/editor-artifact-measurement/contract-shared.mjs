import { createHash } from "node:crypto";

import {
  EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION,
  EDITOR_ARTIFACT_FAMILY_COUNT,
  EDITOR_ARTIFACT_QUERY_KINDS,
} from "./equivalence-shared.mjs";

export {
  EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION,
  EDITOR_ARTIFACT_FAMILY_COUNT,
  EDITOR_ARTIFACT_QUERY_KINDS,
};

export const EDITOR_ARTIFACT_RECEIPT_SCHEMA_VERSION = 2;
export const EDITOR_ARTIFACT_SELECTION_INPUT_SCHEMA_VERSION = 2;
export const DEFAULT_EDITOR_ARTIFACT_RECEIPT_PATH =
  "target/playground/editor-artifact-measurement/receipt-v2.json";
export const CHECKED_EDITOR_ARTIFACT_RECEIPT_PATH =
  "docs/workstreams/web-wasm-playground/editor-artifact-receipt-v2.json";

export const EDITOR_ARTIFACT_VARIANTS = Object.freeze(["full", "editor"]);
export const EDITOR_ARTIFACT_MODES = Object.freeze(["cold", "warm"]);
export const PRIMARY_LATENCY_METRICS = Object.freeze([
  "workerReadyMs",
  "firstDiagnosticsMs",
  "mainFirstResultMs",
]);

export const SECONDARY_LATENCY_METRICS = Object.freeze([
  "mainCompileInitializeMs",
  "workerCompileInitializeMs",
]);
export const ALL_LATENCY_METRICS = Object.freeze([
  ...PRIMARY_LATENCY_METRICS,
  ...SECONDARY_LATENCY_METRICS,
]);
export const MAX_PRIMARY_LATENCY_REGRESSION_RATIO = 0.05;
export const MAX_PRIMARY_LATENCY_REGRESSION_MS = 20;
export const SHA256_PATTERN = /^[a-f0-9]{64}$/u;

export function assertExactRecord(value, keys, label) {
  if (!isRecord(value)) throw new TypeError(`${label} must be an object.`);
  const expected = [...keys].sort();
  const actual = Object.keys(value).sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new TypeError(
      `${label} must contain exactly: ${expected.join(", ")}.`,
    );
  }
  return value;
}

export function cloneOwnedValue(value, label) {
  try {
    return structuredClone(value);
  } catch (error) {
    throw new TypeError(
      `${label} must contain structured-cloneable evidence: ${String(error)}.`,
      { cause: error },
    );
  }
}

export function deepFreeze(value, seen = new WeakSet()) {
  if (value === null || typeof value !== "object" || seen.has(value)) {
    return value;
  }
  // Receipt data is JSON-shaped. Typed arrays are intentionally not part of
  // the receipt, but avoid attempting to freeze them if a future field adds one.
  if (ArrayBuffer.isView(value)) return value;
  seen.add(value);
  for (const member of Object.values(value)) deepFreeze(member, seen);
  return Object.freeze(value);
}

export function expectExactStringArray(value, expected, label) {
  if (
    !Array.isArray(value) ||
    value.length !== expected.length ||
    value.some((entry, index) => entry !== expected[index])
  ) {
    throw new TypeError(`${label} must match the canonical ordered values.`);
  }
}

export function expectIsoDateTime(value, label) {
  const text = expectString(value, label);
  const timestamp = Date.parse(text);
  if (
    !Number.isFinite(timestamp) ||
    new Date(timestamp).toISOString() !== text
  ) {
    throw new TypeError(`${label} must be a canonical ISO date-time.`);
  }
  return text;
}

export function expectString(value, label) {
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(`${label} must be a non-empty string.`);
  }
  return value;
}

export function expectEnum(value, values, label) {
  if (!values.includes(value)) {
    throw new TypeError(`${label} must be one of ${values.join(", ")}.`);
  }
  return value;
}

export function expectSha256(value, label) {
  if (typeof value !== "string" || !SHA256_PATTERN.test(value)) {
    throw new TypeError(`${label} must be a lowercase SHA-256 digest.`);
  }
  return value;
}

export function expectPositiveInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new TypeError(`${label} must be a positive integer.`);
  }
}

export function expectNonNegativeFinite(value, label) {
  if (!Number.isFinite(value) || value < 0) {
    throw new TypeError(`${label} must be a non-negative finite number.`);
  }
}

export function sha256Text(value) {
  return createHash("sha256").update(value).digest("hex");
}

export function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
