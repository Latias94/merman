import * as assert from "node:assert/strict";
import { createHash } from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";
import { describe, it } from "node:test";

import {
  EDITOR_RENAME_POLICIES,
  SEMANTIC_TOKEN_DESCRIPTOR,
  SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
  VSCODE_CUSTOM_TOKEN_MODIFIERS,
  VSCODE_CUSTOM_TOKEN_TYPES,
  VSCODE_MERMAID_SEMANTIC_HIGHLIGHTING_ENABLED,
} from "../generated/token-descriptor.js";
import {
  VSCODE_LANGUAGECLIENT_STANDARD_TOKEN_MODIFIERS,
  VSCODE_LANGUAGECLIENT_STANDARD_TOKEN_TYPES,
  VSCODE_LANGUAGECLIENT_DESCRIPTOR_TOKEN_MODIFIERS,
  VSCODE_LANGUAGECLIENT_DESCRIPTOR_TOKEN_TYPES,
  assertLanguageServerEditorContract,
} from "../semantic-token-contract.js";

interface TokenEquivalenceCase {
  source: string;
  source_sha256: string;
  packed_words: number[];
  packed_sha256: string;
  detection_validity: string;
}

interface TokenEquivalenceEvidence {
  generated_by: string;
  source_manifest: string;
  evidence_digest: string;
  schema_version: number;
  descriptor_digest: string;
  packed_encoding: string;
  words_per_token: number;
  family_cases: TokenEquivalenceCase[];
  recovery_cases: TokenEquivalenceCase[];
}

interface MutableInitializeResult {
  capabilities: {
    semanticTokensProvider: {
      legend: {
        tokenTypes: string[];
        tokenModifiers: string[];
      };
    };
    experimental: {
      merman: {
        editorLanguage: {
          schemaVersion: number;
          descriptorDigest: string;
          packedEncoding: string;
          wordsPerToken: number;
          renamePolicies: string[];
        };
      };
    };
  };
}

