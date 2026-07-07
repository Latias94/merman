import * as vscode from "vscode";

let languageServerOutputChannel: vscode.LogOutputChannel | undefined;

export function ensureLanguageServerOutputChannel(
  context: vscode.ExtensionContext,
): vscode.LogOutputChannel {
  if (languageServerOutputChannel) {
    return languageServerOutputChannel;
  }

  const outputChannel = vscode.window.createOutputChannel(
    "Merman Language Server",
    { log: true },
  );
  languageServerOutputChannel = outputChannel;

  let disposed = false;
  context.subscriptions.push({
    dispose: () => {
      if (disposed) {
        return;
      }
      disposed = true;
      if (languageServerOutputChannel === outputChannel) {
        languageServerOutputChannel = undefined;
      }
      outputChannel.dispose();
    },
  });

  return outputChannel;
}
