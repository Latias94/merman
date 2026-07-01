import * as vscode from "vscode";

import { extractPreviewInput } from "./preview-source.js";
import {
  mermaidSourceCommandSourceId,
  mermaidSourceCommandUri,
  type MermaidSourceCommandArgument,
} from "./source-actions.js";
import {
  type PreviewBackground,
  type PreviewDiagramTheme,
  type PreviewDiagnosticTarget,
  type PreviewDiagnostics,
  type PreviewDisplayMode,
  type PreviewSnapshot,
} from "./preview-model.js";
import {
  isPreviewDiagramTheme,
  isPreviewDisplayMode,
  isPreviewFromWebviewMessage,
  snapshotMessagePayload,
  type PreviewFromWebviewMessage,
  type PreviewToWebviewMessage,
} from "./preview-messages.js";
import { previewCliBackground } from "./preview-background.js";
import {
  planPreviewUpdate,
  type PreviewAction,
  type PreviewUpdateReason,
} from "./preview-policy.js";
import { renderMermanSource } from "./renderer.js";
import {
  defaultExportPath,
  exportFilters,
  type ExportFormat,
} from "./export-options.js";
import { PreviewRenderQueue } from "./preview-render.js";
import { PreviewSession } from "./preview-session.js";
import { assertSafePreviewSvg } from "./preview-svg-safety.js";
import { PreviewWebviewClient } from "./preview-webview-client.js";
import { collectMermanPreviewDiagnostics } from "./preview-diagnostics.js";

const PREVIEW_COMMAND = "merman.openPreview";
const TOGGLE_PREVIEW_LOCK_COMMAND = "merman.togglePreviewLock";
const PREVIEW_TITLE = "Merman Preview";
const RENDER_DEBOUNCE_MS = 180;

export function registerPreview(context: vscode.ExtensionContext): void {
  const controller = new MermanPreviewController(context);
  context.subscriptions.push(controller);
}

class MermanPreviewController implements vscode.Disposable {
  private readonly outputChannel: vscode.LogOutputChannel;
  private panel: vscode.WebviewPanel | undefined;
  private renderTimer: NodeJS.Timeout | undefined;
  private readonly renderQueue = new PreviewRenderQueue();
  private readonly session = new PreviewSession();
  private readonly webviewClient: PreviewWebviewClient;
  private readonly disposables: vscode.Disposable[] = [];

  constructor(private readonly context: vscode.ExtensionContext) {
    this.webviewClient = new PreviewWebviewClient(context.extensionUri);
    this.outputChannel = vscode.window.createOutputChannel("Merman Preview", { log: true });
    this.disposables.push(this.outputChannel);
    this.disposables.push(
      vscode.commands.registerCommand(
        PREVIEW_COMMAND,
        async (target?: MermaidSourceCommandArgument) => {
          await this.open(target);
        },
      ),
    );
    this.disposables.push(
      vscode.commands.registerCommand(TOGGLE_PREVIEW_LOCK_COMMAND, () => {
        this.setLocked(!this.session.isLocked, true);
      }),
    );
    this.disposables.push(
      vscode.window.onDidChangeActiveTextEditor(() => {
        this.scheduleRefresh("active-editor");
      }),
    );
    this.disposables.push(
      vscode.window.onDidChangeTextEditorSelection((event) => {
        if (event.textEditor === vscode.window.activeTextEditor) {
          this.scheduleRefresh("selection");
        }
      }),
    );
    this.disposables.push(
      vscode.workspace.onDidChangeTextDocument((event) => {
        const trackedEditor = this.resolvePreviewEditor();
        if (trackedEditor && event.document === trackedEditor.document) {
          this.scheduleRefresh("document-change");
        }
      }),
    );
    this.disposables.push(
      vscode.languages.onDidChangeDiagnostics((event) => {
        const trackedEditor = this.resolvePreviewEditor();
        const trackedUri = trackedEditor?.document.uri;
        if (trackedUri && event.uris.some((uri) => uri.toString() === trackedUri.toString())) {
          this.scheduleRefresh("diagnostics");
        }
      }),
    );
  }

  dispose(): void {
    this.clearPendingRender();
    this.panel?.dispose();
    this.panel = undefined;
    for (const disposable of this.disposables) {
      disposable.dispose();
    }
  }