describe("generated semantic-token contract", () => {
  it("accepts the canonical VS Code legend projection and editor identity", () => {
    assert.doesNotThrow(() =>
      assertLanguageServerEditorContract(validInitializeResult()),
    );
  });

  it("derives the VS Code capability projection from descriptor canonical order", () => {
    const supportedTypes = new Set<string>(VSCODE_LANGUAGECLIENT_STANDARD_TOKEN_TYPES);
    const supportedModifiers = new Set<string>(VSCODE_LANGUAGECLIENT_STANDARD_TOKEN_MODIFIERS);
    assert.deepEqual(
      VSCODE_LANGUAGECLIENT_DESCRIPTOR_TOKEN_TYPES,
      SEMANTIC_TOKEN_DESCRIPTOR.tokenTypes
        .filter(({ lspName }) => supportedTypes.has(lspName))
        .map(({ lspName }) => lspName),
    );
    assert.deepEqual(
      VSCODE_LANGUAGECLIENT_DESCRIPTOR_TOKEN_MODIFIERS,
      SEMANTIC_TOKEN_DESCRIPTOR.modifiers
        .filter(({ lspName }) => supportedModifiers.has(lspName))
        .map(({ lspName }) => lspName),
    );
    assert.deepEqual(
      SEMANTIC_TOKEN_DESCRIPTOR.renamePolicies,
      EDITOR_RENAME_POLICIES,
    );
  });

  it("fails closed on stale contract identity or invalid legend projections", () => {
    const staleDigest = validInitializeResult();
    staleDigest.capabilities.experimental.merman.editorLanguage.descriptorDigest =
      "sha256:stale";
    assert.throws(
      () => assertLanguageServerEditorContract(staleDigest),
      /editor descriptor digest/,
    );

    const reorderedRenamePolicies = validInitializeResult();
    reorderedRenamePolicies.capabilities.experimental.merman.editorLanguage.renamePolicies.reverse();
    assert.throws(
      () => assertLanguageServerEditorContract(reorderedRenamePolicies),
      /rename policies/,
    );

    const reorderedLegend = validInitializeResult();
    reorderedLegend.capabilities.semanticTokensProvider.legend.tokenTypes.reverse();
    assert.throws(
      () => assertLanguageServerEditorContract(reorderedLegend),
      /semantic token types/,
    );

    const duplicateLegendItem = validInitializeResult();
    duplicateLegendItem.capabilities.semanticTokensProvider.legend.tokenTypes.splice(
      1,
      0,
      duplicateLegendItem.capabilities.semanticTokensProvider.legend.tokenTypes[0]!,
    );
    assert.throws(
      () => assertLanguageServerEditorContract(duplicateLegendItem),
      /canonical order/,
    );

    const unknownLegendItem = validInitializeResult();
    unknownLegendItem.capabilities.semanticTokensProvider.legend.tokenTypes.push("unknown");
    assert.throws(
      () => assertLanguageServerEditorContract(unknownLegendItem),
      /outside descriptor/,
    );

    const unnegotiatedLegendItem = validInitializeResult();
    unnegotiatedLegendItem.capabilities.semanticTokensProvider.legend.tokenTypes.splice(
      3,
      0,
      "mermanDelimiter",
    );
    assert.throws(
      () => assertLanguageServerEditorContract(unnegotiatedLegendItem),
      /not declared by vscode-languageclient@10\.1\.0/,
    );

    const unnegotiatedModifier = validInitializeResult();
    unnegotiatedModifier.capabilities.semanticTokensProvider.legend.tokenModifiers.splice(
      2,
      0,
      "mermanReference",
    );
    assert.throws(
      () => assertLanguageServerEditorContract(unnegotiatedModifier),
      /not declared by vscode-languageclient@10\.1\.0/,
    );

    const missingStandardLegendItem = validInitializeResult();
    missingStandardLegendItem.capabilities.semanticTokensProvider.legend.tokenModifiers.pop();
    assert.throws(
      () => assertLanguageServerEditorContract(missingStandardLegendItem),
      /VS Code supported descriptor items/,
    );

    const incompatiblePacking = validInitializeResult();
    incompatiblePacking.capabilities.experimental.merman.editorLanguage.wordsPerToken = 4;
    assert.throws(
      () => assertLanguageServerEditorContract(incompatiblePacking),
      /packed token record width/,
    );
  });

  it("verifies the shared 35-family and malformed-recovery evidence byte for byte", () => {
    const evidencePath = path.join(
      process.cwd(),
      "..",
      "..",
      "editor-language",
      "token-equivalence-v1.json",
    );
    const evidence = JSON.parse(
      fs.readFileSync(evidencePath, "utf8"),
    ) as TokenEquivalenceEvidence;

    assert.equal(evidence.schema_version, 1);
    assert.equal(evidence.descriptor_digest, SEMANTIC_TOKEN_DESCRIPTOR_DIGEST);
    assert.equal(evidence.packed_encoding, SEMANTIC_TOKEN_DESCRIPTOR.packed.encoding);
    assert.equal(evidence.words_per_token, SEMANTIC_TOKEN_DESCRIPTOR.packed.recordWidth);
    assert.equal(evidence.family_cases.length, 35);
    assert.equal(evidence.recovery_cases.length, 1);
    assert.equal(evidence.recovery_cases[0]?.detection_validity, "recoverable-invalid");

    for (const tokenCase of [
      ...evidence.family_cases,
      ...evidence.recovery_cases,
    ]) {
      assert.equal(sha256(tokenCase.source), tokenCase.source_sha256);
      assert.equal(
        sha256(JSON.stringify(tokenCase.packed_words)),
        tokenCase.packed_sha256,
      );
      assert.ok(tokenCase.packed_words.length > 0);
      assert.equal(tokenCase.packed_words.length % evidence.words_per_token, 0);
    }

    const payload = {
      schema_version: evidence.schema_version,
      descriptor_digest: evidence.descriptor_digest,
      packed_encoding: evidence.packed_encoding,
      words_per_token: evidence.words_per_token,
      family_cases: evidence.family_cases,
      recovery_cases: evidence.recovery_cases,
    };
    assert.equal(sha256(JSON.stringify(payload)), evidence.evidence_digest);
  });

  it("projects custom token theming into the VS Code manifest", () => {
    const manifest = JSON.parse(
      fs.readFileSync(path.join(process.cwd(), "package.json"), "utf8"),
    ) as {
      contributes: {
        semanticTokenTypes: unknown;
        semanticTokenModifiers: unknown;
        semanticTokenScopes: unknown;
        configurationDefaults: Record<string, Record<string, unknown>>;
      };
    };
    const expectedTypes = VSCODE_CUSTOM_TOKEN_TYPES.map(
      ({ scopes: _scopes, ...contribution }) => contribution,
    );
    const expectedScopes = Object.fromEntries(
      VSCODE_CUSTOM_TOKEN_TYPES.map(({ id, scopes }) => [id, [...scopes]]),
    );

    assert.deepEqual(manifest.contributes.semanticTokenTypes, expectedTypes);
    assert.deepEqual(
      manifest.contributes.semanticTokenModifiers,
      VSCODE_CUSTOM_TOKEN_MODIFIERS,
    );
    assert.deepEqual(manifest.contributes.semanticTokenScopes, [
      { language: "mermaid", scopes: expectedScopes },
    ]);
    assert.equal(
      manifest.contributes.configurationDefaults["[mermaid]"]?.[
        "editor.semanticHighlighting.enabled"
      ],
      VSCODE_MERMAID_SEMANTIC_HIGHLIGHTING_ENABLED,
    );
  });
});

function validInitializeResult(): MutableInitializeResult {
  return {
    capabilities: {
      semanticTokensProvider: {
        legend: {
          tokenTypes: [...VSCODE_LANGUAGECLIENT_DESCRIPTOR_TOKEN_TYPES],
          tokenModifiers: [...VSCODE_LANGUAGECLIENT_DESCRIPTOR_TOKEN_MODIFIERS],
        },
      },
      experimental: {
        merman: {
          editorLanguage: {
            schemaVersion: SEMANTIC_TOKEN_DESCRIPTOR.schemaVersion,
            descriptorDigest: SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
            packedEncoding: SEMANTIC_TOKEN_DESCRIPTOR.packed.encoding,
            wordsPerToken: SEMANTIC_TOKEN_DESCRIPTOR.packed.recordWidth,
            renamePolicies: [...EDITOR_RENAME_POLICIES],
          },
        },
      },
    },
  };
}

function sha256(value: string): string {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}
