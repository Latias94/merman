import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  RevealOutputChannelOn,
  StateChangeEvent,
  ServerOptions,
  State,
  Trace,
  TransportKind,
  vsdiag,
} from "vscode-languageclient/node";

import {
  getAnalysisSettings,
  getDiagnosticsSettings,
  getDidChangeConfigurationPayload,
  getServerSettings,
  getTraceSetting,
} from "./config.js";
import { resolveMermanBinary } from "./binaries.js";
import {
  emptyDocumentDiagnosticReport,
  projectOwnedDiagnostics,
  projectOwnedDocumentDiagnosticReport,
} from "./diagnostic-ownership.js";
import { workspaceRoots } from "./workspace.js";

export const RULE_CATALOG_METHOD = "merman/ruleCatalog";
export const CONFIG_SCHEMA_METHOD = "merman/configSchema";

export interface LspRuleCatalogEntry {
  id: string;
  description: string;
  evidence: string[];
  default_severity: string;
  category: string;
  default_enabled: boolean;
  default_profile: string;
  origin: string;
  configurable: boolean;
  fixable: boolean;
}

export interface RuleCatalogResponse {
  version: number;
  rules: LspRuleCatalogEntry[];
}

export interface ConfigSchemaResponse {
  version: number;
  rule_catalog_method: string;
  accepted_roots: string[];
  profiles: string[];
  severities: string[];
  configurable_rule_ids: string[];
  schema: unknown;
}

export async function createLanguageClient(
  context: vscode.ExtensionContext,
): Promise<LanguageClient> {
  const outputChannel = vscode.window.createOutputChannel(
    "Merman Language Server",
    { log: true },
  );
  context.subscriptions.push(outputChannel);

  const serverOptions = await resolveServerOptions(context, outputChannel);
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "mermaid" },
      { scheme: "untitled", language: "mermaid" },
      { scheme: "file", language: "markdown" },
      { scheme: "untitled", language: "markdown" },
      { scheme: "file", language: "mdx" },
      { scheme: "untitled", language: "mdx" },
      { scheme: "file", pattern: "**/*.mdx" },
    ],
    outputChannel,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
    initializationOptions: {
      analysis: getAnalysisSettings(),
    },
    synchronize: {
      fileEvents: [
        vscode.workspace.createFileSystemWatcher("**/*.{mmd,mermaid,md,markdown,mdx}"),
      ],
    },
    markdown: {
      isTrusted: false,
      supportHtml: false,
    },
    middleware: {
      handleDiagnostics(uri, diagnostics, next) {
        next(uri, projectOwnedDiagnostics(diagnostics, getDiagnosticsSettings()));
      },
      async provideDiagnostics(document, previousResultId, token, next) {
        const settings = getDiagnosticsSettings();
        if (!settings.enabled) {
          return emptyDocumentDiagnosticReport() as vsdiag.DocumentDiagnosticReport;
        }
        const report = await next(document, previousResultId, token);
        return report
          ? (projectOwnedDocumentDiagnosticReport(
              report,
              settings,
            ) as vsdiag.DocumentDiagnosticReport)
          : report;
      },
      async provideWorkspaceDiagnostics(resultIds, token, resultReporter, next) {
        if (!getDiagnosticsSettings().enabled) {
          resultReporter(null);
          return { items: [] };
        }
        return next(resultIds, token, resultReporter);
      },
    },
  };

  const client = new LanguageClient(
    "merman-lsp",
    "Merman Language Server",
    serverOptions,
    clientOptions,
  );
  await client.setTrace(toTrace(getTraceSetting()));
  client.onDidChangeState((event: StateChangeEvent) => {
    outputChannel.appendLine(
      `[state] ${event.oldState.toString()} -> ${event.newState.toString()}`,
    );
  });
  return client;
}

export async function pushConfiguration(client: LanguageClient): Promise<void> {
  await client.sendNotification("workspace/didChangeConfiguration", {
    settings: getDidChangeConfigurationPayload(),
  });
}

export async function fetchRuleCatalog(client: LanguageClient): Promise<RuleCatalogResponse> {
  return client.sendRequest<RuleCatalogResponse>(RULE_CATALOG_METHOD);
}

export async function fetchConfigSchema(client: LanguageClient): Promise<ConfigSchemaResponse> {
  return client.sendRequest<ConfigSchemaResponse>(CONFIG_SCHEMA_METHOD);
}

export function serverStateLabel(state: State): string {
  switch (state) {
    case State.Starting:
      return "Starting";
    case State.Running:
      return "Running";
    case State.StartFailed:
      return "Failed";
    case State.Stopped:
    default:
      return "Stopped";
  }
}

async function resolveServerOptions(
  context: vscode.ExtensionContext,
  outputChannel: vscode.OutputChannel,
): Promise<ServerOptions> {
  const settings = getServerSettings();
  const invocation = resolveMermanBinary({
    binaryName: "merman-lsp",
    packageName: "merman-lsp",
    extensionPath: context.extensionUri.fsPath,
    workspaceRoots: workspaceRoots(),
    directArgs: settings.args,
    explicitPath: settings.path,
    useCargoRun: settings.useCargoRun,
    cargoArgs: settings.cargoArgs,
    workspaceTrusted: vscode.workspace.isTrusted,
  });
  outputChannel.appendLine(
    `[server] ${invocation.label}: ${invocation.command} ${invocation.args.join(" ")}`.trim(),
  );
  const options = invocation.cwd ? { cwd: invocation.cwd } : undefined;
  return {
    run: {
      command: invocation.command,
      args: invocation.args,
      options,
      transport: TransportKind.stdio,
    },
    debug: {
      command: invocation.command,
      args: invocation.args,
      options,
      transport: TransportKind.stdio,
    },
  };
}

function toTrace(value: "off" | "messages" | "verbose"): Trace {
  switch (value) {
    case "messages":
      return Trace.Messages;
    case "verbose":
      return Trace.Verbose;
    default:
      return Trace.Off;
  }
}
