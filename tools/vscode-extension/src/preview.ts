import * as cp from "node:child_process";
import * as vscode from "vscode";

import { extractPreviewInput, type PreviewInput } from "./preview-source.js";
import { findWorkspaceDebugBinary, workspaceRoot } from "./workspace.js";

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
  private renderVersion = 0;
  private activeRender: cp.ChildProcess | undefined;
  private readonly disposables: vscode.Disposable[] = [];

  constructor(private readonly context: vscode.ExtensionContext) {
    this.outputChannel = vscode.window.createOutputChannel("Merman Preview", { log: true });
    this.disposables.push(this.outputChannel);
    this.disposables.push(
      vscode.commands.registerCommand(PREVIEW_COMMAND, async () => {
        await this.open();
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
        if (event.document === vscode.window.activeTextEditor?.document) {
          this.scheduleRefresh("document-change");
        }
      }),
    );
    this.disposables.push(
      vscode.languages.onDidChangeDiagnostics((event) => {
        const activeUri = vscode.window.activeTextEditor?.document.uri;
        if (activeUri && event.uris.some((uri) => uri.toString() === activeUri.toString())) {
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

  private async open(): Promise<void> {
    if (!this.panel) {
      this.panel = vscode.window.createWebviewPanel(
        "mermanPreview",
        PREVIEW_TITLE,
        vscode.ViewColumn.Beside,
        {
          enableScripts: false,
          retainContextWhenHidden: true,
        },
      );
      this.panel.onDidDispose(() => {
        this.clearPendingRender();
        this.panel = undefined;
      }, null, this.disposables);
      this.panel.onDidChangeViewState(() => {
        if (this.panel?.visible) {
          this.scheduleRefresh("panel-visible");
        }
      }, null, this.disposables);
    } else {
      this.panel.reveal(vscode.ViewColumn.Beside, false);
    }

    this.scheduleRefresh("manual-open", true);
  }

  private scheduleRefresh(reason: string, immediate = false): void {
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

  private async refresh(reason: string): Promise<void> {
    const panel = this.panel;
    if (!panel) {
      return;
    }

    const editor = vscode.window.activeTextEditor;
    const input = editor ? extractPreviewInput(editor) : null;
    if (!input) {
      panel.title = PREVIEW_TITLE;
      panel.webview.html = renderPreviewHtml(panel.webview, undefined, undefined, undefined, {
        heading: "No Mermaid source available",
        detail: "Open a .mmd, .mermaid, or Markdown document with a Mermaid fence, then run Merman: Open Preview.",
      });
      return;
    }

    const diagnostics = editor
      ? collectPreviewDiagnostics(editor.document.uri, input.diagnosticRange)
      : undefined;

    panel.title = `${PREVIEW_TITLE}: ${input.title}`;
    const currentRender = ++this.renderVersion;
    panel.webview.html = renderPreviewHtml(panel.webview, input, undefined, diagnostics, {
      heading: "Rendering preview",
      detail: `Source: ${input.subtitle}`,
    });

    try {
      this.outputChannel.info(`refresh=${reason} source="${input.title}"`);
      const svg = await this.renderSvg(input.source);
      if (!this.panel || currentRender !== this.renderVersion) {
        return;
      }
      panel.webview.html = renderPreviewHtml(panel.webview, input, svg, diagnostics);
    } catch (error) {
      if (!this.panel || currentRender !== this.renderVersion) {
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      this.outputChannel.error(message);
      panel.webview.html = renderPreviewHtml(panel.webview, input, undefined, diagnostics, {
        heading: "Render failed",
        detail: message,
      });
    }
  }

  private async renderSvg(source: string): Promise<string> {
    const invocation = resolveCliInvocation();
    if (!invocation) {
      throw new Error("No workspace root found for merman-cli preview.");
    }

    this.activeRender?.kill();
    return new Promise<string>((resolve, reject) => {
      const child = cp.spawn(invocation.command, invocation.args, {
        cwd: invocation.cwd,
        env: process.env,
        stdio: "pipe",
      });
      this.activeRender = child;

      let stdout = "";
      let stderr = "";
      child.stdout?.setEncoding("utf8");
      child.stderr?.setEncoding("utf8");
      child.stdout?.on("data", (chunk: string) => {
        stdout += chunk;
      });
      child.stderr?.on("data", (chunk: string) => {
        stderr += chunk;
      });
      child.on("error", (error) => {
        reject(error);
      });
      child.on("close", (code, signal) => {
        if (this.activeRender === child) {
          this.activeRender = undefined;
        }
        if (signal === "SIGTERM") {
          return reject(new Error("Preview render was superseded by a newer update."));
        }
        if (code !== 0) {
          return reject(
            new Error(stderr.trim() || `merman-cli exited with status ${code ?? "unknown"}`),
          );
        }
        resolve(stdout);
      });
      child.stdin?.end(source, "utf8");
    });
  }

  private clearPendingRender(): void {
    if (this.renderTimer) {
      clearTimeout(this.renderTimer);
      this.renderTimer = undefined;
    }
    this.activeRender?.kill();
    this.activeRender = undefined;
  }
}

function resolveCliInvocation():
  | { command: string; args: string[]; cwd?: string }
  | undefined {
  const binary = findWorkspaceDebugBinary("merman-cli");
  if (binary) {
    return {
      command: binary,
      args: ["-q", "-i", "-", "-o", "-", "-e", "svg"],
      cwd: workspaceRoot(),
    };
  }

  const root = workspaceRoot();
  if (!root) {
    return undefined;
  }

  return {
    command: "cargo",
    args: ["run", "-q", "-p", "merman-cli", "--", "-q", "-i", "-", "-o", "-", "-e", "svg"],
    cwd: root,
  };
}

function renderPreviewHtml(
  webview: vscode.Webview,
  input?: PreviewInput,
  svg?: string,
  diagnostics?: PreviewDiagnostics,
  message?: { heading: string; detail: string },
): string {
  const title = input ? escapeHtml(input.title) : PREVIEW_TITLE;
  const subtitle = input ? escapeHtml(input.subtitle) : "No active Mermaid source";
  const diagnosticsSection = renderDiagnosticsSection(diagnostics);
  const body = svg
    ? `<section class="canvas">${svg}</section>`
    : `<section class="empty"><h2>${escapeHtml(message?.heading ?? "No preview")}</h2><p>${escapeHtml(message?.detail ?? "")}</p></section>`;

  return `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta
      http-equiv="Content-Security-Policy"
      content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; img-src ${webview.cspSource} data:;"
    />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>${title}</title>
    <style>
      :root {
        color-scheme: light dark;
      }
      body {
        margin: 0;
        font-family: var(--vscode-font-family);
        color: var(--vscode-editor-foreground);
        background: var(--vscode-editor-background);
      }
      .frame {
        min-height: 100vh;
        display: grid;
        grid-template-rows: auto auto 1fr;
      }
      .meta {
        padding: 12px 16px;
        border-bottom: 1px solid var(--vscode-panel-border);
        background: var(--vscode-sideBar-background);
      }
      .meta h1 {
        margin: 0 0 4px;
        font-size: 13px;
        font-weight: 600;
      }
      .meta p {
        margin: 0;
        color: var(--vscode-descriptionForeground);
        font-size: 12px;
      }
      .diagnostics {
        padding: 10px 16px 12px;
        border-bottom: 1px solid var(--vscode-panel-border);
        background: color-mix(in srgb, var(--vscode-editor-background) 88%, var(--vscode-sideBar-background) 12%);
      }
      .diagnostics-summary {
        margin: 0 0 8px;
        font-size: 12px;
        color: var(--vscode-descriptionForeground);
      }
      .diagnostics-list {
        margin: 0;
        padding: 0;
        list-style: none;
        display: grid;
        gap: 6px;
      }
      .diagnostic-item {
        margin: 0;
        padding: 8px 10px;
        border-left: 3px solid transparent;
        background: color-mix(in srgb, var(--vscode-editor-background) 92%, var(--vscode-sideBar-background) 8%);
      }
      .diagnostic-item[data-severity="error"] {
        border-left-color: var(--vscode-errorForeground);
      }
      .diagnostic-item[data-severity="warning"] {
        border-left-color: var(--vscode-editorWarning-foreground);
      }
      .diagnostic-item[data-severity="info"] {
        border-left-color: var(--vscode-editorInfo-foreground);
      }
      .diagnostic-item[data-severity="hint"] {
        border-left-color: var(--vscode-terminal-ansiBrightBlack);
      }
      .diagnostic-header {
        display: flex;
        gap: 8px;
        align-items: baseline;
        flex-wrap: wrap;
        margin: 0 0 4px;
        font-size: 12px;
      }
      .diagnostic-severity {
        font-weight: 600;
      }
      .diagnostic-location,
      .diagnostic-source {
        color: var(--vscode-descriptionForeground);
      }
      .diagnostic-message {
        margin: 0;
        font-size: 12px;
        line-height: 1.45;
        white-space: pre-wrap;
      }
      .canvas,
      .empty {
        padding: 18px;
        box-sizing: border-box;
      }
      .canvas {
        overflow: auto;
      }
      .canvas svg {
        max-width: 100%;
        height: auto;
      }
      .empty h2 {
        margin: 0 0 8px;
        font-size: 14px;
      }
      .empty p {
        margin: 0;
        white-space: pre-wrap;
        color: var(--vscode-descriptionForeground);
        line-height: 1.5;
      }
    </style>
  </head>
  <body>
    <main class="frame">
      <header class="meta">
        <h1>${title}</h1>
        <p>${subtitle}</p>
      </header>
      ${diagnosticsSection}
      ${body}
    </main>
  </body>
</html>`;
}

interface PreviewDiagnosticItem {
  severityLabel: string;
  severityKey: "error" | "warning" | "info" | "hint";
  line: number;
  column: number;
  source?: string;
  code?: string;
  message: string;
}

interface PreviewDiagnostics {
  summary: string;
  visibleCount: number;
  totalCount: number;
  items: PreviewDiagnosticItem[];
}

function collectPreviewDiagnostics(
  uri: vscode.Uri,
  diagnosticRange: { startLine: number; endLine: number },
): PreviewDiagnostics {
  const diagnostics = vscode.languages
    .getDiagnostics(uri)
    .filter((diagnostic) => isDiagnosticInRange(diagnostic, diagnosticRange))
    .sort(compareDiagnostics);

  const items = diagnostics.slice(0, DIAGNOSTICS_PREVIEW_LIMIT).map((diagnostic) => ({
    severityLabel: diagnosticSeverityLabel(diagnostic.severity),
    severityKey: diagnosticSeverityKey(diagnostic.severity),
    line: diagnostic.range.start.line + 1,
    column: diagnostic.range.start.character + 1,
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

function renderDiagnosticsSection(diagnostics?: PreviewDiagnostics): string {
  if (!diagnostics) {
    return "";
  }

  const suffix =
    diagnostics.totalCount > diagnostics.visibleCount
      ? ` Showing first ${diagnostics.visibleCount} of ${diagnostics.totalCount}.`
      : diagnostics.totalCount > 0
        ? ` Showing ${diagnostics.totalCount}.`
        : "";

  if (diagnostics.items.length === 0) {
    return `<section class="diagnostics"><p class="diagnostics-summary">${escapeHtml(`${diagnostics.summary}. No issues in the active preview range.`)}</p></section>`;
  }

  const items = diagnostics.items
    .map((item) => {
      const headerParts = [
        `<span class="diagnostic-severity">${escapeHtml(item.severityLabel)}</span>`,
        `<span class="diagnostic-location">Ln ${item.line}, Col ${item.column}</span>`,
      ];
      const sourceLabel = [item.source, item.code].filter(Boolean).join(": ");
      if (sourceLabel) {
        headerParts.push(`<span class="diagnostic-source">${escapeHtml(sourceLabel)}</span>`);
      }
      return `<li class="diagnostic-item" data-severity="${item.severityKey}">
        <p class="diagnostic-header">${headerParts.join("")}</p>
        <p class="diagnostic-message">${escapeHtml(item.message)}</p>
      </li>`;
    })
    .join("");

  return `<section class="diagnostics">
    <p class="diagnostics-summary">${escapeHtml(`${diagnostics.summary}.${suffix}`)}</p>
    <ol class="diagnostics-list">${items}</ol>
  </section>`;
}

function isDiagnosticInRange(
  diagnostic: vscode.Diagnostic,
  diagnosticRange: { startLine: number; endLine: number },
): boolean {
  const startLine = diagnostic.range.start.line;
  const endLine = diagnostic.range.end.line;
  return (
    startLine <= diagnosticRange.endLine &&
    endLine >= diagnosticRange.startLine
  );
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

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}
