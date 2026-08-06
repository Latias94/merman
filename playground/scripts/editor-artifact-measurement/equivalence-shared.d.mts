export const EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION: 1;

export const EDITOR_ARTIFACT_QUERY_KINDS: readonly [
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
];

export const EDITOR_ARTIFACT_FAMILY_COUNT: 35;

export function canonicalStringify(value: unknown): string;
export function canonicalize(value: unknown): unknown;
export function compareCanonicalStrings(left: string, right: string): number;