  private async open(target?: MermaidSourceCommandArgument): Promise<void> {
    const shouldRetargetSource = !this.panel || !this.session.isLocked || !this.session.snapshot;
    if (shouldRetargetSource) {
      await this.openResource(target);
    }
    if (!this.panel) {
      this.panel = vscode.window.createWebviewPanel(
        "mermanPreview",
        PREVIEW_TITLE,
        {
          viewColumn: vscode.ViewColumn.Beside,
          preserveFocus: true,
        },
        {
          enableCommandUris: false,
          enableScripts: true,
          localResourceRoots: [vscode.Uri.joinPath(this.context.extensionUri, "media")],
          retainContextWhenHidden: true,
        },
      );
      this.panel.webview.onDidReceiveMessage(
        (message: PreviewFromWebviewMessage) => {
          void this.handleWebviewMessage(message);
        },
        null,
        this.disposables,
      );
      this.panel.onDidDispose(() => {
        this.clearPendingRender();
        this.panel = undefined;
        this.session.reset();
        this.webviewClient.reset();
      }, null, this.disposables);
      this.panel.onDidChangeViewState(() => {
        if (this.panel?.visible) {
          this.scheduleRefresh("panel-visible");
        }
      }, null, this.disposables);
    } else {
      this.panel.reveal(this.panel.viewColumn, true);
    }

    this.ensureWebviewHtml(panelOrThrow(this.panel));
    this.scheduleRefresh(shouldRetargetSource ? "manual-open" : "panel-visible", true);
  }

  private async openResource(target?: MermaidSourceCommandArgument): Promise<void> {
    const resource = mermaidSourceCommandUri(target);
    const sourceId = mermaidSourceCommandSourceId(target);
    if (!resource) {
      const activeEditor = vscode.window.activeTextEditor;
      if (activeEditor && extractPreviewInput(activeEditor)) {
        this.session.clearSelectedSource();
        this.session.rememberResource(activeEditor.document.uri, { preferOnce: true });
      }
      return;
    }
    this.session.rememberResource(resource, { preferOnce: true });
    let editor = vscode.window.activeTextEditor;
    if (editor?.document.uri.toString() !== resource.toString()) {
      const document = await vscode.workspace.openTextDocument(resource);
      editor = await vscode.window.showTextDocument(document, {
        preview: true,
        preserveFocus: true,
      });
    }
    if (sourceId && editor) {
      this.session.selectSource(editor, vscode.window.visibleTextEditors, sourceId);
    } else {
      this.session.clearSelectedSource();
    }
  }

  private scheduleRefresh(reason: PreviewUpdateReason, immediate = false): void {
    if (!this.panel) {
      return;
    }
    if (this.renderTimer) {
      clearTimeout(this.renderTimer);
      this.renderTimer = undefined;
    }
    const refresh = () => {
      void this.refresh(reason);
    };
    if (immediate) {
      refresh();
      return;
    }
    this.renderTimer = setTimeout(refresh, RENDER_DEBOUNCE_MS);
  }

  private async refresh(reason: PreviewUpdateReason): Promise<void> {
    const panel = this.panel;
    if (!panel) {
      return;
    }

    this.ensureWebviewHtml(panel);
    const snapshot = this.createSnapshot();
    const actions = planPreviewUpdate(this.session.snapshot, snapshot, reason);
    if (!snapshot) {
      panel.title = PREVIEW_TITLE;
      this.session.clearSource();
      this.renderQueue.cancelPending();
      await this.applyActions(actions);
      return;
    }

    panel.title = `${PREVIEW_TITLE}: ${snapshot.input.title}`;
    this.session.rememberSnapshot(snapshot);
    await this.applyActions(actions);
  }

  private async applyActions(actions: readonly PreviewAction[]): Promise<void> {
    for (const action of actions) {
      switch (action.type) {
        case "showEmpty":
          await this.postMessage({
            type: "showEmpty",
            heading: "No Mermaid source available",
            detail:
              "Focus a .mmd, .mermaid, or Markdown document with a Mermaid fence, then run Merman: Open Preview.",
          });
          break;
        case "sourceListUpdated":
        case "selectionChanged":
        case "diagnosticsUpdated":
        case "settingsUpdated":
          await this.postMessage({
            type: action.type,
            snapshot: snapshotMessagePayload(action.snapshot),
          });
          break;
        case "renderRequested":
          await this.renderSnapshot(action.snapshot, action.reason);
          break;
      }
    }
  }

