import * as fs from "node:fs";
import * as path from "node:path";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  RevealOutputChannelOn,
  ServerOptions,
  State,
  Trace,
  TransportKind,
} from "vscode-languageclient/node";

import {
  getAnalysisSettings,
  getDidChangeConfigurationPayload,
  getServerSettings,
  getTraceSetting,
} from "./config.js";

export async function createLanguageClient(
  context: vscode.ExtensionContext,
): Promise<LanguageClient> {
  const outputChannel = vscode.window.createOutputChannel(
    "Merman Language Server",
    { log: true },
  );
  context.subscriptions.push(outputChannel);

  const serverOptions = await resolveServerOptions(outputChannel);
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "mermaid" },
      { scheme: "untitled", language: "mermaid" },
      { scheme: "file", language: "markdown" },
      { scheme: "untitled", language: "markdown" },
      { scheme: "file", language: "mdx" },
      { scheme: "untitled", language: "mdx" },
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
      isTrusted: true,
      supportHtml: false,
    },
  };

  const client = new LanguageClient(
    "merman-lsp",
    "Merman Language Server",
    serverOptions,
    clientOptions,
  );
  await client.setTrace(toTrace(getTraceSetting()));
  client.onDidChangeState((event: { oldState: State; newState: State }) => {
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

async function resolveServerOptions(
  outputChannel: vscode.OutputChannel,
): Promise<ServerOptions> {
  const settings = getServerSettings();
  const explicitPath = settings.path;

  if (explicitPath) {
    return createExecutableServerOptions(explicitPath, settings.args, outputChannel);
  }

  if (!settings.useCargoRun) {
    const workspaceBinary = findWorkspaceBinary();
    if (workspaceBinary) {
      return createExecutableServerOptions(workspaceBinary, settings.args, outputChannel);
    }
  }

  return createCargoServerOptions(settings.cargoArgs, settings.args, outputChannel);
}

function createExecutableServerOptions(
  commandPath: string,
  args: string[],
  outputChannel: vscode.OutputChannel,
): ServerOptions {
  outputChannel.appendLine(`[server] executable ${commandPath} ${args.join(" ")}`.trim());
  return {
    run: {
      command: commandPath,
      args,
      transport: TransportKind.stdio,
    },
    debug: {
      command: commandPath,
      args,
      transport: TransportKind.stdio,
    },
  };
}

function createCargoServerOptions(
  cargoArgs: string[],
  serverArgs: string[],
  outputChannel: vscode.OutputChannel,
): ServerOptions {
  const args = ["run", "-p", "merman-lsp", ...cargoArgs, "--", ...serverArgs];
  outputChannel.appendLine(`[server] cargo ${args.join(" ")}`);
  const options = workspaceShellOptions();
  return {
    run: {
      command: "cargo",
      args,
      options,
      transport: TransportKind.stdio,
    },
    debug: {
      command: "cargo",
      args,
      options,
      transport: TransportKind.stdio,
    },
  };
}

function findWorkspaceBinary(): string | undefined {
  for (const folder of vscode.workspace.workspaceFolders ?? []) {
    const binaryPath = path.join(folder.uri.fsPath, "target", "debug", binaryName());
    if (fs.existsSync(binaryPath)) {
      return binaryPath;
    }
  }
  return undefined;
}

function binaryName(): string {
  return process.platform === "win32" ? "merman-lsp.exe" : "merman-lsp";
}

function workspaceShellOptions(): { cwd?: string } {
  return {
    cwd: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
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
