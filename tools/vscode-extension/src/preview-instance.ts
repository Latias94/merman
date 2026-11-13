import * as vscode from "vscode";

import { getPreviewSettings } from "./config.js";
import { previewCliBackground } from "./preview-background.js";
import { collectMermanPreviewDiagnostics } from "./preview-diagnostics.js";
import type { ExportFormat } from "./export-options.js";
import { exportRenderedDiagram } from "./export-workflow.js";
import { errorMessage } from "./error-message.js";
import {
  type PreviewBackground,
  type PreviewDiagramTheme,
  type PreviewDiagnosticTarget,
  type PreviewDiagnostics,
  type PreviewDisplayMode,
  type PreviewSnapshot,
  type PreviewSourceKey,
  previewSourceKeyId,
  samePreviewRenderKey,
  samePreviewSourceKey,
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
import {
  extractPreviewInput,
  extractPreviewInputFromDocument,
  previewSourceIdentity,
} from "./preview-source.js";
import { assertSelfContainedPreviewSvg } from "./preview-svg-safety.js";
import { PreviewWebviewClient } from "./preview-webview-client.js";
import { renderMermanSource } from "./renderer.js";
import {
  mermaidSourceCommandIdentity,
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
  private pendingRender: { snapshot: PreviewSnapshot } | undefined;
  private readonly session: PreviewSession;
  private readonly webviewClient: PreviewWebviewClient;
  private readonly panelDisposables: vscode.Disposable[] = [];
  private readonly activeRenderedExports = new Set<string>();
  private disposed = false;
  private renderedOutputStale = false;
  // An invalidation permits one recovery attempt even if its render inputs are unchanged.
  private renderRecoveryPending = false;
  private pendingWebviewPost = Promise.resolve();
  private openGeneration = 0;

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly outputChannel: vscode.LogOutputChannel,
    private readonly onDispose: (instance: PreviewInstance) => void,
    private readonly onDidChangeActiveState: (instance: PreviewInstance, active: boolean) => void,
    private readonly onDidChangeLockState: (instance: PreviewInstance) => void,
  ) {
    this.session = new PreviewSession(getPreviewSettings());
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

    const openGeneration = ++this.openGeneration;
    const shouldRetargetSource = !this.panel || !this.session.isLocked || !this.session.snapshot;
    if (shouldRetargetSource) {
      await this.openResource(target, openGeneration);
      if (!this.isCurrentOpen(openGeneration)) {
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
    if (this.isCurrentOpen(openGeneration)) {
      this.scheduleRefresh(shouldRetargetSource ? "manual-open" : "panel-visible", true);
    }
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
      void this.refresh(reason).catch((error: unknown) => {
        this.handleRefreshError(error);
      });
    };
    if (immediate) {
      refresh();
      return;
    }
    this.renderTimer = setTimeout(refresh, RENDER_DEBOUNCE_MS);
  }

  invalidateRenderedOutput(reason: PreviewUpdateReason): void {
    if (!this.panel || !this.session.snapshot) {
      return;
    }
    const alreadyStale = this.renderedOutputStale;
    this.renderedOutputStale = true;
    this.renderRecoveryPending = true;
    if (!alreadyStale) {
      this.webviewClient.invalidateRenderedOutput();
    }
    this.pendingRender = undefined;
    const requestId = this.renderQueue.cancelPending();
    void this.postMessage({ type: "renderInvalidated", requestId, reason }).catch((error: unknown) => {
      this.handleRefreshError(error);
    });
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

  trackDocumentChange(event: vscode.TextDocumentChangeEvent): void {
    this.session.trackDocumentChange(event);
  }

  async showSource(): Promise<boolean> {
    const snapshot = this.session.snapshot;
    if (!snapshot) {
      return false;
    }

    const document = await vscode.workspace.openTextDocument(vscode.Uri.parse(snapshot.documentUri));
    const currentInput = extractPreviewInputFromDocument(
      document,
      undefined,
      previewSourceIdentity(snapshot.input),
    );
    if (!currentInput) {
      void vscode.window.showWarningMessage(
        "The preview source is no longer available in the document.",
      );
      return false;
    }
    const sourceRange = currentInput.sourceRange;
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
    this.pendingRender = undefined;
    this.renderQueue.cancelPending();
    this.renderedOutputStale = false;
    this.renderRecoveryPending = false;
    this.panel = undefined;
    this.session.reset();
    this.webviewClient.reset();
    const disposables = this.panelDisposables.splice(0);
    for (const disposable of disposables) {
      disposable.dispose();
    }
  }

  private async openResource(target: MermaidSourceCommandArgument | undefined, openGeneration: number): Promise<void> {
    const resource = mermaidSourceCommandUri(target);
    const source = mermaidSourceCommandIdentity(target) ?? mermaidSourceCommandSourceId(target);
    if (!resource) {
      if (!this.isCurrentOpen(openGeneration)) {
        return;
      }
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
      if (!this.isCurrentOpen(openGeneration)) {
        return;
      }
      editor = await vscode.window.showTextDocument(document, {
        preview: true,
        preserveFocus: true,
      });
      if (!this.isCurrentOpen(openGeneration)) {
        return;
      }
    }
    if (source && editor) {
      this.session.selectSource(editor, vscode.window.visibleTextEditors, source);
    } else {
      this.session.clearSelectedSource();
    }
  }

  private isCurrentOpen(openGeneration: number): boolean {
    return !this.disposed && this.openGeneration === openGeneration;
  }

  private async refresh(reason: PreviewUpdateReason): Promise<void> {
    const panel = this.panel;
    if (!panel) {
      return;
    }

    this.ensureWebviewHtml(panel);
    const snapshot = this.createSnapshot();
    const actions = planPreviewUpdate(
      this.session.snapshot,
      snapshot,
      reason,
      this.renderRecoveryPending && !!snapshot && !this.hasPendingRender(snapshot),
    );
    if (!snapshot) {
      panel.title = PREVIEW_TITLE;
      this.session.clearSource();
      this.pendingRender = undefined;
      this.renderQueue.cancelPending();
      await this.applyActions(actions);
      return;
    }

    panel.title = `${PREVIEW_TITLE}: ${snapshot.input.title}`;
    if (actions.some((action) => action.type === "renderRequested")) {
      this.invalidateRenderedOutput(reason);
    }
    this.session.rememberSnapshot(snapshot);
    await this.applyActions(actions);
  }

  private async applyActions(actions: readonly PreviewAction[]): Promise<void> {
    const postUpdates = async () => {
      const requestId = this.renderQueue.currentRequestId();
      for (const action of actions) {
        if (!this.panel || !this.renderQueue.isCurrentRequest(requestId)) {
          return;
        }
        if ("snapshot" in action && action.snapshot !== this.session.snapshot) {
          return;
        }
        switch (action.type) {
          case "showEmpty":
            await this.postMessage({
              type: "showEmpty",
              requestId,
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
        }
      }
    };
    const render = actions.find((action) => action.type === "renderRequested");
    if (render) {
      await this.renderSnapshot(render.snapshot, render.reason, postUpdates);
    } else {
      await postUpdates();
    }
  }

  private hasPendingRender(snapshot: PreviewSnapshot): boolean {
    return !!this.pendingRender && samePreviewRenderKey(this.pendingRender.snapshot, snapshot);
  }

  private async renderSnapshot(
    snapshot: PreviewSnapshot,
    reason: PreviewUpdateReason,
    beforeRender?: () => Promise<void>,
  ): Promise<void> {
    if (
      !this.session.snapshot ||
      !samePreviewRenderKey(snapshot, this.session.snapshot) ||
      this.hasPendingRender(snapshot)
    ) {
      return;
    }
    const isCurrentRequest = (requestId: number) =>
      !!this.panel &&
      this.renderQueue.isCurrentRequest(requestId) &&
      !!this.session.snapshot &&
      samePreviewRenderKey(snapshot, this.session.snapshot);
    const pending = { snapshot };
    this.pendingRender = pending;
    this.renderedOutputStale = true;
    this.renderRecoveryPending = false;
    try {
      await this.renderQueue.render(snapshot, reason, {
        renderContent: (renderedSnapshot, signal) => this.renderContent(renderedSnapshot, signal),
        postMessage: async (message) => {
          if (message.type === "renderStarted") {
            await beforeRender?.();
            if (!isCurrentRequest(message.requestId)) {
              return;
            }
          }
          const currentSnapshot = this.session.snapshot;
          await this.postMessage(
            currentSnapshot && "snapshot" in message && samePreviewRenderKey(snapshot, currentSnapshot)
              ? { ...message, snapshot: snapshotMessagePayload(currentSnapshot) }
              : message,
          );
        },
        info: (message) => this.outputChannel.info(message),
        error: (message) => this.outputChannel.error(message),
        isCurrentRequest,
        markRendered: (_requestId, renderedSnapshot, content) => {
          this.renderedOutputStale = false;
          this.webviewClient.markRendered(renderedSnapshot, content);
        },
      });
    } finally {
      if (this.pendingRender === pending) {
        this.pendingRender = undefined;
      }
    }
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
      assertSelfContainedPreviewSvg(content);
    }
    return content;
  }

  private ensureWebviewHtml(panel: vscode.WebviewPanel): void {
    this.webviewClient.ensureHtml(panel);
  }

  private async postMessage(message: PreviewToWebviewMessage): Promise<void> {
    const panel = this.panel;
    await this.orderWebviewPost(async () => {
      if (panel && panel === this.panel) {
        await this.webviewClient.post(panel, message);
      }
    });
  }

  private orderWebviewPost(post: () => Promise<void>): Promise<void> {
    const next = this.pendingWebviewPost.then(post);
    this.pendingWebviewPost = next.catch(() => {});
    return next;
  }

  private async acceptWebviewReady(): Promise<void> {
    let snapshotToRender: PreviewSnapshot | undefined;
    await this.orderWebviewPost(async () => {
      let replayedSnapshotUi = false;
      await this.webviewClient.acceptReady(
        this.panel,
        this.session.snapshot,
        async (snapshot) => {
          replayedSnapshotUi = true;
          for (const type of ["sourceListUpdated", "diagnosticsUpdated", "settingsUpdated"] as const) {
            await this.webviewClient.post(this.panel, { type, snapshot: snapshotMessagePayload(snapshot) });
          }
        },
        // Rendering posts lifecycle messages through the same ordered path, after this drain.
        async (snapshot) => { snapshotToRender = snapshot; },
      );
      if (!replayedSnapshotUi && this.session.snapshot) {
        // Buffered lifecycle snapshots can predate settings changes with the same render key.
        await this.webviewClient.post(this.panel, {
          type: "settingsUpdated",
          snapshot: snapshotMessagePayload(this.session.snapshot),
        });
      }
    });
    if (snapshotToRender) {
      await this.renderSnapshot(snapshotToRender, "panel-visible");
    }
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
        await this.acceptWebviewReady();
        return;
      case "refresh":
        this.forceRefresh();
        return;
      case "showSource":
        await this.showSource();
        return;
      case "copySvg":
        await this.copySvg(message.svg, message.sourceKey);
        return;
      case "exportRendered":
        await this.exportRendered(message.format, message.sourceKey);
        return;
      case "revealDiagnostic":
        await this.revealCurrentDiagnostic();
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

  private handleRefreshError(error: unknown): void {
    const message = errorMessage(error);
    this.outputChannel.error(`Preview refresh failed: ${message}`);
    void vscode.window.showErrorMessage(`Merman preview refresh failed: ${message}`);
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

  private async copySvg(svg: string, renderedSourceKey: PreviewSourceKey): Promise<void> {
    const snapshot = this.currentRenderedOutputSnapshot(renderedSourceKey);
    if (!snapshot) {
      return;
    }
    assertSelfContainedPreviewSvg(svg);
    await vscode.env.clipboard.writeText(svg);
    void vscode.window.showInformationMessage("Copied Mermaid SVG to clipboard.");
  }

  private async exportRendered(format: ExportFormat, renderedSourceKey: PreviewSourceKey): Promise<void> {
    const snapshot = this.currentRenderedOutputSnapshot(renderedSourceKey);
    if (!snapshot) {
      return;
    }
    const exportKey = `${format}\u0000${previewSourceKeyId(renderedSourceKey)}`;
    if (this.activeRenderedExports.has(exportKey)) {
      return;
    }

    const documentUri = vscode.Uri.parse(snapshot.documentUri);
    this.activeRenderedExports.add(exportKey);
    try {
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
    } finally {
      this.activeRenderedExports.delete(exportKey);
    }
  }

  private currentRenderedOutputSnapshot(renderedSourceKey: PreviewSourceKey): PreviewSnapshot | undefined {
    const snapshot = this.session.snapshot;
    if (!snapshot) {
      void vscode.window.showWarningMessage(
        "Open a Mermaid preview before copying or exporting the rendered diagram.",
      );
      return undefined;
    }
    if (this.renderedOutputStale || !samePreviewSourceKey(snapshot.sourceKey, renderedSourceKey)) {
      void vscode.window.showWarningMessage(
        "Wait for the latest Mermaid preview to finish rendering before copying or exporting.",
      );
      return undefined;
    }
    return snapshot;
  }

  private async revealCurrentDiagnostic(): Promise<void> {
    const snapshot = this.session.snapshot;
    const target = snapshot?.diagnostics?.firstTarget;
    if (!snapshot || !target || target.uri !== snapshot.documentUri) {
      return;
    }
    await revealDiagnosticTarget(target);
  }
}

function collectPreviewDiagnostics(
  uri: vscode.Uri,
  diagnosticRange: { startLine: number; endLine: number },
  documentVersion: number,
): PreviewDiagnostics {
  return collectMermanPreviewDiagnostics(
    vscode.languages.getDiagnostics(uri),
    uri.toString(),
    diagnosticRange,
    documentVersion,
  );
}

async function revealDiagnosticTarget(target: PreviewDiagnosticTarget): Promise<void> {
  if (!isValidDiagnosticTarget(target)) {
    return;
  }
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

function isValidDiagnosticTarget(target: PreviewDiagnosticTarget): boolean {
  return (
    typeof target.uri === "string" &&
    isNonNegativeInteger(target.startLine) &&
    isNonNegativeInteger(target.startCharacter) &&
    isNonNegativeInteger(target.endLine) &&
    isNonNegativeInteger(target.endCharacter) &&
    (target.startLine < target.endLine ||
      (target.startLine === target.endLine && target.startCharacter <= target.endCharacter))
  );
}

function isNonNegativeInteger(value: number): boolean {
  return Number.isInteger(value) && value >= 0;
}
