import { canonicalStringify } from "./equivalence-shared.mjs";
import {
  EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION,
  EDITOR_ARTIFACT_FAMILY_COUNT,
  EDITOR_ARTIFACT_QUERY_KINDS,
  EDITOR_ARTIFACT_VARIANTS,
  assertExactRecord,
  cloneOwnedValue,
  deepFreeze,
  expectEnum,
  expectExactStringArray,
  expectSha256,
  expectString,
  sha256Text,
} from "./contract-shared.mjs";

export function compareEditorArtifactEquivalence(variants) {
  return compareEditorArtifactEquivalenceOwned(
    cloneOwnedValue(variants, "editor semantic-equivalence variants"),
  );
}

export function compareEditorArtifactEquivalenceOwned(variants) {
  assertExactRecord(
    variants,
    EDITOR_ARTIFACT_VARIANTS,
    "editor semantic-equivalence variants",
  );
  const full = validateEquivalenceMatrix(variants.full, "full");
  const editor = validateEquivalenceMatrix(variants.editor, "editor");
  const mismatches = [];

  for (
    let familyIndex = 0;
    familyIndex < full.families.length;
    familyIndex += 1
  ) {
    const fullFamily = full.families[familyIndex];
    const editorFamily = editor.families[familyIndex];
    for (const field of [
      "diagramType",
      "baselineId",
      "fixture",
      "sourceSha256",
    ]) {
      if (fullFamily[field] !== editorFamily[field]) {
        mismatches.push(`families[${familyIndex}].${field}`);
      }
    }
    for (
      let queryIndex = 0;
      queryIndex < fullFamily.queries.length;
      queryIndex += 1
    ) {
      const fullQuery = fullFamily.queries[queryIndex];
      const editorQuery = editorFamily.queries[queryIndex];
      if (fullQuery.kind !== editorQuery.kind) {
        mismatches.push(`families[${familyIndex}].queries[${queryIndex}].kind`);
      }
      if (fullQuery.outcome !== editorQuery.outcome) {
        mismatches.push(
          `families[${familyIndex}].queries[${queryIndex}].outcome`,
        );
      }
      if (fullQuery.sha256 !== editorQuery.sha256) {
        mismatches.push(
          `families[${familyIndex}].queries[${queryIndex}].sha256`,
        );
      }
    }
  }
  if (full.aggregateSha256 !== editor.aggregateSha256) {
    mismatches.push("aggregateSha256");
  }

  return deepFreeze({
    schemaVersion: EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION,
    familyCount: EDITOR_ARTIFACT_FAMILY_COUNT,
    queryCount: EDITOR_ARTIFACT_QUERY_KINDS.length,
    cellCount:
      EDITOR_ARTIFACT_FAMILY_COUNT * EDITOR_ARTIFACT_QUERY_KINDS.length,
    queryKinds: EDITOR_ARTIFACT_QUERY_KINDS,
    variants: Object.freeze({ full, editor }),
    exact: mismatches.length === 0,
    mismatches: Object.freeze(mismatches),
  });
}

