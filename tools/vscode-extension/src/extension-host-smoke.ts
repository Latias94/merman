import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import * as vscode from "vscode";

import {
  SEMANTIC_TOKEN_DESCRIPTOR,
  SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
  SEMANTIC_TOKEN_RECORD_WIDTH,
  SEMANTIC_TOKEN_VALID_MODIFIER_MASK,
} from "./generated/token-descriptor.js";
import { assertSemanticTokenLegendProjection } from "./semantic-token-contract.js";

const EXTENSION_ID = "latias94.merman-vscode";
const PROVIDER_TIMEOUT_MS = 15_000;

interface TokenEquivalenceCase {
  readonly id: string;
  readonly family: string;
  readonly source: string;
  readonly packed_words: number[];
}

interface TokenEquivalenceEvidence {
  readonly schema_version: number;
  readonly descriptor_digest: string;
  readonly packed_encoding: string;
  readonly words_per_token: number;
  readonly family_cases: TokenEquivalenceCase[];
  readonly recovery_cases: TokenEquivalenceCase[];
}

const tokenEquivalenceEvidence = JSON.parse(
  fs.readFileSync(
    path.resolve(__dirname, "../../../contracts/editor-language", "token-equivalence-v1.json"),
    "utf8",
  ),
) as TokenEquivalenceEvidence;

