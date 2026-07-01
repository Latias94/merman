import * as assert from "node:assert/strict";
import Module from "node:module";
import type * as vscode from "vscode";
import { describe, it } from "node:test";

type CommandHandler = (target?: unknown) => Promise<void> | void;

interface FakeDisposable {
  dispose(): void;
}

interface FakePanel {
  readonly webview: {
    html: string;
    readonly cspSource: string;
    asWebviewUri(uri: { toString(): string }): { toString(): string };
    onDidReceiveMessage(
      listener: (message: unknown) => void,
      thisArg?: unknown,
      disposables?: FakeDisposable[],
    ): FakeDisposable;
    postMessage(message: unknown): Promise<boolean>;
  };
  readonly viewColumn: number;
  visible: boolean;
  title: string;
  reveal(viewColumn: number, preserveFocus?: boolean): void;
  onDidDispose(listener: () => void, thisArg?: unknown, disposables?: FakeDisposable[]): FakeDisposable;
  onDidChangeViewState(
    listener: () => void,
    thisArg?: unknown,
    disposables?: FakeDisposable[],
  ): FakeDisposable;
  dispose(): void;
}

class FakePreviewHost {
  readonly commands = new Map<string, CommandHandler>();
  readonly panels: FakePanel[] = [];
  readonly warnings: string[] = [];
  readonly revealCalls: Array<{ viewColumn: number; preserveFocus?: boolean }> = [];
  readonly showTextDocumentCalls: Array<{ documentUri: string; preserveFocus?: boolean }> = [];
  readonly subscriptions: FakeDisposable[] = [];
  readonly activeDocument = textDocument("file:///workspace/notes.txt", "notes.txt", "plain text", "plaintext");
  readonly targetDocument = textDocument(
    "file:///workspace/example.mmd",
    "example.mmd",
    "flowchart TD\nA --> B\n",
  );
  private readonly disposables: FakeDisposable[] = [];

  readonly vscode = {
    ViewColumn: {
      Beside: 2,
    },
    Uri: {
      file: (fsPath: string) => uri(`file://${fsPath}`, fsPath),
      joinPath: (base: { toString(): string }, ...segments: string[]) =>
        uri(`${base.toString().replace(/\/$/, "")}/${segments.join("/")}`),
      parse: (value: string) => uri(value),
    },
    commands: {
      registerCommand: (command: string, handler: CommandHandler) => {
        this.commands.set(command, handler);
        return this.disposable(() => {
          this.commands.delete(command);
        });
      },
    },
    window: {
      activeTextEditor: textEditor(this.activeDocument),
      visibleTextEditors: [textEditor(this.activeDocument)],
      createOutputChannel: () => ({
        info: () => {},
        error: () => {},
        dispose: () => {},
      }),
      createWebviewPanel: (
        _viewType: string,
        title: string,
        viewOptions: { viewColumn: number; preserveFocus?: boolean },
      ) => {
        assert.equal(viewOptions.viewColumn, 2);
        assert.equal(viewOptions.preserveFocus, true);
        const panel = this.createPanel(title, viewOptions.viewColumn);
        this.panels.push(panel);
        return panel;
      },
      onDidChangeActiveTextEditor: () => this.disposable(),
      onDidChangeTextEditorSelection: () => this.disposable(),
      showTextDocument: async (document: unknown, options: { preserveFocus?: boolean }) => {
        assert.equal(document, this.targetDocument);
        assert.equal(options.preserveFocus, true);
        this.showTextDocumentCalls.push({
          documentUri: this.targetDocument.uri.toString(),
          preserveFocus: options.preserveFocus,
        });
        return textEditor(this.targetDocument);
      },
      showWarningMessage: (message: string) => {
        this.warnings.push(message);
        return Promise.resolve(undefined);
      },
      showInformationMessage: () => Promise.resolve(undefined),
      showErrorMessage: () => Promise.resolve(undefined),
      showSaveDialog: () => Promise.resolve(undefined),
    },
    workspace: {
      openTextDocument: async (resource: { toString(): string }) => {
        assert.equal(resource.toString(), this.targetDocument.uri.toString());
        return this.targetDocument;
      },
      asRelativePath: (target: { toString(): string }) => target.toString(),
      onDidChangeTextDocument: () => this.disposable(),
    },
    languages: {
      getDiagnostics: () => [],
      onDidChangeDiagnostics: () => this.disposable(),
    },
    env: {
      clipboard: {
        writeText: async () => {},
      },
    },
    Range: class {},
    Position: class {},
  };

