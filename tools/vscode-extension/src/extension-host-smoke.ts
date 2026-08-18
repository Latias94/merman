import * as assert from "node:assert/strict";
import * as path from "node:path";
import * as vscode from "vscode";

const EXTENSION_ID = "latias94.merman-vscode";
const PROVIDER_TIMEOUT_MS = 15_000;
const LSP_SEMANTIC_TOKEN_WORDS = 5;

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
  const version = editorDocument.version;
  const legend = await eventually(
    () =>
      vscode.commands.executeCommand<vscode.SemanticTokensLegend | undefined>(
        "vscode.provideDocumentSemanticTokensLegend",
        editorDocument.uri,
      ),
    (value): value is vscode.SemanticTokensLegend => value !== undefined,
    "expected the Merman semantic-token legend",
  );
  assert.ok(legend.tokenTypes.length > 0, "expected at least one standard token type");

  const semanticTokens = await semanticTokensFor(editorDocument);
  assert.ok(semanticTokens.data.length > 0, "expected Tree-sitter syntax tokens from Merman");
  assertPackedTokens(semanticTokens.data, legend);
  const symbols = await vscode.commands.executeCommand<
    Array<vscode.DocumentSymbol | vscode.SymbolInformation> | undefined
  >("vscode.executeDocumentSymbolProvider", editorDocument.uri);
  assert.ok(symbols && symbols.length > 0, "expected parser-backed document symbols");
  const hovers = await vscode.commands.executeCommand<vscode.Hover[] | undefined>(
    "vscode.executeHoverProvider",
    editorDocument.uri,
    new vscode.Position(2, 1),
  );
  assert.ok(hovers && hovers.length > 0, "expected parser-backed hover");

  const definitions = await vscode.commands.executeCommand<
    Array<vscode.Location | vscode.LocationLink> | undefined
  >(
    "vscode.executeDefinitionProvider",
    editorDocument.uri,
    new vscode.Position(3, 1),
  );
  assert.ok(
    definitions && definitions.length > 0,
    "expected parser-backed definition",
  );
  const references = await vscode.commands.executeCommand<vscode.Location[] | undefined>(
    "vscode.executeReferenceProvider",
    editorDocument.uri,
    new vscode.Position(2, 1),
  );
  assert.ok(
    references && references.length > 0,
    "expected parser-backed references",
  );
  const rename = await vscode.commands.executeCommand<vscode.WorkspaceEdit | undefined>(
    "vscode.executeDocumentRenameProvider",
    editorDocument.uri,
    new vscode.Position(2, 1),
    "Renamed",
  );
  assert.ok(rename, "expected parser-backed rename support");
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

  const recoveryDocument = await vscode.workspace.openTextDocument({
    language: "mermaid",
    content: "flowchart TD\n  Before -->\n",
  });
  await vscode.window.showTextDocument(recoveryDocument);
  const recoveryTokens = await semanticTokensFor(recoveryDocument);
  assertPackedTokens(recoveryTokens.data, legend);
  assert.ok(recoveryTokens.data.length > 0, "expected syntax tokens for incomplete Flowchart");
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
    "expected a semantic-token response from the Merman LSP adapter",
  );
}

function assertPackedTokens(words: Uint32Array, legend: vscode.SemanticTokensLegend): void {
  assert.equal(words.length % LSP_SEMANTIC_TOKEN_WORDS, 0);
  const validModifierMask = legend.tokenModifiers.reduce(
    (mask, _modifier, index) => mask | (1 << index),
    0,
  );
  for (let offset = 0; offset < words.length; offset += LSP_SEMANTIC_TOKEN_WORDS) {
    assert.ok(words[offset + 2]! > 0);
    assert.ok(words[offset + 3]! < legend.tokenTypes.length);
    assert.equal(words[offset + 4]! & ~validModifierMask, 0);
  }
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
