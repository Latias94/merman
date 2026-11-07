import * as assert from "node:assert/strict";
import Module from "node:module";
import type * as vscode from "vscode";
import { describe, it } from "node:test";

import type { PreviewSnapshotMessagePayload } from "../preview-messages.js";
import { legacyThemeDiscovery, type ImportedTheme, type ThemeDiscovery } from "../theme-selection.js";
import { pairedThemeDiscovery } from "./theme-fixtures.js";

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
      listener: (message: unknown) => void | Promise<void>,
      thisArg?: unknown,
      disposables?: FakeDisposable[],
    ): FakeDisposable;
    postMessage(message: unknown): Promise<boolean>;
  };
  readonly viewColumn: number;
  active: boolean;
  visible: boolean;
  disposed: boolean;
  postedMessages: unknown[];
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

interface DeferredRender {
  promise: Promise<void>;
  release(): void;
}

class FakePreviewHost {
  readonly commands = new Map<string, CommandHandler>();
  readonly panels: FakePanel[] = [];
  readonly renderCalls: Array<{ source: string; format?: string; outputPath?: string; background?: string; theme?: string; themePreset?: string; themeAppearance?: string; themeFileContent?: string }> = [];
  readonly renderSignals: AbortSignal[] = [];
  readonly clipboardWrites: string[] = [];
  readonly writtenFiles: Array<{ path: string; data: string }> = [];
  readonly webviewOptions: Array<{
    enableCommandUris?: boolean;
    enableScripts?: boolean;
    localResourceRoots?: Array<{ toString(): string }>;
    retainContextWhenHidden?: boolean;
  }> = [];
  readonly warnings: string[] = [];
  readonly errors: string[] = [];
  readonly informationMessages: string[] = [];
  readonly outputErrors: string[] = [];
  readonly revealCalls: Array<{ viewColumn: number; preserveFocus?: boolean }> = [];
  readonly showTextDocumentCalls: Array<{
    documentUri: string;
    preserveFocus?: boolean;
    selection?: unknown;
  }> = [];
  readonly subscriptions: FakeDisposable[] = [];
  readonly codeLensProviders: vscode.CodeLensProvider[] = [];
  private saveDialogCounter = 0;
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
  private readonly colorThemeListeners: Array<(theme: { kind: number }) => void> = [];
  activeColorThemeKind = 1;
  themeDiscovery: ThemeDiscovery = legacyThemeDiscovery();
  saveDialogDelay: Promise<void> | undefined;
  importDelay: Promise<void> | undefined;
  importError: Error | undefined;
  imported: ImportedTheme = {
    content: '{"schema_version":1,"kind":"design","design":{"tokens":{"private":{"type":"paint","value":"#123456","description":"raw-content"}},"canvas":{"base":{"ref":"private"}},"appearances":{"light":{},"dark":{}},"default_appearance":"light"}}',
    label: "reading.theme.json",
    supportedAppearances: ["light", "dark"],
    defaultAppearance: "light",
  };
  readonly themeFiles: string[] = [];
  private readonly textDocumentChangeListeners: Array<(event: { document: vscode.TextDocument }) => void> = [];
  private readonly textDocumentCloseListeners: Array<(document: vscode.TextDocument) => void> = [];
  private readonly documents = new Map<string, vscode.TextDocument>();
  private readonly deferredDocumentOpens = new Map<string, { promise: Promise<void>; release: () => void }>();
  private readonly deferredRenders: DeferredRender[] = [];
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
        get activeColorTheme() {
          return { kind: host.activeColorThemeKind };
        },
        onDidChangeActiveColorTheme: (listener: (theme: { kind: number }) => void) => {
          host.colorThemeListeners.push(listener);
          return host.disposable();
        },
        get activeTextEditor() {
          return host.activeEditor;
        },
        get visibleTextEditors() {
          return host.visibleEditors;
        },
        createOutputChannel: () => ({
          info: () => {},
          error: (message: string) => {
            host.outputErrors.push(message);
          },
          dispose: () => {},
        }),
        createWebviewPanel: (
          _viewType: string,
          title: string,
          viewOptions: { viewColumn: number; preserveFocus?: boolean },
          webviewOptions: {
            enableCommandUris?: boolean;
            enableScripts?: boolean;
            localResourceRoots?: Array<{ toString(): string }>;
            retainContextWhenHidden?: boolean;
          },
        ) => {
          assert.equal(viewOptions.viewColumn, 2);
          assert.equal(viewOptions.preserveFocus, true);
          host.webviewOptions.push(webviewOptions);
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
        showInformationMessage: (message: string) => {
          host.informationMessages.push(message);
          return Promise.resolve(undefined);
        },
        showErrorMessage: (message: string) => {
          host.errors.push(message);
          return Promise.resolve(undefined);
        },
        showSaveDialog: async (options?: { defaultUri?: vscode.Uri; filters?: Record<string, string[]> }) => {
          await host.saveDialogDelay;
          this.saveDialogCounter += 1;
          const extension = saveDialogExtension(options);
          if (this.saveDialogCounter === 1) {
            return Promise.resolve(
              uri(
                `vscode-remote://ssh-remote+linux/c%3A/Users/frank/export-one.${extension}`,
                `C:\\Users\\frank\\export-one.${extension}`,
              ),
            );
          }
          return Promise.resolve(
            uri(
              `file:///workspace/export-${this.saveDialogCounter}.${extension}`,
              `/workspace/export-${this.saveDialogCounter}.${extension}`,
            ),
          );
        },
        showOpenDialog: async () => [uri("file:///private/reading.theme.json", "/private/reading.theme.json")],
      },
      workspace: {
        getWorkspaceFolder: (resource: { toString(): string }) => ({
          uri: uri(resource.toString().split("/").slice(0, 4).join("/")),
        }),
        getConfiguration: () => ({
          get: (key: string, fallback: unknown) => {
            switch (key) {
              case "preview.diagramTheme":
                return "source";
              case "preview.displayMode":
                return "svg";
              case "preview.background":
                return "paper";
              case "languageIntelligence.enabled":
                return false;
              default:
                return fallback;
            }
          },
        }),
        fs: {
          writeFile: async (fileUri: vscode.Uri, data: Uint8Array) => {
            host.writtenFiles.push({
              path: fileUri.fsPath,
              data: Buffer.from(data).toString("utf8"),
            });
          },
        },
        openTextDocument: async (resource: { toString(): string }) => {
          const document = host.documents.get(resource.toString());
          assert.ok(document, `Unexpected document ${resource.toString()}`);
          const deferred = host.deferredDocumentOpens.get(resource.toString());
          if (deferred) {
            await deferred.promise;
            host.deferredDocumentOpens.delete(resource.toString());
          }
          return document;
        },
        asRelativePath: (target: { toString(): string }) => target.toString(),
        onDidChangeTextDocument: (listener: (event: { document: vscode.TextDocument }) => void) => {
          host.textDocumentChangeListeners.push(listener);
          return host.disposable();
        },
        onDidCloseTextDocument: (listener: (document: vscode.TextDocument) => void) => {
          host.textDocumentCloseListeners.push(listener);
          return host.disposable();
        },
        onDidChangeConfiguration: () => host.disposable(),
      },
      languages: {
        getDiagnostics: () => [],
        onDidChangeDiagnostics: () => host.disposable(),
        registerCodeLensProvider: (_selector: unknown, provider: vscode.CodeLensProvider) => {
          host.codeLensProviders.push(provider);
          return host.disposable();
        },
      },
      env: {
        clipboard: {
          writeText: async (value: string) => {
            host.clipboardWrites.push(value);
          },
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
      CodeLens: class {
        constructor(readonly range: unknown, readonly command: vscode.Command) {}
      },
      EventEmitter: class {
        readonly event = () => host.disposable();
        fire(): void {}
        dispose(): void {}
      },
    };
  })();

  setActiveDocument(document: vscode.TextDocument): void {
    const editor = textEditor(document);
    const existingIndex = this.visibleEditors.findIndex((visible) => sameUri(visible.document.uri, document.uri));
    if (existingIndex < 0) {
      this.visibleEditors.push(editor);
    } else {
      this.visibleEditors[existingIndex] = editor;
    }
    this.documents.set(document.uri.toString(), document);
    this.activeEditor = editor;
    for (const listener of this.activeTextEditorListeners) {
      listener();
    }
  }

  fireDocumentChange(document: vscode.TextDocument): void {
    for (const listener of this.textDocumentChangeListeners) {
      listener({ document });
    }
  }

  changeColorTheme(kind: number): void {
    this.activeColorThemeKind = kind;
    for (const listener of this.colorThemeListeners) {
      listener({ kind });
    }
  }

  fireDocumentClose(document: vscode.TextDocument): void {
    for (const listener of this.textDocumentCloseListeners) {
      listener(document);
    }
  }

  deferOpenTextDocument(documentUri: vscode.Uri): () => void {
    let release: () => void = () => {};
    const promise = new Promise<void>((resolve) => {
      release = resolve;
    });
    this.deferredDocumentOpens.set(documentUri.toString(), { promise, release });
    return release;
  }

  deferNextRender(): () => void {
    let release: () => void = () => {};
    const promise = new Promise<void>((resolve) => {
      release = resolve;
    });
    this.deferredRenders.push({ promise, release });
    return release;
  }

  takeDeferredRender(): DeferredRender | undefined {
    return this.deferredRenders.shift();
  }

  private createPanel(title: string, viewColumn: number): FakePanel {
    const disposeListeners: Array<() => void> = [];
    const viewStateListeners: Array<() => void> = [];
    const messageListeners: Array<(message: unknown) => void | Promise<void>> = [];
    const panel: FakePanel = {
      viewColumn,
      active: true,
      visible: true,
      disposed: false,
      postedMessages: [],
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
        postMessage: async (message) => {
          panel.postedMessages.push(message);
          return true;
        },
      },
      reveal: (column, preserveFocus) => {
        this.revealCalls.push({ viewColumn: column, preserveFocus });
      },
      receive: async (message) => {
        await Promise.all(messageListeners.map((listener) => listener(message)));
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
  it("keeps a user-imported recipe when slower configured-file initialization finishes later", async () => {
    const host = new FakePreviewHost();
    host.themeDiscovery = pairedThemeDiscovery();
    host.vscode.workspace.getConfiguration = () => ({
      get: (key: string, fallback: unknown) => key === "preview.diagramTheme" ? "file"
        : key === "preview.themeFile" ? "themes/reading.theme.json" : fallback,
    });
    host.imported = { ...host.imported, content: "configured-old-content" };
    let releaseConfigured: () => void = () => {};
    host.importDelay = new Promise<void>((resolve) => { releaseConfigured = resolve; });
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    const opening = host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await flushPreviewWork();
    assert.equal(host.themeFiles.length, 1);
    host.importDelay = undefined;
    host.imported = { ...host.imported, content: "user-picked-new-content" };
    await host.panels[0]?.receive({ type: "importTheme" });
    await flushPreviewWork();
    releaseConfigured();
    await opening;
    await host.panels[0]?.receive({ type: "ready" });
    await flushPreviewWork();
    assert.equal(host.themeFiles.length, 2);
    assert.equal(host.renderCalls.at(-1)?.themeFileContent, "user-picked-new-content");
    for (const subscription of host.subscriptions) subscription.dispose();
  });

  it("resolves configured recipe paths from an explicit open target in another workspace", async () => {
    const host = new FakePreviewHost();
    host.themeDiscovery = pairedThemeDiscovery();
    const active = textDocument("file:///workspace-one/active.mmd", "/workspace-one/active.mmd", "flowchart TD\nA-->B");
    const target = textDocument("file:///workspace-two/target.mmd", "/workspace-two/target.mmd", "sequenceDiagram\nA->>B: hi");
    host.setActiveDocument(target);
    host.setActiveDocument(active);
    host.vscode.workspace.getConfiguration = () => ({
      get: (key: string, fallback: unknown) => key === "preview.diagramTheme" ? "file"
        : key === "preview.themeFile" ? "themes/reading.theme.json" : fallback,
    });
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(target.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await flushPreviewWork();
    assert.equal(lastRenderedSnapshot(host.panels[0]).documentUri, target.uri.toString());
    assert.deepEqual(host.themeFiles, ["file:///workspace-two/themes/reading.theme.json"]);
    for (const subscription of host.subscriptions) subscription.dispose();
  });

  it("updates Follow fallback guidance without repainting a single-appearance theme", async () => {
    const host = new FakePreviewHost();
    host.themeDiscovery = pairedThemeDiscovery();
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    const panel = host.panels[0]!;
    await panel.receive({ type: "ready" });
    await panel.receive({ type: "setDiagramTheme", theme: "preset:ghibli" });
    await flushPreviewWork();
    const count = host.renderCalls.length;
    host.changeColorTheme(2);
    await flushPreviewWork();
    const settings = panel.postedMessages.filter((message) => (message as { type: string }).type === "settingsUpdated").at(-1) as { snapshot: PreviewSnapshotMessagePayload };
    assert.match(settings.snapshot.themeNotice!, /does not provide dark/);
    assert.equal(host.renderCalls.length, count);
    for (const subscription of host.subscriptions) subscription.dispose();
  });

  it("does not surface a rejected import after a newer selection superseded it", async () => {
    const host = new FakePreviewHost();
    host.themeDiscovery = pairedThemeDiscovery();
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    const panel = host.panels[0]!;
    await panel.receive({ type: "ready" });
    let releaseImport: () => void = () => {};
    host.importDelay = new Promise<void>((resolve) => { releaseImport = resolve; });
    host.importError = new Error("obsolete import rejection");
    await panel.receive({ type: "importTheme" });
    await flushPreviewWork();
    await panel.receive({ type: "setDiagramTheme", theme: "preset:editor" });
    releaseImport();
    await flushPreviewWork();
    assert.equal(lastRenderedSnapshot(panel).diagramTheme, "preset:editor");
    assert.equal(host.errors.length, 0);
    for (const subscription of host.subscriptions) subscription.dispose();
  });

  it("admits imports atomically, rejects delayed adoption, and never sends recipe content or paths to the webview", async () => {
    const host = new FakePreviewHost();
    host.themeDiscovery = pairedThemeDiscovery();
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    const panel = host.panels[0]!;
    await panel.receive({ type: "ready" });
    await flushPreviewWork();
    host.importError = new Error("invalid inactive dark appearance");
    await panel.receive({ type: "importTheme" });
    await flushPreviewWork();
    assert.equal(lastRenderedSnapshot(panel).diagramTheme, "source");
    assert.match(host.errors.at(-1)!, /invalid inactive/);

    host.importError = undefined;
    let releaseImport: () => void = () => {};
    host.importDelay = new Promise<void>((resolve) => { releaseImport = resolve; });
    await panel.receive({ type: "importTheme" });
    await flushPreviewWork();
    await panel.receive({ type: "setDiagramTheme", theme: "preset:editor" });
    releaseImport();
    await flushPreviewWork();
    assert.equal(lastRenderedSnapshot(panel).diagramTheme, "preset:editor");

    host.importDelay = undefined;
    await panel.receive({ type: "importTheme" });
    await flushPreviewWork();
    assert.equal(lastRenderedSnapshot(panel).diagramTheme, "file");
    assert.equal(host.renderCalls.at(-1)?.themeFileContent, host.imported.content);
    const wire = JSON.stringify(panel.postedMessages);
    assert.equal(wire.includes("/private/reading.theme.json"), false);
    assert.equal(wire.includes("raw-content"), false);
    assert.equal(wire.includes("reading.theme.json"), true);
    for (const subscription of host.subscriptions) subscription.dispose();
  });

  it("keeps identical theme reimports ready for copy and SVG/PNG export", async () => {
    const host = new FakePreviewHost();
    host.themeDiscovery = pairedThemeDiscovery();
    host.activeColorThemeKind = 2;
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    const panel = host.panels[0]!;
    await panel.receive({ type: "ready" });
    await panel.receive({ type: "importTheme" });
    await flushPreviewWork();
    const sourceKey = lastRenderedSourceKey(panel);
    const renderCount = host.renderCalls.length;
    panel.postedMessages.splice(0);

    await panel.receive({ type: "importTheme" });
    await flushPreviewWork();

    assert.equal(host.themeFiles.length, 2);
    assert.equal(host.renderCalls.length, renderCount);
    assert.deepEqual(panel.postedMessages, []);
    const svg = '<svg viewBox="0 0 10 10"></svg>';
    await panel.receive({ type: "copySvg", svg, sourceKey });
    await panel.receive({ type: "exportRendered", format: "svg", sourceKey });
    await flushPreviewWork();
    await panel.receive({ type: "exportRendered", format: "png", sourceKey });
    await flushPreviewWork();

    assert.deepEqual(host.clipboardWrites, [svg]);
    assert.equal(host.writtenFiles.length, 1);
    assert.deepEqual(host.renderCalls.slice(renderCount).map((call) => [
      call.format, call.themeFileContent, call.themeAppearance,
    ]), [
      ["svg", host.imported.content, "dark"],
      ["svg", host.imported.content, "dark"],
      ["png", undefined, undefined],
    ]);
    assert.deepEqual(host.informationMessages.slice(-2), ["Exported export-one.svg.", "Exported export-2.png."]);
    assert.deepEqual(host.warnings, []);
    assert.deepEqual(host.errors, []);
    for (const subscription of host.subscriptions) subscription.dispose();
  });

  it("updates a reimported theme name without discarding its ready output or replay cache", async () => {
    const host = new FakePreviewHost();
    host.themeDiscovery = pairedThemeDiscovery();
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    const panel = host.panels[0]!;
    await panel.receive({ type: "ready" });
    await panel.receive({ type: "importTheme" });
    await flushPreviewWork();
    const sourceKey = lastRenderedSourceKey(panel);
    const renderCount = host.renderCalls.length;
    panel.postedMessages.splice(0);
    host.imported = { ...host.imported, label: "Renamed reading.theme.json" };

    await panel.receive({ type: "importTheme" });
    await flushPreviewWork();

    assert.equal(host.renderCalls.length, renderCount);
    assert.deepEqual(panel.postedMessages.map((message) => (message as { type: string }).type), ["settingsUpdated"]);
    const settings = panel.postedMessages[0] as { snapshot: PreviewSnapshotMessagePayload };
    assert.equal(settings.snapshot.themeLabel, host.imported.label);
    assert.equal(settings.snapshot.themeChoices?.find((choice) => choice.id === "file")?.label, host.imported.label);
    assert.deepEqual(settings.snapshot.sourceKey, sourceKey);
    await panel.receive({ type: "ready" });
    await flushPreviewWork();
    assert.equal(host.renderCalls.length, renderCount);
    assert.equal(lastRenderedSnapshot(panel).themeLabel, host.imported.label);
    await panel.receive({ type: "copySvg", svg: "<svg></svg>", sourceKey });
    await panel.receive({ type: "exportRendered", format: "svg", sourceKey });
    await flushPreviewWork();
    assert.deepEqual(host.clipboardWrites, ["<svg></svg>"]);
    assert.equal(host.writtenFiles.length, 1);
    assert.deepEqual(host.warnings, []);
    for (const subscription of host.subscriptions) subscription.dispose();
  });

  it("recovers an invalidated theme reimport when an edit outside the active fence leaves render inputs unchanged", async () => {
    const host = new FakePreviewHost();
    host.themeDiscovery = pairedThemeDiscovery();
    const original = textDocument(
      "file:///workspace/notes.md", "notes.md", "Before\n```mermaid\nflowchart LR\nA --> B\n```\n", "markdown", 1,
    );
    host.setActiveDocument(original);
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(original.uri);
    const panel = host.panels[0]!;
    await panel.receive({ type: "ready" });
    await panel.receive({ type: "importTheme" });
    await flushPreviewWork();
    const sourceKey = lastRenderedSourceKey(panel);
    const renderCount = host.renderCalls.length;
    panel.postedMessages.splice(0);
    const edited = textDocument(
      "file:///workspace/notes.md", "notes.md", "After\n```mermaid\nflowchart LR\nA --> B\n```\n", "markdown", 2,
    );
    host.setActiveDocument(edited);
    host.fireDocumentChange(edited);

    await panel.receive({ type: "importTheme" });
    await flushPreviewWork();

    assert.equal(host.renderCalls.length, renderCount + 1);
    assert.deepEqual(lastRenderedSourceKey(panel), sourceKey);
    assert.equal((panel.postedMessages.at(-1) as { type: string }).type, "renderSucceeded");
    await panel.receive({ type: "copySvg", svg: "<svg></svg>", sourceKey });
    await panel.receive({ type: "exportRendered", format: "svg", sourceKey });
    await flushPreviewWork();
    assert.deepEqual(host.clipboardWrites, ["<svg></svg>"]);
    assert.equal(host.writtenFiles.length, 1);
    assert.deepEqual(host.warnings, []);
    for (const subscription of host.subscriptions) subscription.dispose();
  });

  it("invalidates changed recipe output and keeps the applied recipe usable after a rejected import", async () => {
    const host = new FakePreviewHost();
    host.themeDiscovery = pairedThemeDiscovery();
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    const panel = host.panels[0]!;
    await panel.receive({ type: "ready" });
    await panel.receive({ type: "importTheme" });
    await flushPreviewWork();
    const previousKey = lastRenderedSourceKey(panel);
    const renderCount = host.renderCalls.length;
    host.imported = {
      ...host.imported,
      content: '{"schema_version":1,"kind":"design","design":{"canvas":{"base":"#654321"},"appearances":{"light":{},"dark":{}},"default_appearance":"light"}}',
    };
    const applied = host.imported.content;
    const releaseRender = host.deferNextRender();
    panel.postedMessages.splice(0);
    await panel.receive({ type: "importTheme" });
    await flushPreviewWork();

    assert.equal(host.renderCalls.length, renderCount + 1);
    assert.equal(host.renderCalls.at(-1)?.themeFileContent, applied);
    assert.ok(panel.postedMessages.some((message) => (message as { type: string }).type === "renderInvalidated"));
    assert.equal(panel.postedMessages.some((message) => (message as { type: string }).type === "renderSucceeded"), false);
    await panel.receive({ type: "copySvg", svg: "<svg></svg>", sourceKey: previousKey });
    await panel.receive({ type: "exportRendered", format: "svg", sourceKey: previousKey });
    assert.deepEqual(host.clipboardWrites, []);
    assert.equal(host.writtenFiles.length, 0);
    assert.equal(host.warnings.length, 2);
    releaseRender();
    await flushPreviewWork();
    const sourceKey = lastRenderedSourceKey(panel);
    assert.notDeepEqual(sourceKey, previousKey);
    panel.postedMessages.splice(0);
    host.importError = new Error("invalid inactive dark appearance");

    await panel.receive({ type: "importTheme" });
    await flushPreviewWork();

    assert.equal(host.renderCalls.length, renderCount + 1);
    assert.deepEqual(panel.postedMessages, []);
    assert.match(host.errors.at(-1)!, /invalid inactive dark appearance/);
    await panel.receive({ type: "copySvg", svg: "<svg></svg>", sourceKey });
    await panel.receive({ type: "exportRendered", format: "svg", sourceKey });
    await flushPreviewWork();
    assert.deepEqual(host.clipboardWrites, ["<svg></svg>"]);
    assert.equal(host.writtenFiles.length, 1);
    assert.equal(host.renderCalls.at(-1)?.themeFileContent, applied);
    assert.equal(host.warnings.length, 2);
    for (const subscription of host.subscriptions) subscription.dispose();
  });

  it("follows editor appearances across hidden locked panels without retargeting their sources", async () => {
    const host = new FakePreviewHost();
    host.themeDiscovery = pairedThemeDiscovery();
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    const lockedPanel = host.panels[0]!;
    await lockedPanel.receive({ type: "ready" });
    await lockedPanel.receive({ type: "setDiagramTheme", theme: "preset:editor" });
    await flushPreviewWork();
    await host.commands.get("merman.togglePreviewLock")?.();
    lockedPanel.setActive(false);
    await host.commands.get("merman.openPreview")?.(host.secondDocument.uri);
    await host.panels[1]?.receive({ type: "ready" });
    await flushPreviewWork();
    host.renderCalls.splice(0);

    host.changeColorTheme(2);
    await flushPreviewWork();
    assert.deepEqual(host.renderCalls.map((call) => [call.source, call.themePreset, call.themeAppearance]), [
      ["flowchart TD\nA --> B\n", "editor", "dark"],
    ]);
    const darkKey = lastRenderedSourceKey(lockedPanel);
    host.changeColorTheme(3);
    await flushPreviewWork();
    assert.equal(host.renderCalls.length, 1);
    assert.deepEqual(lastRenderedSourceKey(lockedPanel), darkKey);
    host.changeColorTheme(4);
    await flushPreviewWork();
    assert.equal(host.renderCalls.at(-1)?.themeAppearance, "light");
    assert.notDeepEqual(lastRenderedSourceKey(lockedPanel), darkKey);
    for (const subscription of host.subscriptions) subscription.dispose();
  });

  it("keeps explicit and single appearances stable while Follow cancels stale output", async () => {
    const host = new FakePreviewHost();
    host.themeDiscovery = pairedThemeDiscovery();
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    const panel = host.panels[0]!;
    await panel.receive({ type: "ready" });
    await panel.receive({ type: "setDiagramTheme", theme: "preset:editor" });
    await panel.receive({ type: "setAppearance", appearance: "light" });
    await flushPreviewWork();
    const count = host.renderCalls.length;
    host.changeColorTheme(2);
    await flushPreviewWork();
    assert.equal(host.renderCalls.length, count);

    await panel.receive({ type: "setAppearance", appearance: "follow-editor" });
    await flushPreviewWork();
    const releaseRender = host.deferNextRender();
    const darkKey = lastRenderedSourceKey(panel);
    host.changeColorTheme(1);
    await flushPreviewWork();
    const signal = host.renderSignals.at(-1)!;
    assert.equal(signal.aborted, false);
    await panel.receive({ type: "exportRendered", format: "svg", sourceKey: darkKey });
    assert.equal(host.writtenFiles.length, 0);
    host.changeColorTheme(2);
    assert.equal(signal.aborted, true);
    releaseRender();
    await flushPreviewWork();
    assert.equal(host.renderCalls.at(-1)?.themeAppearance, "dark");
    const currentKey = lastRenderedSourceKey(panel);
    assert.equal(currentKey.themeSelection, darkKey.themeSelection);

    await panel.receive({ type: "setDiagramTheme", theme: "preset:ghibli" });
    await flushPreviewWork();
    const singleCount = host.renderCalls.length;
    host.changeColorTheme(1);
    await flushPreviewWork();
    assert.equal(host.renderCalls.length, singleCount);
    for (const subscription of host.subscriptions) subscription.dispose();
  });

  it("keeps a matching in-flight render when only the viewer background changes", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    const panel = host.panels[0]!;
    await panel.receive({ type: "ready" });
    await flushPreviewWork();
    const renderCount = host.renderCalls.length;
    const releaseRender = host.deferNextRender();
    try {
      await panel.receive({ type: "setDisplayMode", mode: "ascii" });
      await waitUntil(() => host.renderCalls.length > renderCount);
      const signal = host.renderSignals.at(-1)!;
      panel.postedMessages.splice(0);

      await panel.receive({ type: "setViewerBackground", background: "dark" });
      await flushPreviewWork();

      assert.deepEqual(host.renderCalls.slice(renderCount).map((call) => call.format), ["ascii"]);
      assert.equal(signal.aborted, false);
      assert.deepEqual(panel.postedMessages.map((message) => (message as { type: string }).type), ["settingsUpdated"]);
      releaseRender();
      await flushPreviewWork();
      assert.equal(lastRenderedSnapshot(panel).displayMode, "ascii");
      assert.equal(lastRenderedSnapshot(panel).viewerBackground, "dark");
      assert.deepEqual(host.errors, []);
    } finally {
      releaseRender();
      for (const subscription of host.subscriptions) subscription.dispose();
    }
  });

  it("keeps a matching render queued while its settings update is still posting", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    const panel = host.panels[0]!;
    await panel.receive({ type: "ready" });
    await flushPreviewWork();
    const renderCount = host.renderCalls.length;
    let releaseSettings: () => void = () => {};
    const settingsDelay = new Promise<void>((resolve) => { releaseSettings = resolve; });
    const postMessage = panel.webview.postMessage;
    let settingsPending = false;
    panel.webview.postMessage = async (message) => {
      const update = message as { type: string; snapshot?: PreviewSnapshotMessagePayload };
      if (!settingsPending && update.type === "settingsUpdated" && update.snapshot?.displayMode === "ascii") {
        settingsPending = true;
        await settingsDelay;
      }
      return postMessage(message);
    };
    try {
      await panel.receive({ type: "setDisplayMode", mode: "ascii" });
      await waitUntil(() => settingsPending);
      await panel.receive({ type: "setViewerBackground", background: "dark" });
      await flushPreviewWork();

      assert.equal(host.renderCalls.length, renderCount);
      releaseSettings();
      await flushPreviewWork();

      assert.deepEqual(host.renderCalls.slice(renderCount).map((call) => call.format), ["ascii"]);
      assert.equal(lastRenderedSnapshot(panel).displayMode, "ascii");
      assert.equal(lastRenderedSnapshot(panel).viewerBackground, "dark");
      assert.deepEqual(host.errors, []);
    } finally {
      releaseSettings();
      for (const subscription of host.subscriptions) subscription.dispose();
    }
  });

  it("does not start an obsolete render after a delayed settings update finishes", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    const panel = host.panels[0]!;
    await panel.receive({ type: "ready" });
    await flushPreviewWork();
    const renderCount = host.renderCalls.length;
    let releaseSettings: () => void = () => {};
    const settingsDelay = new Promise<void>((resolve) => { releaseSettings = resolve; });
    const postMessage = panel.webview.postMessage;
    let settingsPending = false;
    panel.webview.postMessage = async (message) => {
      const update = message as { type: string; snapshot?: PreviewSnapshotMessagePayload };
      if (update.type === "settingsUpdated" && update.snapshot?.displayMode === "ascii") {
        settingsPending = true;
        await settingsDelay;
      }
      return postMessage(message);
    };
    try {
      await panel.receive({ type: "setDisplayMode", mode: "ascii" });
      await waitUntil(() => settingsPending);
      assert.equal(host.renderCalls.length, renderCount);

      await panel.receive({ type: "setDisplayMode", mode: "unicode" });
      await flushPreviewWork();
      assert.equal(lastRenderedSnapshot(panel).displayMode, "unicode");
      releaseSettings();
      await flushPreviewWork();

      assert.deepEqual(host.renderCalls.slice(renderCount).map((call) => call.format), ["unicode"]);
      assert.equal(lastRenderedSnapshot(panel).displayMode, "unicode");
      assert.deepEqual(host.errors, []);
    } finally {
      releaseSettings();
      for (const subscription of host.subscriptions) subscription.dispose();
    }
  });

  it("freezes preview export before a delayed save dialog and keeps viewer backing out of image settings", async () => {
    const host = new FakePreviewHost();
    host.themeDiscovery = pairedThemeDiscovery();
    const { registerPreview } = loadPreviewModule(host);
    registerPreview({ extensionUri: uri("file:///extension"), subscriptions: host.subscriptions } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    const panel = host.panels[0]!;
    await panel.receive({ type: "ready" });
    await panel.receive({ type: "setDiagramTheme", theme: "preset:editor" });
    await panel.receive({ type: "setBackground", background: "theme" });
    await flushPreviewWork();
    const count = host.renderCalls.length;
    await panel.receive({ type: "setViewerBackground", background: "dark" });
    await flushPreviewWork();
    assert.equal(host.renderCalls.length, count);
    let releaseDialog: () => void = () => {};
    host.saveDialogDelay = new Promise<void>((resolve) => { releaseDialog = resolve; });
    await panel.receive({ type: "exportRendered", format: "svg", sourceKey: lastRenderedSourceKey(panel) });
    await flushPreviewWork();
    host.changeColorTheme(2);
    await flushPreviewWork();
    releaseDialog();
    await flushPreviewWork();
    assert.equal(host.renderCalls.at(-1)?.themeAppearance, "light");
    assert.equal(host.renderCalls.at(-1)?.background, undefined);
    assert.equal(host.writtenFiles.length, 1);
    for (const subscription of host.subscriptions) subscription.dispose();
  });

  it("shares extraction across CodeLens and independently configured panels with language intelligence disabled", async () => {
    const host = new FakePreviewHost();
    const { registerPreview, registerSourceCodeLens } = loadPreviewModule(host);
    const reads = { lineAt: 0 };
    const document = textDocument(
      "file:///workspace/notes.md",
      "notes.md",
      "```mermaid\nflowchart LR\n```\n\n```mermaid\nsequenceDiagram\n```",
      "markdown",
      1,
      reads,
    );
    host.setActiveDocument(document);
    const context = {
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext;
    registerPreview(context);
    registerSourceCodeLens(context);
    const provider = host.codeLensProviders[0];
    assert.ok(provider);
    const lenses = await provider.provideCodeLenses(document, {} as vscode.CancellationToken);
    assert.equal(lenses?.length, 4);
    await provider.provideCodeLenses(document, {} as vscode.CancellationToken);

    await host.commands.get("merman.openPreview")?.(lenses?.[2]?.command?.arguments?.[0]);
    await host.panels[0]?.receive({ type: "ready" });
    await host.commands.get("merman.togglePreviewLock")?.();
    await host.commands.get("merman.openPreview")?.(lenses?.[0]?.command?.arguments?.[0]);
    await host.panels[1]?.receive({ type: "ready" });
    await host.panels[0]?.receive({ type: "setDiagramTheme", theme: "dark" });
    await host.panels[1]?.receive({ type: "setBackground", background: "transparent" });
    await host.commands.get("merman.refreshPreview")?.();
    await flushPreviewWork();

    const locked = lastRenderedSnapshot(host.panels[0]);
    const followed = lastRenderedSnapshot(host.panels[1]);
    assert.equal(locked.sourceId, "fence-2");
    assert.equal(locked.locked, true);
    assert.equal(locked.diagramTheme, "dark");
    assert.equal(locked.background, "paper");
    assert.equal(followed.sourceId, "fence-1");
    assert.equal(followed.locked, false);
    assert.equal(followed.diagramTheme, "source");
    assert.equal(followed.background, "transparent");
    assert.equal(reads.lineAt, document.lineCount);

    host.fireDocumentChange(document);
    await host.panels[1]?.receive({ type: "copySvg", svg: "<svg></svg>", sourceKey: followed.sourceKey });
    assert.deepEqual(host.clipboardWrites, []);
    assert.equal(reads.lineAt, document.lineCount);

    await host.commands.get("merman.refreshPreview")?.();
    await flushPreviewWork();
    await host.panels[0]?.receive({ type: "exportRendered", format: "svg", sourceKey: locked.sourceKey });
    await flushPreviewWork();
    assert.equal(host.writtenFiles.length, 1);
    assert.equal(host.renderCalls.at(-1)?.source, "sequenceDiagram");
    assert.equal(reads.lineAt, document.lineCount);

    Object.defineProperty(document, "isClosed", { value: true });
    host.fireDocumentClose(document);
    const closedLenses = await provider.provideCodeLenses(document, {} as vscode.CancellationToken);
    assert.deepEqual(closedLenses, []);
    assert.equal(reads.lineAt, document.lineCount);

    const reopenedReads = { lineAt: 0 };
    const reopened = textDocument(
      "file:///workspace/notes.md",
      "notes.md",
      "```mermaid\npie title Reopened\n```",
      "markdown",
      1,
      reopenedReads,
    );
    host.setActiveDocument(reopened);
    const reopenedLenses = await provider.provideCodeLenses(reopened, {} as vscode.CancellationToken);
    assert.equal(reopenedLenses?.length, 2);
    assert.equal(reopenedReads.lineAt, reopened.lineCount);
    for (const subscription of host.subscriptions) {
      subscription.dispose();
    }
  });

  it("renders replacement documents with the same URI and version from their own source facts", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);
    const original = textDocument("file:///workspace/notes.md", "notes.md", "```mermaid\nflowchart LR\n```", "markdown");
    const replacement = textDocument("file:///workspace/notes.md", "notes.md", "```mermaid\nsequenceDiagram\n```", "markdown");
    host.setActiveDocument(original);
    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);
    await host.commands.get("merman.openPreview")?.();
    await host.panels[0]?.receive({ type: "ready" });
    await flushPreviewWork();
    const staleSourceKey = lastRenderedSourceKey(host.panels[0]);

    host.setActiveDocument(replacement);
    host.fireDocumentChange(replacement);
    await host.panels[0]?.receive({ type: "exportRendered", format: "svg", sourceKey: staleSourceKey });
    assert.deepEqual(host.writtenFiles, []);

    await host.commands.get("merman.refreshPreview")?.();
    await flushPreviewWork();
    assert.equal(host.renderCalls.at(-1)?.source, "sequenceDiagram");
    assert.notDeepEqual(lastRenderedSourceKey(host.panels[0]), staleSourceKey);
    for (const subscription of host.subscriptions) {
      subscription.dispose();
    }
  });

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
    assert.deepEqual(host.webviewOptions.map((options) => ({
      enableCommandUris: options.enableCommandUris,
      enableScripts: options.enableScripts,
      localResourceRoots: options.localResourceRoots?.map((resource) => resource.toString()),
      retainContextWhenHidden: options.retainContextWhenHidden,
    })), [
      {
        enableCommandUris: false,
        enableScripts: true,
        localResourceRoots: ["file:///extension/media"],
        retainContextWhenHidden: true,
      },
    ]);
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

  it("keeps an empty preview unlocked and retargets it without stealing editor focus", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.();
    assert.equal(host.panels.length, 1);
    assert.deepEqual(host.showTextDocumentCalls, []);

    await host.panels[0]?.receive({ type: "setLocked", locked: true });
    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);

    assert.equal(host.panels.length, 1);
    assert.deepEqual(host.showTextDocumentCalls.map(({ documentUri, preserveFocus }) => ({
      documentUri,
      preserveFocus,
    })), [
      { documentUri: "file:///workspace/example.mmd", preserveFocus: true },
    ]);
    assert.deepEqual(host.revealCalls, [{ viewColumn: 2, preserveFocus: true }]);
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

    assert.deepEqual(host.renderCalls.map((call) => call.source), [
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

    assert.deepEqual(host.renderCalls.map((call) => call.source), ["flowchart TD\nA --> B\n"]);
  });

  it("reopens a follow preview after its panel is closed", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    assert.equal(host.panels.length, 1);
    host.panels[0]?.dispose();

    await host.commands.get("merman.openPreview")?.(host.secondDocument.uri);

    assert.equal(host.panels.length, 2);
    assert.equal(host.panels[0]?.disposed, true);
    assert.equal(host.panels[1]?.disposed, false);
    assert.deepEqual(host.showTextDocumentCalls.map((call) => call.documentUri), [
      "file:///workspace/example.mmd",
      "file:///workspace/second.mmd",
    ]);
  });

  it("keeps the newest follow preview target when concurrent opens resolve out of order", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    const releaseFirstOpen = host.deferOpenTextDocument(host.targetDocument.uri);
    const firstOpen = host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await flushPreviewWork();

    const secondOpen = host.commands.get("merman.openPreview")?.(host.secondDocument.uri);
    await secondOpen;
    releaseFirstOpen();
    await firstOpen;
    await flushPreviewWork();

    assert.equal(host.panels.length, 1);
    assert.deepEqual(host.showTextDocumentCalls.map((call) => call.documentUri), [
      "file:///workspace/second.mmd",
    ]);
    assert.deepEqual(host.renderCalls.map((call) => call.source), [
      "sequenceDiagram\nA->>B: hi\n",
    ]);
  });

  it("replays preview state when the webview becomes ready again", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await flushPreviewWork();
    host.panels[0]?.postedMessages.splice(0);

    await host.panels[0]?.receive({ type: "ready" });
    await flushPreviewWork();

    assert.deepEqual(host.panels[0]?.postedMessages.map((message) => (message as { type?: string }).type), [
      "sourceListUpdated",
      "diagnosticsUpdated",
      "settingsUpdated",
      "renderSucceeded",
    ]);
  });

  it("reveals source for the preview that sends a webview showSource message", async () => {
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

    await host.panels[0]?.receive({ type: "showSource" });

    const sourceReveal = host.showTextDocumentCalls.at(-1);
    assert.equal(sourceReveal?.documentUri, "file:///workspace/example.mmd");
    assert.equal(sourceReveal?.preserveFocus, false);
  });

  it("keeps copy and export actions scoped to the sending preview instance", async () => {
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
    const firstSourceKey = lastRenderedSourceKey(host.panels[0]);
    const secondSourceKey = lastRenderedSourceKey(host.panels[1]);
    host.renderCalls.splice(0);

    await host.panels[0]?.receive({ type: "copySvg", svg: "<svg id=\"first\"></svg>", sourceKey: firstSourceKey });
    await host.panels[1]?.receive({ type: "copySvg", svg: "<svg id=\"second\"></svg>", sourceKey: secondSourceKey });
    await host.panels[0]?.receive({ type: "exportRendered", format: "svg", sourceKey: firstSourceKey });
    await host.panels[1]?.receive({ type: "exportRendered", format: "png", sourceKey: secondSourceKey });
    await flushPreviewWork();

    assert.deepEqual(host.clipboardWrites, [
      "<svg id=\"first\"></svg>",
      "<svg id=\"second\"></svg>",
    ]);
    assert.deepEqual(host.renderCalls.map((call) => ({
      source: call.source,
      format: call.format,
      background: call.background,
    })), [
      { source: "flowchart TD\nA --> B\n", format: "svg", background: "white" },
      { source: "sequenceDiagram\nA->>B: hi\n", format: "svg", background: "white" },
      { source: "<svg viewBox=\"0 0 10 10\"></svg>", format: "png", background: "white" },
    ]);
    assert.deepEqual(host.informationMessages.slice(-2), [
      "Exported export-one.svg.",
      "Exported export-2.png.",
    ]);
  });

  it("deduplicates concurrent exports for the same rendered source and format", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await flushPreviewWork();
    const sourceKey = lastRenderedSourceKey(host.panels[0]);
    const panel = host.panels[0];
    assert.ok(panel, "Expected preview panel");
    host.renderCalls.splice(0);

    const release = host.deferNextRender();
    const firstExport = panel.receive({ type: "exportRendered", format: "svg", sourceKey });
    await waitUntil(() => host.renderCalls.length === 1);

    await panel.receive({ type: "exportRendered", format: "svg", sourceKey });
    assert.equal(host.renderCalls.length, 1);

    release();
    await firstExport;
    await flushPreviewWork();

    assert.equal(host.renderCalls.length, 1);
    assert.deepEqual(host.informationMessages.slice(-1), ["Exported export-one.svg."]);
  });

  it("refuses to export when the webview rendered source key is stale", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await flushPreviewWork();
    const staleSourceKey = lastRenderedSourceKey(host.panels[0]);

    await host.panels[0]?.receive({ type: "setBackground", background: "dark" });
    await flushPreviewWork();
    host.renderCalls.splice(0);

    await host.panels[0]?.receive({ type: "exportRendered", format: "svg", sourceKey: staleSourceKey });

    assert.deepEqual(host.renderCalls, []);
    assert.deepEqual(host.warnings.slice(-1), [
      "Wait for the latest Mermaid preview to finish rendering before copying or exporting.",
    ]);
  });

  it("refuses copy and export immediately after a tracked document changes", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await flushPreviewWork();
    const staleSourceKey = lastRenderedSourceKey(host.panels[0]);
    host.renderCalls.splice(0);

    host.fireDocumentChange(host.targetDocument);
    await host.panels[0]?.receive({
      type: "copySvg",
      svg: "<svg id=\"old\"></svg>",
      sourceKey: staleSourceKey,
    });
    await host.panels[0]?.receive({ type: "exportRendered", format: "svg", sourceKey: staleSourceKey });

    assert.deepEqual(host.clipboardWrites, []);
    assert.deepEqual(host.renderCalls, []);
    assert.deepEqual(host.warnings.slice(-2), [
      "Wait for the latest Mermaid preview to finish rendering before copying or exporting.",
      "Wait for the latest Mermaid preview to finish rendering before copying or exporting.",
    ]);
  });

  it("posts fresh invalidation tombstones for repeated document changes before the next render", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await flushPreviewWork();
    host.panels[0]?.postedMessages.splice(0);

    host.fireDocumentChange(host.targetDocument);
    host.fireDocumentChange(host.targetDocument);
    await flushPreviewWork();

    assert.deepEqual(
      host.panels[0]?.postedMessages
        .map((message) => (message as { type?: string }).type)
        .filter((type) => type === "renderInvalidated"),
      ["renderInvalidated", "renderInvalidated"],
    );
  });

  it("allows a new settings invalidation after a successful render clears stale state", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await flushPreviewWork();

    await host.panels[0]?.receive({ type: "setBackground", background: "dark" });
    await flushPreviewWork();
    host.panels[0]?.postedMessages.splice(0);

    await host.panels[0]?.receive({ type: "setDisplayMode", mode: "ascii" });
    await flushPreviewWork();

    assert.deepEqual(
      host.panels[0]?.postedMessages
        .map((message) => (message as { type?: string }).type)
        .filter((type) => type === "renderInvalidated"),
      ["renderInvalidated"],
    );
  });

  it("cancels an in-flight stale render when another document change arrives", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await flushPreviewWork();
    host.panels[0]?.postedMessages.splice(0);
    host.renderSignals.splice(0);

    host.fireDocumentChange(host.targetDocument);
    const releaseRender = host.deferNextRender();
    await host.commands.get("merman.refreshPreview")?.();
    await waitUntil(() => host.renderSignals.length === 1);

    assert.equal(host.renderSignals[0]?.aborted, false);
    host.fireDocumentChange(host.targetDocument);
    assert.equal(host.renderSignals[0]?.aborted, true);

    releaseRender();
    await flushPreviewWork();

    assert.equal(
      host.panels[0]?.postedMessages.some(
        (message) => (message as { type?: string }).type === "renderSucceeded",
      ),
      false,
    );
    host.panels[0]?.dispose();
  });

  it("reports failed webview copy messages without rejecting the message dispatch", async () => {
    const host = new FakePreviewHost();
    const { registerPreview } = loadPreviewModule(host);

    registerPreview({
      extensionUri: uri("file:///extension"),
      subscriptions: host.subscriptions,
    } as unknown as vscode.ExtensionContext);

    await host.commands.get("merman.openPreview")?.(host.targetDocument.uri);
    await host.panels[0]?.receive({ type: "ready" });
    await flushPreviewWork();
    const sourceKey = lastRenderedSourceKey(host.panels[0]);
    await host.panels[0]?.receive({
      type: "copySvg",
      svg: "<svg><script>alert(1)</script></svg>",
      sourceKey,
    });
    await flushPreviewWork();

    assert.deepEqual(host.clipboardWrites, []);
    assert.match(host.outputErrors.at(-1) ?? "", /Preview webview message failed: .*active/);
    assert.match(host.errors.at(-1) ?? "", /Merman preview action failed: .*active/);
  });
});

