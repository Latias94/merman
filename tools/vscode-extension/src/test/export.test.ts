import * as assert from "node:assert/strict";
import Module from "node:module";
import { describe, it } from "node:test";
import type * as vscode from "vscode";

type CommandHandler = (target?: unknown) => Promise<void> | void;

interface FakeDisposable {
  dispose(): void;
}

class FakeExportHost {
  readonly commands = new Map<string, CommandHandler>();
  readonly subscriptions: FakeDisposable[] = [];
  readonly renderCalls: Array<{
    source: string;
    format?: string;
    outputPath?: string;
    signalLabel?: string;
  }> = [];
  readonly writtenFiles: Array<{ path: string; data: string; encoding?: string }> = [];
  readonly saveDialogs: vscode.SaveDialogOptions[] = [];
  readonly warnings: string[] = [];
  readonly infos: string[] = [];
  readonly errors: string[] = [];
  readonly clipboardWrites: string[] = [];
  readonly executedCommands: Array<{ command: string; args: unknown[] }> = [];
  readonly progressTitles: string[] = [];
  quickPickLabel: string | undefined;
  renderStdout = '<svg viewBox="0 0 10 10"><a href="#node">ok</a></svg>';
  saveDialogResult: vscode.Uri | undefined = uri(
    "file:///workspace/out.svg",
    "C:\\workspace\\out.svg",
  );

  readonly mermaidDocument = textDocument(
    "file:///workspace/diagram.mmd",
    "C:\\workspace\\diagram.mmd",
    "flowchart TD\nA --> B\n",
  );
  readonly markdownDocument = textDocument(
    "file:///workspace/notes.md",
    "C:\\workspace\\notes.md",
    [
      "# Notes",
      "```mermaid",
      "flowchart TD",
      "A --> B",
      "```",
      "text",
      "```mermaid",
      "sequenceDiagram",
      "A->>B: hi",
      "```",
    ].join("\n"),
    "markdown",
  );
  readonly plainDocument = textDocument(
    "file:///workspace/plain.txt",
    "C:\\workspace\\plain.txt",
    "plain text",
    "plaintext",
  );

  private readonly documents = new Map<string, vscode.TextDocument>();
  private activeEditor: vscode.TextEditor | undefined = textEditor(this.mermaidDocument);

  constructor() {
    for (const document of [
      this.mermaidDocument,
      this.markdownDocument,
      this.plainDocument,
    ]) {
      this.documents.set(document.uri.toString(), document);
    }
  }

  readonly vscode = (() => {
    const host = this;
    return {
      ProgressLocation: {
        Notification: 15,
      },
      Uri: {
        file: (fsPath: string) => uri(`file://${fsPath}`, fsPath),
        parse: (value: string) => uri(value),
      },
      commands: {
        registerCommand: (command: string, handler: CommandHandler) => {
          host.commands.set(command, handler);
          return host.disposable(() => {
            host.commands.delete(command);
          });
        },
        executeCommand: async (command: string, ...args: unknown[]) => {
          host.executedCommands.push({ command, args });
        },
      },
      window: {
        get activeTextEditor() {
          return host.activeEditor;
        },
        createOutputChannel: () => ({
          error: (message: string) => {
            host.errors.push(message);
          },
          dispose: () => {},
        }),
        showSaveDialog: async (options: vscode.SaveDialogOptions) => {
          host.saveDialogs.push(options);
          return host.saveDialogResult;
        },
        showQuickPick: async (items: readonly { label: string }[]) =>
          items.find((item) => item.label === host.quickPickLabel),
        showWarningMessage: async (message: string) => {
          host.warnings.push(message);
          return undefined;
        },
        showInformationMessage: async (message: string) => {
          host.infos.push(message);
          return undefined;
        },
        showErrorMessage: async (message: string) => {
          host.errors.push(message);
          return undefined;
        },
        withProgress: async (
          options: { title?: string },
          task: () => Promise<void>,
        ) => {
          if (options.title) {
            host.progressTitles.push(options.title);
          }
          await task();
        },
      },
      workspace: {
        fs: {
          writeFile: async (fileUri: vscode.Uri, data: Uint8Array) => {
            host.writtenFiles.push({
              path: fileUri.fsPath,
              data: Buffer.from(data).toString("utf8"),
            });
          },
        },
        openTextDocument: async (resource: vscode.Uri) => {
          const document = host.documents.get(resource.toString());
          assert.ok(document, `Unexpected document ${resource.toString()}`);
          return document;
        },
      },
      env: {
        clipboard: {
          writeText: async (value: string) => {
            host.clipboardWrites.push(value);
          },
        },
      },
    };
  })();