  private async renderSnapshot(snapshot: PreviewSnapshot, reason: PreviewUpdateReason): Promise<void> {
    await this.renderQueue.render(snapshot, reason, {
      renderContent: (renderedSnapshot, signal) => this.renderContent(renderedSnapshot, signal),
      postMessage: (message) => this.postMessage(message),
      info: (message) => this.outputChannel.info(message),
      error: (message) => this.outputChannel.error(message),
      isCurrentRequest: (requestId) => !!this.panel && this.renderQueue.isCurrentRequest(requestId),
      markRendered: (_requestId, renderedSnapshot, content) => this.webviewClient.markRendered(renderedSnapshot, content),
    });
  }

  private createSnapshot(): PreviewSnapshot | undefined {
    return this.session.createSnapshot(
      vscode.window.activeTextEditor,
      vscode.window.visibleTextEditors,
      collectPreviewDiagnostics,
    );
  }

  private async renderContent(snapshot: PreviewSnapshot, signal: AbortSignal): Promise<string> {
    const result = await renderMermanSource({
      context: this.context,
      source: snapshot.input.source,
      format: snapshot.displayMode,
      theme: snapshot.diagramTheme,
      background: previewCliBackground(snapshot.background),
      outputChannel: this.outputChannel,
      signalLabel: "preview",
      signal,
    });
    const content = result.stdout.toString("utf8");
    if (snapshot.displayMode === "svg") {
      assertSafePreviewSvg(content);
    }
    return content;
  }

  private ensureWebviewHtml(panel: vscode.WebviewPanel): void {
    this.webviewClient.ensureHtml(panel);
  }

  private async postMessage(message: PreviewToWebviewMessage): Promise<void> {
    await this.webviewClient.post(this.panel, message);
  }

  private clearPendingRender(): void {
    if (this.renderTimer) {
      clearTimeout(this.renderTimer);
      this.renderTimer = undefined;
    }
  }

  private resolvePreviewEditor(): vscode.TextEditor | undefined {
    return this.session.resolvePreviewEditor(
      vscode.window.activeTextEditor,
      vscode.window.visibleTextEditors,
    );
  }

  private async handleWebviewMessage(message: PreviewFromWebviewMessage): Promise<void> {
    if (!isPreviewFromWebviewMessage(message)) {
      return;
    }
    switch (message.type) {
      case "ready":
        await this.webviewClient.acceptReady(
          this.panel,
          this.session.snapshot,
          (snapshot) =>
            this.applyActions([
              { type: "sourceListUpdated", snapshot },
              { type: "diagnosticsUpdated", snapshot },
              { type: "settingsUpdated", snapshot },
            ]),
          (snapshot) => this.renderSnapshot(snapshot, "panel-visible"),
        );
        return;
      case "copySvg":
        await vscode.env.clipboard.writeText(message.svg);
        void vscode.window.showInformationMessage("Copied Mermaid SVG to clipboard.");
        return;
      case "exportRendered":
        await this.exportRendered(message.format);
        return;
      case "revealDiagnostic":
        await revealDiagnosticTarget(parseDiagnosticTarget(message.target));
        return;
      case "selectSource":
        this.selectSource(message.sourceId);
        return;
      case "setLocked":
        this.setLocked(message.locked, false);
        return;
      case "setDiagramTheme":
        this.setDiagramTheme(message.theme);
        return;
      case "setDisplayMode":
        this.setDisplayMode(message.mode);
        return;
      case "setBackground":
        this.setBackground(message.background);
        return;
    }
  }

  private selectSource(sourceId: string): void {
    if (
      !this.session.selectSource(
        vscode.window.activeTextEditor,
        vscode.window.visibleTextEditors,
        sourceId,
      )
    ) {
      return;
    }
    this.scheduleRefresh("source-select", true);
  }