  private createPanel(title: string, viewColumn: number): FakePanel {
    let disposed = false;
    const disposeListeners: Array<() => void> = [];
    const viewStateListeners: Array<() => void> = [];
    const panel: FakePanel = {
      viewColumn,
      visible: true,
      title,
      webview: {
        html: "",
        cspSource: "vscode-resource:",
        asWebviewUri: (resource) => ({
          toString: () => resource.toString(),
        }),
        onDidReceiveMessage: () => this.disposable(),
        postMessage: async () => true,
      },
      reveal: (column, preserveFocus) => {
        this.revealCalls.push({ viewColumn: column, preserveFocus });
      },
      onDidDispose: (listener, _thisArg, disposables) => {
        disposeListeners.push(listener);
        const disposable = this.disposable();
        disposables?.push(disposable);
        return disposable;
      },
      onDidChangeViewState: (listener, _thisArg, disposables) => {
        viewStateListeners.push(listener);
        const disposable = this.disposable();
        disposables?.push(disposable);
        return disposable;
      },
      dispose: () => {
        if (disposed) {
          return;
        }
        disposed = true;
        for (const listener of disposeListeners) {
          listener();
        }
      },
    };
    void viewStateListeners;
    return panel;
  }

  private disposable(onDispose: () => void = () => {}): FakeDisposable {
    const disposable = {
      dispose: onDispose,
    };
    this.disposables.push(disposable);
    return disposable;
  }
}

describe("preview manager", () => {
  it("registers preview commands and reuses the single preview panel", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host.vscode);
    const target = uri("file:///workspace/example.mmd", "example.mmd");

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    assert.equal(host.commands.has("merman.openPreview"), true);
    assert.equal(host.commands.has("merman.togglePreviewLock"), true);

    await host.commands.get("merman.openPreview")?.(target);
    assert.equal(host.panels.length, 1);
    assert.match(host.panels[0]?.webview.html ?? "", /Merman Preview/);
    assert.deepEqual(host.showTextDocumentCalls, [
      { documentUri: "file:///workspace/example.mmd", preserveFocus: true },
    ]);

    await host.commands.get("merman.openPreview")?.();
    assert.equal(host.panels.length, 1);
    assert.deepEqual(host.revealCalls, [{ viewColumn: 2, preserveFocus: true }]);
  });

  it("warns instead of creating a preview when locking without an instance", () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host.vscode);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    host.commands.get("merman.togglePreviewLock")?.();

    assert.equal(host.panels.length, 0);
    assert.deepEqual(host.warnings, ["Open a Mermaid preview before locking it to a source."]);
  });
});

function loadPreviewModule(vscodeStub: unknown): typeof import("../preview.js") {
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
      return vscodeStub;
    }
    return originalLoad.call(this, request, parent, isMain);
  };
  try {
    delete require.cache[require.resolve("../preview.js")];
    delete require.cache[require.resolve("../preview-instance.js")];
    delete require.cache[require.resolve("../preview-webview-client.js")];
    return require("../preview.js") as typeof import("../preview.js");
  } finally {
    moduleWithLoad._load = originalLoad;
  }
}

function textEditor(document: ReturnType<typeof textDocument>): vscode.TextEditor {
  return {
    document,
    selection: {
      active: {
        line: 0,
      },
    },
  } as unknown as vscode.TextEditor;
}

function textDocument(
  uriValue: string,
  fileName: string,
  text: string,
  languageId = "mermaid",
): vscode.TextDocument {
  const lines = text.split(/\r?\n/);
  return {
    uri: uri(uriValue, fileName),
    fileName,
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
    toString: () => value,
  } as unknown as vscode.Uri;
}
