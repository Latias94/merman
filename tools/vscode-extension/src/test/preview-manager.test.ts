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
  active: boolean;
  visible: boolean;
  disposed: boolean;
  title: string;
  reveal(viewColumn: number, preserveFocus?: boolean): void;
  receive(message: unknown): Promise<void>;
  setActive(active: boolean): void;
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
  readonly renderCalls: string[] = [];
  readonly warnings: string[] = [];
  readonly revealCalls: Array<{ viewColumn: number; preserveFocus?: boolean }> = [];
  readonly showTextDocumentCalls: Array<{
    documentUri: string;
    preserveFocus?: boolean;
    selection?: unknown;
  }> = [];
  readonly subscriptions: FakeDisposable[] = [];
  readonly activeDocument = textDocument("file:///workspace/notes.txt", "notes.txt", "plain text", "plaintext");
  readonly targetDocument = textDocument(
    "file:///workspace/example.mmd",
    "example.mmd",
    "flowchart TD\nA --> B\n",
  );
  readonly secondDocument = textDocument(
    "file:///workspace/second.mmd",
    "second.mmd",
    "sequenceDiagram\nA->>B: hi\n",
  );
  readonly thirdDocument = textDocument(
    "file:///workspace/third.mmd",
    "third.mmd",
    "stateDiagram-v2\n[*] --> Ready\n",
  );
  private readonly disposables: FakeDisposable[] = [];
  private readonly activeTextEditorListeners: Array<() => void> = [];
  private readonly documents = new Map<string, vscode.TextDocument>();
  private activeEditor = textEditor(this.activeDocument);
  private readonly visibleEditors: vscode.TextEditor[] = [this.activeEditor];

  constructor() {
    for (const document of [
      this.activeDocument,
      this.targetDocument,
      this.secondDocument,
      this.thirdDocument,
    ]) {
      this.documents.set(document.uri.toString(), document);
    }
  }

  readonly vscode = (() => {
    const host = this;
    return {
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
          host.commands.set(command, handler);
          return host.disposable(() => {
            host.commands.delete(command);
          });
        },
      },
      window: {
        get activeTextEditor() {
          return host.activeEditor;
        },
        get visibleTextEditors() {
          return host.visibleEditors;
        },
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
          const panel = host.createPanel(title, viewOptions.viewColumn);
          host.panels.push(panel);
          return panel;
        },
        onDidChangeActiveTextEditor: (listener: () => void) => {
          host.activeTextEditorListeners.push(listener);
          return host.disposable();
        },
        onDidChangeTextEditorSelection: () => host.disposable(),
        showTextDocument: async (
          document: vscode.TextDocument,
          options: { preserveFocus?: boolean; selection?: unknown } = {},
        ) => {
          const editor = textEditor(document);
          if (!host.visibleEditors.some((visible) => sameUri(visible.document.uri, document.uri))) {
            host.visibleEditors.push(editor);
          }
          if (options.preserveFocus !== true) {
            host.activeEditor = editor;
          }
          host.showTextDocumentCalls.push({
            documentUri: document.uri.toString(),
            preserveFocus: options.preserveFocus,
            selection: options.selection,
          });
          return editor;
        },
        showWarningMessage: (message: string) => {
          host.warnings.push(message);
          return Promise.resolve(undefined);
        },
        showInformationMessage: () => Promise.resolve(undefined),
        showErrorMessage: () => Promise.resolve(undefined),
        showSaveDialog: () => Promise.resolve(undefined),
      },
      workspace: {
        openTextDocument: async (resource: { toString(): string }) => {
          const document = host.documents.get(resource.toString());
          assert.ok(document, `Unexpected document ${resource.toString()}`);
          return document;
        },
        asRelativePath: (target: { toString(): string }) => target.toString(),
        onDidChangeTextDocument: () => host.disposable(),
      },
      languages: {
        getDiagnostics: () => [],
        onDidChangeDiagnostics: () => host.disposable(),
      },
      env: {
        clipboard: {
          writeText: async () => {},
        },
      },
      Range: class {
        constructor(
          readonly start: unknown,
          readonly end: unknown,
        ) {}
      },
      Position: class {
        constructor(
          readonly line: number,
          readonly character: number,
        ) {}
      },
    };
  })();

  setActiveDocument(document: vscode.TextDocument): void {
    const editor = textEditor(document);
    if (!this.visibleEditors.some((visible) => sameUri(visible.document.uri, document.uri))) {
      this.visibleEditors.push(editor);
    }
    this.activeEditor = editor;
    for (const listener of this.activeTextEditorListeners) {
      listener();
    }
  }

  private createPanel(title: string, viewColumn: number): FakePanel {
    const disposeListeners: Array<() => void> = [];
    const viewStateListeners: Array<() => void> = [];
    const messageListeners: Array<(message: unknown) => void> = [];
    const panel: FakePanel = {
      viewColumn,
      active: true,
      visible: true,
      disposed: false,
      title,
      webview: {
        html: "",
        cspSource: "vscode-resource:",
        asWebviewUri: (resource) => ({
          toString: () => resource.toString(),
        }),
        onDidReceiveMessage: (listener, _thisArg, disposables) => {
          messageListeners.push(listener);
          const disposable = this.disposable();
          disposables?.push(disposable);
          return disposable;
        },
        postMessage: async () => true,
      },
      reveal: (column, preserveFocus) => {
        this.revealCalls.push({ viewColumn: column, preserveFocus });
      },
      receive: async (message) => {
        for (const listener of messageListeners) {
          listener(message);
        }
        await Promise.resolve();
      },
      setActive: (active) => {
        panel.active = active;
        panel.visible = active;
        for (const listener of viewStateListeners) {
          listener();
        }
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
        if (panel.disposed) {
          return;
        }
        panel.disposed = true;
        for (const listener of disposeListeners) {
          listener();
        }
      },
    };
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
  it("registers preview commands and reuses the follow preview panel", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);
    const target = uri("file:///workspace/example.mmd", "example.mmd");

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    assert.equal(host.commands.has("merman.openPreview"), true);
    assert.equal(host.commands.has("merman.togglePreviewLock"), true);
    assert.equal(host.commands.has("merman.refreshPreview"), true);
    assert.equal(host.commands.has("merman.showPreviewSource"), true);

    await host.commands.get("merman.openPreview")?.(target);
    assert.equal(host.panels.length, 1);
    assert.match(host.panels[0]?.webview.html ?? "", /Merman Preview/);
    assert.deepEqual(host.showTextDocumentCalls.map(({ documentUri, preserveFocus }) => ({
      documentUri,
      preserveFocus,
    })), [
      { documentUri: "file:///workspace/example.mmd", preserveFocus: true },
    ]);

    await host.commands.get("merman.openPreview")?.();
    assert.equal(host.panels.length, 1);
    assert.deepEqual(host.revealCalls, [{ viewColumn: 2, preserveFocus: true }]);
  });

  it("warns instead of creating a preview when locking without an instance", () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    host.commands.get("merman.togglePreviewLock")?.();

    assert.equal(host.panels.length, 0);
    assert.deepEqual(host.warnings, ["Open a Mermaid preview before locking it to a source."]);
  });

  it("opens a new follow preview when the active preview is locked", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await host.commands.get("merman.togglePreviewLock")?.();
    assert.equal(host.panels.length, 1);
    assert.match(host.panels[0]?.title ?? "", /example\.mmd/);

    await host.commands.get("merman.openPreview")?.(host.secondDocument.uri);

    assert.equal(host.panels.length, 2);
    assert.equal(host.panels[0]?.disposed, false);
    assert.equal(host.panels[1]?.disposed, false);
    assert.deepEqual(host.showTextDocumentCalls.map((call) => call.documentUri), [
      "file:///workspace/example.mmd",
      "file:///workspace/second.mmd",
    ]);
  });

  it("keeps follow routing in sync when a preview is locked from the webview", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await host.panels[0]?.receive({ type: "setLocked", locked: true });
    await host.commands.get("merman.openPreview")?.(host.secondDocument.uri);

    assert.equal(host.panels.length, 2);
    assert.equal(host.panels[0]?.disposed, false);
    assert.equal(host.panels[1]?.disposed, false);
  });

  it("routes lock toggles to the active preview instance", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await host.commands.get("merman.togglePreviewLock")?.();
    await host.commands.get("merman.openPreview")?.(host.secondDocument.uri);
    await host.panels[1]?.receive({ type: "ready" });

    host.panels[0]?.setActive(true);
    host.panels[1]?.setActive(false);
    await host.commands.get("merman.togglePreviewLock")?.();

    assert.equal(host.panels[1]?.disposed, true);
    assert.equal(host.panels[0]?.disposed, false);
  });

  it("removes only the disposed instance from manager routing", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await host.commands.get("merman.togglePreviewLock")?.();
    await host.commands.get("merman.openPreview")?.(host.secondDocument.uri);
    assert.equal(host.panels.length, 2);

    host.panels[1]?.dispose();
    await host.commands.get("merman.openPreview")?.(host.thirdDocument.uri);

    assert.equal(host.panels.length, 3);
    assert.equal(host.panels[0]?.disposed, false);
    assert.equal(host.panels[1]?.disposed, true);
    assert.equal(host.panels[2]?.disposed, false);
  });

  it("reveals the active preview source range", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await host.commands.get("merman.showPreviewSource")?.();

    const sourceReveal = host.showTextDocumentCalls.at(-1);
    assert.equal(sourceReveal?.documentUri, "file:///workspace/example.mmd");
    assert.equal(sourceReveal?.preserveFocus, false);
    const selection = sourceReveal?.selection as {
      start?: { line?: number; character?: number };
      end?: { line?: number; character?: number };
    };
    assert.equal(selection.start?.line, 0);
    assert.equal(selection.start?.character, 0);
    assert.equal(selection.end?.line, 2);
  });

  it("forces all previews to render from the refresh command", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await host.commands.get("merman.togglePreviewLock")?.();
    await host.commands.get("merman.openPreview")?.(host.secondDocument.uri);
    await host.panels[1]?.receive({ type: "ready" });
    await flushPreviewWork();
    host.renderCalls.splice(0);

    await host.commands.get("merman.refreshPreview")?.();
    await flushPreviewWork();

    assert.deepEqual(host.renderCalls, [
      "flowchart TD\nA --> B\n",
      "sequenceDiagram\nA->>B: hi\n",
    ]);
  });

  it("forces only the sending preview to render from the webview refresh message", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await host.commands.get("merman.togglePreviewLock")?.();
    await host.commands.get("merman.openPreview")?.(host.secondDocument.uri);
    await host.panels[1]?.receive({ type: "ready" });
    await flushPreviewWork();
    host.renderCalls.splice(0);

    await host.panels[0]?.receive({ type: "refresh" });
    await flushPreviewWork();

    assert.deepEqual(host.renderCalls, ["flowchart TD\nA --> B\n"]);
  });
});

function loadPreviewModule(host: FakePreviewHost): typeof import("../preview.js") {
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
    if (request === "./renderer.js") {
      return {
        renderMermanSource: async (renderRequest: { source: string }) => {
          host.renderCalls.push(renderRequest.source);
          return {
            stdout: Buffer.from("<svg viewBox=\"0 0 10 10\"></svg>"),
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
    delete require.cache[require.resolve("../preview.js")];
    delete require.cache[require.resolve("../preview-instance.js")];
    delete require.cache[require.resolve("../preview-webview-client.js")];
    delete require.cache[require.resolve("../renderer.js")];
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

function sameUri(first: vscode.Uri, second: vscode.Uri): boolean {
  return first.toString() === second.toString();
}

async function flushPreviewWork(): Promise<void> {
  await new Promise((resolve) => setImmediate(resolve));
}

function uri(value: string, fsPath = value): vscode.Uri {
  return {
    fsPath,
    toString: () => value,
  } as unknown as vscode.Uri;
}
