import {
  SEMANTIC_TOKEN_DESCRIPTOR,
  SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
  SEMANTIC_TOKEN_MODIFIER_LSP_NAMES,
  SEMANTIC_TOKEN_TYPE_LSP_NAMES,
} from "./generated/token-descriptor.js";

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
  const legend = asRecord(provider?.legend);
  assertStringArray(
    legend?.tokenTypes,
    SEMANTIC_TOKEN_TYPE_LSP_NAMES,
    "semantic token types",
  );
  assertStringArray(
    legend?.tokenModifiers,
    SEMANTIC_TOKEN_MODIFIER_LSP_NAMES,
    "semantic token modifiers",
  );

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
}

function assertStringArray(
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

function assertEqual(actual: unknown, expected: unknown, name: string): void {
  if (actual !== expected) {
    throw contractError(
      `${name} does not match descriptor ${SEMANTIC_TOKEN_DESCRIPTOR_DIGEST}`,
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