function validateEquivalenceMatrix(value, label) {
  const matrix = assertExactRecord(
    value,
    [
      "aggregateSha256",
      "cellCount",
      "families",
      "familyCount",
      "queryCount",
      "queryKinds",
      "schemaVersion",
    ],
    `${label} equivalence matrix`,
  );
  if (matrix.schemaVersion !== EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION) {
    throw new TypeError(`${label} equivalence schemaVersion is invalid.`);
  }
  if (matrix.familyCount !== EDITOR_ARTIFACT_FAMILY_COUNT) {
    throw new TypeError(
      `${label} equivalence matrix must contain ${EDITOR_ARTIFACT_FAMILY_COUNT} families.`,
    );
  }
  if (matrix.queryCount !== EDITOR_ARTIFACT_QUERY_KINDS.length) {
    throw new TypeError(
      `${label} equivalence matrix must contain ${EDITOR_ARTIFACT_QUERY_KINDS.length} query kinds.`,
    );
  }
  if (matrix.cellCount !== matrix.familyCount * matrix.queryCount) {
    throw new TypeError(`${label} equivalence cellCount is inconsistent.`);
  }
  expectExactStringArray(
    matrix.queryKinds,
    EDITOR_ARTIFACT_QUERY_KINDS,
    `${label} equivalence queryKinds`,
  );
  if (
    !Array.isArray(matrix.families) ||
    matrix.families.length !== matrix.familyCount
  ) {
    throw new TypeError(`${label} equivalence families are incomplete.`);
  }

  let previousDiagramType = null;
  for (const [familyIndex, familyValue] of matrix.families.entries()) {
    const family = assertExactRecord(
      familyValue,
      ["baselineId", "diagramType", "fixture", "queries", "sourceSha256"],
      `${label} equivalence family ${familyIndex}`,
    );
    const diagramType = expectString(
      family.diagramType,
      `${label} family ${familyIndex} diagramType`,
    );
    if (previousDiagramType !== null && diagramType <= previousDiagramType) {
      throw new TypeError(
        `${label} equivalence families must be unique and sorted by diagramType.`,
      );
    }
    previousDiagramType = diagramType;
    expectString(
      family.baselineId,
      `${label} family ${diagramType} baselineId`,
    );
    expectString(family.fixture, `${label} family ${diagramType} fixture`);
    expectSha256(
      family.sourceSha256,
      `${label} family ${diagramType} sourceSha256`,
    );
    if (
      !Array.isArray(family.queries) ||
      family.queries.length !== matrix.queryCount
    ) {
      throw new TypeError(
        `${label} family ${diagramType} must contain every query digest.`,
      );
    }
    for (const [queryIndex, queryValue] of family.queries.entries()) {
      const query = assertExactRecord(
        queryValue,
        ["kind", "outcome", "sha256"],
        `${label} family ${diagramType} query ${queryIndex}`,
      );
      if (query.kind !== EDITOR_ARTIFACT_QUERY_KINDS[queryIndex]) {
        throw new TypeError(
          `${label} family ${diagramType} query order is invalid at ${queryIndex}.`,
        );
      }
      expectEnum(
        query.outcome,
        ["error", "result"],
        `${label} family ${diagramType} ${query.kind} outcome`,
      );
      expectSha256(
        query.sha256,
        `${label} family ${diagramType} ${query.kind} sha256`,
      );
    }
  }

  const aggregateSha256 = expectSha256(
    matrix.aggregateSha256,
    `${label} aggregateSha256`,
  );
  const body = {
    schemaVersion: matrix.schemaVersion,
    familyCount: matrix.familyCount,
    queryCount: matrix.queryCount,
    cellCount: matrix.cellCount,
    queryKinds: matrix.queryKinds,
    families: matrix.families,
  };
  const expectedAggregate = sha256Text(canonicalStringify(body));
  if (aggregateSha256 !== expectedAggregate) {
    throw new TypeError(
      `${label} equivalence aggregateSha256 does not bind its matrix.`,
    );
  }
  return matrix;
}

export function validateEquivalenceComparison(value) {
  const comparison = assertExactRecord(
    value,
    [
      "cellCount",
      "exact",
      "familyCount",
      "mismatches",
      "queryCount",
      "queryKinds",
      "schemaVersion",
      "variants",
    ],
    "editor semantic-equivalence comparison",
  );
  if (
    comparison.schemaVersion !== EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION ||
    comparison.familyCount !== EDITOR_ARTIFACT_FAMILY_COUNT ||
    comparison.queryCount !== EDITOR_ARTIFACT_QUERY_KINDS.length ||
    comparison.cellCount !==
      EDITOR_ARTIFACT_FAMILY_COUNT * EDITOR_ARTIFACT_QUERY_KINDS.length
  ) {
    throw new TypeError("Editor semantic-equivalence comparison is invalid.");
  }
  expectExactStringArray(
    comparison.queryKinds,
    EDITOR_ARTIFACT_QUERY_KINDS,
    "editor semantic-equivalence queryKinds",
  );
  if (typeof comparison.exact !== "boolean") {
    throw new TypeError("Editor semantic-equivalence exact must be boolean.");
  }
  if (
    !Array.isArray(comparison.mismatches) ||
    comparison.mismatches.some((entry) => typeof entry !== "string") ||
    comparison.exact !== (comparison.mismatches.length === 0)
  ) {
    throw new TypeError("Editor semantic-equivalence mismatches are invalid.");
  }
  assertExactRecord(
    comparison.variants,
    EDITOR_ARTIFACT_VARIANTS,
    "editor semantic-equivalence compared variants",
  );
  validateEquivalenceMatrix(comparison.variants.full, "full");
  validateEquivalenceMatrix(comparison.variants.editor, "editor");
  // Do not trust a caller-provided `exact` flag or mismatch list. Recompute
  // the comparison from the owned matrices so eligibility is always derived
  // from every family/query digest.
  return compareEditorArtifactEquivalence(comparison.variants);
}
