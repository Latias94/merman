import * as vscode from "vscode";
import { LanguageClient, State, StateChangeEvent } from "vscode-languageclient/node";

import { getLanguageIntelligenceSettings } from "./config.js";
import {
  createLanguageClient,
  type LspRuleCatalogEntry,
  fetchConfigSchema,
  fetchRuleCatalog,
  pushConfiguration,
  serverStateLabel,
} from "./server.js";
import { registerSourceCodeLens } from "./codelens.js";
import { registerExport } from "./export.js";
import {
  LANGUAGE_INTELLIGENCE_SETTING,
  languageIntelligenceDisabledMessage,
  shouldStartLanguageClient,
} from "./language-intelligence.js";
import { registerPreview } from "./preview.js";

let client: LanguageClient | undefined;
let statusItem: vscode.StatusBarItem | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  ensureStatusItem(context);
  registerPreview(context);
  registerExport(context);
  registerSourceCodeLens(context);

  context.subscriptions.push({
    dispose: () => {
      void deactivate();
    },
  });

  context.subscriptions.push(
    vscode.commands.registerCommand("merman.restartLanguageServer", async () => {
      if (!languageIntelligenceEnabled()) {
        updateDisabledStatus();
        void vscode.window.showWarningMessage(languageIntelligenceDisabledMessage());
        return;
      }

      await restartClient(context);
      void vscode.window.showInformationMessage("Merman language server restarted.");
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("merman.showRuleCatalog", async () => {
      if (!languageIntelligenceEnabled()) {
        void vscode.window.showWarningMessage(languageIntelligenceDisabledMessage());
        return;
      }
      if (!client) {
        void vscode.window.showWarningMessage("Merman language server is not running.");
        return;
      }
      const response = await fetchRuleCatalog(client);
      await showRuleCatalogPicker(response.rules);
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("merman.showConfigSchema", async () => {
      if (!languageIntelligenceEnabled()) {
        void vscode.window.showWarningMessage(languageIntelligenceDisabledMessage());
        return;
      }
      if (!client) {
        void vscode.window.showWarningMessage("Merman language server is not running.");
        return;
      }
      const response = await fetchConfigSchema(client);
      await showJsonDocument("json", response);
    }),
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration(async (event) => {
      if (!event.affectsConfiguration("merman")) {
        return;
      }

      if (event.affectsConfiguration(LANGUAGE_INTELLIGENCE_SETTING)) {
        await reconcileLanguageClient(context);
        return;
      }

      if (!languageIntelligenceEnabled()) {
        updateDisabledStatus();
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

      if (client) {
        await pushConfiguration(client);
      }
    }),
  );

  await reconcileLanguageClient(context);
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
  await startClient(context, "Restarting language server");
}

async function reconcileLanguageClient(context: vscode.ExtensionContext): Promise<void> {
  if (!languageIntelligenceEnabled()) {
    await deactivate();
    updateDisabledStatus();
    return;
  }

  if (!client) {
    await startClient(context, "Starting language server");
    return;
  }

  await pushConfiguration(client);
}

async function startClient(context: vscode.ExtensionContext, tooltip: string): Promise<void> {
  ensureStatusItem(context);
  client = await createLanguageClient(context);
  wireClientStatus(client);
  updateStatusBar("Starting", tooltip);
  await client.start();
  await pushConfiguration(client);
}

function languageIntelligenceEnabled(): boolean {
  return shouldStartLanguageClient(getLanguageIntelligenceSettings());
}

function updateDisabledStatus(): void {
  updateStatusBar("Disabled", languageIntelligenceDisabledMessage());
}

function wireClientStatus(activeClient: LanguageClient): void {
  updateStatusBar(serverStateLabel(activeClient.state));
  activeClient.onDidChangeState((event: StateChangeEvent) => {
    updateStatusBar(serverStateLabel(event.newState));
    if (event.newState === State.StartFailed) {
      void vscode.window.showErrorMessage("Merman language server failed to start.");
    }
  });
}

function ensureStatusItem(context: vscode.ExtensionContext): void {
  if (statusItem) {
    statusItem.show();
    return;
  }
  statusItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 20);
  statusItem.command = "merman.restartLanguageServer";
  statusItem.show();
  context.subscriptions.push(statusItem);
}

function updateStatusBar(stateLabel: string, tooltip?: string): void {
  if (!statusItem) {
    return;
  }
  statusItem.text = `$(symbol-event) Merman ${stateLabel}`;
  statusItem.tooltip = tooltip ?? "Click to restart the Merman language server";
}

async function showRuleCatalogPicker(rules: LspRuleCatalogEntry[]): Promise<void> {
  const items = rules.map((rule) => ({
    label: rule.id,
    description: `${rule.default_severity} · ${rule.default_profile}`,
    detail: rule.description,
    rule,
  }));
  const picked = await vscode.window.showQuickPick(items, {
    placeHolder: "Select a Merman rule to inspect",
    matchOnDescription: true,
    matchOnDetail: true,
  });
  if (!picked) {
    return;
  }
  await showMarkdownDocument(renderRuleMarkdown(picked.rule));
}

async function showJsonDocument(language: string, payload: unknown): Promise<void> {
  const document = await vscode.workspace.openTextDocument({
    language,
    content: `${JSON.stringify(payload, null, 2)}\n`,
  });
  await vscode.window.showTextDocument(document, {
    preview: false,
  });
}

async function showMarkdownDocument(content: string): Promise<void> {
  const document = await vscode.workspace.openTextDocument({
    language: "markdown",
    content,
  });
  await vscode.window.showTextDocument(document, {
    preview: false,
  });
}

function renderRuleMarkdown(rule: LspRuleCatalogEntry): string {
  const evidence =
    rule.evidence.length > 0
      ? rule.evidence.map((entry) => `- ${entry}`).join("\n")
      : "- None";
  return [
    `# ${rule.id}`,
    "",
    rule.description,
    "",
    `- Default severity: ${rule.default_severity}`,
    `- Default profile: ${rule.default_profile}`,
    `- Category: ${rule.category}`,
    `- Origin: ${rule.origin}`,
    `- Enabled by default: ${rule.default_enabled ? "yes" : "no"}`,
    `- Configurable: ${rule.configurable ? "yes" : "no"}`,
    `- Quickfix available: ${rule.fixable ? "yes" : "no"}`,
    "",
    "## Evidence",
    "",
    evidence,
    "",
  ].join("\n");
}
