import * as vscode from "vscode";
import { LanguageClient } from "vscode-languageclient/node";

import { createLanguageClient, pushConfiguration } from "./server.js";

let client: LanguageClient | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  client = await createLanguageClient(context);
  context.subscriptions.push({
    dispose: () => {
      void deactivate();
    },
  });

  context.subscriptions.push(
    vscode.commands.registerCommand("merman.restartLanguageServer", async () => {
      await restartClient(context);
      void vscode.window.showInformationMessage("Merman language server restarted.");
    }),
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration(async (event) => {
      if (!event.affectsConfiguration("merman")) {
        return;
      }
      if (!client) {
        return;
      }

      const serverShapeChanged =
        event.affectsConfiguration("merman.server.path") ||
        event.affectsConfiguration("merman.server.args") ||
        event.affectsConfiguration("merman.server.useCargoRun") ||
        event.affectsConfiguration("merman.server.cargoArgs");

      if (serverShapeChanged) {
        await restartClient(context);
        return;
      }

      await pushConfiguration(client);
    }),
  );

  await client.start();
  await pushConfiguration(client);
}

export async function deactivate(): Promise<void> {
  if (!client) {
    return;
  }
  const activeClient = client;
  client = undefined;
  await activeClient.stop();
}

async function restartClient(context: vscode.ExtensionContext): Promise<void> {
  await deactivate();
  client = await createLanguageClient(context);
  await client.start();
  await pushConfiguration(client);
}
