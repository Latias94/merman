import type { PreviewInput } from "./preview-source.js";

const PREVIEW_TITLE = "Merman Preview";

export interface PreviewHtmlResources {
  cspSource: string;
  stylesUri: string;
  scriptUri: string;
}

export interface RenderPreviewHtmlRequest {
  resources: PreviewHtmlResources;
  input?: PreviewInput;
  svg?: string;
  diagnostics?: PreviewDiagnostics;
  message?: { heading: string; detail: string };
  sources?: readonly PreviewInput[];
  pinned?: boolean;
  diagramTheme?: PreviewDiagramTheme;
}

export type PreviewDiagramTheme = "source" | "default" | "dark" | "forest" | "neutral" | "base";

export interface PreviewDiagnosticItem {
  severityLabel: string;
  severityKey: "error" | "warning" | "info" | "hint";
  line: number;
  column: number;
  target: PreviewDiagnosticTarget;
  source?: string;
  code?: string;
  message: string;
  hasQuickFixes: boolean;
}

export interface PreviewDiagnostics {
  summary: string;
  visibleCount: number;
  totalCount: number;
  items: PreviewDiagnosticItem[];
}

export interface PreviewDiagnosticTarget {
  uri: string;
  startLine: number;
  startCharacter: number;
  endLine: number;
  endCharacter: number;
}

export function renderPreviewHtml(request: RenderPreviewHtmlRequest): string {
  const nonce = createNonce();
  const title = request.input ? escapeHtml(request.input.title) : PREVIEW_TITLE;
  const subtitle = request.input ? escapeHtml(request.input.subtitle) : "No active Mermaid source";
  const diagnosticsSection = renderDiagnosticsSection(request.diagnostics);
  const body = request.svg
    ? `<section class="viewport" aria-label="Mermaid preview canvas"><div class="stage"><div class="canvas">${request.svg}</div></div></section>`
    : `<section class="empty"><h2>${escapeHtml(request.message?.heading ?? "No preview")}</h2><p>${escapeHtml(request.message?.detail ?? "")}</p></section>`;

  return `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta
      http-equiv="Content-Security-Policy"
      content="default-src 'none'; script-src 'nonce-${nonce}'; style-src ${request.resources.cspSource}; img-src ${request.resources.cspSource} data:;"
    />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>${title}</title>
    <link rel="stylesheet" href="${escapeHtml(request.resources.stylesUri)}" />
  </head>
  <body>
    <main class="frame" data-theme="light" data-background="transparent">
      <header class="meta">
        <div class="meta-top">
          <div>
            <h1>${title}</h1>
            <p>${subtitle}</p>
          </div>
          ${renderToolbar(request.input, request.sources ?? [], request.pinned === true, request.diagramTheme ?? "source")}
        </div>
      </header>
      ${diagnosticsSection}
      ${body}
    </main>
    <script nonce="${nonce}" src="${escapeHtml(request.resources.scriptUri)}"></script>
  </body>
</html>`;
}

function renderToolbar(
  input: PreviewInput | undefined,
  sources: readonly PreviewInput[],
  pinned: boolean,
  diagramTheme: PreviewDiagramTheme,
): string {
  const sourceSelect =
    sources.length > 1
      ? `<select data-action="source" title="Preview source">${sources
          .map((source) => {
            const selected = input?.sourceId === source.sourceId ? " selected" : "";
            return `<option value="${escapeHtml(source.sourceId)}"${selected}>${escapeHtml(source.subtitle)}</option>`;
          })
          .join("")}</select>`
      : "";

  return `<nav class="toolbar" aria-label="Preview controls">
    ${sourceSelect}
    <span class="toolbar-group">
      <button type="button" data-action="zoom-out" title="Zoom out">-</button>
      <span class="zoom-readout" data-zoom-value>100%</span>
      <button type="button" data-action="zoom-in" title="Zoom in">+</button>
      <button type="button" data-action="fit" title="Fit to view">Fit</button>
      <button type="button" data-action="reset" title="Reset to actual size">1:1</button>
    </span>
    <span class="toolbar-group">
      <select data-action="diagram-theme" title="Mermaid theme">
        ${renderThemeOption("source", "Source", diagramTheme)}
        ${renderThemeOption("default", "Default", diagramTheme)}
        ${renderThemeOption("dark", "Dark", diagramTheme)}
        ${renderThemeOption("forest", "Forest", diagramTheme)}
        ${renderThemeOption("neutral", "Neutral", diagramTheme)}
        ${renderThemeOption("base", "Base", diagramTheme)}
      </select>
      <select data-action="background" title="Preview background">
        <option value="transparent">Transparent</option>
        <option value="paper">Paper</option>
        <option value="dark">Dark</option>
      </select>
    </span>
    <span class="toolbar-group">
      <button type="button" data-action="copy-svg" title="Copy SVG">Copy SVG</button>
      <button type="button" data-action="pin" aria-pressed="${pinned ? "true" : "false"}" title="Pin preview source">${pinned ? "Pinned" : "Pin"}</button>
    </span>
  </nav>`;
}

function renderThemeOption(
  value: PreviewDiagramTheme,
  label: string,
  selectedTheme: PreviewDiagramTheme,
): string {
  const selected = value === selectedTheme ? " selected" : "";
  return `<option value="${value}"${selected}>${label}</option>`;
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
      const target = escapeHtml(JSON.stringify(item.target));
      const actions = item.hasQuickFixes
        ? `<p class="diagnostic-actions"><button type="button" class="diagnostic-action" data-action="quick-fix" data-target="${target}" title="Request available quick fixes">Quick Fixes</button></p>`
        : "";
      return `<li class="diagnostic-item" data-severity="${item.severityKey}">
        <button type="button" class="diagnostic-button" data-action="diagnostic" data-target="${target}" title="Open diagnostic location in editor">
          <p class="diagnostic-header">${headerParts.join("")}</p>
          <p class="diagnostic-message">${escapeHtml(item.message)}</p>
        </button>
        ${actions}
      </li>`;
    })
    .join("");

  return `<section class="diagnostics">
    <p class="diagnostics-summary">${escapeHtml(`${diagnostics.summary}.${suffix}`)}</p>
    <ol class="diagnostics-list">${items}</ol>
  </section>`;
}

function createNonce(): string {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let out = "";
  for (let index = 0; index < 32; index += 1) {
    out += alphabet[Math.floor(Math.random() * alphabet.length)] ?? "A";
  }
  return out;
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}
