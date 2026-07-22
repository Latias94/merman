import {
  EDITOR_RENAME_POLICIES,
  SEMANTIC_TOKEN_DESCRIPTOR,
  SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
  SEMANTIC_TOKEN_MODIFIER_LSP_NAMES,
  SEMANTIC_TOKEN_TYPE_LSP_NAMES,
} from "./generated/token-descriptor.js";

// `SemanticTokensFeature.fillClientCapabilities` in vscode-languageclient@10.0.1
// announces these standard LSP names. The extension pins that dependency, so this
// is the client capability against which the server's descriptor projection is
// validated at startup.
export const VSCODE_LANGUAGECLIENT_STANDARD_TOKEN_TYPES = [
  "namespace",
  "type",
  "class",
  "enum",
  "interface",
  "struct",
  "typeParameter",
  "parameter",
  "variable",
  "property",
  "enumMember",
  "event",
  "function",
  "method",
  "macro",
  "keyword",
  "comment",
  "string",
  "number",
  "regexp",
  "operator",
  "decorator",
  "label",
] as const;

export const VSCODE_LANGUAGECLIENT_STANDARD_TOKEN_MODIFIERS = [
  "declaration",
  "definition",
  "readonly",
  "static",
  "deprecated",
  "abstract",
  "async",
  "modification",
  "documentation",
  "defaultLibrary",
] as const;

export const VSCODE_LANGUAGECLIENT_DESCRIPTOR_TOKEN_TYPES = descriptorProjection(
  SEMANTIC_TOKEN_TYPE_LSP_NAMES,
  VSCODE_LANGUAGECLIENT_STANDARD_TOKEN_TYPES,
);

export const VSCODE_LANGUAGECLIENT_DESCRIPTOR_TOKEN_MODIFIERS = descriptorProjection(
  SEMANTIC_TOKEN_MODIFIER_LSP_NAMES,
  VSCODE_LANGUAGECLIENT_STANDARD_TOKEN_MODIFIERS,
);

interface InitializeResultLike {
  capabilities?: {
    semanticTokensProvider?: unknown;
    experimental?: unknown;
  };
}

interface SemanticTokenProviderLike {
  legend?: {
    tokenTypes?: unknown;
    tokenModifiers?: unknown;
  };
}

export function assertLanguageServerEditorContract(
  initializeResult: InitializeResultLike | undefined,
): void {
  const capabilities = initializeResult?.capabilities;
  if (!capabilities) {
    throw contractError("the server returned no initialize capabilities");
  }

  const provider = asRecord(capabilities.semanticTokensProvider) as
    | SemanticTokenProviderLike
    | undefined;
  assertSemanticTokenLegendProjection(provider?.legend);

  const experimental = asRecord(capabilities.experimental);
  const merman = asRecord(experimental?.merman);
  const editorLanguage = asRecord(merman?.editorLanguage);
  assertEqual(
    editorLanguage?.schemaVersion,
    SEMANTIC_TOKEN_DESCRIPTOR.schemaVersion,
    "editor schema",
  );
  assertEqual(
    editorLanguage?.descriptorDigest,
    SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
    "editor descriptor digest",
  );
  assertEqual(
    editorLanguage?.packedEncoding,
    SEMANTIC_TOKEN_DESCRIPTOR.packed.encoding,
    "packed token encoding",
  );
  assertEqual(
    editorLanguage?.wordsPerToken,
    SEMANTIC_TOKEN_DESCRIPTOR.packed.recordWidth,
    "packed token record width",
  );
  assertExactStringArray(
    editorLanguage?.renamePolicies,
    EDITOR_RENAME_POLICIES,
    "rename policies",
  );
}

export function assertSemanticTokenLegendProjection(legend: unknown): void {
  const record = asRecord(legend);
  assertCanonicalDescriptorProjection(
    record?.tokenTypes,
    SEMANTIC_TOKEN_TYPE_LSP_NAMES,
    VSCODE_LANGUAGECLIENT_DESCRIPTOR_TOKEN_TYPES,
    "semantic token types",
  );
  assertCanonicalDescriptorProjection(
    record?.tokenModifiers,
    SEMANTIC_TOKEN_MODIFIER_LSP_NAMES,
    VSCODE_LANGUAGECLIENT_DESCRIPTOR_TOKEN_MODIFIERS,
    "semantic token modifiers",
  );
}

function descriptorProjection(
  descriptorNames: readonly string[],
  supportedNames: readonly string[],
): string[] {
  const supported = new Set(supportedNames);
  return descriptorNames.filter((name) => supported.has(name));
}

function assertCanonicalDescriptorProjection(
  actual: unknown,
  descriptorNames: readonly string[],
  requiredNames: readonly string[],
  name: string,
): void {
  if (!Array.isArray(actual) || actual.some((value) => typeof value !== "string")) {
    throw contractError(
      `${name} are not a string-array descriptor projection ${SEMANTIC_TOKEN_DESCRIPTOR_DIGEST}`,
    );
  }

  let nextDescriptorIndex = 0;
  for (const value of actual) {
    const descriptorIndex = descriptorNames.indexOf(value);
    if (descriptorIndex === -1) {
      throw contractError(
        `${name} contain ${JSON.stringify(value)} outside descriptor ${SEMANTIC_TOKEN_DESCRIPTOR_DIGEST}`,
      );
    }
    if (!requiredNames.includes(value)) {
      throw contractError(
        `${name} contain ${JSON.stringify(value)} not declared by vscode-languageclient@10.0.1`,
      );
    }
    if (descriptorIndex < nextDescriptorIndex) {
      throw contractError(
        `${name} are not in descriptor canonical order ${SEMANTIC_TOKEN_DESCRIPTOR_DIGEST}`,
      );
    }
    nextDescriptorIndex = descriptorIndex + 1;
  }

  const missing = requiredNames.filter((name) => !actual.includes(name));
  if (missing.length > 0) {
    throw contractError(
      `${name} omit VS Code supported descriptor items ${missing.join(", ")} from ${SEMANTIC_TOKEN_DESCRIPTOR_DIGEST}`,
    );
  }
}

function assertEqual(actual: unknown, expected: unknown, name: string): void {
  if (actual !== expected) {
    throw contractError(
      `${name} does not match descriptor ${SEMANTIC_TOKEN_DESCRIPTOR_DIGEST}`,
    );
  }
}

function assertExactStringArray(
  actual: unknown,
  expected: readonly string[],
  name: string,
): void {
  if (
    !Array.isArray(actual) ||
    actual.length !== expected.length ||
    actual.some((value, index) => value !== expected[index])
  ) {
    throw contractError(
      `${name} do not match descriptor ${SEMANTIC_TOKEN_DESCRIPTOR_DIGEST}`,
    );
  }
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return undefined;
  }
  return value as Record<string, unknown>;
}

function contractError(reason: string): Error {
  return new Error(`Merman language server editor contract mismatch: ${reason}.`);
}
