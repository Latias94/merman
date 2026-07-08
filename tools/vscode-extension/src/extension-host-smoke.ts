import * as assert from "node:assert/strict";
import * as path from "node:path";
import * as vscode from "vscode";

const EXTENSION_ID = "latias94.merman-vscode";

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
  const completions = await vscode.commands.executeCommand<vscode.CompletionList>(
    "vscode.executeCompletionItemProvider",
    document.uri,
    new vscode.Position(0, 4),
  );
  assert.ok(
    completions.items.some((item) => completionLabel(item) === "flowchart TD"),
    "expected Mermaid LSP completion items from the packaged language server",
  );
}

function completionLabel(item: vscode.CompletionItem): string {
  return typeof item.label === "string" ? item.label : item.label.label;
}
