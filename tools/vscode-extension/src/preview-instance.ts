import * as vscode from "vscode";

import { previewCliBackground } from "./preview-background.js";
import { collectMermanPreviewDiagnostics } from "./preview-diagnostics.js";
import type { ExportFormat } from "./export-options.js";
import { exportRenderedDiagram } from "./export-workflow.js";
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
import {
  planPreviewUpdate,
  type PreviewAction,
  type PreviewUpdateReason,
} from "./preview-policy.js";
import { PreviewRenderQueue } from "./preview-render.js";
import { PreviewSession } from "./preview-session.js";
import { extractPreviewInput } from "./preview-source.js";
import { assertSafePreviewSvg } from "./preview-svg-safety.js";
import { PreviewWebviewClient } from "./preview-webview-client.js";
import { renderMermanSource } from "./renderer.js";
import {
  mermaidSourceCommandSourceId,
  mermaidSourceCommandUri,
  type MermaidSourceCommandArgument,
} from "./source-actions.js";

const PREVIEW_TITLE = "Merman Preview";
const RENDER_DEBOUNCE_MS = 180;
export const EMPTY_PREVIEW_LOCK_WARNING = "Open a Mermaid preview before locking it to a source.";

export class PreviewInstance implements vscode.Disposable {
  private panel: vscode.WebviewPanel | undefined;
  private renderTimer: NodeJS.Timeout | undefined;
  private readonly renderQueue = new PreviewRenderQueue();
  private readonly session = new PreviewSession();
  private readonly webviewClient: PreviewWebviewClient;
  private readonly panelDisposables: vscode.Disposable[] = [];
  private disposed = false;

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly outputChannel: vscode.LogOutputChannel,
    private readonly onDispose: (instance: PreviewInstance) => void,
    private readonly onDidChangeActiveState: (instance: PreviewInstance, active: boolean) => void,
    private readonly onDidChangeLockState: (instance: PreviewInstance) => void,
  ) {
    this.webviewClient = new PreviewWebviewClient(context.extensionUri);
  }

  get isLocked(): boolean {
    return this.session.isLocked;
  }

  get hasSnapshot(): boolean {
    return !!this.session.snapshot;
  }

  dispose(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    this.panel?.dispose();
    this.disposePanelState();
    this.onDispose(this);
  }

  async open(target?: MermaidSourceCommandArgument): Promise<void> {
    if (this.disposed) {
      return;
    }

    const shouldRetargetSource = !this.panel || !this.session.isLocked || !this.session.snapshot;
    if (shouldRetargetSource) {
      await this.openResource(target);
      if (this.disposed) {
        return;
      }
    }
    let panel = this.panel;
    if (!panel) {
      panel = this.createPanel();
    } else {
      panel.reveal(panel.viewColumn, true);
    }

    this.ensureWebviewHtml(panel);
    this.scheduleRefresh(shouldRetargetSource ? "manual-open" : "panel-visible", true);
  }

  scheduleRefresh(reason: PreviewUpdateReason, immediate = false): void {
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

  forceRefresh(): void {
    this.scheduleRefresh("manual-refresh", true);
  }

  resolvePreviewEditor(): vscode.TextEditor | undefined {
    return this.session.resolvePreviewEditor(
      vscode.window.activeTextEditor,
      vscode.window.visibleTextEditors,
    );
  }

  setLocked(locked: boolean, notify: boolean): boolean {
    if (locked && !this.session.snapshot) {
      if (notify) {
        void vscode.window.showWarningMessage(EMPTY_PREVIEW_LOCK_WARNING);
      }
      return false;
    }
    if (!this.session.setLocked(locked)) {
      return false;
    }
    this.scheduleRefresh("lock", true);
    if (notify) {
      void vscode.window.showInformationMessage(
        locked
          ? "Merman preview is locked to the current source."
          : "Merman preview is following the active source.",
      );
    }
    this.onDidChangeLockState(this);
    return true;
  }

  tracksDocument(uri: vscode.Uri): boolean {
    const trackedUri = this.resolvePreviewEditor()?.document.uri.toString() ?? this.session.snapshot?.documentUri;
    return trackedUri === uri.toString();
  }

  async showSource(): Promise<boolean> {
    const snapshot = this.session.snapshot;
    if (!snapshot) {
      return false;
    }

    const document = await vscode.workspace.openTextDocument(vscode.Uri.parse(snapshot.documentUri));
    const sourceRange = snapshot.input.sourceRange;
    const endLine = Math.min(sourceRange.endLine, Math.max(document.lineCount - 1, 0));
    const startLine = Math.min(sourceRange.startLine, endLine);
    const endCharacter = document.lineAt(endLine).text.length;
    const range = new vscode.Range(
      new vscode.Position(startLine, 0),
      new vscode.Position(endLine, endCharacter),
    );
    await vscode.window.showTextDocument(document, {
      preview: false,
      preserveFocus: false,
      selection: range,
    });
    return true;
  }

  private createPanel(): vscode.WebviewPanel {
    const panel = vscode.window.createWebviewPanel(
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
    this.panel = panel;
    panel.webview.onDidReceiveMessage(
      (message: PreviewFromWebviewMessage) => {
        void this.handleWebviewMessage(message).catch((error: unknown) => {
          this.handleWebviewMessageError(error);
        });
      },
      null,
      this.panelDisposables,
    );
    panel.onDidDispose(() => {
      this.handlePanelDisposed();
    }, null, this.panelDisposables);
    panel.onDidChangeViewState(() => {
      if (this.panel?.visible) {
        this.scheduleRefresh("panel-visible");
      }
      this.onDidChangeActiveState(this, this.panel?.active === true);
    }, null, this.panelDisposables);
    return panel;
  }

  private handlePanelDisposed(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    this.disposePanelState();
    this.onDispose(this);
  }

  private disposePanelState(): void {
    this.clearPendingRender();
    this.renderQueue.cancelPending();
    this.panel = undefined;
    this.session.reset();
    this.webviewClient.reset();
    const disposables = this.panelDisposables.splice(0);
    for (const disposable of disposables) {
      disposable.dispose();
    }
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
      case "refresh":
        this.forceRefresh();
        return;
      case "showSource":
        await this.showSource();
        return;
      case "copySvg":
        assertSafePreviewSvg(message.svg);
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

  private handleWebviewMessageError(error: unknown): void {
    const message = errorMessage(error);
    this.outputChannel.error(`Preview webview message failed: ${message}`);
    void vscode.window.showErrorMessage(`Merman preview action failed: ${message}`);
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

  private async exportRendered(format: ExportFormat): Promise<void> {
    const snapshot = this.session.snapshot;
    if (!snapshot) {
      void vscode.window.showWarningMessage(
        "Open a Mermaid preview before exporting the rendered diagram.",
      );
      return;
    }

    const documentUri = vscode.Uri.parse(snapshot.documentUri);
    await exportRenderedDiagram({
      context: this.context,
      outputChannel: this.outputChannel,
      sourceUri: documentUri,
      exportBaseName: snapshot.input.exportBaseName,
      source: snapshot.input.source,
      format,
      theme: snapshot.diagramTheme,
      background: previewCliBackground(snapshot.background),
      signalLabel: `preview-export-${format}`,
      failureMessagePrefix: "Merman preview export failed",
    });
  }
}

function collectPreviewDiagnostics(
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

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
