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
  assert.equal(
    merman.get("languageIntelligence.enabled"),
    fixtureName === "extension-host-lsp-failure",
  );
  assert.equal(merman.get("preview.diagramTheme"), "source");
  assert.equal(merman.get("preview.displayMode"), "svg");
  assert.equal(merman.get("preview.background"), "paper");

  await vscode.commands.executeCommand("merman.restartLanguageServer");
}