  setActiveDocument(document: vscode.TextDocument | undefined): void {
    this.activeEditor = document ? textEditor(document) : undefined;
  }

  private disposable(onDispose: () => void = () => {}): FakeDisposable {
    return {
      dispose: onDispose,
    };
  }
}

describe("export commands", () => {
  it("registers command-level export and copy workflows", () => {
    const host = new FakeExportHost();
    const { registerExport } = loadExportModule(host);

    registerExport({
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    assert.equal(host.commands.has("merman.export"), true);
    assert.equal(host.commands.has("merman.exportSvg"), true);
    assert.equal(host.commands.has("merman.exportPng"), true);
    assert.equal(host.commands.has("merman.copySvg"), true);
    assert.equal(host.commands.has("merman.copyPng"), true);
  });

  it("exports active Mermaid SVG through the save dialog", async () => {
    const host = new FakeExportHost();
    const { registerExport } = loadExportModule(host);

    registerExport({
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.exportSvg")?.();

    assert.deepEqual(host.renderCalls.map(({ source, format, outputPath, signalLabel }) => ({
      source,
      format,
      outputPath,
      signalLabel,
    })), [
      {
        source: "flowchart TD\nA --> B\n",
        format: "svg",
        outputPath: undefined,
        signalLabel: "export-svg",
      },
    ]);
    assert.deepEqual(host.writtenFiles, [
      {
        path: "C:\\workspace\\out.svg",
        data: host.renderStdout,
      },
    ]);
    assert.equal(host.saveDialogs[0]?.saveLabel, "Export SVG");
    assert.deepEqual(host.saveDialogs[0]?.filters, { "SVG image": ["svg"] });
    assert.deepEqual(host.infos, ["Exported out.svg."]);
  });

  it("exports the requested Markdown fence without using the active editor", async () => {
    const host = new FakeExportHost();
    host.setActiveDocument(host.plainDocument);
    host.saveDialogResult = uri("file:///workspace/out.png", "C:\\workspace\\out.png");
    const { registerExport } = loadExportModule(host);

    registerExport({
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.exportPng")?.({
      uri: host.markdownDocument.uri,
      sourceId: "fence-2",
    });

    assert.deepEqual(host.renderCalls.map(({ source, format, outputPath, signalLabel }) => ({
      source,
      format,
      outputPath,
      signalLabel,
    })), [
      {
        source: "sequenceDiagram\nA->>B: hi",
        format: "png",
        outputPath: "C:\\workspace\\out.png",
        signalLabel: "export-png",
      },
    ]);
    assert.deepEqual(host.writtenFiles, []);
    assert.equal(host.saveDialogs[0]?.saveLabel, "Export PNG");
    assert.deepEqual(host.saveDialogs[0]?.filters, { "PNG image": ["png"] });
  });

  it("honors the generic export picker open-after-export preset", async () => {
    const host = new FakeExportHost();
    host.quickPickLabel = "SVG and Open";
    const { registerExport } = loadExportModule(host);

    registerExport({
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.export")?.();

    assert.deepEqual(host.executedCommands, [
      {
        command: "vscode.open",
        args: [host.saveDialogResult],
      },
    ]);
    assert.deepEqual(host.writtenFiles.map((file) => file.path), ["C:\\workspace\\out.svg"]);
  });

  it("copies rendered SVG to the VS Code clipboard", async () => {
    const host = new FakeExportHost();
    const { registerExport } = loadExportModule(host);

    registerExport({
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.copySvg")?.();

    assert.deepEqual(host.clipboardWrites, [host.renderStdout]);
    assert.equal(host.saveDialogs.length, 0);
    assert.deepEqual(host.infos, ["Copied Mermaid SVG to clipboard."]);
  });

  it("warns instead of rendering when no Mermaid source is focused", async () => {
    const host = new FakeExportHost();
    host.setActiveDocument(host.plainDocument);
    const { registerExport } = loadExportModule(host);

    registerExport({
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.exportSvg")?.();

    assert.deepEqual(host.warnings, [
      "Focus a Mermaid file or a Markdown Mermaid fence before exporting.",
    ]);
    assert.deepEqual(host.renderCalls, []);
    assert.deepEqual(host.writtenFiles, []);
  });
});

function loadExportModule(host: FakeExportHost): typeof import("../export.js") {
  type LoadModule = (this: unknown, request: string, parent: unknown, isMain: boolean) => unknown;
  const moduleWithLoad = Module as typeof Module & { _load: LoadModule };
  const originalLoad = moduleWithLoad._load;
  moduleWithLoad._load = function patchedLoad(
    this: unknown,
    request: string,
    parent: unknown,
    isMain: boolean,
  ): unknown {
    if (request === "vscode") {
      return host.vscode;
    }
    if (request === "node:fs/promises") {
      return {
        writeFile: async (filePath: string, data: Buffer | string, encoding?: string) => {
          host.writtenFiles.push({
            path: filePath,
            data: Buffer.isBuffer(data) ? data.toString("utf8") : data,
            encoding,
          });
        },
        mkdtemp: async () => "C:\\workspace\\tmp",
        readFile: async () => Buffer.from("png"),
        rm: async () => {},
      };
    }
    if (request === "./renderer.js") {
      return {
        renderMermanSource: async (renderRequest: {
          source: string;
          format?: string;
          outputPath?: string;
          signalLabel?: string;
        }) => {
          host.renderCalls.push({
            source: renderRequest.source,
            format: renderRequest.format,
            outputPath: renderRequest.outputPath,
            signalLabel: renderRequest.signalLabel,
          });
          return {
            stdout: Buffer.from(host.renderStdout),
            stderr: "",
            invocation: {
              command: "merman-cli",
              args: [],
              source: "test",
            },
          };
        },
      };
    }
    return originalLoad.call(this, request, parent, isMain);
  };
  try {
    delete require.cache[require.resolve("../export.js")];
    delete require.cache[require.resolve("../renderer.js")];
    return require("../export.js") as typeof import("../export.js");
  } finally {
    moduleWithLoad._load = originalLoad;
  }
}

function textEditor(document: vscode.TextDocument): vscode.TextEditor {
  return {
    document,
    selection: {
      active: {
        line: 0,
        character: 0,
      },
    },
  } as unknown as vscode.TextEditor;
}

function textDocument(
  uriValue: string,
  fsPath: string,
  text: string,
  languageId = "mermaid",
): vscode.TextDocument {
  const lines = text.split(/\r?\n/);
  return {
    uri: uri(uriValue, fsPath),
    fileName: fsPath,
    languageId,
    version: 1,
    lineCount: lines.length,
    getText: () => text,
    lineAt: (lineIndex: number) => ({
      text: lines[lineIndex] ?? "",
    }),
  } as unknown as vscode.TextDocument;
}

function uri(value: string, fsPath = value): vscode.Uri {
  return {
    fsPath,
    path: uriPath(value, fsPath),
    toString: () => value,
  } as unknown as vscode.Uri;
}

function uriPath(value: string, fsPath: string): string {
  try {
    return new URL(value).pathname;
  } catch {
    return fsPath.replaceAll("\\", "/");
  }
}
