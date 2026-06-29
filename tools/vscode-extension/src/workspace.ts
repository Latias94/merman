import * as fs from "node:fs";
import * as path from "node:path";
import * as vscode from "vscode";

export function workspaceRoot(): string | undefined {
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}

export function binaryFileName(baseName: string): string {
  return process.platform === "win32" ? `${baseName}.exe` : baseName;
}

export function findWorkspaceDebugBinary(baseName: string): string | undefined {
  for (const folder of vscode.workspace.workspaceFolders ?? []) {
    const binaryPath = path.join(folder.uri.fsPath, "target", "debug", binaryFileName(baseName));
    if (fs.existsSync(binaryPath)) {
      return binaryPath;
    }
  }
  return undefined;
}