function loadPreviewModule(
  host: FakePreviewHost,
): typeof import("../preview.js") & typeof import("../codelens.js") {
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
    if (request === "./theme-runtime.js") {
      return {
        discoverThemes: async () => host.themeDiscovery,
        inspectThemeFile: async (_context: unknown, resource: { toString(): string }) => {
          host.themeFiles.push(resource.toString());
          const candidate = host.imported;
          const failure = host.importError;
          await host.importDelay;
          if (failure) throw failure;
          return candidate;
        },
      };
    }
    if (request === "./renderer.js") {
      return {
        renderMermanSource: async (renderRequest: {
          source: string;
          format?: string;
          outputPath?: string;
          background?: string;
          theme?: string;
          themePreset?: string;
          themeAppearance?: string;
          themeFileContent?: string;
          signal?: AbortSignal;
        }) => {
          host.renderCalls.push({
            source: renderRequest.source,
            format: renderRequest.format,
            outputPath: renderRequest.outputPath,
            background: renderRequest.background,
            ...(renderRequest.theme ? { theme: renderRequest.theme } : {}),
            ...(renderRequest.themePreset ? { themePreset: renderRequest.themePreset } : {}),
            ...(renderRequest.themeAppearance ? { themeAppearance: renderRequest.themeAppearance } : {}),
            ...(renderRequest.themeFileContent ? { themeFileContent: renderRequest.themeFileContent } : {}),
          });
          if (renderRequest.signal) {
            host.renderSignals.push(renderRequest.signal);
          }
          const deferred = host.takeDeferredRender();
          if (deferred) {
            await deferred.promise;
          }
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
    delete require.cache[require.resolve("../export-workflow.js")];
    delete require.cache[require.resolve("../renderer.js")];
    delete require.cache[require.resolve("../codelens.js")];
    delete require.cache[require.resolve("../render-settings.js")];
    delete require.cache[require.resolve("../config.js")];
    return {
      ...(require("../preview.js") as typeof import("../preview.js")),
      ...(require("../codelens.js") as typeof import("../codelens.js")),
    };
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
  version = 1,
  reads?: { lineAt: number },
): vscode.TextDocument {
  const lines = text.split(/\r?\n/);
  return {
    uri: uri(uriValue, fileName),
    fileName,
    languageId,
    version,
    lineCount: lines.length,
    getText: () => {
      assert.notEqual(languageId, "markdown", "Markdown extraction must use lineAt");
      return text;
    },
    lineAt: (lineIndex: number) => {
      if (reads) {
        reads.lineAt += 1;
      }
      return { text: lines[lineIndex] ?? "" };
    },
  } as unknown as vscode.TextDocument;
}

function sameUri(first: vscode.Uri, second: vscode.Uri): boolean {
  return first.toString() === second.toString();
}

async function flushPreviewWork(): Promise<void> {
  await new Promise((resolve) => setImmediate(resolve));
}

async function waitUntil(predicate: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (predicate()) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  assert.fail("Condition was not met");
}

function lastRenderedSourceKey(panel: FakePanel | undefined): PreviewSnapshotMessagePayload["sourceKey"] {
  return lastRenderedSnapshot(panel).sourceKey;
}

function lastRenderedSnapshot(panel: FakePanel | undefined): PreviewSnapshotMessagePayload {
  assert.ok(panel, "Expected preview panel");
  for (let index = panel.postedMessages.length - 1; index >= 0; index -= 1) {
    const message = panel.postedMessages[index] as {
      type?: string;
      snapshot?: PreviewSnapshotMessagePayload;
    };
    if (message.type === "renderSucceeded" && message.snapshot?.sourceKey) {
      return message.snapshot;
    }
  }
  assert.fail("Expected a renderSucceeded message with a sourceKey");
}

function saveDialogExtension(
  options: { defaultUri?: vscode.Uri; filters?: Record<string, string[]> } | undefined,
): string {
  const value = options?.defaultUri?.fsPath ?? options?.defaultUri?.path ?? options?.defaultUri?.toString() ?? "";
  const defaultUriExtension = /\.([A-Za-z0-9]+)$/.exec(value)?.[1];
  if (defaultUriExtension) {
    return defaultUriExtension;
  }
  return Object.values(options?.filters ?? {}).flat()[0] ?? "svg";
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
