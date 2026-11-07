import * as vscode from "vscode";

import { getPreviewSettings } from "./config.js";
import { previewCliBackground } from "./preview-background.js";
import { configuredThemeFileUri } from "./render-settings.js";
import { resolvedRenderSettings } from "./render-options.js";
import { discoverThemes, inspectThemeFile } from "./theme-runtime.js";
import {
  editorThemeAppearance,
  isThemePreference,
  themeChoice,
  type ThemeAppearancePreference,
  type ThemeDiscovery,
} from "./theme-selection.js";
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
  getPreviewSourceSnapshot,
  previewSourceIdentity,
  resolvePreviewInputIdentity,
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
  private readonly session: PreviewSession;
  private readonly webviewClient: PreviewWebviewClient;
  private readonly panelDisposables: vscode.Disposable[] = [];
  private readonly activeRenderedExports = new Set<string>();
  private disposed = false;
  private renderedOutputStale = false;
  private openGeneration = 0;
  private themeGeneration = 0;
  private discovery: ThemeDiscovery | undefined;
  private themeInitialization: Promise<void> | undefined;
  private themesReady = false;

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
    await this.initializeThemes();
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
    if (!alreadyStale) {
      this.webviewClient.invalidateRenderedOutput();
    }
    const requestId = this.renderQueue.cancelPending();
    void this.postMessage({ type: "renderInvalidated", requestId, reason }).catch((error: unknown) => {
      this.handleRefreshError(error);
    });
  }

  forceRefresh(): void {
    this.scheduleRefresh("manual-refresh", true);
  }

  handleEditorAppearanceChanged(kind: number): void {
    if (!this.themesReady || this.disposed) {
      return;
    }
    try {
      this.session.setHostAppearance(editorThemeAppearance(kind));
      this.scheduleRefresh("editor-appearance", true);
    } catch (error) {
      this.handleRefreshError(error);
    }
  }

  private async initializeThemes(): Promise<void> {
    if (!this.themeInitialization) {
      this.themeInitialization = (async () => {
        const revision = this.session.themeSelectionRevision;
        const discovery = await discoverThemes(this.context);
        if (this.disposed) {
          return;
        }
        this.discovery = discovery;
        let imported;
        if (this.session.diagramTheme === "file") {
          while (!this.disposed) {
            const resourceUri = this.session.configurationResourceUri;
            const resource = resourceUri ? vscode.Uri.parse(resourceUri) : undefined;
            const settings = getPreviewSettings(resource);
            try {
              imported = await inspectThemeFile(this.context, configuredThemeFileUri(settings.themeFile, resource), discovery);
            } catch (error) {
              if (this.disposed || revision !== this.session.themeSelectionRevision) {
                imported = undefined;
                break;
              }
              if (resourceUri !== this.session.configurationResourceUri) continue;
              throw error;
            }
            if (resourceUri === this.session.configurationResourceUri) break;
          }
        }
        if (!this.disposed) {
          this.session.configureThemes(discovery, editorThemeAppearance(vscode.window.activeColorTheme.kind),
            revision === this.session.themeSelectionRevision ? imported : undefined);
          this.themesReady = true;
        }
      })().catch((error: unknown) => {
        this.themeInitialization = undefined;
        throw error;
      });
    }
    await this.themeInitialization;
  }

  get trackedDocumentUri(): string | undefined {
    const source = this.session.resolveSource(
      vscode.window.activeTextEditor,
      vscode.window.visibleTextEditors,
    );
    return source?.editor.document.uri.toString() ?? this.session.snapshot?.documentUri;
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

  trackDocumentChange(event: vscode.TextDocumentChangeEvent): void {
    this.session.trackDocumentChange(event);
  }

  async showSource(): Promise<boolean> {
    const snapshot = this.session.snapshot;
    if (!snapshot) {
      return false;
    }

    const document = await vscode.workspace.openTextDocument(vscode.Uri.parse(snapshot.documentUri));
    const currentInput = resolvePreviewInputIdentity(
      getPreviewSourceSnapshot(document).inputs,
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
    this.renderQueue.cancelPending();
    this.renderedOutputStale = false;
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
      if (activeEditor && getPreviewSourceSnapshot(activeEditor.document).inputs.length > 0) {
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
    if (!this.themesReady) {
      await this.initializeThemes();
    }
    if (this.disposed || this.panel !== panel) {
      return;
    }

    this.ensureWebviewHtml(panel);
    const snapshot = this.createSnapshot(reason === "editor-appearance");
    const actions = planPreviewUpdate(
      this.session.snapshot,
      snapshot,
      reason,
      this.renderedOutputStale && !!snapshot && !this.renderQueue.hasPending(snapshot),
    );
    if (!snapshot) {
      panel.title = PREVIEW_TITLE;
      this.session.clearSource();
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

  private async renderSnapshot(
    snapshot: PreviewSnapshot,
    reason: PreviewUpdateReason,
    beforeRender?: () => Promise<void>,
  ): Promise<void> {
    if (
      !this.session.snapshot ||
      !samePreviewRenderKey(snapshot, this.session.snapshot) ||
      this.renderQueue.hasPending(snapshot)
    ) {
      return;
    }
    await this.renderQueue.render(snapshot, reason, {
      beforeRender,
      renderContent: (renderedSnapshot, signal) => this.renderContent(renderedSnapshot, signal),
      postMessage: (message) => {
        const currentSnapshot = this.session.snapshot;
        return this.postMessage(
          currentSnapshot && "snapshot" in message && samePreviewRenderKey(snapshot, currentSnapshot)
            ? { ...message, snapshot: snapshotMessagePayload(currentSnapshot) }
            : message,
        );
      },
      info: (message) => this.outputChannel.info(message),
      error: (message) => this.outputChannel.error(message),
      isCurrentRequest: (requestId) =>
        !!this.panel &&
        this.renderQueue.isCurrentRequest(requestId) &&
        !!this.session.snapshot &&
        samePreviewRenderKey(snapshot, this.session.snapshot),
      markRendered: (_requestId, renderedSnapshot, content) => {
        this.renderedOutputStale = false;
        this.webviewClient.markRendered(renderedSnapshot, content);
      },
    });
  }

  private createSnapshot(keepSource = false): PreviewSnapshot | undefined {
    return this.session.createSnapshot(
      keepSource ? undefined : vscode.window.activeTextEditor,
      vscode.window.visibleTextEditors,
      collectPreviewDiagnostics,
      keepSource,
      vscode.workspace.textDocuments,
    );
  }

  private async renderContent(snapshot: PreviewSnapshot, signal: AbortSignal): Promise<string> {
    const result = await renderMermanSource({
      context: this.context,
      source: snapshot.input.source,
      format: snapshot.displayMode,
      ...resolvedRenderSettings(snapshot.resolvedTheme, previewCliBackground(snapshot.background)),
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
      case "setAppearance":
        this.setAppearance(message.appearance);
        return;
      case "setViewerBackground":
        if (this.session.setViewerBackground(message.background)) {
          this.scheduleRefresh("viewer-background", true);
        }
        return;
      case "importTheme":
        await this.importTheme();
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
    const choice = this.discovery
      ? themeChoice(this.discovery, theme) ?? this.session.snapshot?.themeChoices?.find((entry) => entry.id === theme)
      : undefined;
    if (!isPreviewDiagramTheme(theme) || !this.discovery ||
      !choice?.available ||
      !this.session.setDiagramTheme(theme)) {
      return;
    }
    this.themeGeneration += 1;
    this.scheduleRefresh("diagram-theme", true);
  }

  private setAppearance(preference: ThemeAppearancePreference): void {
    if (!isThemePreference(preference) || !this.session.setAppearance(preference)) {
      return;
    }
    this.themeGeneration += 1;
    this.scheduleRefresh("appearance", true);
  }

  private async importTheme(): Promise<void> {
    const discovery = this.discovery;
    if (!discovery?.inspect) {
      throw new Error("Importing theme recipe JSON requires an updated CLI with theme inspect support.");
    }
    const generation = ++this.themeGeneration;
    const files = await vscode.window.showOpenDialog({
      canSelectMany: false,
      canSelectFolders: false,
      filters: { "Theme recipe JSON": ["json"] },
      openLabel: "Import theme recipe",
      title: "Import a portable theme recipe JSON",
    });
    if (!files?.[0] || this.disposed || generation !== this.themeGeneration) {
      return;
    }
    let imported;
    try {
      imported = await inspectThemeFile(this.context, files[0], discovery);
    } catch (error) {
      if (this.disposed || generation !== this.themeGeneration) return;
      throw error;
    }
    if (this.disposed || generation !== this.themeGeneration) {
      return;
    }
    this.session.importTheme(imported);
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
        ...resolvedRenderSettings(snapshot.resolvedTheme, previewCliBackground(snapshot.background)),
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