export async function run(): Promise<void> {
  const extension = vscode.extensions.getExtension(EXTENSION_ID);
  assert.ok(extension, `expected ${EXTENSION_ID} to be installed in the extension host`);

  await extension.activate();

  const commands = await vscode.commands.getCommands(true);
  for (const command of [
    "merman.restartLanguageServer",
    "merman.openPreview",
    "merman.togglePreviewLock",
    "merman.refreshPreview",
    "merman.showPreviewSource",
    "merman.export",
    "merman.exportSvg",
    "merman.exportPng",
    "merman.copySvg",
    "merman.copyPng",
    "merman.showRuleCatalog",
    "merman.showConfigSchema",
  ]) {
    assert.ok(commands.includes(command), `expected command ${command} to be registered`);
  }

  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
  const fixtureName = workspaceFolder ? path.basename(workspaceFolder.uri.fsPath) : "";
  const merman = vscode.workspace.getConfiguration("merman");
  assert.equal(merman.get("languageIntelligence.enabled"), true);
  assert.equal(merman.get("preview.diagramTheme"), "source");
  assert.equal(merman.get("preview.displayMode"), "svg");
  assert.equal(merman.get("preview.background"), "paper");

  const restartOutcome = await vscode.commands.executeCommand("merman.restartLanguageServer");
  if (fixtureName === "extension-host-lsp-failure") {
    assert.equal(restartOutcome, "failed");
    return;
  }

  assert.equal(restartOutcome, "restarted");
  assert.ok(workspaceFolder, "expected extension-host smoke to run with a workspace folder");
  const document = await vscode.workspace.openTextDocument({
    language: "mermaid",
    content: "flow",
  });
  assert.equal(document.languageId, "mermaid");
  await vscode.window.showTextDocument(document);
  const completions = await vscode.commands.executeCommand<vscode.CompletionList | undefined>(
    "vscode.executeCompletionItemProvider",
    document.uri,
    new vscode.Position(0, 4),
  );
  assert.ok(completions, "expected the packaged language server completion provider");
  assert.ok(
    completions.items.some((item) => completionLabel(item) === "flowchart TD"),
    "expected Mermaid LSP completion items from the packaged language server",
  );

  const source = "flowchart\nsubgraph group\nAlpha --> Beta\nAlpha --> Gamma\nend\n";
  const editorDocument = await vscode.workspace.openTextDocument({
    language: "mermaid",
    content: source,
  });
  await vscode.window.showTextDocument(editorDocument);
  assert.equal(
    vscode.workspace
      .getConfiguration("editor", editorDocument)
      .get("semanticHighlighting.enabled"),
    true,
    "expected the generated Mermaid semantic-highlighting default",
  );
  const version = editorDocument.version;
  const legend = await eventually(
    () =>
      vscode.commands.executeCommand<vscode.SemanticTokensLegend | undefined>(
        "vscode.provideDocumentSemanticTokensLegend",
        editorDocument.uri,
      ),
    (value): value is vscode.SemanticTokensLegend => value !== undefined,
    "expected the generated Merman semantic-token legend",
  );
  assertSemanticTokenLegendProjection(legend);

  const semanticTokens = await semanticTokensFor(editorDocument);
  assert.ok(semanticTokens.data.length > 0, "expected parser-backed semantic tokens from Merman");
  assertPackedTokens(semanticTokens.data, legend);
  const symbols = await vscode.commands.executeCommand<
    Array<vscode.DocumentSymbol | vscode.SymbolInformation> | undefined
  >("vscode.executeDocumentSymbolProvider", editorDocument.uri);
  assert.ok(symbols && symbols.length > 0, "expected symbols from the semantic-token snapshot");
  const hovers = await vscode.commands.executeCommand<vscode.Hover[] | undefined>(
    "vscode.executeHoverProvider",
    editorDocument.uri,
    new vscode.Position(2, 1),
  );
  assert.ok(hovers && hovers.length > 0, "expected hover from the semantic-token snapshot");

  const definitions = await vscode.commands.executeCommand<
    Array<vscode.Location | vscode.LocationLink> | undefined
  >(
    "vscode.executeDefinitionProvider",
    editorDocument.uri,
    new vscode.Position(3, 1),
  );
  assert.ok(
    definitions && definitions.length > 0,
    "expected definition from the semantic-token snapshot",
  );
  const references = await vscode.commands.executeCommand<vscode.Location[] | undefined>(
    "vscode.executeReferenceProvider",
    editorDocument.uri,
    new vscode.Position(2, 1),
  );
  assert.ok(
    references && references.length > 0,
    "expected references from the semantic-token snapshot",
  );
  const rename = await vscode.commands.executeCommand<vscode.WorkspaceEdit | undefined>(
    "vscode.executeDocumentRenameProvider",
    editorDocument.uri,
    new vscode.Position(2, 1),
    "Renamed",
  );
  assert.ok(rename, "expected rename support from the semantic-token snapshot");
  const renameEdits = rename.entries().flatMap(([, edits]) => edits);
  assert.ok(renameEdits.length >= 2, "expected declaration and reference rename edits");
  assert.ok(renameEdits.every((edit) => edit.newText === "Renamed"));

  const directionDiagnostics = await eventually(
    () => Promise.resolve(vscode.languages.getDiagnostics(editorDocument.uri)),
    (value) =>
      value.some(
        (diagnostic) =>
          diagnostic.source === "merman" &&
          diagnosticCode(diagnostic) === "merman.authoring.flowchart.explicit_direction",
      ),
    "expected the snapshot-owned explicit-direction diagnostic",
  );
  const directionDiagnostic = directionDiagnostics.find(
    (diagnostic) =>
      diagnostic.source === "merman" &&
      diagnosticCode(diagnostic) === "merman.authoring.flowchart.explicit_direction",
  )!;
  await eventually(
    () =>
      vscode.commands.executeCommand<Array<vscode.CodeAction | vscode.Command> | undefined>(
        "vscode.executeCodeActionProvider",
        editorDocument.uri,
        directionDiagnostic.range,
        vscode.CodeActionKind.QuickFix.value,
      ),
    (value) =>
      value?.some((action) => action.title === "Insert `TB` into the flowchart header") ?? false,
    "expected quick fix from the diagnostic snapshot",
  );
  assert.equal(editorDocument.version, version, "language queries must not mutate the document");

  assert.equal(tokenEquivalenceEvidence.schema_version, SEMANTIC_TOKEN_DESCRIPTOR.schemaVersion);
  assert.equal(tokenEquivalenceEvidence.descriptor_digest, SEMANTIC_TOKEN_DESCRIPTOR_DIGEST);
  assert.equal(
    tokenEquivalenceEvidence.packed_encoding,
    SEMANTIC_TOKEN_DESCRIPTOR.packed.encoding,
  );
  assert.equal(tokenEquivalenceEvidence.words_per_token, SEMANTIC_TOKEN_RECORD_WIDTH);
  assert.equal(tokenEquivalenceEvidence.family_cases.length, 35);
  assert.equal(tokenEquivalenceEvidence.recovery_cases.length, 1);
  for (const tokenCase of tokenEquivalenceEvidence.family_cases) {
    const familyDocument = await vscode.workspace.openTextDocument({
      language: "mermaid",
      content: tokenCase.source,
    });
    const actual = await semanticTokensFor(familyDocument);
    assertPackedTokens(actual.data, legend);
    assert.deepEqual(
      Array.from(actual.data),
      projectPackedTokens(tokenCase.packed_words, legend),
      `${tokenCase.id} (${tokenCase.family}) VS Code semantic-token projection`,
    );
  }

  const recovery = tokenEquivalenceEvidence.recovery_cases[0]!;
  const recoveryDocument = await vscode.workspace.openTextDocument({
    language: "mermaid",
    content: recovery.source,
  });
  await vscode.window.showTextDocument(recoveryDocument);
  const recoveryTokens = await semanticTokensFor(recoveryDocument);
  assertPackedTokens(recoveryTokens.data, legend);
  assert.deepEqual(
    Array.from(recoveryTokens.data),
    projectPackedTokens(recovery.packed_words, legend),
    "recoverable Flowchart VS Code packed token sequence",
  );
  const recoveryDiagnostics = await eventually(
    () => Promise.resolve(vscode.languages.getDiagnostics(recoveryDocument.uri)),
    (value) => value.some((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error),
    "expected parser diagnostics for the recoverable Flowchart source",
  );
  assert.ok(recoveryDiagnostics.some((diagnostic) => diagnostic.source === "merman"));
}

function completionLabel(item: vscode.CompletionItem): string {
  return typeof item.label === "string" ? item.label : item.label.label;
}

async function semanticTokensFor(document: vscode.TextDocument): Promise<vscode.SemanticTokens> {
  return eventually(
    () =>
      vscode.commands.executeCommand<vscode.SemanticTokens | undefined>(
        "vscode.provideDocumentSemanticTokens",
        document.uri,
      ),
    (value): value is vscode.SemanticTokens => value !== undefined,
    "expected a semantic-token response from Merman",
  );
}

function assertPackedTokens(words: Uint32Array, legend: vscode.SemanticTokensLegend): void {
  assert.equal(words.length % SEMANTIC_TOKEN_RECORD_WIDTH, 0);
  const validModifierMask = legend.tokenModifiers.reduce(
    (mask, _modifier, index) => mask | (1 << index),
    0,
  );
  for (let offset = 0; offset < words.length; offset += SEMANTIC_TOKEN_RECORD_WIDTH) {
    assert.ok(words[offset + 2]! > 0);
    assert.ok(words[offset + 3]! < legend.tokenTypes.length);
    assert.equal(words[offset + 4]! & ~validModifierMask, 0);
  }
}

function projectPackedTokens(
  canonicalWords: readonly number[],
  legend: vscode.SemanticTokensLegend,
): number[] {
  assert.equal(canonicalWords.length % SEMANTIC_TOKEN_RECORD_WIDTH, 0);

  const projectedWords: number[] = [];
  let canonicalLine = 0;
  let canonicalStart = 0;
  let projectedPreviousLine = 0;
  let projectedPreviousStart = 0;

  for (let offset = 0; offset < canonicalWords.length; offset += SEMANTIC_TOKEN_RECORD_WIDTH) {
    const deltaLine = canonicalWords[offset]!;
    canonicalLine += deltaLine;
    canonicalStart =
      deltaLine === 0
        ? canonicalStart + canonicalWords[offset + 1]!
        : canonicalWords[offset + 1]!;
    const length = canonicalWords[offset + 2]!;
    const canonicalTypeCode = canonicalWords[offset + 3]!;
    const canonicalModifierBits = canonicalWords[offset + 4]!;
    assert.equal(canonicalModifierBits & ~SEMANTIC_TOKEN_VALID_MODIFIER_MASK, 0);

    const descriptorType = SEMANTIC_TOKEN_DESCRIPTOR.tokenTypes.find(
      ({ code }) => code === canonicalTypeCode,
    );
    assert.ok(descriptorType, `unknown descriptor token type code ${canonicalTypeCode}`);
    const projectedType = legend.tokenTypes.indexOf(descriptorType.lspName);
    if (projectedType === -1) {
      continue;
    }

    let projectedModifierBits = 0;
    for (const modifier of SEMANTIC_TOKEN_DESCRIPTOR.modifiers) {
      if ((canonicalModifierBits & modifier.bit) === 0) {
        continue;
      }
      const projectedModifier = legend.tokenModifiers.indexOf(modifier.lspName);
      if (projectedModifier !== -1) {
        projectedModifierBits |= 1 << projectedModifier;
      }
    }

    const projectedDeltaLine = canonicalLine - projectedPreviousLine;
    const projectedDeltaStart =
      projectedDeltaLine === 0
        ? canonicalStart - projectedPreviousStart
        : canonicalStart;
    projectedWords.push(
      projectedDeltaLine,
      projectedDeltaStart,
      length,
      projectedType,
      projectedModifierBits,
    );
    projectedPreviousLine = canonicalLine;
    projectedPreviousStart = canonicalStart;
  }

  return projectedWords;
}

function diagnosticCode(diagnostic: vscode.Diagnostic): string | number | undefined {
  const code = diagnostic.code;
  return typeof code === "object" && code !== null ? code.value : code;
}

async function eventually<T, Ready extends T>(
  query: () => PromiseLike<T>,
  ready: (value: T) => value is Ready,
  failure: string,
): Promise<Ready>;
async function eventually<T>(
  query: () => PromiseLike<T>,
  ready: (value: T) => boolean,
  failure: string,
): Promise<T>;
async function eventually<T>(
  query: () => PromiseLike<T>,
  ready: (value: T) => boolean,
  failure: string,
): Promise<T> {
  const deadline = Date.now() + PROVIDER_TIMEOUT_MS;
  let value = await query();
  while (!ready(value) && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 50));
    value = await query();
  }
  assert.ok(ready(value), failure);
  return value;
}
