/**
 * Shared, dependency-free helpers for the browser equivalence probe and its
 * Node-side receipt contract. Keeping the canonical form here prevents the
 * browser and receipt verifier from silently hashing different encodings.
 */

export const EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION = 1;

export const EDITOR_ARTIFACT_QUERY_KINDS = Object.freeze([
  "diagnostics",
  "diagramDetection",
  "codeActions",
  "completions",
  "documentSymbols",
  "hover",
  "definition",
  "references",
  "prepareRename",
  "rename",
  "semanticTokens",
]);

export const EDITOR_ARTIFACT_FAMILY_COUNT = 35;

export function canonicalStringify(value) {
  return JSON.stringify(canonicalize(value));
}

export function compareCanonicalStrings(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

export function canonicalize(value) {
  if (value === null) return null;
  if (typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new TypeError("Equivalence values must contain finite numbers.");
    }
    return Object.is(value, -0) ? 0 : value;
  }
  if (value instanceof Uint32Array) return Array.from(value);
  if (Array.isArray(value)) return value.map(canonicalize);
  if (typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .filter((key) => value[key] !== undefined)
        .map((key) => [key, canonicalize(value[key])]),
    );
  }
  throw new TypeError(
    `Equivalence values cannot contain ${typeof value} values.`,
  );
}