  private setDiagramTheme(theme: PreviewDiagramTheme): void {
    if (!isPreviewDiagramTheme(theme) || !this.session.setDiagramTheme(theme)) {
      return;
    }
    this.scheduleRefresh("diagram-theme", true);
  }

  private setDisplayMode(displayMode: PreviewDisplayMode): void {
    if (!isPreviewDisplayMode(displayMode) || !this.session.setDisplayMode(displayMode)) {
      return;
    }
    this.scheduleRefresh("display-mode", true);
  }

  private setBackground(background: PreviewBackground): void {
    if (!this.session.setBackground(background)) {
      return;
    }
    this.scheduleRefresh("background", true);
  }

  private setLocked(locked: boolean, notify: boolean): void {
    if (locked && !this.session.snapshot) {
      if (notify) {
        void vscode.window.showWarningMessage(
          "Open a Mermaid preview before locking it to a source.",
        );
      }
      return;
    }
    if (!this.session.setLocked(locked)) {
      return;
    }
    this.scheduleRefresh("lock", true);
    if (notify) {
      void vscode.window.showInformationMessage(
        locked
          ? "Merman preview is locked to the current source."
          : "Merman preview is following the active source.",
      );
    }
  }

  private async exportRendered(format: ExportFormat): Promise<void> {
    const snapshot = this.session.snapshot;
    if (!snapshot) {
      void vscode.window.showWarningMessage(
        "Open a Mermaid preview before exporting the rendered diagram.",
      );
      return;
    }

    const documentUri = vscode.Uri.parse(snapshot.documentUri);
    const defaultUri =
      documentUri.scheme === "file"
        ? vscode.Uri.file(defaultExportPath(documentUri.fsPath, snapshot.input.exportBaseName, format))
        : undefined;
    const target = await vscode.window.showSaveDialog({
      defaultUri,
      filters: exportFilters(format),
      saveLabel: `Export ${format.toUpperCase()}`,
    });
    if (!target) {
      return;
    }

    try {
      await renderMermanSource({
        context: this.context,
        source: snapshot.input.source,
        format,
        theme: snapshot.diagramTheme,
        background: previewCliBackground(snapshot.background),
        outputPath: target.fsPath,
        outputChannel: this.outputChannel,
        signalLabel: `preview-export-${format}`,
      });
      void vscode.window.showInformationMessage(`Exported ${vscode.workspace.asRelativePath(target, false)}.`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.outputChannel.error(message);
      void vscode.window.showErrorMessage(`Merman preview export failed: ${message}`);
    }
  }
}

function panelOrThrow(panel: vscode.WebviewPanel | undefined): vscode.WebviewPanel {
  if (!panel) {
    throw new Error("Preview panel is not available");
  }
  return panel;
}

export function collectPreviewDiagnostics(
  uri: vscode.Uri,
  diagnosticRange: { startLine: number; endLine: number },
): PreviewDiagnostics {
  return collectMermanPreviewDiagnostics(
    vscode.languages.getDiagnostics(uri),
    uri.toString(),
    diagnosticRange,
  );
}

function parseDiagnosticTarget(raw: string): PreviewDiagnosticTarget {
  const parsed = JSON.parse(raw) as Partial<PreviewDiagnosticTarget>;
  if (
    typeof parsed.uri !== "string" ||
    typeof parsed.startLine !== "number" ||
    typeof parsed.startCharacter !== "number" ||
    typeof parsed.endLine !== "number" ||
    typeof parsed.endCharacter !== "number"
  ) {
    throw new Error("Invalid preview diagnostic target");
  }
  return {
    uri: parsed.uri,
    startLine: parsed.startLine,
    startCharacter: parsed.startCharacter,
    endLine: parsed.endLine,
    endCharacter: parsed.endCharacter,
  };
}

async function revealDiagnosticTarget(target: PreviewDiagnosticTarget): Promise<void> {
  const document = await vscode.workspace.openTextDocument(vscode.Uri.parse(target.uri));
  const range = new vscode.Range(
    new vscode.Position(target.startLine, target.startCharacter),
    new vscode.Position(target.endLine, target.endCharacter),
  );
  await vscode.window.showTextDocument(document, {
    preview: false,
    preserveFocus: false,
    selection: range,
  });
}
