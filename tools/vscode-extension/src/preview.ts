import * as vscode from "vscode";

import {
  type PreviewDiagramTheme,
  type PreviewDiagnosticTarget,
  type PreviewDiagnostics,
  type PreviewSnapshot,
} from "./preview-model.js";
import {
  isPreviewDiagramTheme,
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
import { renderMermanSource } from "./renderer.js";
import { PreviewRenderQueue } from "./preview-render.js";
import { PreviewSession } from "./preview-session.js";
import { PreviewWebviewClient } from "./preview-webview-client.js";

const PREVIEW_COMMAND = "merman.openPreview";
const PREVIEW_TITLE = "Merman Preview";
const RENDER_DEBOUNCE_MS = 180;
const DIAGNOSTICS_PREVIEW_LIMIT = 8;

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
      vscode.commands.registerCommand(PREVIEW_COMMAND, async (resource?: vscode.Uri) => {
        await this.open(resource);
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

  private async open(resource?: vscode.Uri): Promise<void> {
    await this.openResource(resource);
    if (!this.panel) {
      this.panel = vscode.window.createWebviewPanel(
        "mermanPreview",
        PREVIEW_TITLE,
        vscode.ViewColumn.Beside,
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
      this.panel.reveal(vscode.ViewColumn.Beside, false);
    }

    this.ensureWebviewHtml(panelOrThrow(this.panel));
    this.scheduleRefresh("manual-open", true);
  }

  private async openResource(resource?: vscode.Uri): Promise<void> {
    if (!resource) {
      return;
    }
    const activeEditor = vscode.window.activeTextEditor;
    if (activeEditor?.document.uri.toString() === resource.toString()) {
      return;
    }
    this.session.rememberResource(resource);
    const document = await vscode.workspace.openTextDocument(resource);
    await vscode.window.showTextDocument(document, {
      preview: true,
      preserveFocus: false,
    });
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
      renderSvg: (source) => this.renderSvg(source),
      postMessage: (message) => this.postMessage(message),
      info: (message) => this.outputChannel.info(message),
      error: (message) => this.outputChannel.error(message),
      isCurrentRequest: (requestId) => !!this.panel && this.renderQueue.isCurrentRequest(requestId),
      markRendered: (_requestId, renderedSnapshot, svg) => this.webviewClient.markRendered(renderedSnapshot, svg),
    });
  }

  private createSnapshot(): PreviewSnapshot | undefined {
    return this.session.createSnapshot(
      vscode.window.activeTextEditor,
      vscode.window.visibleTextEditors,
      collectPreviewDiagnostics,
    );
  }

  private async renderSvg(source: string): Promise<string> {
    const result = await renderMermanSource({
      context: this.context,
      source,
      format: "svg",
      theme: this.session.diagramTheme,
      outputChannel: this.outputChannel,
      signalLabel: "preview",
    });
    return result.stdout.toString("utf8");
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
      case "revealDiagnostic":
        await revealDiagnosticTarget(parseDiagnosticTarget(message.target));
        return;
      case "showDiagnosticFixes":
        await showDiagnosticFixes(parseDiagnosticTarget(message.target));
        return;
      case "togglePin":
        this.togglePin();
        return;
      case "selectSource":
        this.selectSource(message.sourceId);
        return;
      case "setDiagramTheme":
        this.setDiagramTheme(message.theme);
        return;
      case "setBackground":
        return;
    }
  }

  private togglePin(): void {
    if (
      !this.session.togglePin(vscode.window.activeTextEditor, vscode.window.visibleTextEditors)
    ) {
      return;
    }
    this.scheduleRefresh("pin-toggle", true);
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
  const diagnostics = deduplicateDiagnostics(
    vscode.languages
      .getDiagnostics(uri)
      .filter((diagnostic) => isDiagnosticInRange(diagnostic, diagnosticRange))
      .sort(compareDiagnostics),
  );

  const items = diagnostics.slice(0, DIAGNOSTICS_PREVIEW_LIMIT).map((diagnostic) => ({
    severityLabel: diagnosticSeverityLabel(diagnostic.severity),
    severityKey: diagnosticSeverityKey(diagnostic.severity),
    line: diagnostic.range.start.line + 1,
    column: diagnostic.range.start.character + 1,
    target: {
      uri: uri.toString(),
      startLine: diagnostic.range.start.line,
      startCharacter: diagnostic.range.start.character,
      endLine: diagnostic.range.end.line,
      endCharacter: diagnostic.range.end.character,
    },
    hasQuickFixes: diagnostic.source === "merman",
    source: diagnostic.source,
    code: diagnosticCodeLabel(diagnostic.code),
    message: diagnostic.message,
  }));

  const counts = {
    error: diagnostics.filter((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Error)
      .length,
    warning: diagnostics.filter(
      (diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Warning,
    ).length,
    info: diagnostics.filter((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Information)
      .length,
    hint: diagnostics.filter((diagnostic) => diagnostic.severity === vscode.DiagnosticSeverity.Hint)
      .length,
  };

  return {
    summary: `${counts.error} errors, ${counts.warning} warnings, ${counts.info} infos, ${counts.hint} hints`,
    visibleCount: items.length,
    totalCount: diagnostics.length,
    items,
  };
}

function deduplicateDiagnostics(diagnostics: readonly vscode.Diagnostic[]): vscode.Diagnostic[] {
  const seen = new Set<string>();
  return diagnostics.filter((diagnostic) => {
    const key = [
      diagnostic.range.start.line,
      diagnostic.range.start.character,
      diagnostic.range.end.line,
      diagnostic.range.end.character,
      diagnostic.severity,
      diagnostic.source ?? "",
      diagnosticCodeLabel(diagnostic.code) ?? "",
      diagnostic.message,
    ].join("\u0000");
    if (seen.has(key)) {
      return false;
    }
    seen.add(key);
    return true;
  });
}

function isDiagnosticInRange(
  diagnostic: vscode.Diagnostic,
  diagnosticRange: { startLine: number; endLine: number },
): boolean {
  const startLine = diagnostic.range.start.line;
  const endLine = diagnostic.range.end.line;
  return startLine <= diagnosticRange.endLine && endLine >= diagnosticRange.startLine;
}

function compareDiagnostics(a: vscode.Diagnostic, b: vscode.Diagnostic): number {
  return (
    diagnosticSeverityRank(a.severity) - diagnosticSeverityRank(b.severity) ||
    a.range.start.line - b.range.start.line ||
    a.range.start.character - b.range.start.character
  );
}

function diagnosticSeverityRank(severity: vscode.DiagnosticSeverity): number {
  switch (severity) {
    case vscode.DiagnosticSeverity.Error:
      return 0;
    case vscode.DiagnosticSeverity.Warning:
      return 1;
    case vscode.DiagnosticSeverity.Information:
      return 2;
    case vscode.DiagnosticSeverity.Hint:
    default:
      return 3;
  }
}

function diagnosticSeverityLabel(severity: vscode.DiagnosticSeverity): string {
  switch (severity) {
    case vscode.DiagnosticSeverity.Error:
      return "Error";
    case vscode.DiagnosticSeverity.Warning:
      return "Warning";
    case vscode.DiagnosticSeverity.Information:
      return "Info";
    case vscode.DiagnosticSeverity.Hint:
    default:
      return "Hint";
  }
}

function diagnosticSeverityKey(
  severity: vscode.DiagnosticSeverity,
): "error" | "warning" | "info" | "hint" {
  switch (severity) {
    case vscode.DiagnosticSeverity.Error:
      return "error";
    case vscode.DiagnosticSeverity.Warning:
      return "warning";
    case vscode.DiagnosticSeverity.Information:
      return "info";
    case vscode.DiagnosticSeverity.Hint:
    default:
      return "hint";
  }
}

function diagnosticCodeLabel(code: vscode.Diagnostic["code"]): string | undefined {
  if (typeof code === "string" || typeof code === "number") {
    return String(code);
  }
  if (code && typeof code === "object" && "value" in code) {
    return String(code.value);
  }
  return undefined;
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

async function showDiagnosticFixes(target: PreviewDiagnosticTarget): Promise<void> {
  const uri = vscode.Uri.parse(target.uri);
  const range = new vscode.Range(
    new vscode.Position(target.startLine, target.startCharacter),
    new vscode.Position(target.endLine, target.endCharacter),
  );
  const actions =
    (await vscode.commands.executeCommand<(vscode.Command | vscode.CodeAction)[]>(
      "vscode.executeCodeActionProvider",
      uri,
      range,
      vscode.CodeActionKind.QuickFix.value,
      DIAGNOSTICS_PREVIEW_LIMIT,
    )) ?? [];

  const applicable = actions.filter((action) => isApplicableQuickFix(action, range));
  if (applicable.length === 0) {
    void vscode.window.showInformationMessage("No quick fixes available for this diagnostic.");
    return;
  }

  if (applicable.length === 1) {
    const onlyAction = applicable[0];
    if (!onlyAction) {
      return;
    }
    await applyCodeActionLike(onlyAction);
    return;
  }

  const picked = await vscode.window.showQuickPick(
    applicable.map((action) => ({
      label: action.title,
      description: isPreferredCodeAction(action) ? "Preferred" : undefined,
      detail: isCodeAction(action) && action.disabled ? action.disabled.reason : undefined,
      action,
    })),
    {
      placeHolder: "Select a quick fix to apply",
      matchOnDescription: true,
      matchOnDetail: true,
    },
  );
  if (!picked?.action) {
    return;
  }

  await applyCodeActionLike(picked.action);
}

function isApplicableQuickFix(
  action: vscode.Command | vscode.CodeAction,
  range: vscode.Range,
): boolean {
  if (isCodeAction(action)) {
    if (action.disabled) {
      return false;
    }
    const diagnostics = action.diagnostics ?? [];
    if (diagnostics.length === 0) {
      return true;
    }
    return diagnostics.some((diagnostic) => diagnostic.range.intersection(range));
  }
  return true;
}

function isCodeAction(action: vscode.Command | vscode.CodeAction): action is vscode.CodeAction {
  return "edit" in action || "kind" in action || "diagnostics" in action || "isPreferred" in action;
}

function isPreferredCodeAction(action: vscode.Command | vscode.CodeAction): boolean {
  return isCodeAction(action) && action.isPreferred === true;
}

async function applyCodeActionLike(action: vscode.Command | vscode.CodeAction): Promise<void> {
  if (isCodeAction(action) && action.edit) {
    const applied = await vscode.workspace.applyEdit(action.edit);
    if (!applied) {
      void vscode.window.showWarningMessage("Failed to apply quick fix edits.");
      return;
    }
  }
  const command = isCodeAction(action) ? action.command : action;
  if (command) {
    await vscode.commands.executeCommand(command.command, ...(command.arguments ?? []));
  }
}
